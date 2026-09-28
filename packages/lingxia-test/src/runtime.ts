import { expect, setExpectScope, setStrayAssertionSink, type LoggedAssertion } from "./expect.js";
import { LiveFixture, SkipSignal, TimeoutError, toReportError } from "./fixture.js";
import { formatValue } from "./format.js";
import { attachText, resolveHost, warnVersionSkew, type ResolvedHost } from "./host.js";
import { captureFrames, fileStem, isUnattributed, resolveOrigin, resolveOwner, slugTitle, type StackFrame } from "./ids.js";
import { renderJUnit } from "./junit.js";
import { watchDialogs } from "./dialogs.js";
import { resetMocks } from "./mock.js";
import { createRedactor } from "./redact.js";
import { clearInline, countStatuses, renderHtml } from "./report.js";
import { coverageSummary, parseManifest } from "./coverage.js";
import { ContractError, ContractLedger, parseOpenApiControl, setActiveOpenApi } from "./openapi.js";
import { matchesTags, parseTagFilter, tagSummary, validateTags } from "./tags.js";
import type { SpecApi } from "./spec-api.js";
import type {
  FailOptions,
  FileOptions,
  Fixture,
  LingxiaTestController,
  RejectExpected,
  SpecBody,
  SpecOptions,
  SpecRequirements,
} from "./types.js";
import type {
  CaseRecord,
  FailurePage,
  FailureRecord,
  JsonReport,
  ListedSpec,
  FailureNetworkCall,
  ProtocolReport,
  RunSubject,
  SpecStatus,
  PageVisibility,
} from "./report-types.js";
import { SCREEN_LOCKED_NOTE, VISIBILITY_PROBE_BUDGET_MS, VISIBILITY_PROBE_SCRIPT, hiddenCause, pageVisibility } from "./locator.js";
import {
  DEFAULT_SPEC_TIMEOUT_MS,
  FORENSICS_BUDGET_MS,
  MAX_DEFER_BUDGET_MS,
  PACKAGE_NAME,
  VERSION,
  WEDGED_DEFER_BUDGET_MS,
} from "./version.js";
import type { HostRunAutomation, LxAppDriver, ScenarioCall } from "@lingxia/types/automation";
import { cancelTimersOf, describePending, installPendingTracker, pendingOf, restoreRawAuthority, revokeGrant, revokeRawAuthority, runnerClearTimeout, runnerSetTimeout, setPendingOwner, settleCallsOf, trackAutomationRoot, uninstallPendingTracker, type PendingWork } from "./pending.js";

type Annotation = "default" | "skip" | "only" | "fixme" | "fail";

interface RegisteredSpec {
  title: string;
  id?: string;
  covers: string[];
  /** The spec's own tags; the file's `spec.configure` tags join at run start. */
  tags: string[];
  timeout: number;
  timeoutCleanup?: number;
  fresh: boolean;
  restoreProfile: boolean;
  /** `restoreProfile: { keep }`: storage key globs the rollback keeps. */
  restoreKeep?: string[];
  app?: string;
  forensics: boolean;
  reason?: string;
  /** Run inputs the spec needs; unmet, it is skipped. */
  requires: { args: string[]; openapi: boolean };
  /**
   * The options the spec was registered with. The fields above are settled
   * from them and its file's `spec.configure()` defaults when the run starts
   * (the file is only known then).
   */
  own: SpecOptions;
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

/** A `spec.configure()` call, scoped to its file like a hook. */
interface FileConfig {
  frames: StackFrame[];
  options: FileOptions;
}

const specs: RegisteredSpec[] = [];
const hooks: Hook[] = [];
const afterHooks: Hook[] = [];
const resetHooks: Hook[] = [];
/** Why a companion does not reset its mock handlers, said once per run. */
const mockResetReasons = new Set<string>();
const fileConfigs: FileConfig[] = [];
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

/** `restoreProfile: { keep }` → the keep globs; `true`/absent → none. */
function restoreProfileKeep(option: SpecOptions["restoreProfile"]): string[] | undefined {
  if (option === undefined || typeof option === "boolean") return undefined;
  const keep = option && typeof option === "object" ? (option as { keep?: unknown }).keep : undefined;
  if (!Array.isArray(keep) || keep.some((glob) => typeof glob !== "string" || glob.length === 0)) {
    throw new TypeError("restoreProfile must be true or { keep: string[] } of non-empty storage key globs");
  }
  return [...keep];
}

function validateRequires(requires: unknown, where: string): { args: string[]; openapi: boolean } {
  if (requires === undefined) return { args: [], openapi: false };
  if (!requires || typeof requires !== "object") throw new TypeError(`${where} requires must be an object`);
  const { args, openapi } = requires as SpecRequirements;
  if (args !== undefined && (!Array.isArray(args) || args.some((key) => typeof key !== "string" || key.length === 0))) {
    throw new TypeError(`${where} requires.args must be an array of non-empty --arg keys`);
  }
  if (openapi !== undefined && typeof openapi !== "boolean") throw new TypeError(`${where} requires.openapi must be a boolean`);
  return { args: [...new Set(args ?? [])], openapi: openapi === true };
}

/** Checks what `spec()` and `spec.configure()` accept alike; throws on the first problem. */
function validateOptions(options: FileOptions, where: string): void {
  restoreProfileKeep(options.restoreProfile);
  for (const [name, value] of Object.entries({ timeout: options.timeout, timeoutCleanup: options.timeoutCleanup })) {
    if (value !== undefined && (!Number.isFinite(value) || value <= 0)) throw new TypeError(`${where} ${name} must be a positive finite number`);
  }
  validateTags(options.tags, where);
  validateRequires(options.requires, where);
  if (options.covers !== undefined && (!Array.isArray(options.covers) || options.covers.some((id) => typeof id !== "string"))) {
    throw new TypeError(`${where} covers must be an array of strings`);
  }
}

/**
 * The spec's settings: its own options over its file's `spec.configure()`
 * defaults. `tags`, `covers` and `requires` add to the file's.
 */
function settle(item: RegisteredSpec, file: FileOptions | undefined): void {
  const own = item.own;
  const pick = <K extends keyof FileOptions>(key: K): FileOptions[K] => own[key] !== undefined ? own[key] : file?.[key];
  const restoreProfile = pick("restoreProfile");
  const restoreKeep = restoreProfileKeep(restoreProfile);
  const fileRequires = validateRequires(file?.requires, "spec.configure()");
  const ownRequires = validateRequires(own.requires, `spec ${JSON.stringify(item.title)}`);
  item.covers = [...new Set([...(file?.covers ?? []), ...(own.covers ?? [])])];
  item.tags = [...new Set([...validateTags(file?.tags, "spec.configure()"), ...validateTags(own.tags, `spec ${JSON.stringify(item.title)}`)])];
  item.timeout = pick("timeout") ?? DEFAULT_SPEC_TIMEOUT_MS;
  item.timeoutCleanup = pick("timeoutCleanup");
  item.fresh = pick("fresh") === true;
  item.restoreProfile = restoreProfile === true || restoreKeep !== undefined;
  item.restoreKeep = restoreKeep;
  item.app = pick("app");
  item.forensics = pick("forensics") !== false;
  item.reason = pick("reason");
  item.requires = {
    args: [...new Set([...fileRequires.args, ...ownRequires.args])],
    openapi: fileRequires.openapi || ownRequires.openapi,
  };
}

/** Why the run cannot give the spec what it `requires`, or `undefined`. */
function unmetRequirements(
  item: RegisteredSpec,
  args: Readonly<Record<string, string | undefined>>,
  openapi: boolean,
): string | undefined {
  const missing = item.requires.args.filter((key) => args[key] === undefined);
  const needs: string[] = [];
  if (missing.length > 0) {
    needs.push(missing.map((key) => `--arg ${key}=<value>`).join(", ") + " (or --secret-arg)");
  }
  if (item.requires.openapi && !openapi) needs.push("--openapi <document>");
  return needs.length === 0 ? undefined : `Not run: requires ${needs.join(" and ")}.`;
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
  validateOptions(options, `spec ${JSON.stringify(title)}`);
  // The bundle map is installed after the modules run, so keep the raw frames;
  // the authored file is only knowable once the run starts. `frames[0]` is a
  // frame inside this package, identical for every caller, so it can order
  // registrations but must never stand in for identity.
  const frames = captureFrames();
  const item: RegisteredSpec = {
    title,
    id: options.id,
    covers: [],
    tags: [],
    timeout: DEFAULT_SPEC_TIMEOUT_MS,
    fresh: false,
    restoreProfile: false,
    forensics: true,
    requires: { args: [], openapi: false },
    own: { ...options },
    expected,
    annotation,
    body,
    frames,
  };
  // Settled again at run start, with the file's defaults.
  settle(item, undefined);
  specs.push(item);
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
    configure(options: FileOptions): void {
      if (!options || typeof options !== "object") throw new TypeError("spec.configure() requires an options object");
      if ((options as SpecOptions).id !== undefined) throw new TypeError("spec.configure() cannot set `id`: ids are per spec");
      validateOptions(options, "spec.configure()");
      fileConfigs.push({ frames: captureFrames(), options: { ...options } });
    },
  },
);

