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
import type { Redactor } from "./redact.js";
import { rememberInline } from "./report.js";
import { NetworkScope, wrapNetwork } from "./network.js";
import { ActionDeadline, TimeoutError } from "./deadline.js";
import { explainRemoteError, functionDetail, logicScript, pageScript, type RemoteTarget } from "./remote.js";
import { callerLocation, displayLocation, isFrameworkFrame, parseFrames, resolveOrigin } from "./ids.js";
import {
  PageLocator,
  sleep,
  testIdSelector,
  type LocatorResolve,
  type PageLike,
} from "./locator.js";
import type {
  Apps,
  ArgOptions,
  AttachmentRef,
  ExpectOptions,
  Fixture,
  FixtureExpect,
  AssertionRecord,
  Locator,
  LocatorMatchers,
  LocatorOptions,
  TestAutomation,
  RejectExpected,
  RetryMatchers,
  ReportError,
  SourceLocation,
  StepRecord,
  TestApp,
  TestPage,
  JsonValue,
  LogicScope,
  PageDataOptions,
  WaitForOptions,
} from "./types.js";
import {
  DEFAULT_ACTION_TIMEOUT_MS,
  DEFAULT_POLL_INTERVAL_MS,
  DEFAULT_SPEC_TIMEOUT_MS,
  MAX_ACTIONS,
  MAX_EVAL_BUDGET_MS,
  WEDGED_DEFER_BUDGET_MS,
} from "./version.js";
import type { HostRunAutomation as Automation, LxAppDriver, NavDriver, NavWaitOptions, PageDriver } from "@lingxia/types/automation";

export { TimeoutError };

/**
 * Thrown by `t.skip()`. It unwinds the spec like any throw, but the runtime
 * grades it `skipped`, never `failed` — and not `xfail` under `spec.fail`.
 * Internal: a spec never needs to name it.
 */
export class SkipSignal extends Error {
  override readonly name = "SkipSignal";
  constructor(readonly reason: string) {
    super(`skipped: ${reason}`);
  }
}

export type FailurePhase = "beforeEach" | "body" | "defer" | "forensics" | "timeout";

export class LiveFixture implements Fixture {
  readonly apps: Apps;
  readonly automation: TestAutomation;
  readonly args: Readonly<Record<string, string | undefined>>;
  readonly steps: StepRecord[] = [];
  /**
   * `lx.*` members this spec's evals actually reached. Collected so the report
   * can tell an exercised capability from a declared one.
   */
  readonly observed = new Set<string>();
  readonly assertions: AssertionRecord[] = [];
  readonly attachments: AttachmentRef[] = [];
  readonly defers: Array<() => void | Promise<void>> = [];
  private closed = false;
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
  /** Set by `t.skip()`; survives a body that catches the signal. */
  skipReason: string | undefined;
  private readonly stepStack: StepRecord[] = [];
  private rawApp: LxAppDriver;
  /** When this spec's budget started; the runtime arms its timer right after construction. */
  private readonly startedAt: number;
  private readonly networkScope = new NetworkScope();
  /** When the spec's own timer fires; see `budgetRoom()`. */
  private readonly specDeadline: number;

