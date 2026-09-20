import { AssertionError, expect, setAssertionSink } from "./expect.js";
import { LiveFixture, TimeoutError, toReportError } from "./fixture.js";
import { attachText, resolveHost, warnVersionSkew } from "./host.js";
import { captureFrames, fileStem, resolveOrigin, slugTitle, type StackFrame } from "./ids.js";
import { renderJUnit } from "./junit.js";
import { clearInline, countStatuses, renderHtml } from "./report.js";
import type { SpecApi } from "./spec-api.js";
import type {
  CaseRecord,
  Fixture,
  JsonReport,
  LingxiaTestController,
  ProtocolReport,
  RunSubject,
  SpecBody,
  SpecOptions,
  SpecStatus,
} from "./types.js";
import {
  DEFAULT_SPEC_TIMEOUT_MS,
  FORENSICS_BUDGET_MS,
  MAX_DEFER_BUDGET_MS,
  PACKAGE_NAME,
  VERSION,
  WEDGED_DEFER_BUDGET_MS,
} from "./version.js";
import type { LxAppDriver } from "@lingxia/types/automation";

type Annotation = "default" | "skip" | "only" | "fixme" | "fail";

interface RegisteredSpec {
  title: string;
  id?: string;
  covers: string[];
  timeout: number;
  timeoutCleanup?: number;
  fresh: boolean;
  app?: string;
  forensics: boolean;
  reason?: string;
  annotation: Annotation;
  body: SpecBody;
  frames: StackFrame[];
  indexInFile: number;
}

interface Hook {
  frames: StackFrame[];
  fn: SpecBody;
}

const specs: RegisteredSpec[] = [];
const hooks: Hook[] = [];
const afterHooks: Hook[] = [];
const resetHooks: Hook[] = [];
const fileCounts = new Map<string, number>();
let forceRelaunchNext = false;
let trackSurface = false;

/**
 * Measure this run against the whole public `lx` surface, not just the tags
 * the suite declares. For a conformance suite — one that intends to cover
 * every published capability — call this once from the entry. An ordinary
 * lxapp should not: it would read as failing to cover an API it never claimed.
 */
function trackPublicSurface(): void {
  trackSurface = true;
}

function parseArgs(
  optionsOrBody: SpecOptions | SpecBody,
  maybeBody?: SpecBody,
): { options: SpecOptions; body: SpecBody } {
  if (typeof optionsOrBody === "function") {
    return { options: {}, body: optionsOrBody };
  }
  if (typeof maybeBody !== "function") {
    throw new TypeError("spec() requires a function body");
  }
  return { options: optionsOrBody, body: maybeBody };
}

function register(annotation: Annotation, title: string, optionsOrBody: SpecOptions | SpecBody, maybeBody?: SpecBody): void {
  if (typeof title !== "string" || title.length === 0) {
    throw new TypeError("spec() requires a non-empty title");
  }
  const { options, body } = parseArgs(optionsOrBody, maybeBody);
  for (const [name, value] of Object.entries({ timeout: options.timeout, timeoutCleanup: options.timeoutCleanup })) {
    if (value !== undefined && (!Number.isFinite(value) || value <= 0)) throw new TypeError(`${name} must be a positive finite number`);
  }
  // The bundle map is installed after the modules run, so keep the raw frames;
  // the authored file is only knowable once the run starts. `frames[0]` is a
  // frame inside this package, identical for every caller, so it can order
  // registrations but must never stand in for identity.
  const frames = captureFrames();
  const indexInFile = fileCounts.size + 1;
  fileCounts.set(String(indexInFile), indexInFile);
  specs.push({
    title,
    id: options.id,
    covers: [...(options.covers ?? [])],
    timeout: options.timeout ?? DEFAULT_SPEC_TIMEOUT_MS,
    timeoutCleanup: options.timeoutCleanup,
    fresh: options.fresh === true,
    app: options.app,
    forensics: options.forensics !== false,
    reason: options.reason,
    annotation,
    body,
    frames,
    indexInFile,
  });
}

