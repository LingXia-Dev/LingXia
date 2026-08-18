import {
  AssertionError,
  applyMatcher,
  expect as immediateExpect,
  popAssertionSilence,
  pushAssertionSilence,
  setAssertionSink,
} from "./expect.js";
import { formatValue, truncate } from "./format.js";
import { encodeAttachPayload, remapStack, type ResolvedHost } from "./host.js";
import { rememberInline } from "./report.js";
import { callerLocation, displayLocation, parseFrames } from "./ids.js";
import {
  PageLocator,
  sleep,
  testIdSelector,
  type LocatorResolve,
  type PageLike,
} from "./locator.js";
import type {
  Apps,
  AttachmentRef,
  ExpectOptions,
  Fixture,
  FixtureExpect,
  AssertionRecord,
  Locator,
  LocatorMatchers,
  RejectExpected,
  RetryMatchers,
  SourceLocation,
  SpecStatus,
  StepRecord,
  TestApp,
  TestPage,
} from "./types.js";
import {
  DEFAULT_ACTION_TIMEOUT_MS,
  DEFAULT_POLL_INTERVAL_MS,
  DEFAULT_SPEC_TIMEOUT_MS,
  MAX_ACTIONS,
  MAX_EVAL_BUDGET_MS,
  WEDGED_DEFER_BUDGET_MS,
} from "./version.js";
import type { LxAppDriver, PageDriver } from "@lingxia/types/automation";

export class TimeoutError extends Error {
  override readonly name = "TimeoutError";
}

export type FailurePhase = "beforeEach" | "body" | "defer" | "forensics" | "timeout";

export class LiveFixture implements Fixture {
  readonly apps: Apps;
  readonly args: Record<string, string>;
  readonly steps: StepRecord[] = [];
  readonly assertions: AssertionRecord[] = [];
  readonly attachments: AttachmentRef[] = [];
  readonly defers: Array<() => void | Promise<void>> = [];
  aborted = false;
  abortError: Error | null = null;
  private actionSilence = 0;
  private actionCount = 0;
  /** Actions still in flight, so an abort can mark them instead of leaving
   *  them at their optimistic default. */
  private readonly openActions = new Set<StepRecord>();
  cleanupUntil = 0;
  cleanupActive = false;
  lastStepPath: string | undefined;
  failurePhase: FailurePhase | null = null;
  private readonly stepStack: StepRecord[] = [];
  private rawApp: LxAppDriver;

  constructor(
    readonly specId: string,
    rawApp: LxAppDriver,
    private readonly host: ResolvedHost,
    args: Record<string, string>,
    private readonly automation: { lxapp: { (): LxAppDriver; (id: string): LxAppDriver } },
    private readonly specBudgetMs: number = DEFAULT_SPEC_TIMEOUT_MS,
  ) {
    this.rawApp = rawApp;
    this.args = args;
    setAssertionSink((entry) => this.noteAssertion(entry));
    this.apps = {
      lxapp: (appId: string) => this.wrapApp(this.automation.lxapp(appId)),
    };
  }

  get app(): TestApp {
    return this.wrapApp(this.rawApp);
  }

  get raw(): LxAppDriver {
    return this.rawApp;
  }

  step<T>(name: string, body: () => T | Promise<T>): Promise<T> {
    return this.guard(async () => {
      const record: StepRecord = {
        name,
        path: [...this.stepStack.map((step) => step.name), name].join(" > "),
        status: "passed",
        duration_ms: 0,
        steps: [],
        attachments: [],
        assertions: [],
      };
      const parent = this.stepStack[this.stepStack.length - 1];
      (parent ? parent.steps : this.steps).push(record);
      this.stepStack.push(record);
      await this.host.emit({
        type: "step_started",
        name,
        path: record.path,
      });
      const started = Date.now();
      try {
        const result = await body();
        record.duration_ms = Date.now() - started;
        await this.host.emit({
          type: "step_finished",
          name,
          path: record.path,
          status: record.status,
          duration_ms: record.duration_ms,
        });
        return result;
      } catch (error) {
        record.duration_ms = Date.now() - started;
        record.status = error instanceof TimeoutError ? "timeout" : "failed";
        record.error = toReportError(error, record.path);
        this.lastStepPath = record.path;
        await this.host.emit({
          type: "step_finished",
          name,
          path: record.path,
          status: record.status,
          duration_ms: record.duration_ms,
          error: record.error,
        });
        throw error;
      } finally {
        this.stepStack.pop();
      }
    });
  }

