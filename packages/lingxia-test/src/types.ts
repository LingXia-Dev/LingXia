import type { Automation, LxAppDriver, PageDriver, PageQueryResult, PageTarget } from "@lingxia/types/automation";

export type SpecStatus =
  | "passed"
  | "failed"
  | "skipped"
  | "timeout"
  | "xfail"
  | "xpass";

export type StepStatus = "passed" | "failed" | "timeout";

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
  /** Why a skip/fixme spec is registered. Shown in the HTML/JSON report. */
  reason?: string;
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
 * `attached`: exactly one match in the DOM, in the viewport or not (content of
 * a sheet or long page that overflows the Runner viewport). `visible`: that
 * match intersects the viewport. `hidden`: no visible match. `detached`: none.
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

export interface TestPage extends PageDriver {
  testId(id: string, options?: LocatorOptions): Locator;
  css(selector: string, options?: LocatorOptions): Locator;
}

export interface TestApp extends LxAppDriver {
  readonly page: TestPage;
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

export interface Fixture {
  /** Guarded host drivers; use these in tests so actions are traced and stop with the fixture. */
  readonly automation: TestAutomation;
  readonly app: TestApp;
  readonly apps: Apps;
  readonly args: Record<string, string>;
  step<T>(name: string, body: () => T | Promise<T>): Promise<T>;
  expect: FixtureExpect;
  reject(
    operation: () => unknown | Promise<unknown>,
    expected?: RejectExpected,
  ): Promise<unknown>;
  defer(cleanup: () => void | Promise<void>): void;
  attach(name: string, data: unknown): Promise<void>;
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
  args: Record<string, string>;
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
