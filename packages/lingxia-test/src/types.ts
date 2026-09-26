/// <reference types="@lingxia/types/testing" preserve="true" />
import type {
  Automation,
  AutomationErrorCode,
  ClockDriver,
  ProfileRestoreResult,
  LogicLxAppEvalOptions,
  LxAppDriver,
  NetworkDriver,
  PageDriver,
  PageEvalOptions,
  PageQueryResult,
  PageTarget,
  Scenario,
  ScenarioInput,
} from "@lingxia/types/automation";

export type SpecStatus =
  | "passed"
  | "failed"
  | "skipped"
  | "timeout"
  | "xfail"
  | "xpass";

export type StepStatus = "passed" | "failed" | "timeout" | "skipped";

export interface SpecOptions {
  /** Stable id. ASCII titles slug by default; non-ASCII titles need this or become `file-n`. */
  id?: string;
  /** Declared coverage tags. Journeys omit this. */
  covers?: readonly string[];
  /**
   * Selection tags (`unit`, `routed`, `live`, `smoke`, …), merged after the
   * file's `spec.configure({ tags })`. `lxdev test --tag` selects by them and
   * the report summarizes each. Letters, digits and `_ . : / -`.
   */
  tags?: readonly string[];
  /** Spec budget in ms (default 30_000). */
  timeout?: number;
  /** Relaunch the home page before the body. */
  fresh?: boolean;
  /**
   * Snapshot the app's isolated data before this spec and roll it back after,
   * so the next spec never sees this one's writes. Implies `fresh`. Needs an
   * isolated run (`lxdev test --profile`); if the rollback fails, the rest of
   * the run is not run. `{ keep: ['auth.*'] }` rolls back everything except
   * the `lx.getStorage()` keys those globs match, which keep their state at
   * the end of the spec (see `ProfileRestoreOptions`).
   */
  restoreProfile?: boolean | RestoreProfileOptions;
  /** Independent cleanup budget; a pending cleanup stops subsequent specs. */
  timeoutCleanup?: number;
  /** Pin `t.app` to this lxapp id instead of the current one. */
  app?: string;
  /** Skip auto-attached failure forensics (only when capture itself would wedge). */
  forensics?: boolean;
  /** Why a skip/fixme spec is registered. Shown in the HTML/JSON report; `t.skip(reason)` overrides it. */
  reason?: string;
}

/** `restoreProfile: { keep }`. */
export interface RestoreProfileOptions {
  /** `lx.getStorage()` key globs that survive the rollback (`*`, `?`). */
  keep: string[];
}

/** `spec.configure()` options: defaults for every spec in the calling file. */
export interface FileOptions {
  /** Tags every spec of this file carries, before its own `tags`. */
  tags?: readonly string[];
}

/** `spec.fail` options. */
export interface FailOptions extends SpecOptions {
  /**
   * The failure this spec is known to produce. When set, only a body failure
   * matching it grades `xfail`; any other failure grades `failed`. Omit it to
   * accept any body failure.
   */
  expected?: RejectExpected;
}

export type SpecBody = (t: Fixture) => void | Promise<void>;

export interface ExpectOptions {
  timeout?: number;
  interval?: number;
}

/** `click` / `fill` options. */
export interface ActionOptions extends ExpectOptions {
  /**
   * Skip the in-viewport, stability and hit-test waits and dispatch to the
   * element itself; it must still be attached (one match) and enabled. Use it
   * for an element no scroll can bring under the pointer, such as the lower
   * part of an overflowing sheet. The action then proves less about what a
   * user can reach, so prefer the default wherever it works.
   */
  force?: boolean;
}

/** `locator.filter()` options. */
export interface LocatorFilterOptions {
  /**
   * Keep matches whose text contains this string (case-insensitive,
   * whitespace-normalized) or matches this RegExp.
   */
  hasText: string | RegExp;
}

export interface RejectExpected {
  /**
   * The rejection's `code`: an automation driver code (`E_PAGE_NOT_ACTIVE`,
   * `E_ELEMENT_NOT_FOUND`, …; see `AUTOMATION_ERROR_CODES`) or any other
   * string code the operation rejects with.
   */
  code?: AutomationErrorCode | (string & {});
  message?: string | RegExp;
}

