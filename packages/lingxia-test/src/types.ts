/// <reference types="@lingxia/types/testing" preserve="true" />
/// <reference types="@lingxia/types/logic-globals" preserve="true" />
import type {
  Automation,
  AutomationErrorCode,
  BrowserDriver,
  ClockAdvance,
  ClockInstallOptions,
  ClockRunAllOptions,
  ClockState,
  ClockTime,
  DesktopDriver,
  HostRunAutomation,
  LxAppDriver,
  NetworkRouteHandler,
  NetworkRoutePattern,
  PageKey,
  PagePointer,
  PageQueryResult,
  PageScrollOptions,
  PageTarget,
  ProfileRestoreResult,
  ScenarioCallFilter,
  ScenarioInput,
  ScenarioRuleInfo,
  Screenshot,
  TerminalDriver,
} from "@lingxia/types/automation";
import type { ProtocolReport } from "./report-types.js";

/**
 * Every `code` a spec can meet on a rejection or a failed spec: the
 * automation driver codes, a broken OpenAPI contract, a fixture wait that ran
 * out of time, and a runtime skip.
 */
export type TestErrorCode = AutomationErrorCode | "E_OPENAPI_CONTRACT" | "E_TIMEOUT" | "E_SKIPPED";

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
  /**
   * What the spec needs from the run. Unmet, it is reported `skipped` with a
   * reason naming what to pass, and its body never runs. Merged with the
   * file's `spec.configure({ requires })`.
   */
  requires?: SpecRequirements;
}

/** `requires`: run inputs a spec cannot mean anything without. */
export interface SpecRequirements {
  /** `--arg` / `--secret-arg` keys that must be given. */
  args?: readonly string[];
  /** The run must check an OpenAPI contract (`lxdev test --openapi`). */
  openapi?: boolean;
}

/** `restoreProfile: { keep }`. */
export interface RestoreProfileOptions {
  /** `lx.getStorage()` key globs that survive the rollback (`*`, `?`). */
  keep: string[];
}

/**
 * `spec.configure()` options: defaults for every spec in the calling file.
 * Every `SpecOptions` key but `id`; a spec's own option overrides the file's,
 * except `tags`, `covers` and `requires`, which add to it.
 */
export type FileOptions = Omit<SpecOptions, "id">;

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
   * part of an overflowing sheet, or for a desktop app window that another
   * window covers: a forced action is dispatched as DOM events on the
   * element, never as input at its position on screen. The action then
   * proves less about what a user can reach, so prefer the default wherever
   * it works.
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
   * The rejection's `code`: a `TestErrorCode` (driver codes such as
   * `E_PAGE_NOT_ACTIVE`, plus `E_TIMEOUT`, `E_OPENAPI_CONTRACT`; see
   * `TEST_ERROR_CODES`) or any other string code the operation rejects with.
   */
  code?: TestErrorCode | (string & {});
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
  /** Read once; `expect(locator)` retries. */
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
}

/** A page whose declared methods are not typed: any member may be read. */
export type AnyLogicPage<TData = Record<string, unknown>> = LogicPage<TData> & {
  /** Page methods declared in `Page({...})`. */
  readonly [member: string]: unknown;
};

/** The app instance as app Logic sees it (`getApp()`). */
export interface LogicApp {
  readonly [member: string]: unknown;
}

/**
 * What a function passed to `t.app.logic.eval(fn)` receives. It runs inside
 * the app's Logic runtime, so these are the app's own `lx`, `getApp` and
 * `getCurrentPages` — not globals of the test context.
 */
export interface LogicScope {
  readonly lx: Lx & { automation(): Automation };
  getApp<T extends LogicApp = LogicApp>(): T | null;
  getCurrentPages<T extends LogicPage<any> = AnyLogicPage>(): T[];
}

type PageGlobal<K extends string, Fallback> = typeof globalThis extends { [P in K]: infer V } ? V : Fallback;

