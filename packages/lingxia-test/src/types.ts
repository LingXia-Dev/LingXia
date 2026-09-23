/// <reference types="@lingxia/types/testing" preserve="true" />
import type {
  Automation,
  LogicLxAppEvalOptions,
  LxAppDriver,
  NetworkDriver,
  PageDriver,
  PageEvalOptions,
  PageQueryResult,
  PageTarget,
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
  /** Spec budget in ms (default 30_000). */
  timeout?: number;
  /** Relaunch the home page before the body. */
  fresh?: boolean;
  /** Independent cleanup budget; a pending cleanup stops subsequent specs. */
  timeoutCleanup?: number;
  /** Pin `t.app` to this lxapp id instead of the current one. */
  app?: string;
  /** Skip auto-attached failure forensics (only when capture itself would wedge). */
  forensics?: boolean;
  /** Why a skip/fixme spec is registered. Shown in the HTML/JSON report; `t.skip(reason)` overrides it. */
  reason?: string;
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

export interface RejectExpected {
  code?: string;
  message?: string | RegExp;
}

export interface LocatorOptions extends PageTarget {
  /** Zero-based match index; omit to require a unique match. */
  index?: number;
}

/**
 * Locator states are strict about ambiguity (narrow with `.nth()`):
 * - `attached`: exactly one match in the DOM, in the viewport or not (content
 *   of a sheet or long page that overflows the Runner viewport).
 * - `visible`: exactly one match, and it intersects the viewport.
 * - `hidden`: no visible match, including no match at all.
 * - `detached`: no match.
 * Several matches satisfy only `hidden` (when none is visible). The raw
 * `page.waitFor` driver checks the first match instead; see `PageWaitState`.
 */
export type LocatorState = "attached" | "detached" | "visible" | "hidden";

export interface LocatorWaitOptions extends ExpectOptions {
  /** Defaults to `visible`. */
  state?: LocatorState;
}

export interface Locator {
  readonly selector: string;
  /** Wait until the locator reaches `state`; rejects at the timeout. */
  waitFor(options?: LocatorWaitOptions): Promise<void>;
  click(options?: ExpectOptions): Promise<void>;
  fill(text: string, options?: ExpectOptions): Promise<void>;
  type(text: string, options?: ExpectOptions): Promise<void>;
  press(key: string, options?: ExpectOptions): Promise<void>;
  nth(index: number): Locator;
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
  toBeVisible(options?: ExpectOptions): Promise<void>;
  toBeHidden(options?: ExpectOptions): Promise<void>;
  toBeAttached(options?: ExpectOptions): Promise<void>;
  toBeEnabled(options?: ExpectOptions): Promise<void>;
  toBeDisabled(options?: ExpectOptions): Promise<void>;
  toBeEditable(options?: ExpectOptions): Promise<void>;
  toHaveText(expected: string | RegExp, options?: ExpectOptions): Promise<void>;
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

export interface ReportError {
  code?: string;
  data?: unknown;
  phase?: string;
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
  /** Source file the spec was registered from, remapped through the bundle map. */
  file?: string;
  line?: number;
  /** Display group in the report — the spec file's path inside the project. */
  suite?: string;
  status: SpecStatus;
  duration_ms: number;
  covers: string[];
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