export interface LocatorOptions extends PageTarget {
  /** Zero-based match index; omit to require a unique match. */
  index?: number;
}

/**
 * Locator states are strict about ambiguity (narrow with `.nth()`,
 * `.first()`, `.last()` or `.filter()`):
 * - `attached`: exactly one match in the DOM, rendered or not.
 * - `visible`: exactly one match, and it is rendered — a non-empty box, not
 *   `display:none`, `visibility:hidden` or `opacity:0` — whether or not it is
 *   scrolled into the viewport (content below the fold or in an overflowing
 *   sheet is visible).
 * - `inViewport`: exactly one visible match that intersects the viewport.
 * - `hidden`: no visible match, including no match at all.
 * - `detached`: no match.
 * Several matches satisfy only `hidden` (when none is visible). The raw
 * `page.waitFor` driver checks the first match instead; see `PageWaitState`.
 */
export type LocatorState = "attached" | "detached" | "visible" | "hidden" | "inViewport";

export interface LocatorWaitOptions extends ExpectOptions {
  /** Defaults to `visible`. */
  state?: LocatorState;
}

export interface Locator {
  readonly selector: string;
  /** Wait until the locator reaches `state`; rejects at the timeout. */
  waitFor(options?: LocatorWaitOptions): Promise<void>;
  click(options?: ActionOptions): Promise<void>;
  fill(text: string, options?: ActionOptions): Promise<void>;
  type(text: string, options?: ExpectOptions): Promise<void>;
  press(key: string, options?: ExpectOptions): Promise<void>;
  /** Pick the match at `index` (of the filtered matches, after `.filter()`). */
  nth(index: number): Locator;
  first(): Locator;
  last(): Locator;
  /** Narrow the matches; `nth`/`first`/`last` then pick among what is left. */
  filter(options: LocatorFilterOptions): Locator;
  /** Read once; use t.expect for retrying assertions. */
  query(): Promise<PageQueryResult>;
}

/** A value that crosses the eval boundary unchanged. */
export type JsonValue =
  | string
  | number
  | boolean
  | null
  | JsonValue[]
  | { [key: string]: JsonValue };

/** A page instance as app Logic sees it (`getCurrentPages()` entries). */
export interface LogicPage<TData = Record<string, unknown>> {
  readonly route: string;
  readonly data: TData;
  setData(patch: Record<string, unknown>): void;
  /** Resolves once pending `setData` writes reached the View. */
  flush(): Promise<void>;
  /** Page methods declared in `Page({...})`. */
  readonly [member: string]: unknown;
}

/** The app instance as app Logic sees it (`getApp()`). */
export interface LogicApp {
  readonly [member: string]: unknown;
}

/**
 * What a function passed to `t.app.eval(fn)` receives. It runs inside the
 * app's Logic runtime, so these are the app's own `lx`, `getApp` and
 * `getCurrentPages` — not globals of the test context.
 */
export interface LogicScope {
  readonly lx: Lx & { automation(): Automation };
  getApp<T extends LogicApp = LogicApp>(): T | null;
  getCurrentPages<T extends LogicPage<any> = LogicPage>(): T[];
}

type PageGlobal<K extends string> = typeof globalThis extends { [P in K]: infer V } ? V : unknown;

/**
 * What a function passed to `t.app.page.eval(fn)` receives: the page
 * WebView's `document` and `window`. The recommended test tsconfig has no DOM
 * library, so both are `unknown` there; cast to the shape you read.
 */
export interface PageScope {
  readonly document: PageGlobal<"document">;
  readonly window: PageGlobal<"window">;
}

/**
 * A function evaluated in the app's Logic runtime. It is sent as source text
 * (`fn.toString()`), so it must be self-contained: no variables, imports or
 * helpers from the spec. Pass values as JSON arguments instead.
 */
export type LogicFunction<R, A extends JsonValue[]> = (scope: LogicScope, ...args: A) => R | Promise<R>;

/** A function evaluated in the page WebView; self-contained like `LogicFunction`. */
export type PageFunction<R, A extends JsonValue[]> = (scope: PageScope, ...args: A) => R | Promise<R>;