  get expect(): FixtureExpect {
    const fn = ((locator: Locator) => this.locatorMatchers(locator, false)) as FixtureExpect;
    fn.poll = <T>(read: () => T | Promise<T>, options?: ExpectOptions) =>
      this.pollMatchers(read, options, false);
    return fn;
  }

  async reject(
    operation: () => unknown | Promise<unknown>,
    expected: RejectExpected = {},
  ): Promise<unknown> {
    return this.guard(async () => {
      const location = callerLocation();
      let received: unknown;
      let didThrow = false;
      try {
        await operation();
      } catch (error) {
        didThrow = true;
        received = error;
      }
      if (!didThrow) {
        this.noteAssertion({
          matcher: "reject",
          expected: formatValue(expected),
          actual: "resolved",
          passed: false,
        });
        throw new AssertionError(
          "reject",
          undefined,
          expected,
          [
            "Expected the operation to reject.",
            `Expected: ${formatValue(expected)}`,
            "Received: resolved",
            `at ${displayLocation(location.file, location.line, location.column)}`,
            this.stepPathLine(),
          ].filter(Boolean).join("\n"),
        );
      }
      const record = received as { code?: unknown; message?: unknown };
      if (expected.code !== undefined && record.code !== expected.code) {
        this.noteAssertion({
          matcher: "reject",
          expected: formatValue(expected.code),
          actual: formatValue(record.code),
          passed: false,
        });
        throw new AssertionError(
          "reject",
          record.code,
          expected.code,
          [
            "Rejected with the wrong code.",
            `Expected: ${formatValue(expected.code)}`,
            `Received: ${formatValue(record.code)}`,
            `at ${displayLocation(location.file, location.line, location.column)}`,
            this.stepPathLine(),
          ].filter(Boolean).join("\n"),
        );
      }
      if (typeof expected.message === "string") {
        immediateExpect(String(record.message)).toContain(expected.message);
      }
      if (expected.message instanceof RegExp) {
        immediateExpect(String(record.message)).toMatch(expected.message);
      }
      this.noteAssertion({
        matcher: "reject",
        expected: formatValue(expected),
        actual: formatValue({ code: record.code, message: record.message }),
        passed: true,
      });
      return received;
    });
  }

  defer(cleanup: () => void | Promise<void>): void {
    this.defers.push(cleanup);
  }

  async attach(name: string, data: unknown): Promise<void> {
    await this.guard(async () => {
      await this.attachRaw(name, data);
    });
  }

  async attachRaw(name: string, data: unknown): Promise<AttachmentRef> {
    const payload = encodeAttachPayload(data);
    if (typeof data === "object" && data && "base64" in data && !("mimeType" in (data as object))) {
      if (name.endsWith(".png")) payload.mimeType = "image/png";
    }
    const path = `attachments/${this.specId}/${name}`;
    await this.host.attach(path, payload);
    if (payload.mimeType.startsWith("image/")) {
      rememberInline(this.specId, name, {
        dataUrl: `data:${payload.mimeType};base64,${payload.base64}`,
      });
    } else if (isPreviewable(payload.mimeType)) {
      rememberInline(this.specId, name, { text: previewText(data, payload) });
    }
    const ref: AttachmentRef = { name, path, mimeType: payload.mimeType };
    const current = this.stepStack[this.stepStack.length - 1];
    (current ? current.attachments : this.attachments).push(ref);
    return ref;
  }

