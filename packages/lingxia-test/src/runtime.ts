import { AssertionError, expect, setAssertionSink } from "./expect.js";
import { LiveFixture, TimeoutError, protocolStatus, toReportError } from "./fixture.js";
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
  const forbidOnly = host.args.forbidOnly === "1" || host.args["forbid-only"] === "1";
  const hasOnly = specs.some((item) => item.annotation === "only");
  if (hasOnly && forbidOnly) {
    throw new Error("spec.only is registered; lxdev test --forbid-only refuses to run");
  }

  const selected = specs.filter((item) => {
    if (hasOnly && item.annotation !== "only") return false;
    if (!grep) return true;
    const id = resolvedId(item);
    try {
      const pattern = new RegExp(grep);
      return pattern.test(item.title) || pattern.test(id);
    } catch {
      return item.title.includes(grep) || id.includes(grep);
    }
  });

  // Resolve each hook's authored file once, so a `beforeEach` stays scoped to
  // the file that declared it rather than running for every spec in the run.
  const hookFiles = new Map<Hook, string>(
    hooks.map((hook) => [hook, resolveOrigin(hook.frames).file] as const),
  );
  const subject = await describeSubject();
  const cases: CaseRecord[] = [];
  forceRelaunchNext = false;

  for (const item of selected) {
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
      reason: item.reason,
    };
    await host.emit({
      type: "case_started",
      name: record.name,
      full_name: record.full_name,
      timeout_ms: timeout,
      covers: record.covers,
    });

    const caseStarted = Date.now();
    if (item.annotation === "skip" || item.annotation === "fixme") {
      record.status = "skipped";
      record.duration_ms = Date.now() - caseStarted;
      cases.push(record);
      await finishCase(host, record);
      continue;
    }

    const fixture = new LiveFixture(
      id,
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
        await Promise.race([
          bodyResult,
          new Promise<void>((resolve) => {
            setTimeout(resolve, WEDGED_DEFER_BUDGET_MS);
          }),
        ]);
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

    if (status !== "passed" && item.forensics) {
      phase = "forensics";
      try {
        // The whole point of the timeout path is that a wedged app does not
        // stall the run, and these calls bypass `guard` on the raw driver.
        await Promise.race([
          captureForensics(fixture),
          new Promise<void>((resolve) => {
            setTimeout(resolve, FORENSICS_BUDGET_MS);
          }),
        ]);
      } catch (forensicsError) {
        if (status !== "timeout") {
          status = "failed";
          error = forensicsError;
          fixture.failurePhase = "forensics";
        }
      }
    }

    phase = "defer";
    const deferErrors: unknown[] = [];
    // A wedged app must not stall the run, but a healthy one deserves the time
    // its cleanup actually needs — a relaunch plus a page wait routinely
    // outruns the post-timeout budget, and a cut-short cleanup leaks the
    // fixture's state into every spec that follows.
    if (fixture.defers.length > 0) {
      // Cleanup is not spent from the spec's budget: a 3s spec whose defer
      // relaunches a page would otherwise be failed for its own tidying.
      fixture.allowCleanup(
        status === "timeout" ? WEDGED_DEFER_BUDGET_MS : MAX_DEFER_BUDGET_MS,
      );
    }
    for (let index = fixture.defers.length - 1; index >= 0; index -= 1) {
      try {
        await fixture.defers[index]!();
      } catch (deferError) {
        deferErrors.push(deferError);
      }
    }

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
      }
    }

    if (fixture.defers.length > 0) fixture.endCleanup();
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

    record.status = status;
    record.duration_ms = Date.now() - caseStarted;
    record.steps = fixture.steps;
    record.assertions = fixture.assertions;
    record.attachments = fixture.attachments;
    setAssertionSink();
    if (error && status !== "xfail") {
      record.error = toReportError(error, fixture.currentStepPath());
    } else if (status === "xfail" && error) {
      record.error = toReportError(error, fixture.currentStepPath());
    }
    cases.push(record);
    await finishCase(host, record);
  }

  const counts = countStatuses(cases);
  const duration_ms = Date.now() - started;
  const json: JsonReport = {
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
    partial: false,
    filtered: Boolean(grep) || hasOnly,
    duration_ms,
    ...counts,
    cases,
  };

  await attachText(host, "report.json", JSON.stringify(json, null, 2), "application/json");
  await attachText(host, "report.html", renderHtml(json), "text/html; charset=utf-8");
  await attachText(host, "junit.xml", renderJUnit(json), "application/xml; charset=utf-8");

  const protocol: ProtocolReport = {
    total: cases.length,
    passed: cases.filter((item) => protocolStatus(item.status) === "passed").length,
    failed: cases.filter((item) => protocolStatus(item.status) === "failed").length,
    skipped: cases.filter((item) => protocolStatus(item.status) === "skipped").length,
    duration_ms: json.duration_ms,
    cases: cases.map((item) => ({
      name: item.name,
      full_name: item.full_name,
      status: protocolStatus(item.status),
      duration_ms: item.duration_ms,
      error:
        item.error && protocolStatus(item.status) === "failed"
          ? {
              name: item.error.name,
              message: item.error.message,
              stack: item.error.stack,
              causes: [],
            }
          : undefined,
    })),
  };
  return protocol;
}

async function finishCase(
  host: ReturnType<typeof resolveHost>,
  record: CaseRecord,
): Promise<void> {
  await host.emit({
    type: "case_finished",
    name: record.name,
    full_name: record.full_name,
    status: protocolStatus(record.status),
    duration_ms: record.duration_ms,
    error:
      record.error && protocolStatus(record.status) === "failed"
        ? record.error
        : undefined,
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