  constructor(
    readonly specId: string,
    rawApp: LxAppDriver,
    private readonly host: ResolvedHost,
    args: Record<string, string>,
    automation: Automation,
    private readonly specBudgetMs: number = DEFAULT_SPEC_TIMEOUT_MS,
    private readonly redactor?: Redactor,
  ) {
    this.rawApp = rawApp;
    this.startedAt = Date.now();
    this.args = args;
    this.specDeadline = Date.now() + specBudgetMs;
    setAssertionSink((entry) => this.noteAssertion(entry));
    const root = guardObject(automation, this, "", ["lxapp"]);
    this.automation = new Proxy(root, {
      get: (target, prop) => prop === "lxapp"
        ? (appId?: string) => {
          this.assertRunnable();
          return this.wrapApp(appId === undefined ? automation.lxapp() : automation.lxapp(appId));
        }
        : Reflect.get(target, prop),
    }) as TestAutomation;
    this.apps = { lxapp: (appId: string) => this.automation.lxapp(appId) };
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
        if (error instanceof SkipSignal) {
          record.status = "skipped";
          await this.host.emit({
            type: "step_finished",
            name,
            path: record.path,
            status: record.status,
            duration_ms: record.duration_ms,
          });
          throw error;
        }
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

  arg(name: string, options: { required: false; default?: undefined }): string | undefined;
  arg(name: string, options?: ArgOptions): string;
  arg(name: string, options: ArgOptions = {}): string | undefined {
    const value = this.args[name];
    if (value !== undefined) return value;
    if (options.default !== undefined) return options.default;
    if (options.required === false) return undefined;
    throw new Error(
      `Missing test arg "${name}": pass --arg ${name}=<value> ` +
        `(or --secret-arg ${name}=<value>) to lxdev test`,
    );
  }

  waitFor<T>(
    read: () => T | Promise<T>,
    accept: (value: T) => boolean = Boolean,
    options: WaitForOptions = {},
  ): Promise<T> {
    const location = callerLocation();
    const requested = options.timeout ?? DEFAULT_ACTION_TIMEOUT_MS;
    // Past the spec budget the spec timer fires first and reports a bare
    // timeout; ending a little earlier keeps the last value in the error.
    const remaining = this.specBudgetMs - (Date.now() - this.startedAt) - WAIT_FOR_MARGIN_MS;
    const timeout = Math.max(0, Math.min(requested, remaining));
    const interval = options.interval ?? DEFAULT_POLL_INTERVAL_MS;
    const retryIf = options.retryIf ?? isRetryableReadError;
    const detail = truncate(read.name || functionDetail(read), 80);
    return this.act("waitFor", detail, async () => {
      const started = Date.now();
      let attempts = 0;
      let hasValue = false;
      let lastValue: T | undefined;
      let lastError: unknown;
      this.silenceActions();
      try {
        for (;;) {
          attempts += 1;
          try {
            const value = await read();
            lastValue = value;
            hasValue = true;
            lastError = undefined;
            if (accept(value)) return value;
          } catch (error) {
            if (error instanceof TimeoutError || error instanceof SkipSignal || this.aborted) throw error;
            if (!retryIf(error)) throw error;
            lastError = error;
          }
          if (Date.now() - started + interval > timeout) break;
          await sleep(interval);
          if (this.aborted && this.abortError) throw this.abortError;
        }
      } finally {
        this.resumeActions();
      }
      const last = lastError !== undefined
        ? `Last error: ${errorLine(lastError)}`
        : hasValue
          ? `Last value: ${formatValue(lastValue)}${accept === Boolean ? " (waiting for a truthy value)" : " (rejected by accept)"}`
          : "No read completed.";
      throw new Error(
        [
          `t.waitFor timed out after ${Date.now() - started}ms (${attempts} ${attempts === 1 ? "read" : "reads"}).`,
          last,
          timeout < requested ? `Clamped from ${requested}ms to the spec's remaining budget.` : undefined,
          `at ${displayLocation(location.file, location.line, location.column)}`,
          this.stepPathLine(),
        ].filter(Boolean).join("\n"),
      );
    });
  }

  skip(reason: string): never {
    // Cleanup runs after the verdict; a skip there cannot mean anything.
    if (this.cleanupActive) throw new Error("t.skip cannot be called during cleanup (t.defer or afterEach)");
    const text = typeof reason === "string" && reason.trim().length > 0 ? reason : "skipped at runtime";
    this.skipReason ??= text;
    throw new SkipSignal(text);
  }

  async attach(name: string, data: unknown): Promise<void> {
    await this.guard(async () => {
      await this.attachRaw(name, data);
    });
  }

  async attachRaw(name: string, input: unknown): Promise<AttachmentRef> {
    // Declared secrets are masked in the data itself, before it is encoded
    // or previewed, so neither the file nor the report carries them.
    const data = this.redactor ? this.redactor.attachment(input) : input;
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
    await this.host.emit({ type: "step_started", name, path: record.path });
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
    } finally {
      await this.host.emit({ type: "step_finished", name, path: record.path,
        status: record.status, duration_ms: record.duration_ms, error: record.error });
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

  /**
   * Milliseconds an action or assertion may still take. Short of the spec's
   * own deadline by a margin, so the action fails first and names its step
   * instead of losing the race to the anonymous spec timeout. During cleanup
   * the cleanup budget bounds it instead.
   */
  budgetRoom(): number {
    if (this.cleanupActive) return this.cleanupUntil > 0 ? this.cleanupUntil - Date.now() : Number.POSITIVE_INFINITY;
    const margin = Math.min(250, Math.floor(this.specBudgetMs / 20));
    return this.specDeadline - margin - Date.now();
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

  close(): void { this.closed = true; }

  private assertRunnable(): void {
    if (this.closed) throw new Error("Test fixture is closed");
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
    const fixture = this;
    const page = this.wrapPage(driver.page);
    return {
      page,
      nav: guardObject(landingNav(driver.nav), this, "nav."),
      // Lazy and non-throwing: a host without test routing fails the call,
      // never the `t.app` or `t.app.network` read.
      get network() {
        return wrapNetwork(() => driver.network, fixture, fixture.networkScope);
      },
      info: () => this.act("app.info", "", () => driver.info()),
      pages: () => this.act("app.pages", "", () => driver.pages()),
      surfaceLayout: () => this.act("app.surfaceLayout", "", () => driver.surfaceLayout()),
      eval: (options: unknown, ...args: unknown[]) => {
        if (typeof options === "function") {
          const script = logicScript(options, args);
          return this.act("app.eval", summarise(functionDetail(options)), () =>
            remote("t.app.eval", "logic", () => this.evalLogic(driver, { script })));
        }
        return this.act("app.eval", summarise(options), () =>
          this.evalLogic(driver, options as { script: string; timeoutMs?: number }));
      },
      pageData: (options?: PageDataOptions) => {
        const target = options?.page;
        return this.act("app.pageData", target ?? "", async () => {
          const path = target === undefined ? undefined : await pagePath(driver, target);
          const script = logicScript(readPageData, [target ?? null, path ?? null], "t.app.pageData");
          return remote("t.app.pageData", "logic", () => this.evalLogic(driver, { script }));
        });
      },
      callPage: (method: string, ...args: JsonValue[]) =>
        this.act("app.callPage", method, () => {
          const script = logicScript(callPageMethod, [method, args], "t.app.callPage");
          return remote("t.app.callPage", "logic", () => this.evalLogic(driver, { script }));
        }),
    } as TestApp;
  }

  /**
   * Logic eval that asks the runtime which `lx.*` the script reached and
   * hands the caller only the value — the observation is the report's
   * business, not the spec author's, and specs must not have to opt in for
   * their coverage to be measured.
   */
  private async evalLogic(driver: LxAppDriver, options: { script: string; timeoutMs?: number }): Promise<unknown> {
    const result = (await driver.eval({
      ...this.withEvalBudget(options),
      captureCalls: true,
    })) as unknown;
    // The marker, not the shape, identifies the envelope: a script that
    // returns undefined loses its `value` key on the wire, and sniffing
    // for that key handed the envelope itself back as the result.
    if (result && typeof result === "object" && (result as { __lxEval?: unknown }).__lxEval === 1) {
      const { calls, value } = result as { calls?: unknown; value?: unknown };
      if (Array.isArray(calls)) {
        for (const call of calls) {
          if (typeof call === "string") this.observed.add(call);
        }
      }
      return value;
    }
    // An older runtime ignores `captureCalls` and returns the bare value.
    return result;
  }

  /**
   * The driver's own eval default is a flat few seconds, so a call that runs
   * long under load fails a spec that still had most of its budget left. An
   * eval gets a share of the spec's budget instead — a third, capped — never
   * all of it: a call allowed to run the full budget leaves the spec no room
   * to retry, so one stalled call takes the whole spec down with it.
   */
  private withEvalBudget<T extends { timeoutMs?: number }>(options: T): T {
    if (options && typeof options === "object" && options.timeoutMs === undefined) {
      const share = Math.floor(this.specBudgetMs / 3);
      return { ...options, timeoutMs: Math.max(1, Math.min(share, MAX_EVAL_BUDGET_MS)) };
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
      testId: (id: string, options?: LocatorOptions) => this.locator(page, testIdSelector(id), location(), options),
      css: (selector: string, options?: LocatorOptions) => this.locator(page, selector, location(), options),
      eval: (options: unknown, ...args: unknown[]) => {
        if (typeof options === "function") {
          const script = pageScript(options, args);
          return this.act("page.eval", summarise(functionDetail(options)), () =>
            remote("t.app.page.eval", "page", () => page.eval(this.withEvalBudget<{ script: string; timeoutMs?: number }>({ script }))));
        }
        return this.act("page.eval", summarise(options), () =>
          page.eval(this.withEvalBudget(options as { script: string; timeoutMs?: number })));
      },
    };
    const guarded = guardObject(page, this, "page.", Object.keys(overrides));
    return new Proxy(guarded, {
      get(target, prop, receiver) {
        if (typeof prop === "string" && prop in overrides) return overrides[prop];
        return Reflect.get(target, prop, receiver);
      },
    }) as unknown as TestPage;
  }

  private locator(page: PageDriver, selector: string, location: SourceLocation, options?: LocatorOptions): Locator {
    return new PageLocator(
      page as unknown as PageLike,
      (fn) => this.guard(fn),
      <T,>(verb: string, detail: string, op: () => Promise<T>) => this.act(verb, detail, op),
      selector,
      location,
      options,
      () => this.budgetRoom(),
    );
  }

  private locatorMatchers(locator: Locator, inverted: boolean): LocatorMatchers {
    const self = {
      toBeVisible: (options: ExpectOptions | undefined) =>
        this.retryLocator(locator, "toBeVisible", inverted, options, inverted ? "not visible" : "visible"),
      toBeHidden: (options?: ExpectOptions) => this.retryLocator(locator, "toBeHidden", inverted, options, true),
      toBeAttached: (options?: ExpectOptions) => this.retryLocator(locator, "toBeAttached", inverted, options, true),
      toBeEnabled: (options?: ExpectOptions) => this.retryLocator(locator, "toBeEnabled", inverted, options, true),
      toBeDisabled: (options?: ExpectOptions) => this.retryLocator(locator, "toBeDisabled", inverted, options, true),
      toBeEditable: (options?: ExpectOptions) => this.retryLocator(locator, "toBeEditable", inverted, options, true),
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
      toBeGreaterThan: (expected: number) => run("toBeGreaterThan", expected),
      toBeGreaterThanOrEqual: (expected: number) => run("toBeGreaterThanOrEqual", expected),
      toBeLessThan: (expected: number) => run("toBeLessThan", expected),
      toBeLessThanOrEqual: (expected: number) => run("toBeLessThanOrEqual", expected),
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
      const deadline = new ActionDeadline(options?.timeout ?? DEFAULT_ACTION_TIMEOUT_MS, this.budgetRoom());
      const interval = options?.interval ?? DEFAULT_POLL_INTERVAL_MS;
      const context = () => this.deadlineContext(inverted ? `not.${matcher}` : matcher, location);
      let lastResolved: LocatorResolve | undefined;
      let lastError: unknown;
      pushAssertionSilence();
      this.silenceActions();
      try {
        while (!deadline.expired()) {
          try {
            lastResolved = await resolveLocator(locator, deadline, context);
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
          if (deadline.expired()) break;
          await sleep(Math.min(interval, Math.max(1, deadline.remaining())));
          if (this.aborted && this.abortError) throw this.abortError;
        }
      } finally {
        popAssertionSilence();
        this.resumeActions();
      }
      const duration = deadline.elapsed();
      throw this.retryFailure({
        clampNote: deadline.clampNote(),
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
      const deadline = new ActionDeadline(options?.timeout ?? DEFAULT_ACTION_TIMEOUT_MS, this.budgetRoom());
      const interval = options?.interval ?? DEFAULT_POLL_INTERVAL_MS;
      const context = () => this.deadlineContext(`poll ${inverted ? "not." : ""}${matcher}`, location);
      let lastActual: unknown;
      let lastError: unknown;
      pushAssertionSilence();
      this.silenceActions();
      try {
        while (!deadline.expired()) {
          try {
            lastActual = await deadline.call("t.expect.poll read", read, context);
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
          if (deadline.expired()) break;
          await sleep(Math.min(interval, Math.max(1, deadline.remaining())));
          if (this.aborted && this.abortError) throw this.abortError;
        }
      } finally {
        popAssertionSilence();
        this.resumeActions();
      }
      throw this.retryFailure({
        clampNote: deadline.clampNote(),
        matcher: inverted ? `not.${matcher}` : matcher,
        expected,
        actual: lastActual,
        duration: deadline.elapsed(),
        location,
        lastError,
      });
    });
  }

  private deadlineContext(matcher: string, location: SourceLocation): string {
    return [
      `while retrying ${matcher}`,
      `at ${displayLocation(location.source, location.line, location.column)}`,
      this.stepPathLine(),
    ].filter(Boolean).join("\n");
  }

  private retryFailure(input: {
    matcher: string;
    expected: unknown;
    actual: unknown;
    duration: number;
    location: SourceLocation;
    lastError: unknown;
    extra?: string;
    clampNote?: string;
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
      input.clampNote,
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

/** Room left before the spec timer, so `t.waitFor` reports its own failure. */
const WAIT_FOR_MARGIN_MS = 100;

/**
 * `t.waitFor` retries a read that says "not yet" by throwing, but not one
 * that is simply wrong: a TypeError, ReferenceError or SyntaxError will not
 * fix itself, and retrying it only hides the bug behind a timeout.
 */
function isRetryableReadError(error: unknown): boolean {
  if (error instanceof AssertionError) return true;
  const name = error instanceof Error ? error.name : undefined;
  return name !== "TypeError" && name !== "ReferenceError" && name !== "SyntaxError";
}

function errorLine(error: unknown): string {
  return error instanceof Error ? `${error.name}: ${error.message}` : formatValue(error);
}

async function remote<T>(api: string, target: RemoteTarget, op: () => Promise<T>): Promise<T> {
  try {
    return await op();
  } catch (error) {
    throw explainRemoteError(error, api, target);
  }
}

/** A configured page name to its path, so Logic can match `page.route`. */
async function pagePath(driver: LxAppDriver, page: string): Promise<string | undefined> {
  try {
    const pages = await driver.pages();
    return pages.find((entry) => entry.name === page)?.path;
  } catch {
    return undefined;
  }
}

// The two functions below are sent to app Logic as source text: they must
// stay self-contained.

function readPageData(scope: LogicScope, name: string | null, path: string | null): unknown {
  const pages = scope.getCurrentPages();
  const norm = (route: string) => String(route).replace(/^\/+/, "").split("?")[0];
  const page = name === null
    ? pages[pages.length - 1]
    : pages.slice().reverse().find((candidate) => {
      const route = norm(candidate.route);
      return route === norm(name) || (path !== null && route === norm(path));
    });
  if (!page) {
    const open = pages.map((candidate) => candidate.route).join(", ") || "none";
    throw new Error(name === null
      ? "t.app.pageData: no page is open"
      : `t.app.pageData: page ${JSON.stringify(name)} is not in the page stack (open: ${open})`);
  }
  return page.data;
}

async function callPageMethod(scope: LogicScope, method: string, args: unknown[]): Promise<unknown> {
  const pages = scope.getCurrentPages();
  const page = pages[pages.length - 1];
  if (!page) throw new Error("t.app.callPage: no page is open");
  const member = page[method];
  if (typeof member !== "function") {
    throw new Error(`t.app.callPage: page ${JSON.stringify(page.route)} has no method ${JSON.stringify(method)}`);
  }
  return await member.apply(page, args);
}

function resolveLocator(locator: Locator, deadline: ActionDeadline, context: () => string): Promise<LocatorResolve> {
  if (locator instanceof PageLocator) return deadline.call("locator read", () => locator.resolve(), context);
  throw new Error("t.expect() requires a locator from page.testId() or page.css()");
}

function locatorActual(matcher: string, resolved: LocatorResolve | undefined): unknown {
  if (!resolved) return undefined;
  if (matcher === "toBeHidden") return !resolved.visible;
  if (matcher === "toBeAttached") return resolved.attached;
  if (matcher === "toBeEnabled") return resolved.enabled;
  if (matcher === "toBeDisabled") return resolved.enabled === false;
  if (matcher === "toBeEditable") return resolved.editable;
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
  if (["toBeHidden", "toBeAttached", "toBeEnabled", "toBeDisabled", "toBeEditable"].includes(matcher)) {
    if (["toBeEnabled", "toBeDisabled", "toBeEditable"].includes(matcher) && resolved.count !== 1) {
      throw new Error(`Expected one attached element, received ${resolved.count}`);
    }
    applyMatcher("toBe", locatorActual(matcher, resolved), true, inverted);
    return;
  }
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

const LANDING_NAV = new Set<PropertyKey>(["to", "redirect", "switchTab", "relaunch", "back"]);

/**
 * A spec almost always wants the page it navigated to, so fixture navigation
 * waits for the landed page's `onReady` unless the caller picks `waitUntil`
 * (`'commit'` resolves once the stack changed).
 */
function untilReady<T extends NavWaitOptions>(options?: T): T {
  if (options && typeof options === "object" && options.waitUntil !== undefined) return options;
  return { ...(options ?? {}), waitUntil: "ready" } as T;
}

function landingNav(nav: NavDriver): NavDriver {
  return new Proxy(nav, {
    get(target, prop) {
      const value = Reflect.get(target, prop, target);
      if (typeof value !== "function") return value;
      if (!LANDING_NAV.has(prop)) return value.bind(target);
      return (options?: NavWaitOptions) => value.call(target, untilReady(options));
    },
  });
}

function guardObject<T extends object>(
  target: T,
  fixture: LiveFixture,
  path: string,
  skip: PropertyKey[] = [],
): T {
  const cache = new Map<PropertyKey, unknown>();
  return new Proxy(target, {
    get(obj, prop) {
      if (skip.includes(prop)) return Reflect.get(obj, prop, obj);
      if (cache.has(prop)) return cache.get(prop);
      const value = Reflect.get(obj, prop, obj);
      // Rong native class instances are callable objects too. Getter results
      // are driver namespaces; rebinding them as methods loses their members.
      let owner: object | null = obj;
      let accessor = false;
      while (owner) {
        const descriptor = Object.getOwnPropertyDescriptor(owner, prop);
        if (descriptor) { accessor = typeof descriptor.get === "function"; break; }
        owner = Object.getPrototypeOf(owner);
      }
      if (typeof value === "function" && !accessor) {
        const name = `${path}${String(prop)}`;
        const bound = (...args: unknown[]) =>
          fixture.act(name, summarise(args[0]), () => value.apply(obj, args));
        cache.set(prop, bound);
        return bound;
      }
      if (value && (typeof value === "object" || typeof value === "function")) {
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

export function toReportError(error: unknown, step?: string): ReportError {
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
    const details = error as Error & { code?: unknown; data?: unknown };
    let data: unknown;
    try { if (details.data !== undefined) data = JSON.parse(JSON.stringify(details.data)); } catch { /* Non-JSON diagnostic data must not break reporting. */ }
    return {
      code: typeof details.code === "string" ? details.code : undefined,
      data,
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
  const frames = parseFrames(stack);
  const frame = frames.length ? resolveOrigin(frames) : undefined;
  return frame && !isFrameworkFrame(frame.file) ? `${frame.file}:${frame.line}:${frame.column}` : undefined;
}