  abort(reason: Error): void {
    this.aborted = true;
    this.abortError = reason;
    this.failurePhase = "timeout";
    for (const record of this.openActions) {
      record.status = "timeout";
      record.error = toReportError(reason, record.path);
    }
    this.openActions.clear();
  }

  allowCleanup(budgetMs = WEDGED_DEFER_BUDGET_MS): void {
    this.cleanupUntil = Date.now() + budgetMs;
    this.cleanupActive = true;
  }

  endCleanup(): void {
    this.cleanupActive = false;
  }

  /**
   * Record a driver call so a spec that never calls `t.step` still leaves a
   * trace of what it did. Retry loops silence themselves: a five-second poll
   * would otherwise bury the report in a hundred identical rows.
   */
  async act<T>(name: string, detail: string, op: () => T | Promise<T>): Promise<T> {
    if (this.actionSilence > 0) return this.guard(op);
    // Past the cap, keep recording failures: the action that finally breaks is
    // the one row worth having, and dropping it leaves nothing pointing at it.
    if (this.actionCount >= MAX_ACTIONS) {
      try {
        return await this.guard(op);
      } catch (error) {
        this.recordFailedAction(name, detail, error, 0);
        throw error;
      }
    }
    const parent = this.stepStack[this.stepStack.length - 1];
    const siblings = parent ? parent.steps : this.steps;
    // A hand-rolled poll calls the same driver over and over. Collapse the run
    // into one row so the trace reads as what happened, not as a stutter.
    const previous = siblings[siblings.length - 1];
    if (
      previous?.kind === "action" &&
      previous.status === "passed" &&
      previous.name === name &&
      previous.detail === detail
    ) {
      const started = Date.now();
      try {
        const result = await this.guard(op);
        previous.repeat = (previous.repeat ?? 1) + 1;
        previous.duration_ms += Date.now() - started;
        return result;
      } catch (error) {
        // A failure is its own row: it is the one attempt worth reading.
        const failed: StepRecord = {
          name,
          detail,
          kind: "action",
          path: previous.path,
          status: error instanceof TimeoutError ? "timeout" : "failed",
          duration_ms: Date.now() - started,
          steps: [],
          attachments: [],
          assertions: [],
          error: toReportError(error, previous.path),
        };
        siblings.push(failed);
        throw error;
      }
    }
    this.actionCount += 1;
    const record: StepRecord = {
      name,
      detail,
      kind: "action",
      path: [...this.stepStack.map((step) => step.name), name].join(" > "),
      status: "passed",
      duration_ms: 0,
      steps: [],
      attachments: [],
      assertions: [],
    };
    siblings.push(record);
    const started = Date.now();
    // A spec timeout abandons the in-flight call: `run()` stops awaiting the
    // body, so a record left at its optimistic default would serialise as an
    // instant success — the hung call rendered as the fastest one in the trace.
    this.openActions.add(record);
    try {
      const result = await this.guard(op);
      record.duration_ms = Date.now() - started;
      this.openActions.delete(record);
      return result;
    } catch (error) {
      record.duration_ms = Date.now() - started;
      record.status = error instanceof TimeoutError ? "timeout" : "failed";
      record.error = toReportError(error, record.path);
      this.openActions.delete(record);
      throw error;
    }
  }

  private recordFailedAction(
    name: string,
    detail: string,
    error: unknown,
    duration: number,
  ): void {
    const parent = this.stepStack[this.stepStack.length - 1];
    const path = [...this.stepStack.map((step) => step.name), name].join(" > ");
    (parent ? parent.steps : this.steps).push({
      name,
      detail,
      kind: "action",
      path,
      status: error instanceof TimeoutError ? "timeout" : "failed",
      duration_ms: duration,
      steps: [],
      attachments: [],
      assertions: [],
      error: toReportError(error, path),
    });
  }