export interface TestPage extends Omit<PageDriver, "eval"> {
  testId(id: string, options?: LocatorOptions): Locator;
  css(selector: string, options?: LocatorOptions): Locator;
  /** Evaluate a script string in the page WebView. `T` is not validated. */
  eval<T = unknown>(options: PageEvalOptions): Promise<T>;
  /**
   * Run `fn` in the current page's WebView with JSON `args` and resolve to
   * its JSON result. `fn` must be self-contained (see `PageFunction`).
   */
  eval<R, A extends JsonValue[]>(fn: PageFunction<R, A>, ...args: A): Promise<Awaited<R>>;
}

export interface PageDataOptions {
  /** Configured page name or route; defaults to the current page. */
  page?: string;
}

export interface TestApp extends Omit<LxAppDriver, "eval" | "page"> {
  readonly page: TestPage;
  /**
   * Spec-scoped: routes are removed when the spec ends, and `requests()`
   * lists only requests this spec's routes handled (the raw driver's log
   * spans the whole run).
   */
  readonly network: NetworkDriver;
  /**
   * Put the app into a product state from a scenario file (and one of its
   * variants): `http` rules answer Logic `fetch`, `function` rules go to the
   * dev session's companion. Spec-scoped: a second call replaces the first,
   * and the spec's end removes it. A failed spec reports its per-rule hits
   * and the calls that reached it.
   */
  scenario(definition: ScenarioInput, variant?: string): Promise<Scenario>;
  /**
   * Test clock for the app's Logic (`Date`, timers, `performance.now`).
   * Spec-scoped: a clock still installed when the spec ends is uninstalled,
   * and if that drops pending test timers the next spec starts from a
   * relaunched home page.
   */
  readonly clock: ClockDriver;
  /** Evaluate a script string in app Logic and resolve to its value. `T` is not validated. */
  eval<T = unknown>(options: LogicLxAppEvalOptions): Promise<T>;
  /**
   * Run `fn` in the app's Logic runtime with JSON `args` and resolve to its
   * JSON result. `fn` must be self-contained (see `LogicFunction`).
   */
  eval<R, A extends JsonValue[]>(fn: LogicFunction<R, A>, ...args: A): Promise<Awaited<R>>;
  /** Read the current (or named) page's Logic `data`. `T` is not validated. */
  pageData<T = Record<string, unknown>>(options?: PageDataOptions): Promise<T>;
  /** Call a method of the current page's Logic instance and resolve to its JSON result. */
  callPage<R = unknown>(method: string, ...args: JsonValue[]): Promise<Awaited<R>>;
}

/**
 * `t.profile`: checkpoint and roll back the app's isolated data by hand.
 * Needs an isolated run (`lxdev test --profile`); otherwise every call
 * rejects with `E_PROFILE_NOT_ISOLATED`. `checkpoint` and `restore` close the
 * app and reopen it at its initial page; `t.app` follows the reopened app, a
 * `t.app` saved before the call does not.
 */
export interface ProfileFixture {
  /** Snapshot the app's data; resolves an id for `restore`. */
  checkpoint(): Promise<string>;
  /**
   * Roll the app's data back to checkpoint `id`. With `keep`, the
   * `lx.getStorage()` keys those globs match keep their current state.
   */
  restore(id: string, options?: ProfileRestoreOptions): Promise<ProfileRestoreResult>;
  /** Discard checkpoint `id`. */
  drop(id: string): Promise<void>;
}

/** `t.profile.restore` options. */
export interface ProfileRestoreOptions {
  /**
   * `lx.getStorage()` keys whose current state survives the rollback: globs
   * over the whole key (`*` any run of characters, `?` one). A matching key
   * keeps its current value, one added since the checkpoint stays, and one
   * deleted since stays deleted. Files always roll back.
   */
  keep?: readonly string[];
}

export interface TestAutomation extends Omit<Automation, "lxapp"> {
  lxapp(): TestApp;
  lxapp(appId: string): TestApp;
}

export interface Apps {
  lxapp(appId: string): TestApp;
}

export interface RetryMatchers<T> {
  readonly not: RetryMatchers<T>;
  toBe(expected: unknown): Promise<void>;
  toEqual(expected: unknown): Promise<void>;
  toContain(expected: unknown): Promise<void>;
  toMatch(expected: string | RegExp): Promise<void>;
  toBeTruthy(): Promise<void>;
  toBeFalsy(): Promise<void>;
  toBeDefined(): Promise<void>;
  toBeUndefined(): Promise<void>;
  toBeInstanceOf(expected: Function): Promise<void>;
  toBeGreaterThan(expected: number): Promise<void>;
  toBeGreaterThanOrEqual(expected: number): Promise<void>;
  toBeLessThan(expected: number): Promise<void>;
  toBeLessThanOrEqual(expected: number): Promise<void>;
}