/**
 * An element as `t.app.view.eval(fn)` reads it, when the test tsconfig has no
 * DOM library: enough to read text, values and attributes without a cast.
 */
export interface ViewElement {
  readonly tagName: string;
  readonly id: string;
  readonly className: string;
  readonly textContent: string | null;
  readonly innerText?: string;
  /** Inputs, textareas and selects. */
  readonly value?: string;
  /** Checkboxes and radios. */
  readonly checked?: boolean;
  readonly disabled?: boolean;
  readonly children: ArrayLike<ViewElement>;
  getAttribute(name: string): string | null;
  hasAttribute(name: string): boolean;
  querySelector(selector: string): ViewElement | null;
  querySelectorAll(selector: string): ArrayLike<ViewElement>;
  closest(selector: string): ViewElement | null;
  getBoundingClientRect(): { left: number; top: number; width: number; height: number };
}

/** The page's `document` in `t.app.view.eval(fn)` without a DOM library. */
export interface ViewDocument {
  readonly title: string;
  readonly body: ViewElement;
  readonly documentElement: ViewElement;
  readonly activeElement: ViewElement | null;
  querySelector(selector: string): ViewElement | null;
  querySelectorAll(selector: string): ArrayLike<ViewElement>;
  getElementById(id: string): ViewElement | null;
}

/** The page's `window` in `t.app.view.eval(fn)` without a DOM library. */
export interface ViewWindow {
  readonly innerWidth: number;
  readonly innerHeight: number;
  readonly scrollX: number;
  readonly scrollY: number;
  readonly location: { readonly href: string; readonly pathname: string; readonly search: string };
  getComputedStyle(element: ViewElement): { getPropertyValue(name: string): string; readonly [property: string]: unknown };
  readonly [member: string]: unknown;
}

/**
 * What a function passed to `t.app.view.eval(fn)` receives: the page
 * WebView's `document` and `window`. With the DOM library in the test
 * tsconfig they are the DOM's own types; without it (the recommended test
 * tsconfig), `ViewDocument` and `ViewWindow`.
 */
export interface ViewScope {
  readonly document: PageGlobal<"document", ViewDocument>;
  readonly window: PageGlobal<"window", ViewWindow>;
}

/**
 * A function evaluated in the app's Logic runtime. It is sent as source text
 * (`fn.toString()`), so it must be self-contained: no variables, imports or
 * helpers from the spec. Pass values as JSON arguments instead.
 */
export type LogicFunction<R, A extends JsonValue[]> = (scope: LogicScope, ...args: A) => R | Promise<R>;

/** A function evaluated in the page WebView; self-contained like `LogicFunction`. */
export type ViewFunction<R, A extends JsonValue[]> = (scope: ViewScope, ...args: A) => R | Promise<R>;

/**
 * `t.app.view`: the current page's View — locators, function eval, and
 * page-level input. Read elements and act on them through locators; they wait
 * for the element and retry, where a raw driver call would not.
 */
export interface TestView {
  testId(id: string, options?: LocatorOptions): Locator;
  css(selector: string, options?: LocatorOptions): Locator;
  /**
   * Run `fn` in the current page's WebView with JSON `args` and resolve to
   * its JSON result. `fn` must be self-contained (see `ViewFunction`).
   */
  eval<R, A extends JsonValue[]>(fn: ViewFunction<R, A>, ...args: A): Promise<Awaited<R>>;
  /**
   * The same with options: `page` runs it in that page's WebView (a
   * configured page name or live instance id) instead of the current page's,
   * like a locator's `{ page }` — a page kept below the current one, or one a
   * surface shows; `timeout` see `EvalOptions`.
   */
  eval<R, A extends JsonValue[]>(options: ViewEvalOptions, fn: ViewFunction<R, A>, ...args: A): Promise<Awaited<R>>;
  screenshot(options?: PageTarget): Promise<Screenshot>;
  /** Scroll the page DOM by a pixel delta (nearest scrollable container). */
  scroll(options?: PageScrollOptions): Promise<void>;
  /** App-window pointer input at page coordinates. */
  readonly pointer: PagePointer;
  /** App-window keyboard input. */
  readonly key: PageKey;
}