  silenceActions(): void {
    this.actionSilence += 1;
  }

  resumeActions(): void {
    this.actionSilence = Math.max(0, this.actionSilence - 1);
  }

  async guard<T>(op: () => T | Promise<T>): Promise<T> {
    this.assertRunnable();
    const result = await op();
    this.assertRunnable();
    return result;
  }

  currentStepPath(): string | undefined {
    const current = this.stepStack[this.stepStack.length - 1];
    return current?.path ?? this.lastStepPath;
  }

  noteAssertion(entry: { matcher: string; expected: string; actual: string; passed: boolean }): void {
    const record: AssertionRecord = {
      ...entry,
      step: this.currentStepPath(),
    };
    const current = this.stepStack[this.stepStack.length - 1];
    (current ? current.assertions : this.assertions).push(record);
  }

  private assertRunnable(): void {
    if (this.cleanupActive) {
      if (this.cleanupUntil > 0 && Date.now() > this.cleanupUntil) {
        throw new TimeoutError("fixture cleanup budget exceeded");
      }
      return;
    }
    if (this.aborted && this.abortError) throw this.abortError;
  }

  private stepPathLine(): string {
    const path = this.currentStepPath();
    return path ? `in step ${JSON.stringify(path)}` : "";
  }

  private wrapApp(driver: LxAppDriver): TestApp {
    const page = this.wrapPage(driver.page);
    return {
      page,
      nav: guardObject(driver.nav, this, "nav."),
      info: () => this.act("app.info", "", () => driver.info()),
      pages: () => this.act("app.pages", "", () => driver.pages()),
      surfaceLayout: () => this.act("app.surfaceLayout", "", () => driver.surfaceLayout()),
      eval: (options) =>
        this.act("app.eval", summarise(options), () => driver.eval(this.withEvalBudget(options))),
    } as TestApp;
  }

  /**
   * The driver's own eval default is a flat few seconds, so a call that runs
   * long under load fails a spec that still had most of its budget left. An
   * eval inherits the spec's budget unless the spec pins its own.
   */
  private withEvalBudget<T extends { timeoutMs?: number }>(options: T): T {
    if (options && typeof options === "object" && options.timeoutMs === undefined) {
      return { ...options, timeoutMs: Math.min(this.specBudgetMs, MAX_EVAL_BUDGET_MS) };
    }
    return options;
  }

  private wrapPage(page: PageDriver): TestPage {
    const location = () => {
      const frame = callerLocation();
      return { source: frame.file, line: frame.line, column: frame.column };
    };
    // Every override lives in the proxy's `get` trap. Assigning onto the proxy
    // would write straight through to the real driver — `page.eval` would then
    // call itself forever.
    const overrides: Record<string, unknown> = {
      testId: (id: string) => this.locator(page, testIdSelector(id), location()),
      css: (selector: string) => this.locator(page, selector, location()),
      eval: (options: { script: string; timeoutMs?: number }) =>
        this.act("page.eval", summarise(options), () => page.eval(this.withEvalBudget(options))),
    };
    const guarded = guardObject(page, this, "page.", Object.keys(overrides));
    return new Proxy(guarded, {
      get(target, prop, receiver) {
        if (typeof prop === "string" && prop in overrides) return overrides[prop];
        return Reflect.get(target, prop, receiver);
      },
    }) as unknown as TestPage;
  }

  private locator(page: PageDriver, selector: string, location: SourceLocation): Locator {
    return new PageLocator(
      page as unknown as PageLike,
      (fn) => this.guard(fn),
      <T,>(verb: string, detail: string, op: () => Promise<T>) => this.act(verb, detail, op),
      selector,
      location,
    );
  }