export interface LocatorMatchers {
  readonly not: LocatorMatchers;
  /** One rendered match, in the viewport or scrolled out of it. */
  toBeVisible(options?: ExpectOptions): Promise<void>;
  /** One rendered match that intersects the viewport. */
  toBeInViewport(options?: ExpectOptions): Promise<void>;
  toBeHidden(options?: ExpectOptions): Promise<void>;
  toBeAttached(options?: ExpectOptions): Promise<void>;
  toBeEnabled(options?: ExpectOptions): Promise<void>;
  toBeDisabled(options?: ExpectOptions): Promise<void>;
  toBeEditable(options?: ExpectOptions): Promise<void>;
  toHaveText(expected: string | RegExp, options?: ExpectOptions): Promise<void>;
  /** The text contains `expected` (exact substring) or matches the RegExp. */
  toContainText(expected: string | RegExp, options?: ExpectOptions): Promise<void>;
  /**
   * The single match has attribute `name`; with `value`, equal to it (or
   * matching the RegExp). `not.toHaveAttribute(name)` passes when it is absent.
   */
  toHaveAttribute(name: string, value?: string | RegExp, options?: ExpectOptions): Promise<void>;
  toHaveCount(expected: number, options?: ExpectOptions): Promise<void>;
  toHaveValue(expected: string | RegExp, options?: ExpectOptions): Promise<void>;
}

export interface FixtureExpect {
  (locator: Locator): LocatorMatchers;
  poll<T>(read: () => T | Promise<T>, options?: ExpectOptions): RetryMatchers<T>;
}

export interface WaitForOptions {
  /** Default: the action timeout (5000 ms), clamped to the spec's remaining budget. */
  timeout?: number;
  interval?: number;
  /**
   * Whether an error thrown by `read` means "not yet". Default: retry any
   * error except `TypeError`, `ReferenceError` and `SyntaxError`, which are
   * programming mistakes and fail at once.
   */
  retryIf?: (error: unknown) => boolean;
}

export interface ArgOptions {
  /** Throw when the arg is missing. Default: true unless `default` is given. */
  required?: boolean;
  /** Value used when the arg is missing. */
  default?: string;
}

export interface Fixture {
  /** Guarded host drivers; use these in tests so actions are traced and stop with the fixture. */
  readonly automation: TestAutomation;
  readonly app: TestApp;
  readonly apps: Apps;
  readonly profile: ProfileFixture;
  /**
   * The OpenAPI documents this run checks against (`lxdev test --openapi`),
   * or `undefined` without them. A spec that only means something against a
   * contract skips itself: `if (!t.openapi) return t.skip('needs --openapi …')`.
   */
  readonly openapi: OpenApiRun | undefined;
  /** `--arg` / `--secret-arg` values; a missing key is `undefined`. */
  readonly args: Readonly<Record<string, string | undefined>>;
  /**
   * Read one `--arg` value. Missing, it throws naming the `--arg` to pass,
   * unless `default` is given or `required` is false.
   */
  arg(name: string, options: { required: false; default?: undefined }): string | undefined;
  arg(name: string, options?: ArgOptions): string;
  step<T>(name: string, body: () => T | Promise<T>): Promise<T>;
  expect: FixtureExpect;
  reject(
    operation: () => unknown | Promise<unknown>,
    expected?: RejectExpected,
  ): Promise<unknown>;
  /**
   * Call `read` until `accept` (default: truthy) passes and resolve to that
   * value. A thrown error retries only when `retryIf` allows it (see
   * `WaitForOptions`); on timeout the error names the last value or error.
   */
  waitFor<T>(
    read: () => T | Promise<T>,
    accept?: (value: T) => boolean,
    options?: WaitForOptions,
  ): Promise<T>;
  defer(cleanup: () => void | Promise<void>): void;
  attach(name: string, data: unknown): Promise<void>;
  /**
   * Stop this spec and report it `skipped` with `reason`, for a precondition
   * only knowable at run time. Throws; deferred cleanup still runs. Call it
   * from the body or `beforeEach`; it rejects during cleanup.
   */
  skip(reason: string): never;
}