/** Two `spec.configure()` calls of one file: later keys win; lists add up. */
function mergeFileOptions(previous: FileOptions | undefined, next: FileOptions): FileOptions {
  if (!previous) return { ...next };
  const merged: FileOptions = { ...previous, ...Object.fromEntries(Object.entries(next).filter(([, value]) => value !== undefined)) };
  merged.tags = [...new Set([...(previous.tags ?? []), ...(next.tags ?? [])])];
  merged.covers = [...new Set([...(previous.covers ?? []), ...(next.covers ?? [])])];
  merged.requires = {
    args: [...new Set([...(previous.requires?.args ?? []), ...(next.requires?.args ?? [])])],
    openapi: previous.requires?.openapi === true || next.requires?.openapi === true,
  };
  return merged;
}

function sourceOf(item: RegisteredSpec): { file: string; line: number } {
  const origin = resolveOrigin(item.frames);
  return { file: origin.file, line: origin.line };
}

/**
 * Every spec, `spec.configure()` and hook must resolve to the file that
 * registered it: a file-less spec loses its file's tags and hooks without a
 * word, so the run stops and names what it could not place.
 */
function assertAttributed(): void {
  const lost: string[] = [];
  for (const item of specs) {
    const origin = resolveOrigin(item.frames);
    if (isUnattributed(origin.file)) lost.push(`spec ${JSON.stringify(item.title)} (${origin.file}:${origin.line})`);
  }
  const specFiles = new Set(specs.map((item) => sourceOf(item).file));
  for (const [kind, list] of [["configure", fileConfigs], ["beforeEach", hooks], ["afterEach", afterHooks], ["reset", resetHooks]] as const) {
    for (const entry of list as ReadonlyArray<{ frames: StackFrame[] }>) {
      const owner = resolveOwner(entry.frames, specFiles);
      if (isUnattributed(owner.file)) lost.push(`spec.${kind}() (${owner.file}:${owner.line})`);
    }
  }
  if (lost.length === 0) return;
  const shown = lost.slice(0, 10).join(", ") + (lost.length > 10 ? `, and ${lost.length - 10} more` : "");
  throw new Error(`Cannot tell which file registered ${shown}: the bundle source map has no position for the call. ` +
    "Its file's tags and hooks would not apply, so the run stops. Please report this with the spec file and the lxdev version.");
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

/**
 * During a run, the host's own `lx.automation`: the global is replaced with
 * one that hands spec code `rawAutomation()`'s fenced root, so a spec cannot
 * reach around its grant through the global.
 */
let hostAutomation: { lx: { automation: () => HostRunAutomation }; automation: () => HostRunAutomation } | undefined;

function fenceGlobalAutomation(): void {
  const lx = (globalThis as { lx?: { automation?: () => HostRunAutomation } }).lx;
  if (hostAutomation || !lx || typeof lx.automation !== "function") return;
  const automation = lx.automation;
  hostAutomation = { lx: lx as { automation: () => HostRunAutomation }, automation };
  try {
    lx.automation = () => trackAutomationRoot(automation.call(lx));
  } catch {
    hostAutomation = undefined;
  }
}

function unfenceGlobalAutomation(): void {
  if (!hostAutomation) return;
  try { hostAutomation.lx.automation = hostAutomation.automation; } catch { /* the context is going away */ }
  hostAutomation = undefined;
}

function automationRoot(): HostRunAutomation {
  if (hostAutomation) return hostAutomation.automation.call(hostAutomation.lx);
  // The host injects the root as `lx.automation()` in the test context; that
  // binding is the transport, not a typed global of the test program, so a
  // spec that type-checks app Logic sees one meaning of `lx`: the app's.
  const lx = (globalThis as { lx?: { automation?: () => HostRunAutomation } }).lx;
  if (!lx || typeof lx.automation !== "function") {
    throw new Error("The automation root is not available: rawAutomation() runs only inside an lxdev test run");
  }
  return lx.automation();
}

/**
 * The untraced automation root of this run, with host authority. Specs drive
 * the app through `t.app` / `t.automation`, which trace every call and stop
 * with the spec; use this only where no fixture exists (a setup file that
 * waits for the app before the first spec) or to reach the raw driver, e.g.
 * `rawAutomation().lxapp().eval({ script })`.
 */
export function rawAutomation(): HostRunAutomation {
  // Refuses once the run has revoked spec code's automation access.
  return trackAutomationRoot(automationRoot());
}

function pinApp(appId?: string): LxAppDriver {
  const automation = automationRoot();
  return appId ? automation.lxapp(appId) : automation.lxapp();
}

/** How long a closing app under test gets to finish closing before it is reopened. */
const REOPEN_CLOSING_WAIT_MS = 5_000;

/**
 * Reopen the app under test when it is no longer running, and wait for its
 * home page. Resolves `undefined` when it was running (or cannot be told);
 * otherwise what happened, with the error if reopening failed.
 */
async function reopenAppUnderTest(appId: string | undefined): Promise<{ appId: string; error?: string } | undefined> {
  if (!appId) return undefined;
  let root: HostRunAutomation;
  try { root = automationRoot(); } catch { return undefined; }
  const status = async () => {
    const apps = await root.lxapps.list();
    return apps.find((app) => app.appid === appId)?.status;
  };
  try {
    let current = await status();
    const deadline = Date.now() + REOPEN_CLOSING_WAIT_MS;
    while (current === "closing" && Date.now() < deadline) {
      await new Promise<void>((resolve) => { runnerSetTimeout(resolve, 100); });
      current = await status();
    }
    if (current === "opened" || current === "opening" || current === "restarting") return undefined;
  } catch {
    // A host without `lxapps` cannot say; leave the app alone.
    return undefined;
  }
  try {
    await root.lxapps.open({ appid: appId });
    await relaunchHome(root.lxapp(appId));
    return { appId };
  } catch (error) {
    return { appId, error: String((error as Error)?.message ?? error) };
  }
}

/** How long recovering the app under test may take before the run stops. */
const RECOVERY_BUDGET_MS = 20_000;
const RECOVER_COMMAND = "lxdev lxapp restart";

/**
 * After a spec left work pending: the same recovery a spec start and the run
 * start use — reopen the app if it is closed, then relaunch its home page.
 * Resolves `undefined` when the app is back, otherwise why it is not.
 */
async function recoverAppUnderTest(appId: string | undefined): Promise<string | undefined> {
  const reopened = await reopenAppUnderTest(appId);
  if (reopened?.error) return reopened.error;
  try {
    await within(relaunchHome(pinApp(appId)), RECOVERY_BUDGET_MS,
      `relaunching the home page did not finish within ${RECOVERY_BUDGET_MS}ms`);
    return undefined;
  } catch (error) {
    return String((error as Error)?.message ?? error);
  }
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
 * Controls that carry file contents: the report names the files
 * (`openapiFiles`, `coversManifestFile`) instead of repeating them.
 */
const PAYLOAD_CONTROL_KEYS = ["openapi", "coversManifest"];

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

/** `secretArgs` is a JSON list of `--secret-arg` keys. */
function secretKeys(listed: string | undefined): string[] {
  if (!listed) return [];
  let parsed: unknown;
  try {
    parsed = JSON.parse(listed);
  } catch {
    throw new Error(`the host's secretArgs control is not a JSON list: ${JSON.stringify(listed)}`);
  }
  if (!Array.isArray(parsed)) {
    throw new Error(`the host's secretArgs control is not a JSON list: ${JSON.stringify(listed)}`);
  }
  return parsed.filter((key): key is string => typeof key === "string");
}

/** Run the selected specs. */
function run(): Promise<ProtocolReport> {
  return runSpecs(false);
}

/** `lxdev test --list`: the selected specs, in `listed`; nothing runs. */
function list(): Promise<ProtocolReport> {
  return runSpecs(true);
}

async function runSpecs(listOnly: boolean): Promise<ProtocolReport> {
  warnVersionSkew();
  const rawHost = resolveHost();
  const { args, control } = rawHost;
  // Secret args reach the spec through `t.arg()` but never an event or report.
  const redact = createRedactor(args, secretKeys(control.secretArgs));
  const host: ResolvedHost = {
    ...rawHost,
    args,
    emit: (event) => rawHost.emit(redact.deep(event)),
  };
  const started = Date.now();
  clearInline();
  contractSeq = 0;

  assertAttributed();
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
  const recordNetwork = control.recordNetwork === "1";
  if (recordNetwork && !host.networkRecord) throw new Error("--record-network needs a host that records network traffic");
  const pattern = grep ? new RegExp(grep) : undefined;
  const forbidOnly = control.forbidOnly === "1" || control["forbid-only"] === "1";
  const hasOnly = specs.some((item) => item.annotation === "only");
  if (hasOnly && forbidOnly) {
    throw new Error("spec.only is registered; lxdev test --forbid-only refuses to run");
  }

  const tagFilter = control.tags ? parseTagFilter(JSON.parse(control.tags) as string[]) : [];
  const manifest = parseManifest(control.coversManifest);
  const openapi = parseOpenApiControl(control.openapi);
  setActiveOpenApi(openapi);
  const ledger = openapi ? new ContractLedger(openapi) : undefined;
  const specFiles = new Set(specs.map((item) => sourceOf(item).file));
  const fileOptions = new Map<string, FileOptions>();
  for (const config of fileConfigs) {
    const file = resolveOwner(config.frames, specFiles).file;
    if (!specFiles.has(file)) {
      await host.emit({ type: "diagnostic", phase: "collect",
        message: `spec.configure() called from ${file} applies to no spec: no spec file is on its call stack. Call it from a spec file's top level.` });
      continue;
    }
    fileOptions.set(file, mergeFileOptions(fileOptions.get(file), config.options));
  }
  for (const item of specs) settle(item, fileOptions.get(sourceOf(item).file));
  const tagsOf = (item: RegisteredSpec) => item.tags;

  const retries = Number(control.retries ?? 0);
  if (!Number.isInteger(retries) || retries < 0 || retries > 10) throw new Error("retries must be between 0 and 10");
  const selectedIds: string[] | undefined = control.ids ? JSON.parse(control.ids) : undefined;
  const shard = control.shard?.split("/").map(Number);
  if (shard && (shard.length !== 2 || shard.some(n => !Number.isInteger(n) || n < 1) || shard[0]! > shard[1]!)) throw new Error("shard must be INDEX/TOTAL (1-based)");
  const locations = parseLocations(control.locations);
  const selected = specs.filter((item) => {
    if (hasOnly && item.annotation !== "only") return false;
    if (locations && !atLocation(sourceOf(item), locations)) return false;
    if (selectedIds && !selectedIds.includes(resolvedId(item))) return false;
    if (shard && stableHash(resolvedId(item)) % shard[1]! !== shard[0]! - 1) return false;
    if (control.id && resolvedId(item) !== control.id) return false;
    if (tagFilter.length > 0 && !matchesTags(tagsOf(item), tagFilter)) return false;
    if (!grep) return true;
    const id = resolvedId(item);
    return pattern!.test(item.title) || pattern!.test(id);
  });

  if (listOnly) {
    return listing(selected.map((item) => ({ id: resolvedId(item), title: item.title, ...sourceOf(item), tags: tagsOf(item) })));
  }
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
      suite: suiteOf(sourceOf(item).file), timeout_ms: item.timeout, covers: item.covers, tags: tagsOf(item) })) });
  if (manifest) {
    const known = new Set(manifest.map((entry) => entry.id));
    const stray = [...new Set(specs.flatMap((item) => item.covers).filter((id) => !known.has(id)))];
    if (stray.length > 0) {
      await host.emit({ type: "diagnostic", phase: "coverage",
        message: `${stray.length} covers id(s) not in the coverage manifest: ${stray.slice(0, 10).join(", ")}${stray.length > 10 ? ", …" : ""}` });
    }
  }

  // Resolve each hook's spec file once, so a `beforeEach` stays scoped to the
  // file that registered it rather than running for every spec in the run. A
  // hook registered by a shared helper belongs to the spec file that called
  // the helper (the nearest spec file on its registration stack).
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
  // Set only when the run cannot go on safely: recovering the app after a
  // spec left work pending failed, or a restoreProfile rollback did not run.
  let contaminated = false;
  let contaminationReason: string | undefined;

  installPendingTracker();
  restoreRawAuthority();
  fenceGlobalAutomation();
  // Late assertions are charged to the spec the run last abandoned, never
  // to the one running.
  let lastAbandoned: string | undefined;
  setStrayAssertionSink(strayAssertionSink(host, () => lastAbandoned));
  const queue = [...plan];
  const attempts = new Map<string, CaseRecord[]>();
  const executionKey = (id: string, repeat: number) => `${id}#${repeat}`;
  let budgetExhausted: { after: number } | undefined;
  // Set once, when a spec is about to start on a locked screen: every page is
  // hidden until someone unlocks it, which the run cannot do.
  let screenLocked: { after: number } | undefined;
  const executed = () => new Set(cases.map((done) => executionKey(done.id, done.repeat ?? 1))).size;
  for (const planned of queue) {
    const { item, repeat } = planned;
    const id = resolvedId(item);
    const key = executionKey(id, repeat);
    const timeout = item.timeout;
    const source = sourceOf(item);
    // Checked between specs: a spec already running keeps its own timeout.
    if (!budgetExhausted && budget && Date.now() - started >= budget.ms) {
      budgetExhausted = { after: executed() };
      await host.emit({ type: "diagnostic", phase: "budget",
        message: `Run budget of ${Math.round(budget.ms / 1000)}s exhausted after ${budgetExhausted.after}/${plan.length} specs; the rest are reported as not run. Raise --timeout-secs, or split the suite with --shard.` });
    }
    if (!budgetExhausted && !contaminated && !screenLocked && host.screenLocked()) {
      screenLocked = { after: executed() };
      await host.emit({ type: "diagnostic", phase: "screen_locked",
        message: `${capitalize(SCREEN_LOCKED_NOTE)}. Stopped after ${screenLocked.after}/${plan.length} specs; the rest are reported as not run.` });
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
      tags: tagsOf(item),
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
      tags: record.tags,
    });

    const caseStarted = Date.now();
    const unmet = item.annotation === "skip" || item.annotation === "fixme"
      ? undefined
      : unmetRequirements(item, args, openapi !== undefined);
    if (budgetExhausted || contaminated || screenLocked || unmet !== undefined || item.annotation === "skip" || item.annotation === "fixme") {
      if (contaminated) record.reason = contaminationReason ?? `Not run: an earlier spec left the run unable to continue. Recover: ${RECOVER_COMMAND}`;
      else if (budgetExhausted && budget) record.reason = `Not run: the run budget of ${Math.round(budget.ms / 1000)}s was exhausted after ${budgetExhausted.after}/${plan.length} specs.`;
      else if (screenLocked) record.reason = `Not run: ${SCREEN_LOCKED_NOTE}.`;
      else if (unmet !== undefined) record.reason = unmet;
      record.status = "skipped";
      record.duration_ms = Date.now() - caseStarted;
      cases.push(record);
      await finishCase(host, record);
      continue;
    }

    // One spec that leaves the app under test closed (a crash, a dev reload,
    // Logic torn down after a hung eval) must not fail every spec after it.
    const reopenStarted = Date.now();
    const reopened = await reopenAppUnderTest(item.app ?? subject?.appid);
    if (reopened) {
      const previous = cases[cases.length - 1];
      await host.emit({ type: "diagnostic", phase: reopened.error ? "recovery_failed" : "recovery",
        message: `The app under test (${reopened.appId}) was not running before "${item.title}"` +
          (previous ? `; it stopped during or after "${previous.title}" (${previous.status})` : "") +
          `. Reopened it on its home page${reopened.error ? `, which failed: ${reopened.error}` : ""}.` });
    }

    // What the spec installs from here on — routes, a mock scenario, test
    // clocks — belongs to this attempt; the host removes it when the
    // attempt ends, before the next spec starts.
    const attempt = host.beginAttempt?.();
    const fixture = new LiveFixture(
      `${encodeURIComponent(id)}${repeatEach > 1 ? `/repeat-${repeat}` : ""}/attempt-${record.attempt}`,
      // The app under test by id: "current" follows whichever app a spec
      // brought to the front.
      pinApp(item.app ?? subject?.appid),
      host,
      args,
      automationRoot(),
      timeout,
      redact,
      item.app ?? subject?.appid,
    );

    if (reopened) {
      fixture.steps.push({ name: "app.reopen", kind: "action", path: "app.reopen",
        detail: `${reopened.appId} was not running`, status: reopened.error ? "failed" : "passed",
        duration_ms: Date.now() - reopenStarted, steps: [], attachments: [], assertions: [] });
    }

    // Toasts are recorded and modals / action sheets answered for this spec
    // only; the attempt's end removes the watch whatever happens below.
    let dialogWatch: ReturnType<typeof watchDialogs> = undefined;
    let dialogSetupError: unknown;
    try { dialogWatch = watchDialogs(fixture.raw); }
    catch (error) { dialogSetupError = error ?? new Error("dialog watch failed"); }

    let status: SpecStatus = "passed";
    let error: unknown;
    let phase: "beforeEach" | "body" | "defer" | "forensics" | "timeout" = "body";
    const recordingNetwork = recordNetwork && await startNetworkRecording(host);
    if (ledger) {
      await within(Promise.resolve(fixture.raw.network.captureResponses()), CONTRACT_CALL_MS, "captureResponses timed out");
    }

    const shouldRelaunch = item.fresh || item.restoreProfile || forceRelaunchNext;
    forceRelaunchNext = false;
    // `restoreProfile`: snapshot the isolated profile now and roll back to it
    // after the spec. Registered as the first cleanup, so it runs last.
    let profileRestored = false;
    // Timers and fetches from here on belong to this spec, so one it leaves
    // pending can be named.
    const grant = setPendingOwner(record.full_name);
    const stuckWork = (): PendingWork[] => [
      ...fixture.pendingCalls().map((work) => ({ ...work, owner: record.full_name })),
      ...pendingOf(record.full_name),
    ];
    // Work this spec left pending when the run stopped waiting for it.
    // `spec`: spec code (its body, cleanup or calls it did not await) may
    // still run; the runner's own evidence capture is not spec code.
    let stuck: { what: string; work: PendingWork[]; spec: boolean } | undefined;
    // The run gives up on spec code: from here its raw drivers refuse, and
    // what it may still do is decided below.
    const abandon = (what: string, work: PendingWork[]) => {
      stuck ??= { what, work, spec: true };
      revokeGrant(grant, `"${record.full_name}" was abandoned (${what})`);
    };
    const bodyPromise = (async () => {
      // Each spec starts with fresh mock handler state, before its hooks.
      phase = "beforeEach";
      if (dialogSetupError !== undefined) throw dialogSetupError;
      const mockNote = await resetMocks(fixture.raw, mockResetReasons).catch((error: unknown) =>
        `mock handler state was not reset: ${String((error as Error)?.message ?? error)}`);
      if (mockNote) await host.emit({ type: "diagnostic", phase: "mock", message: mockNote });
      phase = "body";
      if (item.restoreProfile) {
        phase = "beforeEach";
        const checkpoint = await fixture.app.profile.checkpoint();
        fixture.defer(async () => {
          try {
            await fixture.app.profile.restore(checkpoint, item.restoreKeep ? { keep: item.restoreKeep } : undefined);
            profileRestored = true;
          } finally {
            // A failed rollback must not leave its copy on disk for the rest of the run.
            await Promise.resolve(fixture.app.profile.drop(checkpoint)).catch(() => {});
          }
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
    let timerHandle: unknown;
    const timer = new Promise<"timeout">((resolve) => {
      timerHandle = runnerSetTimeout(() => {
        timedOut = true;
        fixture.abort(timeoutError);
        resolve("timeout");
      }, timeout);
    });

    // A modal or action sheet with no answer queued fails the spec at once,
    // instead of leaving it waiting on a dialog nobody answers.
    const unanswered = dialogWatch
      ? dialogWatch.unanswered.then((message) => ({ unanswered: message }))
      : new Promise<never>(() => {});

    try {
      const winner = await Promise.race([bodyResult, timer, unanswered]);
      if (typeof winner === "object" && "unanswered" in winner) {
        const dialogError = new Error(`Unanswered dialog: ${winner.unanswered}`);
        status = "failed";
        error = dialogError;
        fixture.abort(dialogError, "failed");
        try { await within(bodyResult, WEDGED_DEFER_BUDGET_MS, "body did not settle after an unanswered dialog"); }
        catch {
          const work = stuckWork();
          abandon("its body after an unanswered dialog", work);
          dialogError.message += `\nThe body did not settle; still pending: ${describePending(work)}.`;
        }
      } else if (winner === "timeout") {
        status = "timeout";
        error = timeoutError;
        phase = "timeout";
        forceRelaunchNext = true;
        try { await within(bodyResult, WEDGED_DEFER_BUDGET_MS, "body did not settle after timeout"); }
        catch {
          const work = stuckWork();
          abandon("its timed-out body", work);
          timeoutError.message += `\nThe body did not settle; still pending: ${describePending(work)}.`;
        }
      } else if (!winner.ok) {
        // The first error wins; calls the body left running are stopped
        // before cleanup all the same.
        if (fixture.unsettledCalls().length > 0 && !timedOut && !fixture.aborted &&
            !await fixture.stopUnsettled(winner.err instanceof Error ? winner.err : new Error(String(winner.err)), WEDGED_DEFER_BUDGET_MS)) {
          abandon("fixture calls its body did not await", stuckWork());
        }
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
      } else {
        // A call the body did not await would turn a failure into a pass.
        const unsettled = fixture.unsettledCalls();
        if (unsettled.length > 0) {
          status = "failed";
          error = new Error(unawaitedMessage(item.title, unsettled));
          fixture.failurePhase = "body";
          if (!await fixture.stopUnsettled(error as Error, WEDGED_DEFER_BUDGET_MS)) {
            abandon("fixture calls its body did not await", stuckWork());
          }
        }
      }
    } finally {
      if (timerHandle !== undefined) runnerClearTimeout(timerHandle);
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
        if (forensicsError instanceof TimeoutError) {
          stuck ??= { what: "failure evidence capture", work: [{ kind: "action", detail: "screenshot, page and log capture", owner: record.full_name }], spec: false };
        }
        await host.emit({ type: "diagnostic", phase: "forensics", message: String(forensicsError) });
      }
    };
    if (status !== "passed" && status !== "skipped") await collectEvidence();
    // Before the defers remove it: the scenario and what reached it.
    const scenarioEvidence = status !== "passed" && status !== "skipped"
      ? await fixture.scenarioScope.report(SCENARIO_EVIDENCE_MS)
      : undefined;

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
        if (!settled) { abandon("its cleanup (t.defer / afterEach)", stuckWork()); break; }
      }
    } else if (hasCleanup) {
      deferErrors.push(new Error("Cleanup skipped because the timed-out body is still running"));
    }
    fixture.endCleanup();

    // A spec that returned with fetches or raw driver calls still in flight
    // could act on the next spec when they settle: give them a moment, and
    // stop the run if they do not settle.
    if (!stuck) {
      const open = await settleCallsOf(record.full_name, WEDGED_DEFER_BUDGET_MS);
      if (open.length > 0) {
        abandon("calls it did not await", open);
        if (status === "passed" || status === "skipped") {
          status = "failed";
          error = new Error(`${JSON.stringify(item.title)} ended while ${describePending(open)} ` +
            `${open.length === 1 ? "was" : "were"} still running, and ${open.length === 1 ? "it" : "they"} did not settle ` +
            `within ${WEDGED_DEFER_BUDGET_MS}ms (was it awaited?)`);
          fixture.failurePhase = "body";
        }
      }
    }
    // Its timers must not fire into the next spec.
    let cancelled = cancelTimersOf(record.full_name);
    if (cancelled > 0 && !stuck) {
      await host.emit({ type: "diagnostic", phase: "cleanup",
        message: `"${record.full_name}" ended with ${cancelled} test-context ${cancelled === 1 ? "timer" : "timers"} pending; ` +
          `cancelled so ${cancelled === 1 ? "it never fires" : "they never fire"} into a later spec.` });
    }

    // An answer the spec queued that no dialog used: the dialog it expected
    // never came.
    let dialogCleanupError: string | undefined;
    let unusedAnswers: string | undefined;
    if (!stuck && dialogWatch) {
      try {
        unusedAnswers = await within(dialogWatch.end(fixture.raw), WEDGED_DEFER_BUDGET_MS, "dialog unwatch timed out");
      } catch (error) {
        dialogCleanupError = `dialog observation could not finish: ${String(error)}`;
      }
    }
    if (unusedAnswers !== undefined) {
      if (status === "passed") {
        status = "failed";
        error = new Error(`${JSON.stringify(item.title)}: ${unusedAnswers}.`);
        fixture.failurePhase = "body";
      } else if (error instanceof Error) {
        error.message += `\nAlso: ${unusedAnswers}.`;
      }
    }

    // Framework cleanup is not the spec's: it runs whether or not the body
    // settled, and a resource it cannot remove fails the spec and stops the
    // run — a later spec would run against it.
    const reclaimFailures: string[] = dialogCleanupError ? [dialogCleanupError] : [];
    if (recordingNetwork) {
      const recordingError = await saveNetworkRecording(host, fixture, record.title);
      if (recordingError !== undefined) reclaimFailures.push(`the network recording was not stopped: ${recordingError}`);
    }
    try {
      reclaimFailures.push(...await within(fixture.reclaim(), RECLAIM_BUDGET_MS,
        `removing the spec's routes, mock scenario and test clocks did not finish within ${RECLAIM_BUDGET_MS}ms`));
    } catch (reclaimError) {
      reclaimFailures.push(errorMessage(reclaimError));
    }
    // Closing the attempt removes what it installed and refuses installs
    // until the next opens, whether or not its code still runs: its drivers
    // were revoked when it was abandoned.
    if (attempt !== undefined) {
      try {
        const swept = await within(host.endAttempt!(attempt), RECLAIM_BUDGET_MS,
          `the host did not finish removing what the spec installed within ${RECLAIM_BUDGET_MS}ms`);
        if (swept.droppedTimers > 0) forceRelaunchNext = true;
        // Left for the host to sweep: installed where the fixture could not
        // track it (the raw driver), so the spec did not clean it up.
        if (swept.clocks > 0) {
          await host.emit({ type: "diagnostic", phase: "clock",
            message: `"${record.full_name}" left ${swept.clocks} test clock${swept.clocks === 1 ? "" : "s"} installed outside t.app.clock; ` +
              `the spec's end removed ${swept.clocks === 1 ? "it" : "them"}` +
              (swept.droppedTimers > 0 ? ` and dropped ${swept.droppedTimers} pending test timer${swept.droppedTimers === 1 ? "" : "s"}; they never fired.` : ".") });
        }
      } catch (sweepError) {
        reclaimFailures.push(`the host could not remove what the spec installed: ${errorMessage(sweepError)}`);
      }
    }
    if (reclaimFailures.length > 0) {
      deferErrors.push(...reclaimFailures.map((failure) => new Error(failure)));
      if (!contaminated) {
        contaminated = true;
        const what = `"${record.full_name}" left test resources the run could not remove (${reclaimFailures.join("; ")}); ` +
          "a later spec would run against them.";
        contaminationReason = `Not run: ${what} Recover: ${RECOVER_COMMAND}`;
        await host.emit({ type: "diagnostic", phase: "run_stopped",
          message: `${what} The remaining specs are not run. Recover: ${RECOVER_COMMAND}` });
      }
    }
    // Test timers dropped with the spec's clock leave the app's polling loops
    // and debounces dead; the next spec starts from a relaunched home page.
    if (fixture.clockScope.dropped > 0) forceRelaunchNext = true;
    if (item.restoreProfile && !profileRestored && !contaminated) {
      // The next spec would start on this spec's data: a stuck cleanup.
      contaminated = true;
      contaminationReason = "Not run: a previous spec's restoreProfile could not roll the app's data back; restart the run.";
    }

    // Routed responses that break the contract fail the spec like a body
    // failure would, so `spec.fail({ expected: { code: 'E_OPENAPI_CONTRACT' } })`
    // can declare one.
    if (ledger) {
      const checked = await checkContract(ledger, fixture, id, host);
      if (checked) {
        record.contract = checked;
        if (checked.violations.length > 0 && status !== "skipped") {
          const contractError = new ContractError(checked.violations);
          if (status === "failed" || status === "timeout") {
            if (error instanceof Error) error.message += `\n${contractError.message}`;
            else error = contractError;
          } else {
            status = "failed";
            error = contractError;
            fixture.failurePhase = "contract";
          }
        }
      }
    }

    if (item.annotation === "fail") {
      // Without `expected`, the declared failure is whatever the body does to
      // fail: a product rejection counts as much as an assertion. With it,
      // only a matching failure is the known one; anything else — a mistyped
      // selector, a dead fixture server — is a real failure. Setup failures,
      // skips and timeouts keep their own verdicts.
      if (status === "failed" && (fixture.failurePhase === "body" || fixture.failurePhase === "contract")) {
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
    // Copies: code of this spec that is still running cannot change a
    // finished record.
    record.steps = [...fixture.steps];
    record.assertions = [...fixture.assertions];
    if (fixture.observed.size > 0) record.observed = [...fixture.observed].sort();
    record.attachments = [...fixture.attachments];
    setExpectScope();
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
      // A spec that never reached a page is not "on" the app's current one.
      const page = (fixture.usedPage || item.fresh ? failurePage : undefined) ?? pageFromErrorData(record.error.data);
      if (page) record.error.page = page;
      const calls = withFunctionCalls(networkCalls(host, caseStarted), scenarioEvidence?.functionCalls ?? []);
      if (calls.length > 0) record.error.network = calls;
      if (scenarioEvidence) record.error.scenario = scenarioEvidence.scenario;
    }
    const history = attempts.get(key) ?? [];
    history.push(record);
    attempts.set(key, history);
    const finished = { ...record, attempts: history.length > 1 ? history.map(attempt => ({ ...attempt })) : undefined,
      flaky: record.status === "passed" && history.length > 1 };
    const previous = cases.findIndex(item => item.id === id && (item.repeat ?? 1) === repeat);
    if (previous >= 0) cases[previous] = finished; else cases.push(finished);
    await finishCase(host, finished);
    if (stuck) {
      // Shared-context user continuations cannot be proven dead. Reclaim and
      // recover for inspection, then stop; only runner-owned evidence work can
      // time out without abandoning user code.
      if (stuck.spec) lastAbandoned = record.full_name;
      const inFlight = stuck.spec ? await abandonedWorkInFlight(record.full_name, fixture) : [];
      cancelled += cancelTimersOf(record.full_name);
      const failure = await recoverAppUnderTest(item.app ?? subject?.appid);
      cancelled += cancelTimersOf(record.full_name);
      const unfenced = stuck.spec && grant ? [...grant.unfenced] : [];
      const summary = `"${record.full_name}" left ${stuck.what} pending: ${describePending(stuck.work)}` +
        (cancelled > 0 ? `; cancelled its ${cancelled} pending ${cancelled === 1 ? "timer" : "timers"}` : "");
      const unsafe = [
        ...(stuck.spec ? ["abandoned code can resume through an untracked promise in this shared JS context"] : []),
        ...(inFlight.length > 0
          ? [`${describePending(inFlight)} ${inFlight.length === 1 ? "is" : "are"} still running after ${WEDGED_DEFER_BUDGET_MS}ms and would resume its code during a later spec`]
          : []),
        ...(unfenced.length > 0
          ? [`it holds host drivers the run cannot revoke (${unfenced.map((name) => `rawAutomation().${name}`).join(", ")})`]
          : []),
      ];
      if (stuck.spec) {
        // Isolation cannot be guaranteed: stop, and revoke all spec code.
        const revokeError = await revokeSpecAuthority(host, undefined, `${summary}; the run stopped`);
        const recovered = failure === undefined
          ? "Relaunched the app under test on its home page for inspection"
          : `Recovering the app failed: ${failure}`;
        const reason = `${summary}. The run stopped: ${unsafe.join("; ")}. ${recovered}` +
          (revokeError ? `; revoking its automation access failed: ${revokeError}` : "") +
          ". Make the spec settle (await its work), or give it a longer timeout.";
        const recover = failure !== undefined || revokeError ? ` Recover: ${RECOVER_COMMAND}` : "";
        if (!contaminated) {
          contaminated = true;
          contaminationReason = `Not run: ${reason}${recover}`;
        }
        forceRelaunchNext = false;
        await host.emit({ type: "diagnostic", phase: failure === undefined ? "run_stopped" : "recovery_failed",
          message: `${reason} The remaining specs are not run.${recover}` });
      } else if (failure === undefined) {
        forceRelaunchNext = false;
        await host.emit({ type: "diagnostic", phase: "recovery",
          message: `${summary}. Relaunched the app under test on its home page; the run continues.` });
      } else if (!contaminated) {
        contaminated = true;
        contaminationReason = `Not run: ${summary}, and recovering the app failed: ${failure}. Recover: ${RECOVER_COMMAND}`;
        if (stuck.spec) await revokeSpecAuthority(host, undefined, `${summary}; the run stopped`);
        await host.emit({ type: "diagnostic", phase: "recovery_failed",
          message: `${summary}, and recovering the app failed: ${failure}. The remaining specs are not run. Recover: ${RECOVER_COMMAND}` });
      }
    }
    setPendingOwner(undefined);
    if (!contaminated && (status === "failed" || status === "timeout") && history.length <= retries) {
      queue.splice(queue.indexOf(planned) + 1, 0, { item: { ...item }, repeat });
    }
  }

  uninstallPendingTracker();
  unfenceGlobalAutomation();
  setStrayAssertionSink();

  // The run ends as each spec starts: with the app under test running, so
  // what comes next (a rerun, `lxdev lxapp`, a developer) does not meet a
  // closed app. Not after pending async work, which reopening could race.
  if (!contaminated && cases.length > 0) {
    const reopened = await reopenAppUnderTest(subject?.appid);
    if (reopened) {
      await host.emit({ type: "diagnostic", phase: reopened.error ? "recovery_failed" : "recovery",
        message: `The app under test (${reopened.appId}) was not running at the end of the run` +
          (reopened.error ? `, and reopening it failed: ${reopened.error}` : "; reopened it on its home page.") });
    }
  }

  const counts = countStatuses(cases);
  const duration_ms = Date.now() - started;
  const reportedControl: Record<string, string> = { ...control };
  for (const key of PAYLOAD_CONTROL_KEYS) delete reportedControl[key];
  const json: JsonReport = {
    schema_version: 1,
    framework: { name: PACKAGE_NAME, version: VERSION },
    meta: {
      started_at: new Date(started).toISOString(),
      duration_ms,
      args: redact.args(args),
      run: redact.deep(reportedControl),
      platform: control.platform,
      framework: args.framework,
      subject,
      surface_coverage: trackSurface,
      ...(budget ? { budget: { ms: budget.ms, auto: budget.auto, planned: plan.length,
        ...(budgetExhausted ? { exhausted_after: budgetExhausted.after } : {}) } } : {}),
      ...(shuffleSeed !== undefined ? { shuffle_seed: shuffleSeed } : {}),
      ...(repeatEach > 1 ? { repeat_each: repeatEach } : {}),
    },
    partial: contaminated || budgetExhausted !== undefined || screenLocked !== undefined,
    filtered: Boolean(grep || control.id || control.ids || shard || locations || tagFilter.length > 0) || hasOnly,
    duration_ms,
    ...counts,
    cases: redact.deep(cases),
  };
  json.failures = failureRecords(json.cases);
  const tagRows = tagSummary(json.cases);
  if (tagRows.length > 0) json.tag_summary = tagRows;
  if (manifest) {
    json.coverage = coverageSummary(manifest, json.cases,
      specs.map((item) => ({ id: resolvedId(item), title: item.title, covers: item.covers })));
  }
  if (ledger) json.openapi = redact.deep(ledger.summary);

  await attachText(host, "report.json", JSON.stringify(json, null, 2), "application/json");
  await attachText(host, "report.html", renderHtml(json), "text/html; charset=utf-8");
  await attachText(host, "junit.xml", renderJUnit(json), "application/xml; charset=utf-8");

  return json;
}

/** Calls a body returned without awaiting; more than this many are counted. */
const LISTED_UNAWAITED = 5;

function unawaitedMessage(title: string, calls: Array<{ call: string; at: string; ageMs: number }>): string {
  const count = calls.length === 1 ? "1 fixture call was" : `${calls.length} fixture calls were`;
  const lines = calls.slice(0, LISTED_UNAWAITED).map((entry) => `  ${entry.call}  at ${entry.at} (started ${entry.ageMs}ms ago)`);
  if (calls.length > LISTED_UNAWAITED) lines.push(`  … and ${calls.length - LISTED_UNAWAITED} more`);
  return [`${JSON.stringify(title)} returned while ${count} still running (was it awaited?):`, ...lines].join("\n");
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

/** How long a run start waits for the host to name its current lxapp. */
const SUBJECT_WAIT_MS = 15_000;

/**
 * The app under test: the host's current lxapp. A host whose runtime just
 * (re)connected may not have shown its app yet, so this waits for it, and
 * opens an app the host names but has not opened. Best-effort: a run
 * against an unreachable app still reports its cases.
 */
async function describeSubject(): Promise<RunSubject | undefined> {
  const deadline = Date.now() + SUBJECT_WAIT_MS;
  let opened: string | undefined;
  for (;;) {
    try {
      const info = (await pinApp().info()) as unknown as Record<string, unknown>;
      return {
        appid: asText(info.appid),
        app_name: asText(info.app_name),
        version: asText(info.version),
        release_type: asText(info.release_type),
        pages: typeof info.pages_count === "number" ? info.pages_count : undefined,
      };
    } catch (error) {
      const message = errorMessage(error);
      const inactive = /lxapp is not active: (\S+)/.exec(message)?.[1];
      if (inactive !== undefined && opened !== inactive) {
        opened = inactive;
        await reopenAppUnderTest(inactive);
        continue;
      }
      if (!(inactive !== undefined || /no current lxapp/.test(message)) || Date.now() >= deadline) return undefined;
      await new Promise<void>((resolve) => { runnerSetTimeout(resolve, 200); });
    }
  }
}

function capitalize(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
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
        ...(error?.network && error.network.length > 0 ? { network: error.network } : {}),
        ...(error?.scenario ? { scenario: error.scenario } : {}),
      };
    });
}