/**
 * Leading options of `t.app.logic.eval(options, fn, ...args)`. Without them
 * an eval may take a third of the spec's budget, at most 10 s.
 */
export interface EvalOptions {
  /**
   * How long `fn` may run, in ms, for work that legitimately takes longer
   * (a download, a transcode). Clamped to the spec's remaining budget.
   */
  timeout?: number;
}

/** Leading options of `t.app.view.eval(options, fn, ...args)`. */
export interface ViewEvalOptions extends PageTarget, EvalOptions {}

/** `t.app.logic.data()` options. */
export interface LogicDataOptions {
  /** Configured page name or route; defaults to the current page. */
  page?: string;
}

/**
 * Names `t.app.logic.call<T>()` accepts: the methods `T` declares, or any
 * name for a page type that does not list its methods (`AnyLogicPage`).
 */
export type LogicPageMethod<T> = string extends keyof T
  ? string
  : { [K in keyof T]-?: T[K] extends (...args: any[]) => unknown ? K : never }[keyof T] & string;

/** What `call<T, K>()` resolves to: `K`'s awaited result, `unknown` untyped. */
export type LogicMethodResult<T, K> = K extends keyof T
  ? T[K] extends (...args: any[]) => infer R ? Awaited<R> : unknown
  : unknown;

/** `t.app.logic`: the app's Logic runtime, read and driven from the spec. */
export interface TestLogic {
  /**
   * Run `fn` in the app's Logic runtime with JSON `args` and resolve to its
   * JSON result. `fn` must be self-contained (see `LogicFunction`).
   */
  eval<R, A extends JsonValue[]>(fn: LogicFunction<R, A>, ...args: A): Promise<Awaited<R>>;
  /** The same with a `timeout` (see `EvalOptions`). */
  eval<R, A extends JsonValue[]>(options: EvalOptions, fn: LogicFunction<R, A>, ...args: A): Promise<Awaited<R>>;
  /** Read the current (or named) page's Logic `data`. `T` is not validated. */
  data<T = Record<string, unknown>>(options?: LogicDataOptions): Promise<T>;
  /**
   * Call a method of the current page's Logic instance and resolve to its
   * JSON result. With a page type, `method` must be one of its methods:
   * `call<TodoPage>('addTodo', 'milk')`; `call<TodoPage, 'count'>('count')`
   * also types the result.
   */
  call<T extends LogicPage<any> = AnyLogicPage, K extends LogicPageMethod<T> = LogicPageMethod<T>>(
    method: K,
    ...args: JsonValue[]
  ): Promise<LogicMethodResult<T, K>>;
}

/**
 * One call the app made, as `route.calls()`, `scenario.calls()` and
 * `t.app.network.calls()` list it.
 */
export interface NetworkCall {
  /** Epoch milliseconds. */
  time: number;
  /** `http`: Logic `fetch` or `Rong.SSE`; `function`: a Worker Function call. */
  kind: "http" | "sse" | "function";
  /** `http`: upper-case method and URL. */
  method?: string;
  url?: string;
  /** `function`: the Function's name. */
  function?: string;
  /** `http`: the answered status; `null` when none was (abort, hang, still pending). */
  status?: number | null;
  /**
   * The request body (parsed when it is JSON; text otherwise), or a
   * Function call's arguments. `null` when there is none or it could not be
   * read without consuming it.
   */
  body?: unknown;
  /** Request headers with lower-case names; calls a route handled only. */
  headers?: Record<string, string>;
  /**
   * What answered: a scenario `rule`, a test `route`, the `real` backend, or
   * the dev session's `companion` default for a Function no rule matched.
   */
  answeredBy: "rule" | "route" | "real" | "companion";
  /** The scenario rule that answered (1-based). */
  rule?: number;
  /** `function`: `result`, `error`, `fault` or `default`. */
  outcome?: string;
  /** Why no scenario rule matched, when rules targeted the call. */
  noMatch?: string;
}