const spec: SpecApi = Object.assign(
  function specFn(title: string, optionsOrBody: SpecOptions | SpecBody, maybeBody?: SpecBody): void {
    register("default", title, optionsOrBody, maybeBody);
  },
  {
    skip(title: string, optionsOrBody?: SpecOptions | SpecBody, maybeBody?: SpecBody): void {
      const body = typeof optionsOrBody === "function" ? optionsOrBody : maybeBody ?? (async () => {});
      const options = typeof optionsOrBody === "function" || optionsOrBody === undefined ? {} : optionsOrBody;
      register("skip", title, options, body);
    },
    only(title: string, optionsOrBody: SpecOptions | SpecBody, maybeBody?: SpecBody): void {
      register("only", title, optionsOrBody, maybeBody);
    },
    fixme(title: string, optionsOrBody?: SpecOptions | SpecBody, maybeBody?: SpecBody): void {
      const body = typeof optionsOrBody === "function" ? optionsOrBody : maybeBody ?? (async () => {});
      const options = typeof optionsOrBody === "function" || optionsOrBody === undefined ? {} : optionsOrBody;
      register("fixme", title, options, body);
    },
    fail(title: string, optionsOrBody: SpecOptions | SpecBody, maybeBody?: SpecBody): void {
      register("fail", title, optionsOrBody, maybeBody);
    },
    reset(fn: SpecBody): void {
      if (typeof fn !== "function") throw new TypeError("spec.reset() requires a function");
      resetHooks.push({ frames: captureFrames(), fn });
    },
    afterEach(fn: SpecBody): void {
      if (typeof fn !== "function") throw new TypeError("spec.afterEach() requires a function");
      afterHooks.push({ frames: captureFrames(), fn });
    },
    beforeEach(fn: SpecBody): void {
      if (typeof fn !== "function") throw new TypeError("spec.beforeEach() requires a function");
      hooks.push({ frames: captureFrames(), fn });
    },
  },
);

function sourceOf(item: RegisteredSpec): { file: string; line: number } {
  const origin = resolveOrigin(item.frames);
  return { file: origin.file, line: origin.line };
}

function resolvedId(item: RegisteredSpec): string {
  if (item.id && item.id.length > 0) return item.id;
  const slug = slugTitle(item.title);
  if (slug) return slug;
  return `${fileStem(sourceOf(item).file)}-${item.indexInFile}`;
}