/** The per-spec attachment `lxdev test --record-network` collects. */
const RECORDED_SCENARIO = "network.scenario.json";

/** Logic network calls a failed spec reports: the last 20 since it started. */
const REPORTED_NETWORK_CALLS = 20;

function networkCalls(host: ResolvedHost, since: number): FailureNetworkCall[] {
  if (!host.networkLog) return [];
  try {
    const calls = host.networkLog(since, REPORTED_NETWORK_CALLS);
    return Array.isArray(calls) ? (calls as FailureNetworkCall[]).slice(-REPORTED_NETWORK_CALLS) : [];
  } catch {
    // The call log is evidence; it never replaces the failure.
    return [];
  }
}

/** How long a failed spec waits for its scenario's calls. */
const SCENARIO_EVIDENCE_MS = 2_000;

/** The Logic calls and the scenario's Function calls, newest 20 by time. */
function withFunctionCalls(calls: FailureNetworkCall[], functions: ScenarioCall[]): FailureNetworkCall[] {
  if (functions.length === 0) return calls;
  const converted: FailureNetworkCall[] = functions.map((call) => ({
    time: call.time,
    kind: "function",
    method: "",
    url: "",
    function: call.function,
    outcome: call.outcome,
    status: null,
    durationMs: null,
    source: call.rule === null ? "network" : "route",
    answeredBy: call.answeredBy,
    ...(call.noMatch ? { noMatch: call.noMatch } : {}),
  }));
  return [...calls, ...converted].sort((a, b) => a.time - b.time).slice(-REPORTED_NETWORK_CALLS);
}

