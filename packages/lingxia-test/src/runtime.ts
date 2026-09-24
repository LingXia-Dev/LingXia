import { expect, setAssertionSink } from "./expect.js";
import { LiveFixture, SkipSignal, TimeoutError, toReportError } from "./fixture.js";
import { formatValue } from "./format.js";
import { attachText, resolveHost, warnVersionSkew, type ResolvedHost } from "./host.js";
import { captureFrames, fileStem, resolveOrigin, resolveOwner, slugTitle, type StackFrame } from "./ids.js";
import { renderJUnit } from "./junit.js";
import { createRedactor } from "./redact.js";
import { clearInline, countStatuses, renderHtml } from "./report.js";
import type { SpecApi } from "./spec-api.js";
import type {
  CaseRecord,
  FailOptions,
  FailurePage,
  FailureRecord,
  Fixture,
  JsonReport,
  LingxiaTestController,
  ProtocolReport,
  RejectExpected,
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
import type { HostRunAutomation, LxAppDriver } from "@lingxia/types/automation";

type Annotation = "default" | "skip" | "only" | "fixme" | "fail";

interface RegisteredSpec {
  title: string;
  id?: string;
  covers: string[];
  timeout: number;
  timeoutCleanup?: number;
  fresh: boolean;
  restoreProfile: boolean;
  app?: string;
  forensics: boolean;
  reason?: string;
  /** `spec.fail` only: the failure the body is expected to produce. */
  expected?: RejectExpected;
  annotation: Annotation;
  body: SpecBody;
  frames: StackFrame[];
  /**
   * 1-based position among the specs of the same file that need a generated
   * id (no `id`, no ASCII slug). Assigned when the run starts: the file is
   * only known once the bundle's source map is installed.
   */
  indexInFile?: number;
}

interface Hook {
  frames: StackFrame[];
  fn: SpecBody;
}

const specs: RegisteredSpec[] = [];
const hooks: Hook[] = [];
const afterHooks: Hook[] = [];
const resetHooks: Hook[] = [];
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
  const expected = (options as FailOptions).expected;
  if (expected !== undefined) {
    if (annotation !== "fail") throw new TypeError("`expected` is only meaningful for spec.fail()");
    if (!expected || typeof expected !== "object") throw new TypeError("spec.fail `expected` must be an object");
    if (expected.code === undefined && expected.message === undefined) {
      throw new TypeError("spec.fail `expected` needs a code or a message");
    }
  }
  for (const [name, value] of Object.entries({ timeout: options.timeout, timeoutCleanup: options.timeoutCleanup })) {
    if (value !== undefined && (!Number.isFinite(value) || value <= 0)) throw new TypeError(`${name} must be a positive finite number`);
  }
  // The bundle map is installed after the modules run, so keep the raw frames;
  // the authored file is only knowable once the run starts. `frames[0]` is a
  // frame inside this package, identical for every caller, so it can order
  // registrations but must never stand in for identity.
  const frames = captureFrames();
  specs.push({
    title,
    id: options.id,
    covers: [...(options.covers ?? [])],
    timeout: options.timeout ?? DEFAULT_SPEC_TIMEOUT_MS,
    timeoutCleanup: options.timeoutCleanup,
    fresh: options.fresh === true,
    restoreProfile: options.restoreProfile === true,
    app: options.app,
    forensics: options.forensics !== false,
    reason: options.reason,
    expected,
    annotation,
    body,
    frames,
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
    fail(title: string, optionsOrBody: FailOptions | SpecBody, maybeBody?: SpecBody): void {
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

function needsGeneratedId(item: RegisteredSpec): boolean {
  return !(item.id && item.id.length > 0) && slugTitle(item.title) === undefined;
}

/**
 * Number the specs that need a generated id, per file and in registration
 * order, so adding a spec to one file never renames a spec in another.
 */
function assignFileIndexes(): void {
  const counts = new Map<string, number>();
  for (const item of specs) {
    if (!needsGeneratedId(item)) continue;
    const file = sourceOf(item).file;
    const next = (counts.get(file) ?? 0) + 1;
    counts.set(file, next);
    item.indexInFile = next;
  }
}

function resolvedId(item: RegisteredSpec): string {
  if (item.id && item.id.length > 0) return item.id;
  const slug = slugTitle(item.title);
  if (slug) return slug;
  if (item.indexInFile === undefined) assignFileIndexes();
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
  const lx = (globalThis as { lx?: { automation?: () => HostRunAutomation } }).lx;
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
  try {
    await app.nav.relaunch({ page: home, waitUntil: "ready" });
  } catch (error) {
    // The home page may hand off to another page itself (a gate, a login
    // redirect); that is the app's own start, not a failed relaunch.
    if (!/was disposed before runtime became ready/.test(String((error as Error)?.message ?? error))) throw error;
  }
}

/**
 * Controls lxdev sent inside `args` before it had a channel of its own. Only
 * read from `args` when the host has no `control`; a current host keeps them
 * apart so a user's `--arg id=…` is just an arg.
 */
const LEGACY_CONTROL_KEYS = new Set([
  "grep", "retries", "shard", "ids", "id", "passWithNoTests", "forbidOnly", "forbid-only",
  "platform", "secretArgs",
]);

/** One planned execution: a spec, and which of its `--repeat-each` runs. */
interface Planned {
  item: RegisteredSpec;
  /** 1-based; always 1 without `--repeat-each`. */
  repeat: number;
}

/**
 * The run budget lxdev asked for: a fixed `budgetMs`, or scaled to the
 * planned executions — `max(budgetMinMs, planned × budgetPerSpecMs)`, capped
 * at `budgetMaxMs`. `undefined` when lxdev sent neither (an older lxdev: the
 * host's own deadline is the only budget).
 */
export function runBudget(control: Record<string, string>, planned: number): { ms: number; auto: boolean } | undefined {
  const fixed = Number(control.budgetMs);
  if (control.budgetMs !== undefined && Number.isFinite(fixed) && fixed > 0) return { ms: fixed, auto: false };
  const perSpec = Number(control.budgetPerSpecMs);
  if (control.budgetPerSpecMs === undefined || !Number.isFinite(perSpec) || perSpec <= 0) return undefined;
  const min = Number(control.budgetMinMs ?? 0);
  const max = Number(control.budgetMaxMs ?? Number.POSITIVE_INFINITY);
  const scaled = Math.max(Number.isFinite(min) ? min : 0, planned * perSpec);
  return { ms: Math.min(scaled, Number.isFinite(max) && max > 0 ? max : scaled), auto: true };
}

/** Seeded Fisher–Yates (mulberry32), so a printed seed reproduces the order. */
export function shuffleWithSeed<T>(items: readonly T[], seed: number): T[] {
  let state = seed >>> 0;
  const next = () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  const out = [...items];
  for (let i = out.length - 1; i > 0; i -= 1) {
    const j = Math.floor(next() * (i + 1));
    [out[i], out[j]] = [out[j]!, out[i]!];
  }
  return out;
}

function splitControl(host: ResolvedHost): { args: Record<string, string>; control: Record<string, string> } {
  if (host.control) return { args: host.args, control: host.control };
  const args: Record<string, string> = {};
  const control: Record<string, string> = {};
  for (const [key, value] of Object.entries(host.args)) {
    (LEGACY_CONTROL_KEYS.has(key) ? control : args)[key] = value;
  }
  return { args, control };
}

/** `secretArgs` is a JSON list of `--secret-arg` keys (older lxdev: comma list). */
function secretKeys(listed: string | undefined): string[] {
  if (!listed) return [];
  try {
    const parsed: unknown = JSON.parse(listed);
    if (Array.isArray(parsed)) return parsed.filter((key): key is string => typeof key === "string");
  } catch {
    // fall through to the comma form
  }
  return listed.split(",").map((key) => key.trim()).filter(Boolean);
}

async function run(): Promise<ProtocolReport> {
  warnVersionSkew();
  const rawHost = resolveHost();
  const { args, control } = splitControl(rawHost);
  // Secret args reach the spec through `t.args` but never an event or report.
  const redact = createRedactor(args, secretKeys(control.secretArgs));
  const host: ResolvedHost = {
    ...rawHost,
    args,
    emit: (event) => rawHost.emit(redact.deep(event)),
  };
  const started = Date.now();
  clearInline();

  assignFileIndexes();
  const ids = new Map<string, string>();
  for (const item of specs) {
    const id = resolvedId(item);
    const previous = ids.get(id);
    if (previous) {
      throw new Error(`Duplicate spec id ${JSON.stringify(id)} (${previous} and ${item.title})`);
    }
    ids.set(id, item.title);
  }

  const grep = control.grep;
  const pattern = grep ? new RegExp(grep) : undefined;
  const forbidOnly = control.forbidOnly === "1" || control["forbid-only"] === "1";
  const hasOnly = specs.some((item) => item.annotation === "only");
  if (hasOnly && forbidOnly) {
    throw new Error("spec.only is registered; lxdev test --forbid-only refuses to run");
  }

  const retries = Number(control.retries ?? 0);
  if (!Number.isInteger(retries) || retries < 0 || retries > 10) throw new Error("retries must be between 0 and 10");
  const selectedIds: string[] | undefined = control.ids ? JSON.parse(control.ids) : undefined;
  const shard = control.shard?.split("/").map(Number);
  if (shard && (shard.length !== 2 || shard.some(n => !Number.isInteger(n) || n < 1) || shard[0]! > shard[1]!)) throw new Error("shard must be INDEX/TOTAL (1-based)");
  const selected = specs.filter((item) => {
    if (hasOnly && item.annotation !== "only") return false;
    if (selectedIds && !selectedIds.includes(resolvedId(item))) return false;
    if (shard && stableHash(resolvedId(item)) % shard[1]! !== shard[0]! - 1) return false;
    if (control.id && resolvedId(item) !== control.id) return false;
    if (!grep) return true;
    const id = resolvedId(item);
    return pattern!.test(item.title) || pattern!.test(id);
  });

  if (selected.length === 0 && control.passWithNoTests !== "1") {
    throw new Error("No tests matched this selection. Check the entry and filters, or use --pass-with-no-tests.");
  }
  const repeatEach = Number(control.repeatEach ?? 1);
  if (!Number.isInteger(repeatEach) || repeatEach < 1 || repeatEach > 100) throw new Error("repeatEach must be between 1 and 100");
  const shuffleSeed = control.shuffle === undefined ? undefined : Number(control.shuffle);
  if (shuffleSeed !== undefined && (!Number.isInteger(shuffleSeed) || shuffleSeed < 0)) throw new Error("shuffle seed must be a non-negative integer");
  const repeated: Planned[] = selected.flatMap((item) =>
    Array.from({ length: repeatEach }, (_, index) => ({ item, repeat: index + 1 })));
  const plan = shuffleSeed === undefined ? repeated : shuffleWithSeed(repeated, shuffleSeed);
  const budget = runBudget(control, plan.length);
  const fullName = (item: RegisteredSpec, repeat: number) =>
    (item.id ? `${item.id} | ${item.title}` : item.title) + (repeatEach > 1 ? ` [repeat ${repeat}/${repeatEach}]` : "");
  // `args` is the masked view, so lxdev can write the same one into any
  // report it has to complete itself.
  await host.emit({ type: "run_started", schema_version: 1, total: plan.length, args: redact.args(args),
    cases: plan.map(({ item, repeat }) => ({ id: resolvedId(item), title: item.title, name: item.title,
      full_name: fullName(item, repeat), ...sourceOf(item), ...(repeatEach > 1 ? { repeat } : {}),
      suite: suiteOf(sourceOf(item).file), timeout_ms: item.timeout, covers: item.covers })) });

  // Resolve each hook's spec file once, so a `beforeEach` stays scoped to the
  // file that registered it rather than running for every spec in the run. A
  // hook registered by a shared helper belongs to the spec file that called
  // the helper (the nearest spec file on its registration stack).
  const specFiles = new Set(specs.map((item) => sourceOf(item).file));
  const hookFiles = new Map<Hook, string>(
    [...hooks, ...afterHooks, ...resetHooks].map((hook) => [hook, resolveOwner(hook.frames, specFiles).file] as const),
  );
  for (const [kind, list] of [["beforeEach", hooks], ["afterEach", afterHooks], ["reset", resetHooks]] as const) {
    for (const hook of list) {
      const file = hookFiles.get(hook)!;
      if (specFiles.has(file)) continue;
      await host.emit({ type: "diagnostic", phase: "collect",
        message: `spec.${kind}() registered from ${file} never runs: no spec file is on its call stack. Call it (or the helper that registers it) from a spec file's top level.` });
    }
  }
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
  let contaminationReason: string | undefined;

  const queue = [...plan];
  const attempts = new Map<string, CaseRecord[]>();
  const executionKey = (id: string, repeat: number) => `${id}#${repeat}`;
  let budgetExhausted: { after: number } | undefined;
  for (const planned of queue) {
    const { item, repeat } = planned;
    const id = resolvedId(item);
    const key = executionKey(id, repeat);
    const timeout = item.timeout;
    const source = sourceOf(item);
    // Checked between specs: a spec already running keeps its own timeout.
    if (!budgetExhausted && budget && Date.now() - started >= budget.ms) {
      budgetExhausted = { after: new Set(cases.map((done) => executionKey(done.id, done.repeat ?? 1))).size };
      await host.emit({ type: "diagnostic", phase: "budget",
        message: `Run budget of ${Math.round(budget.ms / 1000)}s exhausted after ${budgetExhausted.after}/${plan.length} specs; the rest are reported as not run. Raise --timeout-secs, or split the suite with --shard.` });
    }
    const record: CaseRecord = {
      id,
      title: item.title,
      name: item.title,
      full_name: fullName(item, repeat),
      ...(repeatEach > 1 ? { repeat } : {}),
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
      attempt: attempts.get(key)?.length ?? 0,
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
    if (budgetExhausted || contaminated || item.annotation === "skip" || item.annotation === "fixme") {
      if (contaminated) record.reason = contaminationReason ?? "Not run: a previous spec left asynchronous work pending; restart the run.";
      else if (budgetExhausted && budget) record.reason = `Not run: the run budget of ${Math.round(budget.ms / 1000)}s was exhausted after ${budgetExhausted.after}/${plan.length} specs.`;
      record.status = "skipped";
      record.duration_ms = Date.now() - caseStarted;
      cases.push(record);
      await finishCase(host, record);
      continue;
    }

    const fixture = new LiveFixture(
      `${encodeURIComponent(id)}${repeatEach > 1 ? `/repeat-${repeat}` : ""}/attempt-${record.attempt}`,
      pinApp(item.app),
      host,
      args,
      automationRoot(),
      timeout,
      redact,
    );

    let status: SpecStatus = "passed";
    let error: unknown;
    let phase: "beforeEach" | "body" | "defer" | "forensics" | "timeout" = "body";

    const shouldRelaunch = item.fresh || item.restoreProfile || forceRelaunchNext;
    forceRelaunchNext = false;
    // `restoreProfile`: snapshot the isolated profile now and roll back to it
    // after the spec. Registered as the first cleanup, so it runs last.
    let profileRestored = false;
    const bodyPromise = (async () => {
      if (item.restoreProfile) {
        phase = "beforeEach";
        const checkpoint = await fixture.profile.checkpoint();
        fixture.defer(async () => {
          await fixture.profile.restore(checkpoint);
          profileRestored = true;
          await fixture.profile.drop(checkpoint);
        });
        phase = "body";
      }
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
        } else if (winner.err instanceof SkipSignal) {
          status = "skipped";
        } else {
          status = "failed";
          error = winner.err;
          fixture.failurePhase = phase;
        }
      } else if (fixture.skipReason !== undefined) {
        // The body caught the signal; the skip was still requested.
        status = "skipped";
      }
    } finally {
      if (timerHandle !== undefined) clearTimeout(timerHandle);
    }

    let evidenceCollected = false;
    let failurePage: FailurePage | undefined;
    const collectEvidence = async () => {
      if (evidenceCollected || !item.forensics) return;
      evidenceCollected = true;
      try {
        // The whole point of the timeout path is that a wedged app does not
        // stall the run, and these calls bypass `guard` on the raw driver.
        failurePage = await within(captureForensics(fixture), FORENSICS_BUDGET_MS, "failure evidence timed out");
      } catch (forensicsError) {
        // Evidence failures must never replace the product failure.
        if (forensicsError instanceof TimeoutError) contaminated = true;
        await host.emit({ type: "diagnostic", phase: "forensics", message: String(forensicsError) });
      }
    };
    if (status !== "passed" && status !== "skipped") await collectEvidence();

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
    if (item.restoreProfile && !profileRestored && !contaminated) {
      // The next spec would start on this spec's data: a stuck cleanup.
      contaminated = true;
      contaminationReason = "Not run: a previous spec's restoreProfile could not roll the app's data back; restart the run.";
    }

    if (item.annotation === "fail") {
      // Without `expected`, the declared failure is whatever the body does to
      // fail: a product rejection counts as much as an assertion. With it,
      // only a matching failure is the known one; anything else — a mistyped
      // selector, a dead fixture server — is a real failure. Setup failures,
      // skips and timeouts keep their own verdicts.
      if (status === "failed" && fixture.failurePhase === "body") {
        const mismatch = item.expected ? expectedMismatch(error, item.expected) : undefined;
        if (mismatch === undefined) status = "xfail";
        else error = mismatch;
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

    if (status !== "passed" && status !== "skipped") await collectEvidence();
    if (status === "skipped") record.reason = fixture.skipReason ?? record.reason;
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
      // Only the action that produced this very error explains it: an action
      // the spec expected to fail (`t.reject`) must not be blamed later.
      if (fixture.failedAction && fixture.failedAction.error === error) {
        record.error.failedAction = fixture.failedAction.action;
      }
      const page = failurePage ?? pageFromErrorData(record.error.data);
      if (page) record.error.page = page;
    }
    const history = attempts.get(key) ?? [];
    history.push(record);
    attempts.set(key, history);
    const finished = { ...record, attempts: history.length > 1 ? history.map(attempt => ({ ...attempt })) : undefined,
      flaky: record.status === "passed" && history.length > 1 };
    const previous = cases.findIndex(item => item.id === id && (item.repeat ?? 1) === repeat);
    if (previous >= 0) cases[previous] = finished; else cases.push(finished);
    await finishCase(host, finished);
    if (!contaminated && (status === "failed" || status === "timeout") && history.length <= retries) {
      queue.splice(queue.indexOf(planned) + 1, 0, { item: { ...item }, repeat });
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
      args: redact.args(args),
      run: redact.deep(control),
      platform: control.platform,
      framework: args.framework,
      subject,
      surface_coverage: trackSurface,
      ...(budget ? { budget: { ms: budget.ms, auto: budget.auto, planned: plan.length,
        ...(budgetExhausted ? { exhausted_after: budgetExhausted.after } : {}) } } : {}),
      ...(shuffleSeed !== undefined ? { shuffle_seed: shuffleSeed } : {}),
      ...(repeatEach > 1 ? { repeat_each: repeatEach } : {}),
    },
    partial: contaminated || budgetExhausted !== undefined,
    filtered: Boolean(grep || control.id || control.ids || shard) || hasOnly,
    duration_ms,
    ...counts,
    cases: redact.deep(cases),
  };
  json.failures = failureRecords(json.cases);

  await attachText(host, "report.json", JSON.stringify(json, null, 2), "application/json");
  await attachText(host, "report.html", renderHtml(json), "text/html; charset=utf-8");
  await attachText(host, "junit.xml", renderJUnit(json), "application/xml; charset=utf-8");

  return json;
}

/**
 * `undefined` when `error` is the failure `spec.fail` declared; otherwise the
 * error to report, saying what was expected instead.
 */
function expectedMismatch(error: unknown, expected: RejectExpected): Error | undefined {
  const record = (error && typeof error === "object" ? error : {}) as { code?: unknown; message?: unknown };
  const message = error instanceof Error ? error.message : typeof record.message === "string" ? record.message : String(error);
  const problems: string[] = [];
  if (expected.code !== undefined && record.code !== expected.code) {
    problems.push(`code ${formatValue(expected.code)}, got ${formatValue(record.code)}`);
  }
  if (typeof expected.message === "string" && !message.includes(expected.message)) {
    problems.push(`a message containing ${formatValue(expected.message)}`);
  }
  if (expected.message instanceof RegExp) {
    expected.message.lastIndex = 0;
    if (!expected.message.test(message)) problems.push(`a message matching ${String(expected.message)}`);
  }
  if (problems.length === 0) return undefined;
  const header = `spec.fail expected a failure with ${problems.join(" and ")}; the body failed differently:`;
  if (error instanceof Error) {
    error.message = `${header}\n${error.message}`;
    return error;
  }
  return new Error(`${header}\n${message}`);
}

async function finishCase(
  host: ResolvedHost,
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

/** The current page as a driver error's `data.current` names it. */
function pageFromErrorData(data: unknown): FailurePage | undefined {
  const current = data && typeof data === "object" ? (data as { current?: unknown }).current : undefined;
  return current && typeof current === "object" ? toFailurePage(current) : undefined;
}

function toFailurePage(value: unknown): FailurePage | undefined {
  if (!value || typeof value !== "object") return undefined;
  const page = value as { name?: unknown; instanceId?: unknown };
  const name = typeof page.name === "string" ? page.name : null;
  const instanceId = typeof page.instanceId === "string" ? page.instanceId : null;
  return name === null && instanceId === null ? undefined : { name, instanceId };
}

/** `failures[]`: every failing case, flat, with what failed and where. */
export function failureRecords(cases: CaseRecord[]): FailureRecord[] {
  return cases
    .filter((item) => item.status === "failed" || item.status === "timeout" || item.status === "xpass")
    .map((item) => {
      const error = item.error;
      const screenshot = item.attachments.find((attachment) => attachment.name === "failure.png")?.path;
      return {
        id: item.id,
        title: item.title,
        file: item.file,
        line: item.line,
        phase: error?.phase,
        code: error?.code,
        message: error?.message ?? item.status,
        failedAction: error?.failedAction,
        page: error?.page,
        screenshot,
      };
    });
}

/** Attach failure evidence; resolves to the page that was current, if known. */
async function captureForensics(fixture: LiveFixture): Promise<FailurePage | undefined> {
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
  return toFailurePage(route);
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