/** Report group: the authored file path, trimmed to the project-relative part. */
function suiteOf(file: string): string {
  const normalized = file.replace(/\\/g, "/").replace(/^lxdev-test:\/\//, "");
  const trimmed = normalized.replace(/^\.\//, "");
  const cut = trimmed.lastIndexOf("/tests/");
  return cut >= 0 ? trimmed.slice(cut + 1) : trimmed;
}

function automationRoot() {
  const lx = (globalThis as { lx?: { automation?: () => { lxapp: { (): LxAppDriver; (id: string): LxAppDriver } } } }).lx;
  if (!lx || typeof lx.automation !== "function") {
    throw new Error("lx.automation() is not available in this runtime");
  }
  return lx.automation();
}

function pinApp(appId?: string): LxAppDriver {
  const automation = automationRoot();
  return appId ? automation.lxapp(appId) : automation.lxapp();
}

async function relaunchHome(app: LxAppDriver): Promise<void> {
  const pages = await app.pages();
  const home = pages[0]?.name ?? "home";
  await app.nav.relaunch({ page: home });
}

async function run(): Promise<ProtocolReport> {
  warnVersionSkew();
  const host = resolveHost();
  const started = Date.now();
  clearInline();

  const ids = new Map<string, string>();
  for (const item of specs) {
    const id = resolvedId(item);
    const previous = ids.get(id);
    if (previous) {
      throw new Error(`Duplicate spec id ${JSON.stringify(id)} (${previous} and ${item.title})`);
    }
    ids.set(id, item.title);
  }

  const grep = host.args.grep;
  const pattern = grep ? new RegExp(grep) : undefined;
  const forbidOnly = host.args.forbidOnly === "1" || host.args["forbid-only"] === "1";
  const hasOnly = specs.some((item) => item.annotation === "only");
  if (hasOnly && forbidOnly) {
    throw new Error("spec.only is registered; lxdev test --forbid-only refuses to run");
  }

  const retries = Number(host.args.retries ?? 0);
  if (!Number.isInteger(retries) || retries < 0 || retries > 10) throw new Error("retries must be between 0 and 10");
  const selectedIds: string[] | undefined = host.args.ids ? JSON.parse(host.args.ids) : undefined;
  const shard = host.args.shard?.split("/").map(Number);
  if (shard && (shard.length !== 2 || shard.some(n => !Number.isInteger(n) || n < 1) || shard[0]! > shard[1]!)) throw new Error("shard must be INDEX/TOTAL (1-based)");
  const selected = specs.filter((item) => {
    if (hasOnly && item.annotation !== "only") return false;
    if (selectedIds && !selectedIds.includes(resolvedId(item))) return false;
    if (shard && stableHash(resolvedId(item)) % shard[1]! !== shard[0]! - 1) return false;
    if (host.args.id && resolvedId(item) !== host.args.id) return false;
    if (!grep) return true;
    const id = resolvedId(item);
    return pattern!.test(item.title) || pattern!.test(id);
  });

  if (selected.length === 0 && host.args.passWithNoTests !== "1") {
    throw new Error("No tests matched this selection. Check the entry and filters, or use --pass-with-no-tests.");
  }
  await host.emit({ type: "run_started", schema_version: 1, total: selected.length,
    cases: selected.map(item => ({ id: resolvedId(item), title: item.title, name: item.title,
      full_name: item.id ? `${item.id} | ${item.title}` : item.title, ...sourceOf(item),
      suite: suiteOf(sourceOf(item).file), timeout_ms: item.timeout, covers: item.covers })) });

  // Resolve each hook's authored file once, so a `beforeEach` stays scoped to
  // the file that declared it rather than running for every spec in the run.
  const hookFiles = new Map<Hook, string>(
    [...hooks, ...afterHooks, ...resetHooks].map((hook) => [hook, resolveOrigin(hook.frames).file] as const),
  );
  if (retries > 0) {
    for (const item of selected.filter(item => !["skip", "fixme"].includes(item.annotation))) {
      if (!resetHooks.some(hook => hookFiles.get(hook) === sourceOf(item).file)) {
        throw new Error(`Retries require spec.reset() in ${sourceOf(item).file}; relaunching a page does not reset app state.`);
      }
    }
  }
  const subject = await describeSubject();
  const cases: CaseRecord[] = [];
  forceRelaunchNext = false;
  let contaminated = false;

  const queue = [...selected];
  const attempts = new Map<string, CaseRecord[]>();
  for (const item of queue) {
    const id = resolvedId(item);
    const timeout = item.timeout;
    const source = sourceOf(item);
    const record: CaseRecord = {
      id,
      title: item.title,
      name: item.title,
      full_name: item.id ? `${item.id} | ${item.title}` : item.title,
      file: source.file,
      line: source.line,
      suite: suiteOf(source.file),
      status: "passed",
      duration_ms: 0,
      covers: [...item.covers],
      steps: [],
      assertions: [],
      attachments: [],
      timeout_ms: timeout,
      attempt: attempts.get(id)?.length ?? 0,
      reason: item.reason,
    };
    await host.emit({
      type: "case_started",
      id, file: source.file, line: source.line,
      name: record.name,
      full_name: record.full_name,
      timeout_ms: timeout,
      watchdog_timeout_ms: timeout + (item.timeoutCleanup ?? MAX_DEFER_BUDGET_MS) + FORENSICS_BUDGET_MS + WEDGED_DEFER_BUDGET_MS,
      covers: record.covers,
    });

    const caseStarted = Date.now();
    if (contaminated || item.annotation === "skip" || item.annotation === "fixme") {
      if (contaminated) record.reason = "Not run: a previous spec left asynchronous work pending; restart the run.";
      record.status = "skipped";
      record.duration_ms = Date.now() - caseStarted;
      cases.push(record);
      await finishCase(host, record);
      continue;
    }

    const fixture = new LiveFixture(
      `${encodeURIComponent(id)}/attempt-${record.attempt}`,
      pinApp(item.app),
      host,
      host.args,
      automationRoot(),
      timeout,
    );

    let status: SpecStatus = "passed";
    let error: unknown;
    let phase: "beforeEach" | "body" | "defer" | "forensics" | "timeout" = "body";

    const shouldRelaunch = item.fresh || forceRelaunchNext;
    forceRelaunchNext = false;
    const bodyPromise = (async () => {
      if (shouldRelaunch) await relaunchHome(fixture.raw);
      phase = "beforeEach";
      for (const hook of resetHooks) { if (hookFiles.get(hook) === source.file) await hook.fn(fixture); }
      for (const hook of hooks) {
        if (hookFiles.get(hook) === source.file) await hook.fn(fixture);
      }
      phase = "body";
      await item.body(fixture);
    })();
    const bodyResult = bodyPromise.then(
      () => ({ ok: true as const }),
      (err: unknown) => ({ ok: false as const, err }),
    );

    const timeoutError = new TimeoutError(`spec timed out after ${timeout}ms`);
    let timedOut = false;
    let bodySettled = false;
    void bodyResult.then(() => { bodySettled = true; });
    let timerHandle: ReturnType<typeof setTimeout> | undefined;
    const timer = new Promise<"timeout">((resolve) => {
      timerHandle = setTimeout(() => {
        timedOut = true;
        fixture.abort(timeoutError);
        resolve("timeout");
      }, timeout);
    });

    try {
      const winner = await Promise.race([bodyResult, timer]);
      if (winner === "timeout") {
        status = "timeout";
        error = timeoutError;
        phase = "timeout";
        forceRelaunchNext = true;
        try { await within(bodyResult, WEDGED_DEFER_BUDGET_MS, "body did not settle after timeout"); }
        catch { contaminated = true; }
      } else if (!winner.ok) {
        if (timedOut || fixture.aborted) {
          status = "timeout";
          error = timeoutError;
          phase = "timeout";
          forceRelaunchNext = true;
        } else {
          status = "failed";
          error = winner.err;
          fixture.failurePhase = phase;
        }
      }
    } finally {
      if (timerHandle !== undefined) clearTimeout(timerHandle);
    }

    let evidenceCollected = false;
    const collectEvidence = async () => {
      if (evidenceCollected || !item.forensics) return;
      evidenceCollected = true;
      try {
        // The whole point of the timeout path is that a wedged app does not
        // stall the run, and these calls bypass `guard` on the raw driver.
        await within(captureForensics(fixture), FORENSICS_BUDGET_MS, "failure evidence timed out");
      } catch (forensicsError) {
        // Evidence failures must never replace the product failure.
        if (forensicsError instanceof TimeoutError) contaminated = true;
        await host.emit({ type: "diagnostic", phase: "forensics", message: String(forensicsError) });
      }
    };
    if (status !== "passed") await collectEvidence();

    phase = "defer";
    const deferErrors: unknown[] = [];
    const cleanupBudget = item.timeoutCleanup ?? (status === "timeout" ? WEDGED_DEFER_BUDGET_MS : MAX_DEFER_BUDGET_MS);
    const afterEach = afterHooks.filter(hook => hookFiles.get(hook) === source.file);
    const hasCleanup = afterEach.length > 0 || fixture.defers.length > 0;
    function* cleanupTasks() {
      for (const hook of afterEach) yield () => hook.fn(fixture);
      while (fixture.defers.length > 0) yield fixture.defers.pop()!;
    }
    // Never reopen a timed-out fixture while its body can still issue commands.
    if (hasCleanup && bodySettled) {
      fixture.allowCleanup(cleanupBudget);
      const deadline = Date.now() + cleanupBudget;
      for (const cleanup of cleanupTasks()) {
        let settled = false;
        const task = Promise.resolve().then(cleanup).finally(() => { settled = true; });
        try { await within(task, Math.max(0, deadline - Date.now()), "fixture cleanup budget exceeded"); }
        catch (err) { deferErrors.push(err); }
        if (!settled) { contaminated = true; break; }
      }
    } else if (hasCleanup) {
      deferErrors.push(new Error("Cleanup skipped because the timed-out body is still running"));
    }
    fixture.endCleanup();

    if (item.annotation === "fail") {
      const bodyAssertion =
        status === "failed" &&
        fixture.failurePhase === "body" &&
        error instanceof AssertionError;
      if (bodyAssertion) {
        status = "xfail";
      } else if (status === "passed") {
        status = "xpass";
        error = new Error("spec.fail passed (xpass)");
        fixture.failurePhase = "body";
      }
    }

    if (deferErrors.length > 0) {
      // A spec that hung is reported as a timeout; cleanup that could not
      // finish afterwards is a consequence of the hang, not a different
      // verdict, and relabelling it hides what actually happened.
      if (status !== "timeout") {
        status = "failed";
        fixture.failurePhase = "defer";
      }
      const cleanupText = deferErrors
        .map((item) => (item instanceof Error ? item.message : String(item)))
        .join("\n");
      if (error instanceof Error) {
        error.message += `\nCleanup failures:\n${cleanupText}`;
      } else {
        error = new Error(`t.defer failed:\n${cleanupText}`);
      }
    }

    if (status !== "passed") await collectEvidence();
    fixture.close();
    record.status = status;
    record.duration_ms = Date.now() - caseStarted;
    record.steps = fixture.steps;
    record.assertions = fixture.assertions;
    if (fixture.observed.size > 0) record.observed = [...fixture.observed].sort();
    record.attachments = fixture.attachments;
    setAssertionSink();
    if (error) record.error = toReportError(error, fixture.currentStepPath());
    if (record.error) {
      record.error.phase = status === "timeout" ? "timeout" : fixture.failurePhase ?? phase;
      // JSC tail calls can omit the authored assertion frame.
      record.error.location ??= `${source.file}:${source.line}:1`;
    }
    const history = attempts.get(id) ?? [];
    history.push(record);
    attempts.set(id, history);
    const finished = { ...record, attempts: history.map(attempt => ({ ...attempt })),
      flaky: record.status === "passed" && history.length > 1 };
    const previous = cases.findIndex(item => item.id === id);
    if (previous >= 0) cases[previous] = finished; else cases.push(finished);
    await finishCase(host, finished);
    if (!contaminated && (status === "failed" || status === "timeout") && history.length <= retries) {
      queue.splice(queue.indexOf(item) + 1, 0, { ...item });
    }
  }

  const counts = countStatuses(cases);
  const duration_ms = Date.now() - started;
  const json: JsonReport = {
    schema_version: 1,
    framework: { name: PACKAGE_NAME, version: VERSION },
    meta: {
      started_at: new Date(started).toISOString(),
      duration_ms,
      args: { ...host.args },
      platform: host.args.platform,
      framework: host.args.framework,
      subject,
      surface_coverage: trackSurface,
    },
    partial: contaminated,
    filtered: Boolean(grep || host.args.id || host.args.ids || shard) || hasOnly,
    duration_ms,
    ...counts,
    cases,
  };

  await attachText(host, "report.json", JSON.stringify(json, null, 2), "application/json");
  await attachText(host, "report.html", renderHtml(json), "text/html; charset=utf-8");
  await attachText(host, "junit.xml", renderJUnit(json), "application/xml; charset=utf-8");

  return json;
}

async function finishCase(
  host: ReturnType<typeof resolveHost>,
  record: CaseRecord,
): Promise<void> {
  await host.emit({
    type: "case_finished",
    name: record.name,
    full_name: record.full_name,
    status: record.status,
    id: record.id,
    duration_ms: record.duration_ms,
    error: record.error,
    record,

  });
}

/** Best-effort: a run against an unreachable app still reports its cases. */
async function describeSubject(): Promise<RunSubject | undefined> {
  try {
    const info = (await pinApp().info()) as unknown as Record<string, unknown>;
    return {
      appid: asText(info.appid),
      app_name: asText(info.app_name),
      version: asText(info.version),
      release_type: asText(info.release_type),
      pages: typeof info.pages_count === "number" ? info.pages_count : undefined,
    };
  } catch {
    return undefined;
  }
}

function asText(value: unknown): string | undefined {
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

async function captureForensics(fixture: LiveFixture): Promise<void> {
  try {
    const shot = await fixture.raw.page.screenshot();
    const payload = encodeScreenshot(shot);
    if (payload) {
      await fixture.attachRaw("failure.png", payload);
    }
  } catch {
    // Screenshot is best-effort.
  }

  let route: unknown;
  let info: unknown;
  try {
    route = await fixture.raw.nav.current();
  } catch {
    route = undefined;
  }
  try {
    info = await fixture.raw.info();
  } catch {
    info = undefined;
  }
  const forensics = {
    route,
    info,
    step: fixture.currentStepPath() ?? null,
  };
  await fixture.attachRaw("forensics.json", forensics);

  const logs = await fixtureHostLogs();
  if (logs !== undefined) {
    await fixture.attachRaw("logs.txt", logs);
  }
}

async function fixtureHostLogs(): Promise<string | undefined> {
  const host = resolveHost();
  return host.logs();
}

function encodeScreenshot(shot: unknown): { mimeType: string; base64: string } | undefined {
  if (!shot || typeof shot !== "object") return undefined;
  const record = shot as { base64?: unknown; mimeType?: unknown };
  if (typeof record.base64 !== "string") return undefined;
  return { mimeType: "image/png", base64: record.base64 };
}

function reset(): void {
  specs.length = 0;
  hooks.length = 0;
  afterHooks.length = 0;
  resetHooks.length = 0;
  trackSurface = false;
  fileCounts.clear();
  forceRelaunchNext = false;
  clearInline();
  setAssertionSink();
}

const controller: LingxiaTestController = {
  run,
  version: VERSION,
  reset,
};

if (!globalThis.__LINGXIA_TEST__) {
  Object.defineProperty(globalThis, "__LINGXIA_TEST__", {
    value: controller,
    enumerable: false,
    configurable: false,
    writable: false,
  });
}

export { spec, expect, run, reset, resolvedId, trackPublicSurface };
export type { SpecOptions, SpecBody, Fixture };

async function within<T>(task: Promise<T>, ms: number, message: string): Promise<T> {
  let handle: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([task, new Promise<never>((_, reject) => {
      handle = setTimeout(() => reject(new TimeoutError(message)), ms);
    })]);
  } finally { if (handle !== undefined) clearTimeout(handle); }
}

function stableHash(value: string): number {
  let hash = 2166136261;
  for (let i = 0; i < value.length; i++) hash = Math.imul(hash ^ value.charCodeAt(i), 16777619);
  return hash >>> 0;
}