/** `--record-network`: capture this spec's real Logic traffic. */
async function startNetworkRecording(host: ResolvedHost): Promise<boolean> {
  try {
    host.networkRecord!("start");
    return true;
  } catch (error) {
    await host.emit({ type: "diagnostic", phase: "record-network", message: String((error as Error)?.message ?? error) });
    return false;
  }
}

/** Stop this spec's recording and attach it; resolves why it failed, if it did. */
async function saveNetworkRecording(host: ResolvedHost, fixture: LiveFixture, title: string): Promise<string | undefined> {
  let scenario: unknown;
  try {
    scenario = host.networkRecord?.("stop", title);
  } catch (error) {
    return errorMessage(error);
  }
  try {
    if (scenario && typeof scenario === "object") await fixture.attachRaw(RECORDED_SCENARIO, scenario);
  } catch (error) {
    // The recording stopped; only its copy in the report is missing.
    await host.emit({ type: "diagnostic", phase: "record-network", message: errorMessage(error) });
  }
  return undefined;
}

/** How long removing one spec's routes, scenario and clocks may take. */
const RECLAIM_BUDGET_MS = 10_000;

function errorMessage(error: unknown): string {
  return String((error as Error)?.message ?? error);
}

/**
 * Stop spec code of this run from driving the app: `rawAutomation()` and
 * every proxy it handed out refuse from now on, and the host refuses the
 * context's driver calls and removes what the open attempt installed.
 * Resolves why the host could not, if it could not.
 */