/** `waitForCall()` options. */
export interface WaitForCallOptions {
  /** Default: the action timeout (5000 ms), clamped to the spec's remaining budget. */
  timeout?: number;
  interval?: number;
}

/** A test route, spec-scoped: removed when the spec ends. */
export interface TestRoute {
  readonly id: number;
  readonly pattern: string;
  /** Remove the route. Removing one that already expired is not an error. */
  unroute(): Promise<void>;
  /** Calls this route handled, oldest first. */
  calls(): Promise<NetworkCall[]>;
  /**
   * Resolve with the oldest call this route handled that an earlier
   * `waitForCall()` did not already return, waiting for it if there is none
   * yet. On timeout the error lists the route's recent calls.
   */
  waitForCall(options?: WaitForCallOptions): Promise<NetworkCall>;
}

/**
 * `t.app.network`: test routing of the app's Logic `fetch` and `Rong.SSE`,
 * spec-scoped: routes are removed when the spec ends.
 */
export interface TestNetwork {
  /** The newest matching route handles a request; unmatched requests are untouched. */
  route(pattern: NetworkRoutePattern, handler: NetworkRouteHandler): Promise<TestRoute>;
  /** Remove every route of this run for the app. */
  unrouteAll(): Promise<void>;
  /** Calls this spec's routes handled, oldest first. */
  calls(): Promise<NetworkCall[]>;
}

/** `scenario.waitForCall()` target: a rule's target as the file writes it, or its number. */
export type ScenarioCallTarget = ScenarioCallFilter;

/** The scenario `t.app.scenario()` installed, spec-scoped. */
export interface TestScenario {
  readonly name: string | null;
  readonly variant: string | null;
  /** Its rules with their hit counts, read when accessed. */
  readonly rules: ScenarioRuleInfo[];
  /**
   * Calls that reached the scenario since it was installed, oldest first:
   * all of them, or those of one rule target (`{ http: 'GET **\/x' }`,
   * `{ function: 'orders.submit' }`) or rule number (`{ rule: 2 }`).
   */
  calls(filter?: ScenarioCallFilter): Promise<NetworkCall[]>;
  /**
   * Resolve with the oldest call to `target` that an earlier `waitForCall`
   * for the same target did not already return, waiting for one if needed.
   * On timeout the error lists the scenario's recent calls.
   */
  waitForCall(target: ScenarioCallTarget, options?: WaitForCallOptions): Promise<NetworkCall>;
  /** Remove the scenario before the spec ends. */
  remove(): Promise<void>;
}

/**
 * `t.app.clock`: test clock for the app's Logic (`Date`, timers,
 * `performance.now`). Spec-scoped: a clock still installed when the spec
 * ends is uninstalled, and if that drops pending test timers the next spec
 * starts from a relaunched home page.
 */
export interface TestClock {
  /** Put Logic on test time. Rejects with `E_CLOCK_INSTALLED` when a clock already is. */
  install(options?: ClockInstallOptions): Promise<ClockState>;
  /** Advance by `ms`, firing each timer due on the way at its own time. */
  tick(ms: number): Promise<ClockAdvance>;
  /** Fire timers, including those they schedule, until none is left. */
  runAll(options?: ClockRunAllOptions): Promise<ClockAdvance>;
  /** Change what `Date` reads without firing timers. */
  setSystemTime(time: ClockTime): Promise<ClockState>;
  /**
   * Return Logic to real time. Pending test timers are dropped, never fired;
   * the report's trace says how many.
   */
  uninstall(): Promise<void>;
}