export interface Matchers<T> {
  readonly not: Matchers<T>;
  toBe(expected: unknown): void;
  toEqual(expected: unknown): void;
  toContain(expected: unknown): void;
  toMatch(expected: string | RegExp): void;
  toBeTruthy(): void;
  toBeFalsy(): void;
  toBeDefined(): void;
  toBeUndefined(): void;
  toBeInstanceOf(expected: Function): void;
  toThrow(expected?: unknown): void;
  /** Numeric ordering. Comparing anything but numbers fails the assertion. */
  toBeGreaterThan(expected: number): void;
  toBeGreaterThanOrEqual(expected: number): void;
  toBeLessThan(expected: number): void;
  toBeLessThanOrEqual(expected: number): void;
  /**
   * The value matches a schema of the run's OpenAPI documents (`lxdev test
   * --openapi`): a component schema name (`'Device'`), a `'#/…'` ref, or
   * `{ ref, document }` when several documents define it. Without
   * `--openapi` it fails saying so.
   */
  toMatchSchema(schema: string | { ref: string; document?: string }): void;
}

export interface SourceLocation {
  source: string;
  line: number;
  column: number;
}

export interface AssertionRecord {
  matcher: string;
  expected: string;
  actual: string;
  passed: boolean;
  step?: string;
}

export interface StepRecord {
  name: string;
  path: string;
  /** `step` is authored with `t.step`; `action` is a recorded driver call. */
  kind?: "step" | "action";
  /** Short argument summary for an action — a selector, a page, a script head. */
  detail?: string;
  /** Identical consecutive actions collapse into one row with a count. */
  repeat?: number;
  status: StepStatus;
  duration_ms: number;
  error?: ReportError;
  steps: StepRecord[];
  attachments: AttachmentRef[];
  assertions: AssertionRecord[];
}

export interface AttachmentRef {
  name: string;
  path: string;
  mimeType: string;
}

/** A page instance, as a failure names it. */
export interface FailurePage {
  name: string | null;
  instanceId: string | null;
}

/**
 * One Logic `fetch` (or `Rong.SSE` connection) the app made, as a failed
 * spec's report lists it: no bodies, no headers, credentials and
 * `--secret-arg` values masked in the URL.
 */
export interface NetworkCall {
  /** Epoch milliseconds when the request started. */
  time: number;
  /** `function`: a Worker Function call the dev session's companion saw. */
  kind: "fetch" | "sse" | "function";
  /** Empty for a `function` call. */
  method: string;
  /** Empty for a `function` call. */
  url: string;
  /** `function` calls: the Function and `result`/`error`/`fault`/`default`. */
  function?: string;
  outcome?: string;
  /** Response status; `null` when it failed or has not answered. */
  status: number | null;
  /** The rejection, e.g. `TypeError: fetch failed`. */
  error?: string;
  durationMs: number | null;
  /** `route` when a test route answered; `network` when the request was real. */
  source: "route" | "network";
  /** The route that matched, including a `continue` pass-through. */
  route?: { pattern: string; action: string; rule?: number; scenario?: string };
  /** `rule 2 (name:variant)`, `route <pattern>`, `real`, or `companion default`. */
  answeredBy?: string;
  /** Why no scenario rule answered, when rules targeted the call. */
  noMatch?: string;
}

/** The scenario a failed spec had installed, and what each rule answered. */
export interface ScenarioReport {
  /** `name:variant`. */
  label: string;
  name: string | null;
  variant: string | null;
  rules: { index: number; target: string; kind: "http" | "function"; hits: number }[];
}

export interface ReportError {
  code?: string;
  data?: unknown;
  phase?: string;
  /** The app's last Logic network calls before the failure, oldest first. */
  network?: NetworkCall[];
  /** The scenario installed with `t.app.scenario()` when the spec failed. */
  scenario?: ScenarioReport;
  /** The recorded driver action that failed, e.g. `page.click [data-testid=save]`. */
  failedAction?: string;
  /** The page that was current when the spec failed. */
  page?: FailurePage;
  name: string;
  message: string;
  stack?: string;
  matcher?: string;
  expected?: string;
  actual?: string;
  location?: string;
  step?: string;
}