async function revokeSpecAuthority(host: ResolvedHost, attempt: number | undefined, reason: string): Promise<string | undefined> {
  revokeRawAuthority(reason);
  try {
    if (host.revoke) await within(host.revoke(reason), RECLAIM_BUDGET_MS, `the host did not finish revoking within ${RECLAIM_BUDGET_MS}ms`);
    else if (attempt !== undefined && host.endAttempt) await within(host.endAttempt(attempt), RECLAIM_BUDGET_MS, `the host did not finish removing what the spec installed within ${RECLAIM_BUDGET_MS}ms`);
    return undefined;
  } catch (error) {
    return errorMessage(error);
  }
}

/**
 * Work an abandoned spec dispatched that is still running after a grace
 * period: fetches and raw driver calls (and what their continuations start
 * meanwhile), and fixture calls the abort could not stop. Empty when all
 * settled.
 */
async function abandonedWorkInFlight(name: string, fixture: LiveFixture): Promise<PendingWork[]> {
  const deadline = Date.now() + WEDGED_DEFER_BUDGET_MS;
  const open = await settleCallsOf(name, WEDGED_DEFER_BUDGET_MS);
  let calls = fixture.pendingCalls();
  while (calls.length > 0 && Date.now() < deadline) {
    await new Promise<void>((resolve) => { runnerSetTimeout(resolve, 50); });
    calls = fixture.pendingCalls();
  }
  return [...open, ...calls.map((work) => ({ ...work, owner: name }))];
}