/**
 * `t.app`: the app under test. A saved `t.app` (or any part of it) keeps
 * reaching the app after a profile checkpoint or restore reopens it.
 */
export interface TestApp {
  /** The current page's View: locators, `eval(fn)`, screenshots and input. */
  readonly view: TestView;
  /** The app's Logic runtime: `eval(fn)`, page `data()` and `call()`. */
  readonly logic: TestLogic;
  /** Navigation; actions wait for the landed page's `onReady` unless you pass `waitUntil`. */
  readonly nav: LxAppDriver["nav"];
  /** Spec-scoped test routing of Logic `fetch`. */
  readonly network: TestNetwork;
  /**
   * Put the app into a product state from a scenario file (and one of its
   * variants): `http` rules answer Logic `fetch`, `function` rules go to the
   * dev session's companion. Spec-scoped: a second call replaces the first,
   * and the spec's end removes it. A failed spec reports its per-rule hits
   * and the calls that reached it.
   */
  scenario(definition: ScenarioInput, variant?: string): Promise<TestScenario>;
  /** Spec-scoped test clock for the app's Logic. */
  readonly clock: TestClock;
  /** Checkpoint and roll back the app's isolated data. */
  readonly profile: ProfileFixture;
  info: LxAppDriver["info"];
  pages: LxAppDriver["pages"];
  surfaceLayout: LxAppDriver["surfaceLayout"];
}

/** A checkpoint of the app's isolated data. */
export interface ProfileCheckpoint {
  readonly id: string;
}

/**
 * `t.app.profile`: checkpoint and roll back the app's isolated data by hand.
 * Needs an isolated run (`lxdev test --profile`); otherwise every call
 * rejects with `E_PROFILE_NOT_ISOLATED`. `checkpoint` and `restore` close the
 * app and reopen it at its initial page; every fixture app of it, saved or
 * not, follows the reopened app.
 */
export interface ProfileFixture {
  /** Snapshot the app's data. */
  checkpoint(): Promise<ProfileCheckpoint>;
  /**
   * Roll the app's data back to `checkpoint` (or its id). With `keep`, the
   * `lx.getStorage()` keys those globs match keep their current state;
   * `kept` lists the ones that existed.
   */
  restore(checkpoint: ProfileCheckpoint | string, options?: ProfileRestoreOptions): Promise<ProfileRestoreResult>;
  /** Discard `checkpoint`. */
  drop(checkpoint: ProfileCheckpoint | string): Promise<void>;
}

/** `t.app.profile.restore` options. */
export interface ProfileRestoreOptions {
  /**
   * `lx.getStorage()` keys whose current state survives the rollback: globs
   * over the whole key (`*` any run of characters, `?` one). A matching key
   * keeps its current value, one added since the checkpoint stays, and one
   * deleted since stays deleted. Files always roll back.
   */
  keep?: readonly string[];
}

/**
 * `t.automation`: the host-run automation root, traced and stopped with the
 * spec like `t.app`.
 */
export interface TestAutomation extends Omit<HostRunAutomation, "lxapp" | "browser" | "desktop" | "terminal"> {
  /** The current lxapp, or another running one by id, as a fixture app. */
  lxapp(): TestApp;
  lxapp(appId: string): TestApp;
  /**
   * The host app's browser tabs.
   *
   * @privileged host — reading it never throws; on a host without a browser
   * shell each call rejects.
   */
  readonly browser: BrowserDriver;
  /**
   * Local-OS desktop automation (Windows/macOS).
   *
   * @privileged host — reading it never throws; on a host built without
   * desktop automation each call rejects.
   */
  readonly desktop: DesktopDriver;
  /**
   * Native terminal workspace state and pane actions.
   *
   * @privileged host — reading it never throws; on a host without a native
   * terminal each call rejects.
   */
  readonly terminal: TerminalDriver;
}

