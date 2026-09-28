import {
  AssertionError,
  applyMatcher,
  check,
  popAssertionSilence,
  pushAssertionSilence,
  setExpectScope,
} from "./expect.js";
import { firstDifference, formatValue, truncate } from "./format.js";
import type { PendingWork } from "./pending.js";
import { encodeAttachPayload, remapStack, type ResolvedHost } from "./host.js";
import type { Redactor } from "./redact.js";
import { rememberInline } from "./report.js";
import { NetworkScope, wrapNetwork } from "./network.js";
import { ScenarioScope, installScenario } from "./mock.js";
import { ClockScope, wrapClock } from "./clock.js";
import { watchDialogs, wrapDialogs, type DialogWatch } from "./dialogs.js";
import { activeOpenApi } from "./openapi.js";
import { ActionDeadline, TimeoutError, asFixtureTimeout } from "./deadline.js";
import { isTransientTransportError, matchesErrorCode } from "./deadline.js";
import { checkJsonArgs, explainRemoteError, isRetryableRemoteError, functionDetail, logicScript, pageScript, type RemoteTarget } from "./remote.js";
import { callerLocation, displayLocation, isFrameworkFrame, parseFrames, resolveOrigin } from "./ids.js";
import {
  PageLocator,
  normalizeText,
  sleep,
  testIdSelector,
  type LocatorResolve,
  type PageLike,
  type QueryMatch,
  type LocatorTarget,
} from "./locator.js";
import type {
  ArgOptions,
  ExpectOptions,
  Fixture,
  StepScope,
  Locator,
  LocatorMatchers,
  TestAutomation,
  RejectExpected,
  RetryMatchers,
  SourceLocation,
  TestApp,
  PageContract,
  PageSelector,
  PageBindOptions,
  TestPage,
  EvalOptions,
  LogicScope,
  ProfileCheckpoint,
  ProfileFixture,
  ProfileRestoreOptions,
  OpenApiRun,
  TestLogic,
  TestNav,
  NavBackOptions,
  NavOptions,
  NavWaitOptions,
  TestView,
  WaitForOptions,
} from "./types.js";
import type {
  AttachmentRef,
  AssertionRecord,
  ReportError,
  StepRecord,
} from "./report-types.js";
import {
  DEFAULT_ACTION_TIMEOUT_MS,
  DEFAULT_POLL_INTERVAL_MS,
  DEFAULT_SPEC_TIMEOUT_MS,
  MAX_ACTIONS,
  MAX_EVAL_BUDGET_MS,
  WEDGED_DEFER_BUDGET_MS,
} from "./version.js";
import type { HostRunAutomation as Automation, LxAppDriver, NavDriver, PageDriver, PageInfo, PageTarget } from "@lingxia/types/automation";

export { TimeoutError };

/**
 * Thrown by `t.skip()`. It unwinds the spec like any throw, but the runtime
 * grades it `skipped`, never `failed` — and not `xfail` under `spec.fail`.
 * Internal: a spec never needs to name it.
 */
export class SkipSignal extends Error {
  override readonly name = "SkipSignal";
  readonly code = "E_SKIPPED" as const;
  constructor(readonly reason: string) {
    super(`skipped: ${reason}`);
  }
}

export type FailurePhase = "beforeEach" | "body" | "defer" | "forensics" | "timeout" | "contract" | "capture";

export class LiveFixture implements Fixture {
  readonly automation: TestAutomation;
  /** `--arg` / `--secret-arg` values; read through `arg()`. */
  private readonly argValues: Readonly<Record<string, string | undefined>>;
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
  traceTruncated = false;
  private eventSequence = 0;
  /** Actions still in flight, so an abort can mark them instead of leaving
   *  them at their optimistic default. */
  private readonly openActions = new Set<StepRecord>();
  /** Fixture calls that have started and not settled; see `track`. */
  private readonly inFlight = new Set<InFlightCall>();
  cleanupUntil = 0;
  cleanupActive = false;
  /**
   * The spec reached an app page (a locator, view, nav or page-bound Logic
   * call). A failure names the current page only then: a unit spec that
   * never touched one is not "on" whatever page the app happens to show.
   */
  usedPage = false;
  lastStepPath: string | undefined;
  failurePhase: FailurePhase | null = null;
  /** The last recorded action that failed, and the error it failed with. */
  failedAction: { action: string; error: unknown } | undefined;
  /** Locator assertions that timed out, by error: the page instance they were bound to. */
  readonly locatorFailures = new WeakMap<object, string | null>();
  /** Instance ids `t.app.page()` bound, so a trace detail's `#id` is not read from a CSS selector. */
  readonly boundPages = new Set<string>();
  /** A driver timeout does not prove its remote app task has stopped. */
  remoteTimeoutKind: "Logic eval" | "View eval" | "page action" | undefined;
  /** Set by `t.skip()`; survives a body that catches the signal. */
  skipReason: string | undefined;
  private readonly stepStack: StepRecord[] = [];
  /** An overlap cannot be attributed safely without async-local JS context. */
  stepConflict: Error | undefined;
  private readonly stepStarted = new WeakMap<StepRecord, number>();
  /** The app `t.app` is pinned to. */
  private readonly pinned: AppRef;
  /** Every app a fixture app reaches; a profile switch re-selects them. */
  private readonly appRefs: AppRef[] = [];
  private readonly dialogWatches = new Map<string, { ref: AppRef; watch: DialogWatch | undefined; label: (message: string) => string }>();
  private readonly dialogInitializers = new Map<AppRef, Promise<void>>();
  private readonly resolveDialogFailure: (message: string) => void;
  readonly dialogFailure: Promise<string>;
  private readonly hostAutomation: Automation;
  /** When this spec's budget started; the runtime arms its timer right after construction. */
  private readonly startedAt: number;