/** Assertions noted while no spec ran, beyond which they are only counted. */
const LISTED_STRAY_ASSERTIONS = 5;

/** A diagnostic for each assertion that ran while no spec was running. */
function strayAssertionSink(host: ResolvedHost, abandoned: () => string | undefined): (entry: LoggedAssertion) => void {
  let seen = 0;
  return (entry) => {
    seen += 1;
    if (seen > LISTED_STRAY_ASSERTIONS) return;
    const from = abandoned();
    const message = `An assertion ran while no spec was running, so no case records it: ` +
      `expect(received).${entry.matcher} ${entry.passed ? "passed" : "failed"} (expected ${entry.expected}, received ${entry.actual}). ` +
      (from !== undefined
        ? `It comes from ${JSON.stringify(from)}, which the run abandoned; its verdict stands.`
        : "Code of a spec the run stopped waiting for is still running.") +
      (seen === LISTED_STRAY_ASSERTIONS ? " Further ones are not listed." : "");
    void host.emit({ type: "diagnostic", phase: "late_assertion", message, ...(from !== undefined ? { case: from } : {}) }).catch(() => {});
  };
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
  let visibility: PageVisibility | undefined;
  try {
    visibility = pageVisibility(await fixture.raw.page.eval({
      script: VISIBILITY_PROBE_SCRIPT, timeoutMs: VISIBILITY_PROBE_BUDGET_MS }));
  } catch {
    visibility = undefined;
  }
  const forensics = {
    route,
    info,
    step: fixture.currentStepPath() ?? null,
    visibility: visibility ?? null,
  };
  await fixture.attachRaw("forensics.json", forensics);

  const logs = await fixtureHostLogs();
  if (logs !== undefined) {
    await fixture.attachRaw("logs.txt", logs);
  }
  const page = toFailurePage(route);
  const hidden = hiddenCause(visibility?.note);
  return page && hidden ? { ...page, hidden } : page;
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
  fileConfigs.length = 0;
  setActiveOpenApi(undefined);
  hooks.length = 0;
  afterHooks.length = 0;
  resetHooks.length = 0;
  trackSurface = false;
  forceRelaunchNext = false;
  uninstallPendingTracker();
  unfenceGlobalAutomation();
  restoreRawAuthority();
  clearInline();
  setExpectScope();
  setStrayAssertionSink();
}