  private locatorMatchers(locator: Locator, inverted: boolean): LocatorMatchers {
    const self = {
      toBeVisible: (options: ExpectOptions | undefined) =>
        this.retryLocator(locator, "toBeVisible", inverted, options, inverted ? "not visible" : "visible"),
      toHaveText: (expected: string | RegExp, options?: ExpectOptions) =>
        this.retryLocator(locator, "toHaveText", inverted, options, expected),
      toHaveCount: (expected: number, options?: ExpectOptions) =>
        this.retryLocator(locator, "toHaveCount", inverted, options, expected),
      toHaveValue: (expected: string | RegExp, options?: ExpectOptions) =>
        this.retryLocator(locator, "toHaveValue", inverted, options, expected),
    };
    Object.defineProperty(self, "not", {
      get: () => this.locatorMatchers(locator, !inverted),
    });
    return self as LocatorMatchers;
  }

  private pollMatchers<T>(
    read: () => T | Promise<T>,
    options: ExpectOptions | undefined,
    inverted: boolean,
  ): RetryMatchers<T> {
    const run = (matcher: string, expected?: unknown) =>
      this.retryPoll(read, matcher, inverted, options, expected);
    const fixture = this;
    const self: Partial<RetryMatchers<T>> = {
      toBe: (expected: unknown) => run("toBe", expected),
      toEqual: (expected: unknown) => run("toEqual", expected),
      toContain: (expected: unknown) => run("toContain", expected),
      toMatch: (expected: string | RegExp) => run("toMatch", expected),
      toBeTruthy: () => run("toBeTruthy"),
      toBeFalsy: () => run("toBeFalsy"),
      toBeDefined: () => run("toBeDefined"),
      toBeUndefined: () => run("toBeUndefined"),
      toBeInstanceOf: (expected: Function) => run("toBeInstanceOf", expected),
    };
    Object.defineProperty(self, "not", {
      get: () => fixture.pollMatchers(read, options, !inverted),
    });
    return self as RetryMatchers<T>;
  }

  private async retryLocator(
    locator: Locator,
    matcher: string,
    inverted: boolean,
    options: ExpectOptions | undefined,
    expected?: unknown,
  ): Promise<void> {
    await this.guard(async () => {
      const frame = callerLocation();
      const location = { source: frame.file, line: frame.line, column: frame.column };
      const timeout = options?.timeout ?? DEFAULT_ACTION_TIMEOUT_MS;
      const interval = options?.interval ?? DEFAULT_POLL_INTERVAL_MS;
      const started = Date.now();
      let lastResolved: LocatorResolve | undefined;
      let lastError: unknown;
      pushAssertionSilence();
      this.silenceActions();
      try {
        while (Date.now() - started < timeout) {
          try {
            lastResolved = await resolveLocator(locator);
            matchLocator(locator, lastResolved, matcher, expected, inverted);
            this.noteAssertion({
              matcher: inverted ? `not.${matcher}` : matcher,
              expected: formatValue(expected),
              actual: formatValue(locatorActual(matcher, lastResolved)),
              passed: true,
            });
            return;
          } catch (error) {
            if (error instanceof TimeoutError || this.aborted) throw error;
            lastError = error;
          }
          if (Date.now() - started >= timeout) break;
          await sleep(interval);
          if (this.aborted && this.abortError) throw this.abortError;
        }
      } finally {
        popAssertionSilence();
        this.resumeActions();
      }
      const duration = Date.now() - started;
      throw this.retryFailure({
        matcher: inverted ? `not.${matcher}` : matcher,
        expected,
        actual: locatorActual(matcher, lastResolved),
        duration,
        location,
        lastError,
        extra: lastResolved && locator instanceof PageLocator ? locator.missText(lastResolved) : undefined,
      });
    });
  }