export interface CaseRecord {
  attempt?: number;
  attempts?: CaseRecord[];
  flaky?: boolean;
  id: string;
  title: string;
  name: string;
  full_name: string;
  /** Which `--repeat-each` execution this is (1-based); absent without it. */
  repeat?: number;
  /** Source file the spec was registered from, remapped through the bundle map. */
  file?: string;
  line?: number;
  /** Display group in the report — the spec file's path inside the project. */
  suite?: string;
  status: SpecStatus;
  duration_ms: number;
  covers: string[];
  /** Selection tags: the file's `spec.configure` tags, then the spec's own. */
  tags?: string[];
  /**
   * `lxdev test --openapi`: this spec's Logic `fetch` responses checked
   * against the contract. `violations` (routed responses) failed the spec;
   * `warnings` (the real server's) did not.
   */
  contract?: CaseContract;
  /**
   * `lx.*` members the spec's evals actually reached, observed by the runtime
   * rather than declared. A `covers` tag absent from here was claimed but never
   * exercised.
   */
  observed?: string[];
  steps: StepRecord[];
  assertions: AssertionRecord[];
  attachments: AttachmentRef[];
  error?: ReportError;
  timeout_ms: number;
  reason?: string;
}

/** One response that breaks the OpenAPI contract. */
export interface ContractIssue {
  /** The spec id. */
  case: string;
  /** `route`/`patch`: a route shaped it; `network`: the real server. */
  source: "route" | "patch" | "network";
  method: string;
  /** Scheme, host and path only. */
  url: string;
  status: number;
  /** `METHOD /path/{template}` as the document names it. */
  operation: string;
  /** The response schema, as a document pointer. */
  schema: string;
  /** The route that fulfilled or patched it. */
  pattern?: string;
  issues: Array<{ path: string; message: string; schema: string }>;
}

export interface CaseContract {
  /** Responses captured during the spec. */
  checked: number;
  violations: ContractIssue[];
  warnings: ContractIssue[];
}

/** `t.openapi`: the contract a run loaded with `--openapi`. */
export interface OpenApiRun {
  documents: Array<{ name: string; version: string; title?: string }>;
}

/** `report.openapi`: what `--openapi` checked across the run. */
export interface OpenApiSummary {
  documents: Array<{ name: string; version: string; title?: string; operations: number }>;
  /** `ok`, or why responses could not be captured (older host). */
  capture: string;
  /** Captured responses. */
  responses: number;
  /** Responses with a documented JSON schema and a body, validated. */
  validated: number;
  routed: { validated: number; failed: number };
  network: { validated: number; mismatched: number };
  /** Matched an operation but not validated, by why. */
  skipped: { no_schema: number; not_json: number; empty: number; truncated: number };
  /** A status the operation does not document. */
  undocumented: Array<{ operation: string; status: number; source: string; count: number }>;
  /** Requests no operation describes (another API, or a gap in the document). */
  unmatched: Array<{ method: string; path: string; count: number }>;
  /** Real-server mismatches (at most 100). */
  warnings: ContractIssue[];
}

/** `report.tag_summary`: one row per tag, and `(untagged)` for the rest. */
export interface TagSummary {
  tag: string;
  total: number;
  passed: number;
  failed: number;
  skipped: number;
  timeout: number;
  xfail: number;
  xpass: number;
  flaky: number;
  /** No failed, timed-out or xpass spec carries the tag. */
  ok: boolean;
}

/** A spec covering a manifest id, with its outcome in this run. */
export interface CoverageSpec {
  id: string;
  title: string;
  /** `not_run`: registered, but outside this selection. */
  status: SpecStatus | "not_run";
}

/** `report.coverage`: the `--covers-manifest` ids against the suite's `covers`. */
export interface CoverageSummary {
  total: number;
  /** Ids at least one registered spec covers. */
  covered: number;
  /** Covered ids whose specs passed (none broke). */
  passing: number;
  /** Covered ids with a failed, timed-out or xpass spec. */
  failing: number;
  /** Manifest ids no spec covers. */
  uncovered: Array<{ id: string; title?: string }>;
  /** `covers` ids used by specs but missing from the manifest. */
  unknown: Array<{ id: string; specs: string[] }>;
  ids: Array<{
    id: string;
    title?: string;
    status: "passed" | "failed" | "xfail" | "skipped" | "not_run" | "uncovered";
    specs: CoverageSpec[];
  }>;
}