const controller: LingxiaTestController = {
  run,
  list,
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

export { spec, expect, run, list, reset, resolvedId, trackPublicSurface };
export type { SpecOptions, SpecBody, Fixture };

/** Bound on the capture driver calls a contract check makes. */
const CONTRACT_CALL_MS = 5_000;
/** Newest captured response already checked, per run. */
let contractSeq = 0;

/**
 * Check the responses captured since the last spec: routed mismatches come
 * back as violations, the real server's as warnings (also a diagnostic).
 */
async function checkContract(
  ledger: ContractLedger,
  fixture: LiveFixture,
  id: string,
  host: ResolvedHost,
): Promise<NonNullable<CaseRecord["contract"]> | undefined> {
  let records;
  try {
    records = await within(Promise.resolve(fixture.raw.network.responses({ since: contractSeq })), CONTRACT_CALL_MS,
      "reading captured responses timed out");
  } catch (readError) {
    await host.emit({ type: "diagnostic", phase: "contract",
      message: `${id}: captured responses could not be read: ${readError instanceof Error ? readError.message : String(readError)}` });
    return undefined;
  }
  for (const record of records) contractSeq = Math.max(contractSeq, record.seq);
  if (records.length === 0) return undefined;
  const result = ledger.checkSpec(id, records);
  for (const warning of result.warnings.slice(0, 3)) {
    const first = warning.issues[0];
    await host.emit({ type: "diagnostic", phase: "contract",
      message: `${id}: ${warning.operation} → ${warning.status} from the server does not match ${warning.schema}` +
        (first ? ` (at ${first.path || "/"}: ${first.message})` : "") });
  }
  return result;
}

async function within<T>(task: Promise<T>, ms: number, message: string): Promise<T> {
  let handle: unknown;
  try {
    return await Promise.race([task, new Promise<never>((_, reject) => {
      handle = runnerSetTimeout(() => reject(new TimeoutError(message)), ms);
    })]);
  } finally { if (handle !== undefined) runnerClearTimeout(handle); }
}

/**
 * `control.locations`, sent for `lxdev test FILE:LINE`: every entry file,
 * mapped to the line ranges of the spec calls it selects, or `null` for the
 * whole file. A spec registered from any other file is not selected.
 */
type Locations = Map<string, Array<[number, number]> | null>;

function parseLocations(raw: string | undefined): Locations | undefined {
  if (raw === undefined) return undefined;
  const parsed: unknown = JSON.parse(raw);
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new Error(`the host's locations control is not an object: ${JSON.stringify(raw)}`);
  }
  const out: Locations = new Map();
  for (const [file, ranges] of Object.entries(parsed as Record<string, unknown>)) {
    if (ranges !== null && !(Array.isArray(ranges) && ranges.every((r) =>
      Array.isArray(r) && r.length === 2 && r.every((n) => Number.isInteger(n))))) {
      throw new Error(`the host's locations control has a bad range for ${file}`);
    }
    out.set(normalizeFile(file), ranges as Array<[number, number]> | null);
  }
  return out;
}

function normalizeFile(file: string): string {
  return file.replace(/\\/g, "/");
}

function atLocation(source: { file: string; line: number }, locations: Locations): boolean {
  const file = normalizeFile(source.file);
  if (!locations.has(file)) return false;
  const ranges = locations.get(file);
  return ranges === null || ranges!.some(([from, to]) => source.line >= from && source.line <= to);
}

/** `lxdev test --list`: the selection, with nothing run. */
function listing(listed: ListedSpec[]): ProtocolReport {
  return {
    schema_version: 1,
    framework: { name: PACKAGE_NAME, version: VERSION },
    meta: { started_at: new Date().toISOString(), duration_ms: 0, args: {} },
    partial: false,
    filtered: true,
    total: 0, passed: 0, failed: 0, skipped: 0, xfail: 0, xpass: 0, timeout: 0,
    duration_ms: 0,
    cases: [],
    listed,
  };
}

function stableHash(value: string): number {
  let hash = 2166136261;
  for (let i = 0; i < value.length; i++) hash = Math.imul(hash ^ value.charCodeAt(i), 16777619);
  return hash >>> 0;
}