  private async retryPoll<T>(
    read: () => T | Promise<T>,
    matcher: string,
    inverted: boolean,
    options: ExpectOptions | undefined,
    expected?: unknown,
  ): Promise<void> {
    await this.guard(async () => {
      const frame = callerLocation();
      const location = { source: frame.file, line: frame.line, column: frame.column };
      const timeout = options?.timeout ?? DEFAULT_ACTION_TIMEOUT_MS;
      const interval = options?.interval ?? DEFAULT_POLL_INTERVAL_MS;
      const started = Date.now();
      let lastActual: unknown;
      let lastError: unknown;
      pushAssertionSilence();
      this.silenceActions();
      try {
        while (Date.now() - started < timeout) {
          try {
            lastActual = await read();
            applyMatcher(matcher, lastActual, expected, inverted);
            this.noteAssertion({
              matcher: inverted ? `not.${matcher}` : matcher,
              expected: formatValue(expected),
              actual: formatValue(lastActual),
              passed: true,
            });
            return;
          } catch (error) {
            if (error instanceof TimeoutError || this.aborted) throw error;
            lastError = error;
          }
          if (Date.now() - started >= timeout) break;
          await sleep(interval);
          if (this.aborted && this.abortError) throw this.abortError;
        }
      } finally {
        popAssertionSilence();
        this.resumeActions();
      }
      throw this.retryFailure({
        matcher: inverted ? `not.${matcher}` : matcher,
        expected,
        actual: lastActual,
        duration: Date.now() - started,
        location,
        lastError,
      });
    });
  }

  private retryFailure(input: {
    matcher: string;
    expected: unknown;
    actual: unknown;
    duration: number;
    location: SourceLocation;
    lastError: unknown;
    extra?: string;
  }): AssertionError {
    const where = displayLocation(input.location.source, input.location.line, input.location.column);
    const last =
      input.lastError instanceof AssertionError
        ? input.lastError.message
        : input.lastError instanceof Error
          ? input.lastError.message
          : undefined;
    const lines = [
      `Timed out after ${input.duration}ms retrying ${input.matcher}.`,
      input.extra,
      `Expected: ${formatValue(input.expected)}`,
      `Received: ${formatValue(input.actual)}`,
      `Retried for ${input.duration}ms`,
      `at ${where}`,
      this.stepPathLine(),
      last && last !== `Expected: ${formatValue(input.expected)}` ? last : undefined,
    ].filter((line): line is string => Boolean(line));
    this.noteAssertion({
      matcher: input.matcher,
      expected: formatValue(input.expected),
      actual: formatValue(input.actual),
      passed: false,
    });
    return new AssertionError(input.matcher, input.actual, input.expected, lines.join("\n"));
  }
}

function resolveLocator(locator: Locator): Promise<LocatorResolve> {
  if (locator instanceof PageLocator) return locator.resolve();
  throw new Error("t.expect() requires a locator from page.testId() or page.css()");
}

function locatorActual(matcher: string, resolved: LocatorResolve | undefined): unknown {
  if (!resolved) return undefined;
  if (matcher === "toHaveCount") return resolved.count;
  if (matcher === "toHaveText") return resolved.text;
  if (matcher === "toHaveValue") return resolved.value;
  if (matcher === "toBeVisible") return resolved.kind;
  return resolved.kind;
}

function matchLocator(
  locator: Locator,
  resolved: LocatorResolve,
  matcher: string,
  expected: unknown,
  inverted: boolean,
): void {
  if (matcher === "toBeVisible") {
    const pass = resolved.visible && resolved.kind === "unique";
    if (pass === inverted) {
      const detail =
        locator instanceof PageLocator ? locator.missText(resolved) : `resolved to ${resolved.kind}`;
      throw new AssertionError(
        inverted ? "not.toBeVisible" : "toBeVisible",
        resolved.kind,
        inverted ? "not visible" : "visible",
        `${detail}\nExpected: ${inverted ? "not " : ""}visible\nReceived: ${formatValue(resolved.kind)}`,
      );
    }
    return;
  }
  if (matcher === "toHaveCount") {
    applyMatcher("toBe", resolved.count, expected, inverted);
    return;
  }
  if (matcher === "toHaveText") {
    if (expected instanceof RegExp) applyMatcher("toMatch", resolved.text, expected, inverted);
    else applyMatcher("toBe", resolved.text, expected, inverted);
    return;
  }
  if (matcher === "toHaveValue") {
    if (expected instanceof RegExp) applyMatcher("toMatch", resolved.value ?? "", expected, inverted);
    else applyMatcher("toBe", resolved.value, expected, inverted);
  }
}