/** `expect.poll(read)` matchers: they call `read` until the matcher passes. */
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
  toHaveLength(expected: number): Promise<void>;
  toBeGreaterThan(expected: number): Promise<void>;
  toBeGreaterThanOrEqual(expected: number): Promise<void>;
  toBeLessThan(expected: number): Promise<void>;
  toBeLessThanOrEqual(expected: number): Promise<void>;
}

/** `expect(locator)` matchers: they retry until the element passes. */
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

/** What `expect(promise)` gives: nothing to call. Await the promise, or poll a read. */
export interface PromiseNotAllowed {
  readonly "expect(promise)": "await the value first, or retry a read with expect.poll(() => promise)";
}

/** The matchers `expect(subject)` gives for a subject of type `T`. */
export type ExpectResult<T> =
  0 extends 1 & T ? Matchers<any>
    : [T] extends [Locator] ? LocatorMatchers
      : [T] extends [PromiseLike<unknown>] ? PromiseNotAllowed
        : Matchers<T>;

/**
 * The one assertion entry point:
 * - `expect(locator)` retries the matcher until the element passes (await it);
 * - `expect(value)` checks once, synchronously;
 * - `expect.poll(() => read())` calls `read` until the matcher passes (await it).
 * Retries end at `timeout` (default 5000 ms), clamped to the spec's
 * remaining budget; locators and `poll` need a running spec.
 */
export interface Expect {
  <T>(subject: T): ExpectResult<T>;
  poll<T>(read: () => T | Promise<T>, options?: ExpectOptions): RetryMatchers<Awaited<T>>;
}

export interface WaitForOptions<T = unknown> {
  /** When the value is the one to wait for. Default: it is truthy. */
  until?: (value: T) => boolean;
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

/** `t.openapi`: the contract a run loaded with `--openapi`. */
export interface OpenApiRun {
  documents: Array<{ name: string; version: string; title?: string }>;
}

export interface Fixture {
  /** Guarded host drivers; use these in tests so actions are traced and stop with the fixture. */
  readonly automation: TestAutomation;
  readonly app: TestApp;
  /**
   * The OpenAPI documents this run checks against (`lxdev test --openapi`),
   * or `undefined` without them. A spec that only means something against a
   * contract declares it: `requires: { openapi: true }`.
   */
  readonly openapi: OpenApiRun | undefined;
  /**
   * Read one `--arg` / `--secret-arg` value. Missing, it throws naming the
   * `--arg` to pass, unless `default` is given or `required` is false.
   */
  arg(name: string, options: { required: false; default?: undefined }): string | undefined;
  arg(name: string, options?: ArgOptions): string;
  step<T>(name: string, body: () => T | Promise<T>): Promise<T>;
  reject(
    operation: () => unknown | Promise<unknown>,
    expected?: RejectExpected,
  ): Promise<unknown>;
  /**
   * Call `read` until `until` (default: truthy) accepts its value and
   * resolve to that value. A thrown error retries only when `retryIf` allows
   * it (see `WaitForOptions`); on timeout it rejects with a `TimeoutError`
   * (`E_TIMEOUT`) naming the last value or error.
   */
  waitFor<T>(read: () => T | Promise<T>, options?: WaitForOptions<Awaited<T>>): Promise<Awaited<T>>;
  defer(cleanup: () => void | Promise<void>): void;
  attach(name: string, data: unknown): Promise<void>;
  /**
   * Stop this spec and report it `skipped` with `reason`, for a precondition
   * only knowable at run time. Throws; deferred cleanup still runs. Call it
   * from the body or `beforeEach`; it rejects during cleanup.
   */
  skip(reason: string): never;
}

/** Matchers that check once, synchronously. */
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
  /** A string's or array's `length`. */
  toHaveLength(expected: number): void;
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


export interface LingxiaTestController {
  run(): Promise<ProtocolReport>;
  /** The specs `run()` would run, in `listed`, without running them. */
  list(): Promise<ProtocolReport>;
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