  private traceMeta(): { sequence: number; at_ms: number } {
    return { sequence: ++this.eventSequence, at_ms: Math.max(0, Date.now() - this.startedAt) };
  }
  private readonly networkScope = new NetworkScope();
  /** The scenario this spec installed; removed when it ends. */
  readonly scenarioScope = new ScenarioScope();
  /** Test clocks this spec installed; uninstalled when it ends. */
  readonly clockScope = new ClockScope();
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
    appId?: string,
    traceOriginMs?: number,
  ) {
    this.pinned = { appid: appId, driver: rawApp };
    this.appRefs.push(this.pinned);
    this.hostAutomation = automation;
    this.startedAt = traceOriginMs ?? Date.now();
    this.argValues = args;
    this.specDeadline = Date.now() + specBudgetMs;
    let resolveDialogFailure!: (message: string) => void;
    this.dialogFailure = new Promise<string>((resolve) => { resolveDialogFailure = resolve; });
    this.resolveDialogFailure = resolveDialogFailure;
    setExpectScope({
      note: (entry) => this.noteAssertion(entry),
      locator: (locator, message) => this.locatorMatchers(locator, false, message),
      poll: (read, options) => this.pollMatchers(read, options, false, "expect.poll"),
    });
    const root = guardObject(automation, this, "", ["lxapp", ...HOST_TIERS]);
    this.automation = new Proxy(root, {
      get: (target, prop) => prop === "lxapp"
        ? (appId?: string) => {
          this.assertRunnable();
          return this.appOf(appId === undefined ? this.pinned : this.refFor(appId));
        }
        // Reading a host tier never throws, even on a host without it; each
        // call resolves it and rejects there instead.
        : HOST_TIERS.includes(prop)
          ? lazyDriver(() => ({ owner: automation as object, value: Reflect.get(automation, prop) }), this, `${String(prop)}.`)
          : Reflect.get(target, prop),
    }) as unknown as TestAutomation;
  }

  get app(): TestApp {
    return this.appOf(this.pinned);
  }

  get scenario(): Fixture["scenario"] {
    return { use: async (definition, options) => {
      const app = options?.app;
      if (app === undefined) {
        return installScenario(() => this.pinned.driver, this, this.scenarioScope, definition, options?.variant);
      }
      if (typeof app !== "string" || app.trim() === "") {
        throw new TypeError("t.scenario.use { app } must be a non-empty app id");
      }
      // Function rules go to the run's companion, which no app scopes.
      const appScoped = "t.scenario.use { app } takes a scenario of http rules only; function rules are not app-scoped";
      if (hasFunctionRules(definition, options?.variant)) throw new TypeError(appScoped);
      const ref = this.refFor(app);
      const scenario = await installScenario(() => ref.driver, this, this.scenarioScope, definition, options?.variant);
      if (scenario.rules.some((rule) => rule.kind === "function")) {
        await scenario.remove();
        throw new TypeError(appScoped);
      }
      return scenario;
    } };
  }

  get raw(): LxAppDriver {
    return this.pinned.driver;
  }

  async watchPrimaryDialogs(): Promise<void> {
    await this.ensureDialogWatch(this.pinned);
  }

  private ensureDialogWatch(ref: AppRef): Promise<void> {
    const pending = this.dialogInitializers.get(ref);
    if (pending) return pending;
    const task = this.startDialogWatch(ref).finally(() => { this.dialogInitializers.delete(ref); });
    this.dialogInitializers.set(ref, task);
    return task;
  }

  private async startDialogWatch(ref: AppRef): Promise<void> {
    const appid = ref.appid ?? (await new ActionDeadline(DEFAULT_ACTION_TIMEOUT_MS, this.budgetRoom())
      .call("dialog watch app info", () => ref.driver.info(), () => "starting dialog watch")).appId;
    ref.appid = appid;
    if (this.dialogWatches.has(appid)) return;
    const watch = watchDialogs(ref.driver);
    // Only another app's dialog needs its app named.
    const label = (message: string) => ref === this.pinned ? message : `${appid}: ${message}`;
    this.dialogWatches.set(appid, { ref, watch, label });
    if (watch) void watch.unanswered.then((message) => this.resolveDialogFailure(label(message)));
  }

  async endDialogWatches(): Promise<{ unused: string[]; failures: string[] }> {
    const unused: string[] = [];
    const failures: string[] = [];
    for (const [appid, entry] of this.dialogWatches) {
      try {
        const message = await entry.watch?.end(entry.ref.driver);
        if (message) unused.push(entry.label(message));
      } catch (error) {
        failures.push(entry.label(String(error)));
      }
      this.dialogWatches.delete(appid);
    }
    return { unused, failures };
  }

  /** One fixture app per reached app, so a saved handle stays the same object. */
  private appOf(ref: AppRef): TestApp {
    return ref.app ??= this.wrapApp(ref);
  }

  private refFor(appid: string): AppRef {
    return this.appRefs.find((ref) => ref.appid === appid) ??
      this.trackRef({ appid, driver: this.hostAutomation.lxapp(appid) });
  }

  private trackRef(ref: AppRef): AppRef {
    this.appRefs.push(ref);
    return ref;
  }

  get openapi(): OpenApiRun | undefined {
    const index = activeOpenApi();
    if (!index) return undefined;
    return {
      documents: index.documents.map((doc) => ({
        name: doc.name,
        version: doc.version,
        ...(doc.title ? { title: doc.title } : {}),
      })),
    };
  }

  /** `t.app.profile`: it re-selects the app after a switch. */
  private profileFixture(ref: AppRef): ProfileFixture {
    return {
      checkpoint: () => this.act("profile.checkpoint", "", () =>
        this.reopening(ref, async (driver): Promise<ProfileCheckpoint> => ({ id: await driver.profile.checkpoint() }))),
      restore: (checkpoint: ProfileCheckpoint | string, options?: ProfileRestoreOptions) => {
        const id = checkpointId(checkpoint, "t.app.profile.restore");
        return this.act("profile.restore", options?.keep?.length ? `${id} keep ${options.keep.join(",")}` : id, () =>
          this.reopening(ref, (driver) =>
            options?.keep?.length ? driver.profile.restore(id, { keep: [...options.keep] }) : driver.profile.restore(id)));
      },
      drop: (checkpoint: ProfileCheckpoint | string) => {
        const id = checkpointId(checkpoint, "t.app.profile.drop");
        return this.act("profile.drop", id, async () => { await ref.driver.profile.drop(id); });
      },
    };
  }

  /**
   * A profile switch closes the app and reopens it as a new instance, which
   * the old driver no longer reaches: select the same lxapp again after it,
   * for every fixture app that reaches it. Fixture apps read `ref.driver` on
   * each call, so one saved before the switch follows the reopened app.
   */
  private async reopening<T>(ref: AppRef, op: (driver: LxAppDriver) => Promise<T>): Promise<T> {
    const appid = ref.appid ?? (await ref.driver.info()).appId;
    ref.appid = appid;
    // The no-arg alias is `pinned`; an explicit same-app alias can still be
    // reached before its id was known. A closed unrelated app must not block
    // this profile switch just because its handle was read earlier.
    if (this.pinned.appid === undefined && ref !== this.pinned) {
      try { this.pinned.appid = (await this.pinned.driver.info()).appId; } catch { /* unavailable alias */ }
    }
    try {
      return await op(ref.driver);
    } finally {
      for (const reached of this.appRefs) {
        if (reached.appid === appid) reached.driver = this.hostAutomation.lxapp(appid);
      }
    }
  }

  step<T>(name: string, body: (scope: StepScope) => T | Promise<T>): Promise<T> {
    return this.runStep(name, body);
  }

  private runStep<T>(name: string, body: (scope: StepScope) => T | Promise<T>, parent?: StepRecord): Promise<T> {
    return this.track("t.step", name, () => this.guard(async () => {
      const active = this.stepStack[this.stepStack.length - 1];
      if (active !== parent) {
        const conflict = new Error("Overlapping t.step calls cannot share an implicit step owner. Await top-level steps sequentially; nest with the callback scope: t.step('outer', async (step) => step.step('inner', ...)).");
        this.stepConflict ??= conflict;
        throw conflict;
      }
      const record: StepRecord = {
        name,
        ...this.traceMeta(),
        path: parent ? `${parent.path} > ${name}` : name,
        status: "passed",
        duration_ms: 0,
        steps: [],
        attachments: [],
        assertions: [],
      };
      (parent ? parent.steps : this.steps).push(record);
      this.stepStack.push(record);
      const started = Date.now();
      this.stepStarted.set(record, started);
      await this.emitTrace({
        type: "step_started",
        name,
        path: record.path,
      });
      try {
        const scope: StepScope = {
          step: <U>(childName: string, childBody: (scope: StepScope) => U | Promise<U>) =>
            this.runStep(childName, childBody, record),
        };
        const result = await body(scope);
        record.duration_ms = Date.now() - started;
        await this.emitTrace({
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
          await this.emitTrace({
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
        await this.emitTrace({
          type: "step_finished",
          name,
          path: record.path,
          status: record.status,
          duration_ms: record.duration_ms,
          error: record.error,
        });
        throw error;
      } finally {
        this.stepStarted.delete(record);
        const index = this.stepStack.lastIndexOf(record);
        if (index >= 0) this.stepStack.splice(index, 1);
      }
    }));
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
      if (expected.code !== undefined && !matchesErrorCode(received, expected.code)) {
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
        check(String(record.message)).toContain(expected.message);
      }
      if (expected.message instanceof RegExp) {
        check(String(record.message)).toMatch(expected.message);
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
    const value = this.argValues[name];
    if (value !== undefined) return value;
    if (options.default !== undefined) return options.default;
    if (options.required === false) return undefined;
    // Keys are case-sensitive: `LXDEV_SECRET_PASSWORD` is `PASSWORD`.
    const other = Object.keys(this.argValues).find(
      (key) => key !== name && key.toLowerCase() === name.toLowerCase(),
    );
    const hint = other === undefined
      ? ""
      : `; "${other}" was given, and arg keys are case-sensitive ` +
        `(LXDEV_ARG_<KEY> / LXDEV_SECRET_<KEY> keep the case of <KEY>)`;
    throw new Error(
      `Missing test arg "${name}": pass --arg ${name}=<value> ` +
        `(or --secret-arg ${name}=<value>) to lxdev test${hint}`,
    );
  }

  waitFor<T>(read: () => T | Promise<T>, options: WaitForOptions<Awaited<T>> = {}): Promise<Awaited<T>> {
    const location = callerLocation();
    if (typeof options === "function") {
      throw new TypeError("t.waitFor(read, { until }) takes its acceptance test as `until`, not as a second argument");
    }
    const accept: (value: Awaited<T>) => boolean = options.until ?? Boolean;
    const requested = options.timeout ?? DEFAULT_ACTION_TIMEOUT_MS;
    const interval = options.interval ?? DEFAULT_POLL_INTERVAL_MS;
    if (!Number.isFinite(requested) || requested <= 0 || !Number.isFinite(interval) || interval <= 0) {
      throw new TypeError("t.waitFor timeout and interval must be positive finite numbers");
    }
    const retryIf = options.retryIf ?? isRetryableReadError;
    const detail = truncate(read.name || functionDetail(read), 80);
    return this.act("waitFor", detail, async (): Promise<Awaited<T>> => {
      const deadline = new ActionDeadline(requested, this.budgetRoom());
      let attempts = 0;
      let hasValue = false;
      let lastValue: Awaited<T> | undefined;
      let lastError: unknown;
      this.silenceActions();
      try {
        for (;;) {
          if (deadline.expired()) break;
          attempts += 1;
          const before = new Set(this.inFlight);
          try {
            const value = await deadline.call("t.waitFor read", read, () =>
              `${detail} at ${displayLocation(location.file, location.line, location.column)}`) as Awaited<T>;
            if (deadline.expired()) break;
            lastValue = value;
            hasValue = true;
            lastError = undefined;
            if (accept(value)) return value;
          } catch (error) {
            if (error instanceof SkipSignal || this.aborted) throw error;
            if (error instanceof TimeoutError && deadline.expired()) {
              for (const call of this.inFlight) if (!before.has(call)) call.detached = true;
              break;
            }
            if (!retryIf(error)) throw error;
            lastError = error;
          }
          const pause = Math.min(interval, deadline.remaining());
          if (pause <= 0) break;
          await sleep(pause);
          if (this.aborted && this.abortError) throw this.abortError;
        }
      } finally {
        this.resumeActions();
      }
      const last = lastError !== undefined
        ? `Last error: ${errorLine(lastError)}`
        : hasValue
          ? `Last value: ${formatValue(lastValue)}${accept === Boolean ? " (waiting for a truthy value)" : " (rejected by until)"}`
          : "No read completed.";
      throw new TimeoutError(
        [
          `t.waitFor timed out after ${deadline.elapsed()}ms (${attempts} ${attempts === 1 ? "read" : "reads"}).`,
          last,
          deadline.clampNote(),
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

  async attachRaw(name: string, input: unknown, purpose?: AttachmentRef["purpose"]): Promise<AttachmentRef> {
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
    const current = this.stepStack[this.stepStack.length - 1];
    const ref: AttachmentRef = { name, path, mimeType: payload.mimeType, ...this.traceMeta(),
      ...(current ? { step: current.path } : {}), ...(purpose ? { purpose } : {}) };
    (current ? current.attachments : this.attachments).push(ref);
    return ref;
  }

  /**
   * Stop the body: its fixture calls reject with `reason` from now on. A
   * timeout marks the actions still open as timed out; a failure (an
   * unanswered dialog) as failed.
   */
  abort(reason: Error, as: "timeout" | "failed" = "timeout"): void {
    this.aborted = true;
    this.abortError = reason;
    this.failurePhase = as === "timeout" ? "timeout" : "body";
    this.finishOpenSteps(reason, as);
    for (const record of this.openActions) {
      record.status = as;
      record.error = toReportError(reason, record.path);
      this.noteFailedAction(record.name, record.detail, reason);
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
  act<T>(name: string, detail: string, op: () => T | Promise<T>): Promise<T> {
    if (PAGE_ACTION.test(name)) this.usedPage = true;
    return this.track(name, detail, () => this.recordAct(name, detail, async () => {
      try {
        return await op();
      } catch (error) {
        const code = error && typeof error === "object" ? (error as { code?: unknown }).code : undefined;
        if (code === "E_EVAL_TIMEOUT" && (name === "logic.eval" || name === "page.data" || name === "view.eval")) {
          this.remoteTimeoutKind ??= name === "view.eval" ? "View eval" : "Logic eval";
        } else if (code === "E_AUTOMATION_TIMEOUT" && name === "page.action") {
          this.remoteTimeoutKind ??= "page action";
        }
        throw asFixtureTimeout(error);
      }
    }));
  }

  /**
   * Run one fixture call while it counts as in flight, so a body that
   * returns without awaiting it can be named (with where it was started),
   * and a timed-out body can say what it still awaits.
   */
  private track<T>(name: string, detail: string, op: () => Promise<T>, label?: string): Promise<T> {
    const call: InFlightCall = { name, detail, label, started: Date.now(), origin: new Error() };
    this.inFlight.add(call);
    const promise = (async () => {
      try {
        return await op();
      } finally {
        this.inFlight.delete(call);
      }
    })();
    call.promise = promise;
    return promise;
  }

  /** A read `t.waitFor` gave up on is still running in the app. */
  detachedInFlight(): boolean {
    return [...this.inFlight].some((entry) => entry.detached);
  }

  /**
   * The fixture calls still running, as a body that returned without
   * awaiting them left them: what each is and where the spec started it.
   */
  unsettledCalls(): Array<{ call: string; at: string; ageMs: number }> {
    const now = Date.now();
    return [...this.inFlight].filter((entry) => !entry.detached).map((entry) => {
      const frame = resolveOrigin(parseFrames(entry.origin.stack));
      return {
        call: entry.label ?? (entry.detail ? `${entry.name} ${entry.detail}` : entry.name),
        at: displayLocation(frame.file, frame.line, frame.column),
        ageMs: now - entry.started,
      };
    });
  }

  /**
   * Stop the calls a returned body left running: each fails at its next
   * fixture guard or retry, and its rejection is marked handled (nobody
   * awaits it). Resolves `true` once they settled, `false` if one is still
   * waiting on the app after `ms`. The fixture is usable again afterwards,
   * for cleanup.
   */
  async stopUnsettled(reason: Error, ms: number): Promise<boolean> {
    for (const entry of this.inFlight) entry.promise?.catch(() => {});
    this.aborted = true;
    this.abortError = reason;
    const deadline = Date.now() + ms;
    while (this.inFlight.size > 0 && Date.now() < deadline) await sleep(10);
    this.aborted = false;
    this.abortError = null;
    return this.inFlight.size === 0;
  }

  /**
   * Fixture calls that have not returned, e.g. a hung `t.app.logic.eval` a
   * timed-out body still awaits. Unlike the trace rows, this includes calls
   * made from silenced retry loops.
   */
  pendingCalls(): PendingWork[] {
    return [...this.inFlight].map((call) => ({
      kind: /^(?:logic|view)\.eval$|^page\.data$/.test(call.name) ? "eval" as const : "action" as const,
      detail: call.detail ? `${call.name} ${call.detail}` : call.name,
      owner: this.specId,
      at_ms: call.started - this.startedAt,
    }));
  }

  private async recordAct<T>(name: string, detail: string, op: () => T | Promise<T>): Promise<T> {
    if (this.actionSilence > 0) return this.guard(op);
    // Past the cap, keep recording failures: the action that finally breaks is
    // the one row worth having, and dropping it leaves nothing pointing at it.
    if (this.actionCount >= MAX_ACTIONS) {
      this.traceTruncated = true;
      try {
        return await this.guard(op);
      } catch (error) {
        this.recordFailedAction(name, detail, error, 0);
        this.noteFailedAction(name, detail, error);
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
      previous.detail === detail &&
      previous.sequence === this.eventSequence
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
          ...this.traceMeta(),
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
        this.noteFailedAction(name, detail, error);
        throw error;
      }
    }
    this.actionCount += 1;
    const record: StepRecord = {
      name,
      ...this.traceMeta(),
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
    await this.emitTrace({ type: "step_started", name, path: record.path });
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
      this.noteFailedAction(name, detail, error);
      throw error;
    } finally {
      await this.emitTrace({ type: "step_finished", name, path: record.path,
        status: record.status, duration_ms: record.duration_ms, error: record.error });
    }
  }

  /**
   * A trace event is progress for lxdev, not part of the spec: one the
   * transport drops is sent once more and then given up, and never fails the
   * action it describes. The report is built from the fixture's own records.
   */
  private async emitTrace(event: Record<string, unknown>): Promise<void> {
    for (let attempt = 0; attempt < 2; attempt += 1) {
      try {
        await this.host.emit(event);
        return;
      } catch {
        // Next attempt, then give up.
      }
    }
  }

  /**
   * A note beside the trace. A warning unless `level` is `"info"`: expected
   * housekeeping, such as the timers the spec's own clock dropped, which
   * lxdev shows only with `--verbose`.
   */
  async diagnostic(phase: string, message: string, level?: "info"): Promise<void> {
    await this.emitTrace({ type: "diagnostic", phase, message, ...(level ? { level } : {}) });
  }

  /**
   * An idempotent driver read, retried when the transport between the test
   * runtime and the app dropped it. Never used for input or other calls with
   * side effects: those may already have landed.
   */
  async readRetrying<T>(read: () => T | Promise<T>): Promise<T> {
    for (let attempt = 0; ; attempt += 1) {
      try {
        return await read();
      } catch (error) {
        if (attempt >= TRANSPORT_RETRIES || !isTransientTransportError(error) || this.aborted) throw error;
        await sleep(TRANSPORT_RETRY_DELAY_MS * (attempt + 1));
      }
    }
  }

  private noteFailedAction(name: string, detail: string | undefined, error: unknown): void {
    this.failedAction = { action: detail ? `${name} ${detail}` : name, error };
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
      ...this.traceMeta(),
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

  noteAssertion(entry: Omit<AssertionRecord, "step">): void {
    const record: AssertionRecord = {
      ...entry,
      ...this.traceMeta(),
      step: this.currentStepPath(),
    };
    const current = this.stepStack[this.stepStack.length - 1];
    (current ? current.assertions : this.assertions).push(record);
  }

  close(): void {
    this.closed = true;
    this.finishOpenSteps(new Error("step was still running when the spec ended"), "failed");
  }

  private finishOpenSteps(reason: Error, status: "timeout" | "failed"): void {
    for (const record of this.stepStack) {
      if (record.status !== "passed") continue;
      record.status = status;
      record.duration_ms = Date.now() - (this.stepStarted.get(record) ?? Date.now());
      record.error = toReportError(reason, record.path);
    }
  }

  /**
   * Remove what the framework installed for this spec — its routes, mock
   * scenario and test clocks — apart from the spec's own cleanup, and
   * whether or not its body settled. Resolves one message per failure; a
   * resource already gone is removed.
   */
  async reclaim(): Promise<string[]> {
    const failures: string[] = [];
    const tasks: Array<() => Promise<void>> = [
      () => this.networkScope.reclaim(),
      () => this.scenarioScope.reclaim(),
      () => this.clockScope.reclaim((phase, message, level) => this.diagnostic(phase, message, level)),
    ];
    for (const task of tasks) {
      try { await task(); } catch (error) { failures.push(String((error as Error)?.message ?? error)); }
    }
    return failures;
  }

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

  private wrapApp(ref: AppRef): TestApp {
    const fixture = this;
    const driver = () => ref.driver;
    const input = lazyDriver(() => ({ owner: driver().page, value: driver().page }), fixture, "window.") as PageDriver;
    return {
      page: <C extends PageContract>(selector?: PageSelector, options?: PageBindOptions) =>
        this.bindPage<C>(driver, selector, options),
      view: this.wrapView(() => driver().page, undefined),
      window: {
        get pointer() { return input.pointer; },
        get key() { return input.key; },
      },
      logic: this.wrapLogic(driver),
      nav: this.wrapNav(() => driver().nav),
      // Lazy: the driver is read inside each traced call.
      get network() {
        return wrapNetwork(() => driver().network, fixture, fixture.networkScope);
      },
      get profile() {
        return fixture.profileFixture(ref);
      },
      // Lazy and non-throwing like `network`; spec-scoped: uninstalled when
      // the spec ends.
      get clock() {
        return wrapClock(
          () => ({ driver: driver().clock, appid: async () => (await driver().info()).appId }),
          fixture,
          fixture.clockScope,
          () => fixture.hostAutomation,
        );
      },
      // Lazy and non-throwing like `clock`; the runner watches the app under
      // test for each spec.
      get dialogs() {
        return wrapDialogs(() => driver().dialogs, fixture, () => fixture.ensureDialogWatch(ref));
      },
      info: () => this.act("app.info", "", () => this.readRetrying(() => driver().info())),
      pages: () => this.act("app.pages", "", () => this.readRetrying(() => driver().pages())),
      surfaceLayout: () => this.act("app.surfaceLayout", "", () => this.readRetrying(() => driver().surfaceLayout())),
    } as TestApp;
  }

  /**
   * Fixture navigation takes the fixture's option names and waits for the
   * landed page's `onReady` unless the caller picks `waitUntil: 'commit'`;
   * reads retry a dropped transport.
   */
  private wrapNav(nav: () => NavDriver): TestNav {
    const land = (verb: "to" | "redirect" | "switchTab" | "relaunch") => (options: NavOptions) =>
      this.act(`nav.${verb}`, summarise(options), () => nav()[verb](this.navOptions(options, `t.app.nav.${verb}`)));
    const read = <T,>(verb: string, op: () => Promise<T>, detail?: unknown) =>
      this.act(`nav.${verb}`, summarise(detail), () => this.readRetrying(op));
    return {
      to: land("to"),
      redirect: land("redirect"),
      switchTab: land("switchTab"),
      relaunch: land("relaunch"),
      back: (options?: NavBackOptions) =>
        this.act("nav.back", summarise(options), () => nav().back(this.navOptions(options ?? {}, "t.app.nav.back"))),
      current: () => read("current", () => nav().current()),
      info: (options?: PageTarget) => read("info", () => nav().info(options), options),
      stack: () => read("stack", () => nav().stack()),
    };
  }

  private bindPage<C extends PageContract>(
    driver: () => LxAppDriver,
    selector?: PageSelector,
    options?: PageBindOptions,
  ): Promise<TestPage<C>> {
    if (selector !== undefined) {
      const keys = [selector?.name, selector?.instanceId].filter(value => value !== undefined);
      if (keys.length !== 1 || typeof keys[0] !== "string" || keys[0].trim() === "") {
        return Promise.reject(new TypeError("t.app.page(selector): select exactly one non-empty name or instanceId"));
      }
    }
    const timeout = options?.timeout ?? DEFAULT_ACTION_TIMEOUT_MS;
    if (typeof timeout !== "number" || !Number.isFinite(timeout) || timeout <= 0) {
      return Promise.reject(new TypeError("t.app.page(selector, { timeout }) takes a positive number of ms"));
    }
    const wanted = selector === undefined ? "the current page" : selector.name !== undefined
      ? `page ${JSON.stringify(selector.name)}` : `page instance #${selector.instanceId}`;
    return this.act("page.bind", summarise(selector), async () => {
      // Capture this app driver: reopening the app must not retarget the handle.
      const app = driver();
      const deadline = new ActionDeadline(timeout, this.budgetRoom());
      let info: PageInfo | undefined;
      let last = "not open";
      for (;;) {
        try {
          const found = await deadline.call("t.app.page binding read", () => selector
            ? app.nav.info({ page: selector.instanceId ?? selector.name })
            : app.nav.current(), () => `waiting for ${wanted}`);
          if (deadline.expired()) break;
          if (found?.instanceId) { info = found; break; }
          last = "no live instance";
        } catch (error) {
          if (error instanceof TimeoutError && deadline.expired()) break;
          // Not open yet is "not yet"; an unknown page name is an answer.
          if (!matchesErrorCode(error, "E_PAGE_NOT_ACTIVE") && !isTransientTransportError(error)) throw error;
          last = error instanceof Error ? error.message : String(error);
        }
        if (deadline.expired()) {
          throw new TimeoutError(`Timed out after ${deadline.elapsed()}ms waiting for ${wanted} to open: ${last}.` +
            (deadline.clampNote() ? `\n${deadline.clampNote()}` : ""));
        }
        await sleep(Math.min(DEFAULT_POLL_INTERVAL_MS, Math.max(1, deadline.remaining())));
      }
      if (!info) {
        throw new TimeoutError(`Timed out after ${deadline.elapsed()}ms waiting for ${wanted} to open: ${last}.` +
          (deadline.clampNote() ? `\n${deadline.clampNote()}` : ""));
      }
      const instanceId = info.instanceId!;
      if (selector?.name !== undefined) {
        const stack = await deadline.call("t.app.page stack read", () => app.nav.stack(), () =>
          `checking whether ${wanted} is ambiguous`);
        if (deadline.expired()) {
          throw new TimeoutError(`Timed out after ${deadline.elapsed()}ms checking ${wanted}.`);
        }
        const matches = stack.filter(page => page.path === info!.path && page.instanceId);
        if (matches.length > 1) {
          throw new Error(`t.app.page: ${wanted} has ${matches.length} live instances ` +
            `(${matches.map(page => `#${page.instanceId}`).join(", ")}); select one by instanceId`);
        }
      }
      this.boundPages.add(instanceId);
      const view = this.wrapView(() => app.page, instanceId);
      const invoke = (name: string, args: unknown[], timeout?: number) => {
        if (typeof name !== "string" || name.trim() === "") {
          return Promise.reject(new TypeError("page.invoke needs a non-empty action name"));
        }
        if (args.length > 1) {
          return Promise.reject(new TypeError(`page.actions.${name} takes at most one JSON payload`));
        }
        if (timeout !== undefined && (typeof timeout !== "number" || !Number.isFinite(timeout) || timeout <= 0)) {
          return Promise.reject(new TypeError("page.invoke({ timeout }) takes a positive number of ms"));
        }
        checkJsonArgs(args, `page.actions.${name}`);
        // A timeout stops waiting, but cannot undo a side effect already
        // performed by the action. Never retry dispatch automatically.
        return this.act("page.action", `${name} #${instanceId}`, () => {
          const budget = Math.floor(Math.min(timeout ?? Number.POSITIVE_INFINITY, this.budgetRoom()));
          if (budget < 1) {
            throw new TimeoutError(`page.action ${name} #${instanceId} was not dispatched: its action budget expired`);
          }
          return app.page.action({
            page: instanceId, name, ...(args.length > 0 ? { payload: args[0] } : {}),
            timeoutMs: budget,
          });
        });
      };
      const actions = new Proxy(Object.create(null), {
        get: (_, name) => {
          if (typeof name !== "string" || name === "then") return undefined;
          return (...args: unknown[]) => invoke(name, args);
        },
      });
      return {
        instanceId,
        name: info.name ?? info.path,
        view,
        actions,
        invoke: (name: string, options: { payload?: unknown; timeout?: number }) => {
          if (!options || typeof options !== "object" || Array.isArray(options)) {
            return Promise.reject(new TypeError("page.invoke(name, { payload?, timeout? }) needs an options object"));
          }
          return invoke(name, Object.prototype.hasOwnProperty.call(options, "payload") ? [options.payload] : [], options.timeout);
        },
        data: () => this.act("page.data", `#${instanceId}`, () =>
          remote("page.data", "logic", () => this.evalLogic(app, {
            script: logicScript(readPageData, [instanceId], "page.data", "snapshot"),
          }))),
      } as TestPage<C>;
    });
  }

  /** The driver's options for a fixture nav action. */
  private navOptions<T extends NavWaitOptions>(options: T, api: string): Omit<T, "timeout" | "waitUntil"> & DriverNavWait {
    if (!options || typeof options !== "object") throw new TypeError(`${api} takes an options object`);
    if ("timeoutMs" in options) throw new TypeError(`${api} takes { timeout } in ms`);
    const { timeout, waitUntil = "ready", ...rest } = options;
    if (timeout !== undefined && (typeof timeout !== "number" || !Number.isFinite(timeout) || timeout <= 0)) {
      throw new TypeError(`${api}({ timeout }) takes a positive number of ms`);
    }
    if (waitUntil !== "ready") return { ...rest, waitUntil };
    const room = Math.max(1, Math.floor(this.budgetRoom()));
    return { ...rest, waitUntil, timeoutMs: Math.min(timeout ?? NAV_READY_TIMEOUT_MS, room) };
  }

  private wrapLogic(driver: () => LxAppDriver): TestLogic {
    return {
      eval: (...input: unknown[]) => {
        const { options, fn, args } = evalInput(input);
        if (typeof fn !== "function") {
          throw new TypeError("t.app.logic.eval(fn, ...args) takes a function; a script string is for the raw driver (rawAutomation().lxapp().eval({ script }) from @lingxia/test/runner)");
        }
        const script = logicScript(fn, args, "t.app.logic.eval");
        const timeoutMs = this.evalTimeout(options, "t.app.logic.eval");
        const detail = `${summarise(functionDetail(fn))}${args.length ? ` args=${summarise(args)}` : ""}`;
        return this.act("logic.eval", detail, () =>
          remote("t.app.logic.eval", "logic", () => this.evalLogic(driver(), { script, timeoutMs })));
      },
    } as TestLogic;
  }

  /**
   * Logic eval that asks the runtime which `lx.*` the script reached and
   * hands the caller only the value — the observation is the report's
   * business, not the spec author's, and specs must not have to opt in for
   * their coverage to be measured.
   */
  private async evalLogic(driver: LxAppDriver, options: { script: string; timeoutMs?: number }): Promise<unknown> {
    const result = (await driver.eval({
      ...this.withEvalBudget(options, "Logic eval"),
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
    throw new Error(
      "t.app.logic.eval: the host answered without the call-capture envelope; " +
        "it is older than this @lingxia/test — run `lingxia upgrade` and restart the session (`lingxia dev`)",
    );
  }

  /**
   * The driver's own eval default is a flat few seconds, so a call that runs
   * long under load fails a spec that still had most of its budget left. An
   * eval gets `MAX_EVAL_BUDGET_MS` instead, clamped to what the spec has left.
   */
  private withEvalBudget<T extends { timeoutMs?: number }>(options: T, name: string): T {
    const budget = Math.floor(Math.min(options.timeoutMs ?? MAX_EVAL_BUDGET_MS, this.budgetRoom()));
    if (budget < 1) throw new TimeoutError(`${name} was not dispatched: its action budget expired`);
    return { ...options, timeoutMs: budget };
  }

  /**
   * An eval's own `timeout`, clamped to the spec's remaining budget; without
   * one, `withEvalBudget` picks the default.
   */
  private evalTimeout(options: EvalOptions | undefined, api: string): number | undefined {
    const timeout = options?.timeout;
    if (timeout === undefined) return undefined;
    if (typeof timeout !== "number" || !Number.isFinite(timeout) || timeout <= 0) {
      throw new TypeError(`${api}({ timeout }, fn, ...args) takes a positive number of ms`);
    }
    return Math.min(timeout, this.budgetRoom());
  }

  private viewEval(page: () => PageDriver, input: unknown[], api: string, bound?: string): Promise<unknown> {
    const { options, fn, args } = evalInput(input);
    if (typeof fn !== "function") {
      throw new TypeError(`${api}(fn, ...args) takes a function; a script string is for the raw driver (rawAutomation().lxapp().page.eval({ script }) from @lingxia/test/runner)`);
    }
    if (options && "page" in options) {
      throw new TypeError(`${api}: page is not an eval option; bind the page with t.app.page({ name }) and use its view`);
    }
    const script = pageScript(fn, args, api);
    const timeoutMs = this.evalTimeout(options, api);
    const detail = `${summarise(functionDetail(fn))}${args.length ? ` args=${summarise(args)}` : ""}`;
    return this.act("view.eval", bound ? `#${bound} ${detail}` : detail, () =>
      remote(api, "page", () => page().eval(this.withEvalBudget<{ script: string; page?: string; timeoutMs?: number }>({
        script,
        ...(bound ? { page: bound } : {}),
        ...(timeoutMs !== undefined ? { timeoutMs } : {}),
      }, "View eval"))));
  }

  /** `bound` is the immutable instance id captured by app.page(). */
  private wrapView(page: () => PageDriver, bound: string | undefined): TestView {
    const location = () => {
      const frame = callerLocation();
      return { source: frame.file, line: frame.line, column: frame.column };
    };
    // Resolved on each call, like the rest of a fixture app.
    const lazyPage: PageLike = {
      query: (options) => page().query(options) as Promise<QueryMatch>,
      click: (options) => page().click(options),
      fill: (options) => page().fill(options),
      press: (options) => page().press(options),
      type: (options) => page().type(options),
      eval: (options) => page().eval(options),
    };
    const input = lazyDriver(() => ({ owner: page() as object, value: page() }), this, "page.") as PageDriver;
    const target = bound === undefined ? {} : { page: bound };
    const noOptions = (api: string, options: unknown) => {
      if (options !== undefined) {
        throw new TypeError(`${api}() takes no options: bind another page with t.app.page({ name }) and use its view`);
      }
    };
    return {
      testId: (id: string, options?: unknown) => {
        noOptions("view.testId", options);
        return this.locator(lazyPage, testIdSelector(id), location(), target);
      },
      css: (selector: string, options?: unknown) => {
        noOptions("view.css", options);
        return this.locator(lazyPage, selector, location(), target);
      },
      eval: ((...args: unknown[]) => this.viewEval(page, args, bound ? "page.view.eval" : "t.app.view.eval", bound)) as TestView["eval"],
      screenshot: (options?: unknown) => {
        noOptions("view.screenshot", options);
        return this.act("page.screenshot", bound ? `#${bound}` : "", () =>
          this.readRetrying(() => page().screenshot(bound ? target : undefined)));
      },
      scroll: (options) => {
        if (options && "page" in options) {
          throw new TypeError("view.scroll() takes no page option: bind another page with t.app.page({ name }) and use its view");
        }
        return input.scroll({ ...options, ...target });
      },
    };
  }

  private locator(
    page: PageLike,
    selector: string,
    location: SourceLocation,
    options?: LocatorTarget,
  ): Locator {
    this.usedPage = true;
    return new PageLocator(
      page,
      (fn) => this.guard(fn),
      <T,>(verb: string, detail: string, op: () => Promise<T>) => this.act(verb, detail, op),
      selector,
      location,
      options,
      () => this.budgetRoom(),
    );
  }

  private locatorMatchers(locator: Locator, inverted: boolean, message?: string): LocatorMatchers {
    const withMessage = (options?: ExpectOptions): ExpectOptions | undefined =>
      message === undefined || options?.message !== undefined ? options : { ...options, message };
    const self = {
      toBeVisible: (options: ExpectOptions | undefined) =>
        this.retryLocator(locator, "toBeVisible", inverted, withMessage(options), inverted ? "not visible" : "visible"),
      toBeInViewport: (options?: ExpectOptions) =>
        this.retryLocator(locator, "toBeInViewport", inverted, withMessage(options), inverted ? "not in viewport" : "in viewport"),
      toBeHidden: (options?: ExpectOptions) => this.retryLocator(locator, "toBeHidden", inverted, withMessage(options), true),
      toBeAttached: (options?: ExpectOptions) => this.retryLocator(locator, "toBeAttached", inverted, withMessage(options), true),
      toBeEnabled: (options?: ExpectOptions) => this.retryLocator(locator, "toBeEnabled", inverted, withMessage(options), true),
      toBeDisabled: (options?: ExpectOptions) => this.retryLocator(locator, "toBeDisabled", inverted, withMessage(options), true),
      toBeEditable: (options?: ExpectOptions) => this.retryLocator(locator, "toBeEditable", inverted, withMessage(options), true),
      toHaveText: (expected: string | RegExp, options?: ExpectOptions) =>
        this.retryLocator(locator, "toHaveText", inverted, withMessage(options), expected),
      toContainText: (expected: string | RegExp, options?: ExpectOptions) =>
        this.retryLocator(locator, "toContainText", inverted, withMessage(options), expected),
      toHaveAttribute: (name: string, value?: string | RegExp, options?: ExpectOptions) => {
        if (typeof name !== "string" || !name) throw new TypeError("toHaveAttribute needs an attribute name");
        return this.retryLocator(locator, "toHaveAttribute", inverted, withMessage(options), new AttributeExpectation(name, value));
      },
      toHaveCount: (expected: number, options?: ExpectOptions) =>
        this.retryLocator(locator, "toHaveCount", inverted, withMessage(options), expected),
      toHaveValue: (expected: string | RegExp, options?: ExpectOptions) =>
        this.retryLocator(locator, "toHaveValue", inverted, withMessage(options), expected),
    };
    Object.defineProperty(self, "not", {
      get: () => this.locatorMatchers(locator, !inverted, message),
      configurable: true,
    });
    return self as LocatorMatchers;
  }

  private pollMatchers<T>(
    read: () => T | Promise<T>,
    options: ExpectOptions | undefined,
    inverted: boolean,
    api: string,
  ): RetryMatchers<Awaited<T>> {
    const run = (matcher: string, expected?: unknown) =>
      this.retryPoll(read, matcher, inverted, options, expected, api);
    const fixture = this;
    const self: Partial<RetryMatchers<Awaited<T>>> = {
      toBe: (expected: unknown) => run("toBe", expected),
      toEqual: (expected: unknown) => run("toEqual", expected),
      toContain: (expected: unknown) => run("toContain", expected),
      toContainEqual: (expected: unknown) => run("toContainEqual", expected),
      toMatch: (expected: string | RegExp) => run("toMatch", expected),
      toBeTruthy: () => run("toBeTruthy"),
      toBeFalsy: () => run("toBeFalsy"),
      toBeDefined: () => run("toBeDefined"),
      toBeUndefined: () => run("toBeUndefined"),
      toBeInstanceOf: (expected: Function) => run("toBeInstanceOf", expected),
      toHaveLength: (expected: number) => run("toHaveLength", expected),
      toBeGreaterThan: (expected: number) => run("toBeGreaterThan", expected),
      toBeGreaterThanOrEqual: (expected: number) => run("toBeGreaterThanOrEqual", expected),
      toBeLessThan: (expected: number) => run("toBeLessThan", expected),
      toBeLessThanOrEqual: (expected: number) => run("toBeLessThanOrEqual", expected),
    };
    Object.defineProperty(self, "not", {
      get: () => fixture.pollMatchers(read, options, !inverted, api),
      configurable: true,
    });
    return self as RetryMatchers<Awaited<T>>;
  }

  private retryLocator(
    locator: Locator,
    matcher: string,
    inverted: boolean,
    options: ExpectOptions | undefined,
    expected?: unknown,
  ): Promise<void> {
    const target = locator instanceof PageLocator ? locator.describe() : locator.selector;
    const assertion = `${inverted ? "not." : ""}${matcher}`;
    return this.track("expect", `${target} ${assertion}`, () => this.guard(async () => {
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
            lastResolved = await resolveLocator(locator, deadline, context,
              expected instanceof AttributeExpectation ? [expected.name] : []);
            matchLocator(locator, lastResolved, matcher, expected, inverted);
            this.noteAssertion({
              matcher: inverted ? `not.${matcher}` : matcher,
              expected: formatValue(expected),
              actual: formatValue(locatorActual(matcher, lastResolved, expected)),
              passed: true,
              target,
              message: options?.message,
              location: displayLocation(location.source, location.line, location.column),
              duration_ms: deadline.elapsed(),
            });
            return;
          } catch (error) {
            if (error instanceof TimeoutError || this.aborted || !isRetryableReadError(error)) throw error;
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
      const miss = lastResolved && locator instanceof PageLocator ? locator.missText(lastResolved) : undefined;
      throw this.retryFailure({
        clampNote: deadline.clampNote(),
        matcher: inverted ? `not.${matcher}` : matcher,
        expected,
        actual: locatorActual(matcher, lastResolved, expected),
        duration,
        location,
        lastError,
        extra: miss,
        locator: locator instanceof PageLocator ? locator.boundPage : null,
        target,
        message: options?.message,
      });
    }), `expect(${target}).${assertion}`);
  }

  private retryPoll<T>(
    read: () => T | Promise<T>,
    matcher: string,
    inverted: boolean,
    options: ExpectOptions | undefined,
    expected: unknown,
    api: string,
  ): Promise<void> {
    const source = truncate(read.name || functionDetail(read), 60);
    const assertion = `${inverted ? "not." : ""}${matcher}`;
    return this.track(api, `${source} ${assertion}`, () => this.guard(async () => {
      const frame = callerLocation();
      const location = { source: frame.file, line: frame.line, column: frame.column };
      const deadline = new ActionDeadline(options?.timeout ?? DEFAULT_ACTION_TIMEOUT_MS, this.budgetRoom());
      const interval = options?.interval ?? DEFAULT_POLL_INTERVAL_MS;
      const context = () => this.deadlineContext(`${api} ${inverted ? "not." : ""}${matcher}`, location);
      let lastActual: unknown;
      let lastError: unknown;
      pushAssertionSilence();
      this.silenceActions();
      try {
        while (!deadline.expired()) {
          try {
            lastActual = await deadline.call(`${api} read`, read, context);
            applyMatcher(matcher, lastActual, expected, inverted);
            this.noteAssertion({
              matcher: inverted ? `not.${matcher}` : matcher,
              expected: formatValue(expected),
              actual: formatValue(lastActual),
              passed: true,
              target: source,
              message: options?.message,
              location: displayLocation(location.source, location.line, location.column),
              duration_ms: deadline.elapsed(),
            });
            return;
          } catch (error) {
            if (error instanceof TimeoutError || this.aborted || !isRetryableReadError(error)) throw error;
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
        target: source,
        message: options?.message,
      });
    }), `${api}(${source}).${assertion}`);
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
    target?: string;
    message?: string;
    /** A locator assertion: the instance id it is bound to, `null` for the current page. */
    locator?: string | null;
  }): AssertionError {
    const where = displayLocation(input.location.source, input.location.line, input.location.column);
    const last =
      input.lastError instanceof AssertionError
        ? input.lastError.message
        : input.lastError instanceof Error
          ? input.lastError.message
          : undefined;
    const lines = [
      input.message,
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
      difference: ["toBe", "toEqual"].includes(input.matcher)
        ? firstDifference(input.expected, input.actual) : undefined,
      target: input.target,
      message: input.message,
      location: where,
      duration_ms: input.duration,
    });
    const error = new AssertionError(input.matcher, input.actual, input.expected, lines.join("\n"), "E_TIMEOUT");
    if (input.locator !== undefined) this.locatorFailures.set(error, input.locator);
    return error;
  }
}

/** An app the fixture reaches, re-selected when a profile switch reopens it. */
interface AppRef {
  /** Learned at the first profile switch when not known up front. */
  appid: string | undefined;
  driver: LxAppDriver;
  app?: TestApp;
}

interface InFlightCall {
  name: string;
  detail: string;
  /** How a failure names the call, when not `name detail`. */
  label?: string;
  started: number;
  /** Captured when the call started; its stack names the spec line. */
  origin: Error;
  promise?: Promise<unknown>;
  /** A `t.waitFor` read dropped at its deadline: settles alone, never "not awaited". */
  detached?: boolean;
}

/** An inline scenario (or its selected variant) with `function` rules. */
function hasFunctionRules(definition: unknown, variant: string | undefined): boolean {
  if (!definition || typeof definition !== "object") return false;
  const record = definition as { rules?: unknown; variants?: Record<string, { rules?: unknown }> };
  const rules = [
    ...(Array.isArray(record.rules) ? record.rules : []),
    ...(variant !== undefined && Array.isArray(record.variants?.[variant]?.rules) ? record.variants![variant]!.rules as unknown[] : []),
  ];
  return rules.some((rule) => !!rule && typeof rule === "object" && "function" in rule);
}

/** Extra attempts of an idempotent read the transport dropped, and the backoff step. */
const TRANSPORT_RETRIES = 2;
const TRANSPORT_RETRY_DELAY_MS = 100;

/**
 * Host tiers a host may lack. The fixture resolves them per call, so reading
 * `t.automation.desktop` never throws.
 */
const HOST_TIERS: PropertyKey[] = ["browser", "desktop", "terminal"];

/** Driver reads with no side effect, retried when the transport drops them. */
const IDEMPOTENT_READS = new Set(["nav.current", "nav.info", "nav.stack", "page.query", "page.screenshot", "lxapps.list", "lxapps.current"]);

function checkpointId(checkpoint: ProfileCheckpoint | string, api: string): string {
  const id = typeof checkpoint === "string" ? checkpoint : checkpoint?.id;
  if (typeof id !== "string" || id.length === 0) {
    throw new TypeError(`${api} needs the checkpoint t.app.profile.checkpoint() resolved (or its id)`);
  }
  return id;
}

/**
 * `t.waitFor` retries a read that says "not yet" by throwing, but not one
 * that is invalid locally or violates the eval JSON boundary. Remote DOM
 * TypeErrors can resolve as rendering catches up.
 */
function isRetryableReadError(error: unknown): boolean {
  if (error instanceof AssertionError || isRetryableRemoteError(error)) return true;
  const name = error instanceof Error ? error.name : undefined;
  return name !== "TypeError" && name !== "ReferenceError" && name !== "SyntaxError";
}

function errorLine(error: unknown): string {
  return error instanceof Error ? `${error.name}: ${error.message}` : formatValue(error);
}

/**
 * `eval(fn, ...args)` or `eval(options, fn, ...args)`. An object with a
 * `script` key is a script string's options, not eval options: it stays the
 * "function" so the caller refuses it as such.
 */
function evalInput<O extends EvalOptions = EvalOptions>(input: unknown[]): { options: O | undefined; fn: unknown; args: unknown[] } {
  const [first, ...rest] = input;
  if (first !== null && typeof first === "object" && !("script" in first)) {
    const [fn, ...args] = rest;
    return { options: first as O, fn, args };
  }
  return { options: undefined, fn: first, args: rest };
}

async function remote<T>(api: string, target: RemoteTarget, op: () => Promise<T>): Promise<T> {
  try {
    return await op();
  } catch (error) {
    throw explainRemoteError(error, api, target);
  }
}

// Sent to app Logic as source text: it must stay self-contained.
function readPageData(scope: LogicScope, instanceId: string): unknown {
  const page = scope.getPage(instanceId);
  if (!page) throw new Error(`page.data: page instance #${instanceId} is gone`);
  return page.data;
}

function resolveLocator(
  locator: Locator,
  deadline: ActionDeadline,
  context: () => string,
  attributes: readonly string[] = [],
): Promise<LocatorResolve> {
  if (locator instanceof PageLocator) {
    return deadline.call("locator read", () => locator.resolve(undefined, "resolve", attributes), context);
  }
  throw new Error("expect(locator) takes a locator from view.testId() or view.css()");
}

/** `toHaveAttribute`'s expectation; formats as the report shows it. */
class AttributeExpectation {
  constructor(readonly name: string, readonly value: string | RegExp | undefined) {}
  toString(): string {
    return this.value === undefined ? `attribute ${this.name}` : `${this.name}=${formatValue(this.value)}`;
  }
  toJSON(): string {
    return this.toString();
  }
}

function locatorActual(matcher: string, resolved: LocatorResolve | undefined, expected?: unknown): unknown {
  if (!resolved) return undefined;
  if (matcher === "toBeInViewport") {
    return resolved.kind === "unique" ? (resolved.inViewport ? "in viewport" : "outside viewport") : resolved.kind;
  }
  if (matcher === "toContainText") return resolved.text;
  if (matcher === "toHaveAttribute") {
    return expected instanceof AttributeExpectation ? resolved.attributes?.[expected.name] ?? null : undefined;
  }
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
  if (matcher === "toBeInViewport") {
    const pass = resolved.kind === "unique" && resolved.inViewport;
    if (pass === inverted) {
      const detail =
        locator instanceof PageLocator ? locator.missText(resolved) : `resolved to ${resolved.kind}`;
      const actual = String(locatorActual(matcher, resolved));
      throw new AssertionError(
        inverted ? "not.toBeInViewport" : "toBeInViewport",
        actual,
        inverted ? "not in viewport" : "in viewport",
        `${detail}\nExpected: ${inverted ? "not " : ""}in viewport\nReceived: ${formatValue(actual)}`,
      );
    }
    return;
  }
  if (matcher === "toContainText") {
    if (expected instanceof RegExp) applyMatcher("toMatch", resolved.text, expected, inverted);
    else applyMatcher("toContain", resolved.text, typeof expected === "string" ? normalizeText(expected) : expected, inverted);
    return;
  }
  if (matcher === "toHaveAttribute" && expected instanceof AttributeExpectation) {
    if (resolved.count !== 1) throw new Error(`Expected one attached element, received ${resolved.count}`);
    const actual = resolved.attributes?.[expected.name] ?? null;
    const pass = actual !== null && (
      expected.value === undefined ? true
        : expected.value instanceof RegExp ? (expected.value.lastIndex = 0, expected.value.test(actual))
          : actual === expected.value);
    if (pass === inverted) {
      const name = inverted ? "not.toHaveAttribute" : "toHaveAttribute";
      throw new AssertionError(name, formatValue(actual), `${inverted ? "not " : ""}${expected}`,
        `Expected: ${inverted ? "not " : ""}${expected}\nReceived: ${actual === null ? `no ${expected.name} attribute` : `${expected.name}=${formatValue(actual)}`}`);
    }
    return;
  }
  if (matcher === "toHaveCount") {
    applyMatcher("toBe", resolved.count, expected, inverted);
    return;
  }
  if (matcher === "toHaveText") {
    if (expected instanceof RegExp) applyMatcher("toMatch", resolved.text, expected, inverted);
    else applyMatcher("toBe", resolved.text, typeof expected === "string" ? normalizeText(expected) : expected, inverted);
    return;
  }
  if (matcher === "toHaveValue") {
    if (expected instanceof RegExp) applyMatcher("toMatch", resolved.value ?? "", expected, inverted);
    else applyMatcher("toBe", resolved.value, expected, inverted);
  }
}

/** The driver's own bound for `waitUntil: 'ready'`, the fixture's default too. */
const NAV_READY_TIMEOUT_MS = 15_000;

/** Actions that reach a page of the app. */
const PAGE_ACTION = /^(?:page|view|nav)\./;

/** A nav wait as the driver takes it. */
interface DriverNavWait {
  waitUntil: "commit" | "ready";
  timeoutMs?: number;
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
        const bound = IDEMPOTENT_READS.has(name)
          ? (...args: unknown[]) =>
            fixture.act(name, summarise(args[0]), () => fixture.readRetrying(() => value.apply(obj, args)))
          : (...args: unknown[]) => fixture.act(name, summarise(args[0]), () => value.apply(obj, args));
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

/**
 * A driver namespace resolved on each call instead of on read. `resolve`
 * yields the namespace's owner and value; a member read returns another lazy
 * namespace, and a call resolves the chain, then records the call as an
 * action. A host that lacks the tier rejects the call, never the read.
 */
function lazyDriver(
  resolve: () => { owner: object; value: unknown },
  fixture: LiveFixture,
  path: string,
): unknown {
  const cache = new Map<PropertyKey, unknown>();
  const target = function lazy() {};
  return new Proxy(target, {
    get(_, prop) {
      // Not a thenable: `await t.automation.desktop` must not call `then`.
      if (prop === "then" || typeof prop === "symbol") return undefined;
      if (cache.has(prop)) return cache.get(prop);
      const member = lazyDriver(() => {
        const { value } = resolve();
        if (!value || (typeof value !== "object" && typeof value !== "function")) {
          throw new Error(`${path.slice(0, -1)} is not available on this host`);
        }
        return { owner: value as object, value: Reflect.get(value as object, prop, value) };
      }, fixture, `${path}${String(prop)}.`);
      cache.set(prop, member);
      return member;
    },
    apply(_, __, args: unknown[]) {
      const name = path.slice(0, -1);
      return fixture.act(name, summarise(args[0]), () => {
        const { owner, value } = resolve();
        if (typeof value !== "function") throw new TypeError(`${name} is not a function on this host`);
        return (value as (...params: unknown[]) => unknown).apply(owner, args);
      });
    },
  });
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
    const cause = error as AssertionError & { code?: unknown; data?: unknown };
    return {
      code: typeof cause.code === "string" ? cause.code : undefined,
      data: jsonData(cause.data),
      name: error.name,
      message: error.message,
      stack,
      matcher: error.matcher,
      expected: formatValue(error.expected),
      actual: formatValue(error.actual),
      difference: ["toBe", "toEqual"].includes(error.matcher)
        ? firstDifference(error.expected, error.actual) : undefined,
      location: firstLocation(stack, error),
      step,
    };
  }
  if (error instanceof Error) {
    const stack = remapStack(error.stack);
    const details = error as Error & { code?: unknown; data?: unknown };
    return {
      code: typeof details.code === "string" ? details.code : undefined,
      data: jsonData(details.data),
      name: error.name,
      message: error.message,
      stack,
      location: firstLocation(stack, error),
      step,
    };
  }
  return { name: "Error", message: String(error), step };
}

function jsonData(data: unknown): unknown {
  try {
    return data === undefined ? undefined : JSON.parse(JSON.stringify(data));
  } catch {
    // Non-JSON diagnostic data must not break reporting.
    return undefined;
  }
}

function firstLocation(stack: string | undefined, error: Error): string | undefined {
  // V8 starts the stack with the message; a message line (a schema issue,
  // a received value) must not be read as a frame.
  const header = `${error.name}: ${error.message}`;
  const frameText = stack?.startsWith(header) ? stack.slice(header.length) : stack;
  const frames = parseFrames(frameText);
  const frame = frames.length ? resolveOrigin(frames) : undefined;
  return frame && !isFrameworkFrame(frame.file) ? `${frame.file}:${frame.line}:${frame.column}` : undefined;
}