function guardObject<T extends object>(
  target: T,
  fixture: LiveFixture,
  path: string,
  skip: PropertyKey[] = [],
): T {
  const cache = new Map<PropertyKey, unknown>();
  return new Proxy(target, {
    get(obj, prop, receiver) {
      if (skip.includes(prop)) return Reflect.get(obj, prop, receiver);
      if (cache.has(prop)) return cache.get(prop);
      const value = Reflect.get(obj, prop, receiver);
      if (typeof value === "function") {
        const name = `${path}${String(prop)}`;
        const bound = (...args: unknown[]) =>
          fixture.act(name, summarise(args[0]), () => value.apply(obj, args));
        cache.set(prop, bound);
        return bound;
      }
      if (value && typeof value === "object") {
        const nested = guardObject(value as object, fixture, `${path}${String(prop)}.`);
        cache.set(prop, nested);
        return nested;
      }
      return value;
    },
  }) as T;
}

/** The one detail worth showing beside an action: what it acted on. */
function summarise(input: unknown): string {
  if (input === undefined || input === null) return "";
  if (typeof input === "string") return truncate(input, 80);
  if (typeof input !== "object") return String(input);
  const record = input as Record<string, unknown>;
  for (const key of ["css", "page", "script", "url", "text", "id", "name"]) {
    const value = record[key];
    if (typeof value === "string" && value.length > 0) {
      return truncate(key === "script" ? value.replace(/\s+/g, " ").trim() : value, 80);
    }
  }
  return truncate(formatValue(input), 80);
}

/** Text and JSON preview in the report; anything else is named, not embedded. */
function isPreviewable(mimeType: string): boolean {
  return mimeType.startsWith("text/") || mimeType.startsWith("application/json");
}

const MAX_PREVIEW_CHARS = 20_000;

function previewText(data: unknown, payload: { mimeType: string }): string {
  const raw = typeof data === "string"
    ? data
    : payload.mimeType.startsWith("application/json")
      ? safeJson(data)
      : String(data);
  return raw.length > MAX_PREVIEW_CHARS
    ? `${raw.slice(0, MAX_PREVIEW_CHARS)}\n… truncated, open the file for the rest`
    : raw;
}

function safeJson(data: unknown): string {
  try {
    return JSON.stringify(data, null, 2) ?? String(data);
  } catch {
    return String(data);
  }
}

export function toReportError(error: unknown, step?: string): {
  name: string;
  message: string;
  stack?: string;
  matcher?: string;
  expected?: string;
  actual?: string;
  location?: string;
  step?: string;
} {
  if (error instanceof AssertionError) {
    const stack = remapStack(error.stack);
    return {
      name: error.name,
      message: error.message,
      stack,
      matcher: error.matcher,
      expected: formatValue(error.expected),
      actual: formatValue(error.actual),
      location: firstLocation(stack),
      step,
    };
  }
  if (error instanceof Error) {
    const stack = remapStack(error.stack);
    return {
      name: error.name,
      message: error.message,
      stack,
      location: firstLocation(stack),
      step,
    };
  }
  return { name: "Error", message: String(error), step };
}

function firstLocation(stack: string | undefined): string | undefined {
  const frame = parseFrames(stack)[0];
  return frame ? `${frame.file}:${frame.line}:${frame.column}` : undefined;
}

export function protocolStatus(status: SpecStatus): "passed" | "failed" | "skipped" {
  if (status === "skipped" || status === "xfail") return status === "skipped" ? "skipped" : "passed";
  if (status === "passed") return "passed";
  return "failed";
}