/** The app under test, so a report identifies its own subject. */
export interface RunSubject {
  appid?: string;
  app_name?: string;
  version?: string;
  release_type?: string;
  pages?: number;
}

export interface RunMeta {
  started_at: string;
  duration_ms: number;
  /** User args; declared secrets and credential-named keys are `***`. */
  args: Record<string, string>;
  /** lxdev's run controls (grep, ids, shard, retries, …). */
  run?: Record<string, string>;
  platform?: string;
  framework?: string;
  subject?: RunSubject;
  /** The suite opted into measuring the whole published `lx` surface. */
  surface_coverage?: boolean;
  /**
   * The whole-run budget: fixed by `--timeout-secs`, or scaled to the
   * planned executions (`auto`). `exhausted_after` counts the specs that
   * finished before it ran out; the rest are skipped as not run.
   */
  budget?: { ms: number; auto: boolean; planned: number; exhausted_after?: number };
  /** `--shuffle` seed; rerun with `--shuffle=<seed>` for the same order. */
  shuffle_seed?: number;
  /** `--repeat-each` count. */
  repeat_each?: number;
}

/** One failed case, flat, for tools that only need what broke and where. */
export interface FailureRecord {
  id: string;
  title: string;
  file?: string;
  line?: number;
  phase?: string;
  code?: string;
  message: string;
  failedAction?: string;
  page?: FailurePage;
  /** Report-relative path of the failure screenshot, when one was captured. */
  screenshot?: string;
  /** The app's last Logic network calls before the failure (up to 20). */
  network?: NetworkCall[];
  /** The scenario installed when the spec failed. */
  scenario?: ScenarioReport;
}

export interface JsonReport {
  schema_version?: number;
  framework: { name: string; version: string };
  meta: RunMeta;
  partial: boolean;
  filtered: boolean;
  total: number;
  passed: number;
  failed: number;
  skipped: number;
  xfail: number;
  xpass: number;
  timeout: number;
  duration_ms: number;
  cases: CaseRecord[];
  /** Every failed, timed-out or xpass case, flattened from `cases`. */
  failures?: FailureRecord[];
  /** Per-tag outcome, when any spec is tagged. */
  tag_summary?: TagSummary[];
  /** `--covers-manifest` summary. */
  coverage?: CoverageSummary;
  /** `--openapi` contract summary. */
  openapi?: OpenApiSummary;
}

export type ProtocolReport = JsonReport;

export interface LingxiaTestController {
  run(): Promise<ProtocolReport>;
  readonly version: string;
  /** Clears the registry. Used by this package's own Node tests. */
  reset(): void;
}

export interface AutomationHost {
  args?: Record<string, string>;
  /** lxdev run controls, kept apart from the user's `args`. */
  control?: Record<string, string>;
  attach?: (
    name: string,
    artifact: { mimeType: string; base64: string },
  ) => void | Promise<void>;
  emit?: (event: Record<string, unknown>) => void | Promise<void>;
  report?: (event: Record<string, unknown>) => void | Promise<void>;
  logs?: () => string | string[] | Promise<string | string[]>;
  /** Logic network calls since `sinceMs`, newest `limit`. */
  networkLog?: (sinceMs: number, limit?: number) => unknown;
  /** `lxdev test --record-network`: start, or stop and return the scenario. */
  networkRecord?: (command: "start" | "stop", name?: string) => unknown;
}

declare global {
  // eslint-disable-next-line no-var
  var __LINGXIA_TEST__: LingxiaTestController | undefined;
  // eslint-disable-next-line no-var
  var __LINGXIA_AUTOMATION_HOST__: AutomationHost | undefined;
  // eslint-disable-next-line no-var
  var __RONG_TEST_HOST__: AutomationHost | undefined;
  // eslint-disable-next-line no-var
  var __LINGXIA_TEST_SOURCE_MAP__: unknown;
  // eslint-disable-next-line no-var
  var __LINGXIA_CLI_VERSION__: string | undefined;
}
