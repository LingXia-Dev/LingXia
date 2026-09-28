/// <reference types="@lingxia/types/testing" preserve="true" />
/// <reference types="@lingxia/types/logic-globals" preserve="true" />
import type { PageContract } from "@lingxia/types/page";
export type { PageContract } from "@lingxia/types/page";
import type {
  ActionSheetAnswer,
  ActionSheetRecord,
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
  ModalAnswer,
  ModalRecord,
  NetworkRouteHandler,
  NetworkRoutePattern,
  PageInfo,
  PageKey,
  PagePointer,
  PageScrollOptions,
  PageTarget,
  ProfileRestoreResult,
  ScenarioCallFilter,
  ScenarioInput,
  ScenarioRuleInfo,
  Screenshot,
  TerminalDriver,
  ToastRecord,
} from "@lingxia/types/automation";

/**
 * Every `code` a spec can meet on a rejection or a failed spec: the
 * automation driver codes, a broken OpenAPI contract, a fixture call that
 * ran out of time, and a runtime skip. A driver timeout reaches a spec as
 * `E_TIMEOUT`, with the driver's own code in `error.cause.code`.
 */
export type TestErrorCode =
  | Exclude<AutomationErrorCode, "E_AUTOMATION_TIMEOUT" | "E_EVAL_TIMEOUT" | "E_DESKTOP_TIMEOUT">
  | "E_OPENAPI_CONTRACT"
  | "E_TIMEOUT"
  | "E_SKIPPED";

/**
 * The app's own error codes, empty until the app declares them by merging:
 *
 * ```ts
 * declare module '@lingxia/test' {
 *   interface AppErrorCodes { E_QUOTA: true }
 * }
 * ```
 *
 * A declared code is then accepted wherever an expected `code` is.
 */
// eslint-disable-next-line @typescript-eslint/no-empty-interface
export interface AppErrorCodes {}

/** A `code` a spec may expect: LingXia's `TestErrorCode`s and the app's declared ones. */
export type ExpectedErrorCode = TestErrorCode | Extract<keyof AppErrorCodes, string>;

export interface SpecOptions {
  /** Stable id. ASCII titles slug by default; non-ASCII titles need this or become `file-n`. */
  id?: string;
  /** Declared coverage tags. Journeys omit this. */
  covers?: readonly string[];
  /**
   * Selection tags (`routed`, `live`, `smoke`, …), merged after the
   * file's `spec.configure({ tags })`. `lxdev test --tag` selects by them and
   * the report summarizes each. Letters, digits and `_ . : / -`.
   */
  tags?: readonly string[];
  /** Spec budget in ms (default 30_000). */
  timeout?: number;
  /**
   * Relaunch the app on this page before the body (and its `beforeEach`
   * hooks): a fresh instance, every other page unloaded.
   */
  start?: SpecStart;
  /**
   * Snapshot the app's isolated data before this spec and roll it back after,
   * so the next spec never sees this one's writes. Relaunches the app on
   * `start`, or on its home page. Needs an
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

/** `start`: the page a spec begins on. */
export interface SpecStart {
  /** Configured page name (from lxapp.json). */
  page: string;
  /** Query forwarded to the page. */
  query?: Record<string, unknown>;
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

/** Retrying assertion and wait options. */
export interface ExpectOptions {
  /** Default 5000 ms, clamped to the spec's remaining budget. */
  timeout?: number;
  /** Poll interval in ms (default 50). */
  interval?: number;
}

/** `type` / `press` options. */
export interface InputOptions {
  /** Default 5000 ms, clamped to the spec's remaining budget. */
  timeout?: number;
}

/** `click` / `fill` options. */
export interface ActionOptions extends InputOptions {
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
   * `TEST_ERROR_CODES`) or one the app declared in `AppErrorCodes`. Both are
   * closed, so a code that no longer exists or was never declared does not
   * compile.
   */
  code?: ExpectedErrorCode;
  message?: string | RegExp;
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
  type(text: string, options?: InputOptions): Promise<void>;
  press(key: string, options?: InputOptions): Promise<void>;
  /** Pick the match at `index` (of the filtered matches, after `.filter()`). */
  nth(index: number): Locator;
  first(): Locator;
  last(): Locator;
  /** Narrow the matches; `nth`/`first`/`last` then pick among what is left. */
  filter(options: LocatorFilterOptions): Locator;
  /*
   * One-shot reads: they neither wait nor retry. Assert with
   * `expect(locator)`, which does.
   */
  /** How many elements match now. */
  count(): Promise<number>;
  /** Whether exactly one match is rendered now; several matches reject. */
  isVisible(): Promise<boolean>;
  /** The single match's text as the user sees it, whitespace-collapsed; rejects unless exactly one matches. */
  textContent(): Promise<string>;
  /** The single match's value (input, textarea, select); rejects unless exactly one matches, or it has no value. */
  inputValue(): Promise<string>;
  /** The single match's attribute, `null` when absent; rejects unless exactly one matches. */
  getAttribute(name: string): Promise<string | null>;
}

/** A value that crosses the eval boundary unchanged. */
export type JsonValue =
  | string
  | number
  | boolean
  | null
  | JsonValue[]
  | { [key: string]: JsonValue };

/** JSON members preserve their shape; unsupported values reject at runtime. */
type JsonMember<R> =
  0 extends 1 & R ? any
    : unknown extends R ? unknown
    // Do not recursively expand the recursive JsonValue union.
    : R extends JsonValue ? R
    : R extends (...args: never[]) => unknown ? never
    : R extends Date | RegExp | Map<unknown, unknown> | Set<unknown> | WeakMap<object, unknown> | WeakSet<object> | PromiseLike<unknown> | symbol | bigint ? never
    : R extends URL | URLSearchParams | Response | Request | Headers | Blob | ArrayBuffer | ArrayBufferView | Error ? never
    : R extends ViewElement | ViewDocument | ViewWindow ? never
    : R extends readonly unknown[] ? { [K in keyof R]: JsonMember<R[K]> }
    : R extends object ? { [K in keyof R]: JsonMember<R[K]> }
    : never;

/** A remote result is JSON, or top-level void; nested undefined is rejected. */
export type Jsonable<R> = R extends undefined | void ? R : JsonMember<R>;

type CheckedResult<R> = [Awaited<R>] extends [Jsonable<Awaited<R>>] ? unknown : {
  readonly __resultMustBeJson: "Return JSON values or top-level void";
};
type JsonArgs<A extends unknown[]> = A & { [K in keyof A]: JsonMember<A[K]> };

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
  /** A live page by instance id, surface pages included; `undefined` once it is gone. */
  getPage<T extends LogicPage<any> = AnyLogicPage>(instanceId: string): T | undefined;
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
export type LogicFunction<R, A extends unknown[]> = ((scope: LogicScope, ...args: A) => R | Promise<R>) & CheckedResult<R>;

/** A function evaluated in the page WebView; self-contained like `LogicFunction`. */
export type ViewFunction<R, A extends unknown[]> = ((scope: ViewScope, ...args: A) => R | Promise<R>) & CheckedResult<R>;

/**
 * A page's View — locators, function eval, and DOM scrolling. `t.app.view`
 * is whichever page is current at each call; `(await t.app.page()).view` is
 * one fixed page instance. Read elements and act on them through locators;
 * they wait for the element and retry, where a raw driver call would not.
 */
export interface TestView {
  testId(id: string): Locator;
  css(selector: string): Locator;
  /**
   * Run `fn` in the page's WebView with JSON `args` and resolve to its JSON
   * result. `fn` must be self-contained (see `ViewFunction`).
   */
  eval<R, A extends unknown[]>(fn: ViewFunction<R, A>, ...args: JsonArgs<A>): Promise<Awaited<R>>;
  /** The same with a `timeout` (see `EvalOptions`). */
  eval<R, A extends unknown[]>(options: EvalOptions, fn: ViewFunction<R, A>, ...args: JsonArgs<A>): Promise<Awaited<R>>;
  screenshot(): Promise<Screenshot>;
  /** Scroll the page DOM by a pixel delta (nearest scrollable container). */
  scroll(options?: Omit<PageScrollOptions, "page">): Promise<void>;
}

/** Native app-window input, independent of a page selector or DOM focus. */
export interface TestWindow {
  /** Pointer input; coordinates and window selection follow the native driver. */
  readonly pointer: PagePointer;
  readonly key: PageKey;
}

/**
 * Leading options of `eval(options, fn, ...args)`. Without them an eval may
 * take 10 s, clamped to the spec's remaining budget.
 */
export interface EvalOptions {
  /**
   * How long `fn` may run, in ms, for work that legitimately takes longer
   * (a download, a transcode). Clamped to the spec's remaining budget.
   */
  timeout?: number;
}

/**
 * `t.app.page()` selector: a configured page name, which must match exactly
 * one live instance, or a live instance id.
 */
export type PageSelector = { name: string; instanceId?: never } | { instanceId: string; name?: never };

/** `t.app.page()` options. */
export interface PageBindOptions {
  /**
   * How long to wait for a matching live instance, in ms (default 5000),
   * clamped to the spec's remaining budget.
   */
  timeout?: number;
}

/** What a page action that takes more than one parameter becomes. */
export interface UnaryActionsOnly {
  readonly __pageActionsTakeOnePayload: "Page actions take at most one JSON payload; change the action to take an object";
}

/**
 * A contract's actions as a test calls them: each takes at most one JSON
 * payload and resolves to its JSON result once Logic settles it.
 * Generators (streamed actions) are not callable from a test.
 */
export type PageActionCalls<A> = {
  readonly [K in keyof A as A[K] extends (...args: any[]) => any ? K : never]:
    A[K] extends (...args: any[]) => AsyncGenerator<any, any, any> | Generator<any, any, any>
      ? never
      : A[K] extends (...args: infer P) => infer R
        ? P extends [] | [unknown?]
          ? (...args: JsonArgs<P> & CheckedResult<R>) => Promise<Jsonable<Awaited<R>>>
          : UnaryActionsOnly
        : never;
};

/**
 * One live page instance, fixed when `t.app.page()` resolved: it never
 * follows navigation or a replacement, and every call on it rejects once
 * the instance is gone. Type it with the page's `PageContract`.
 */
export interface TestPage<C extends PageContract> {
  readonly instanceId: string;
  /** Configured page name. */
  readonly name: string;
  /** This instance's View. */
  readonly view: TestView;
  /** This instance's Logic `data`, as `setData` delivers it to the View. */
  data(): Promise<C["data"]>;
  /** The contract's public actions, invoked through the page's own bridge. */
  readonly actions: PageActionCalls<C["actions"]>;
}

/**
 * `t.app.logic`: the app's Logic runtime, for what the UI cannot show. Read
 * page state with `(await t.app.page()).data()`; call actions through
 * `page.actions`.
 */
export interface TestLogic {
  /**
   * Run `fn` in the app's Logic runtime with JSON `args` and resolve to its
   * JSON result. `fn` must be self-contained (see `LogicFunction`).
   */
  eval<R, A extends unknown[]>(fn: LogicFunction<R, A>, ...args: JsonArgs<A>): Promise<Awaited<R>>;
  /** The same with a `timeout` (see `EvalOptions`). */
  eval<R, A extends unknown[]>(options: EvalOptions, fn: LogicFunction<R, A>, ...args: JsonArgs<A>): Promise<Awaited<R>>;
}

/** Nav actions' wait. */
export interface NavWaitOptions {
  /**
   * `'ready'` (default) resolves after the landed page's `onReady` and
   * rejects if the app replaces it first; `'commit'` resolves once the page
   * stack changed.
   */
  waitUntil?: "commit" | "ready";
  /** Bound for `'ready'` in ms (default 15000), clamped to the spec's remaining budget. */
  timeout?: number;
}

/** `t.app.nav.to` / `redirect` / `switchTab` / `relaunch` options. */
export interface NavOptions extends NavWaitOptions {
  /** Configured page name (from lxapp.json). */
  page: string;
  /** Query forwarded to the destination page. */
  query?: Record<string, unknown>;
}

/** `t.app.nav.back` options. */
export interface NavBackOptions extends NavWaitOptions {
  /** Number of pages to pop (default 1). */
  delta?: number;
}

/** `t.app.nav`: the page stack. Actions resolve to the landed page. */
export interface TestNav {
  /** Push a page onto the stack. */
  to(options: NavOptions): Promise<PageInfo>;
  /** Replace the current page (rejects tab-bar targets). */
  redirect(options: NavOptions): Promise<PageInfo>;
  /** Switch to a configured tab page. */
  switchTab(options: NavOptions): Promise<PageInfo>;
  /** Unload every page (cached tab pages included) and open a fresh instance of a page. */
  relaunch(options: NavOptions): Promise<PageInfo>;
  back(options?: NavBackOptions): Promise<PageInfo>;
  current(): Promise<PageInfo>;
  /** Status of a configured page by name; omit `page` for the current page. */
  info(options?: PageTarget): Promise<PageInfo>;
  stack(): Promise<PageInfo[]>;
}

/**
 * One call the app made, as `route.calls()`, a scenario's `calls()` and
 * `t.app.network.calls()` list it.
 */
export interface NetworkCall {
  /**
   * Increasing within the log the call was read from, never reused: calls
   * in one millisecond differ by it. A route's calls share the host's
   * request log; a scenario counts its HTTP calls and the companion its
   * Function calls separately.
   */
  seq: number;
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
   * What answered: a scenario `rule`, a test `route`, a `mock` handler
   * (`mocks/`), the `real` backend, or the dev session's `companion` for a
   * Function no rule matched.
   */
  answeredBy: "rule" | "route" | "mock" | "real" | "companion";
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
  remove(): Promise<void>;
  /** Calls this route handled, oldest first. */
  calls(): Promise<NetworkCall[]>;
  /**
   * Resolve with the oldest call this route handled that an earlier
   * `waitForCall()` did not already return, waiting for it if there is none
   * yet. On timeout the error lists the route's recent calls. The host's
   * request log is bounded (1000 requests, 16 MiB of bodies, across apps);
   * when it dropped this route's calls before a wait read them, the wait
   * fails saying so rather than skip them, and the next one resumes after
   * the gap.
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
  /** Calls this spec's routes handled, oldest first. */
  calls(): Promise<NetworkCall[]>;
}

/** `scenario.waitForCall()` target: a rule's target as the file writes it, or its number. */
export type ScenarioCallTarget = ScenarioCallFilter;

/** `t.scenario`. */
export interface TestScenarios {
  /**
   * Put `t.app` into a product state from a scenario file (and one of its
   * variants), on top of the mock selection: `http` rules answer Logic
   * `fetch`, `function` rules go to the dev session's companion.
   * Spec-scoped: a second call replaces the first, and the spec's end
   * removes it. A failed spec reports its per-rule hits and the calls that
   * reached it.
   */
  use(definition: ScenarioInput, options?: { variant?: string }): Promise<TestScenario>;
}

/** The scenario `t.scenario.use()` installed, spec-scoped. */
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
   * On timeout the error lists the scenario's recent calls. A scenario
   * keeps its last 200 HTTP calls and the companion its last Function
   * calls; when calls were dropped before a wait read them, the wait fails
   * saying so, and the next one resumes after the gap.
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
 * `t.app.dialogs`: the dialogs the app's Logic opens during this spec, all
 * recorded. Toasts are always drawn. Modals (`lx.showModal`, `alert`,
 * `confirm`) are drawn, for the spec to tap, until it queues a modal answer;
 * from then on they are answered from the queue, never drawn, and one that
 * finds no answer fails the spec at once, naming it. Likewise action sheets.
 * An answer no dialog used fails the spec when it ends. Each spec starts
 * with nothing recorded or queued; outside a test run dialogs draw as usual.
 */
export interface TestDialogs {
  /** Toasts presented so far, oldest first. Poll it: `expect.poll(() => t.app.dialogs.toasts())`. */
  toasts(): Promise<ToastRecord[]>;
  /** Modals opened so far, with the answer each got (the user's, when drawn). */
  modals(): Promise<ModalRecord[]>;
  /** `lx.showActionSheet` calls so far, with the answer each got. */
  actionSheets(): Promise<ActionSheetRecord[]>;
  /**
   * Answer the next modal: `{ confirm: true }` confirms, `{ confirm: false }`
   * cancels. From now on the spec's modals are answered, never drawn.
   */
  answerNextModal(answer: ModalAnswer): Promise<void>;
  /**
   * Answer the next action sheet: `{ index }` picks that item, `{ cancel: true }`
   * dismisses it. From now on the spec's action sheets are answered, never drawn.
   */
  answerNextActionSheet(answer: ActionSheetAnswer): Promise<void>;
}

/**
 * `t.app`: the app under test. A saved `t.app` (or any part of it) keeps
 * reaching the app after a profile checkpoint or restore reopens it.
 */
export interface TestApp {
  /**
   * Bind one live page instance: the current page, or the one `selector`
   * names (a page kept below the current one, or one a surface shows),
   * waiting for it to open. The handle never follows navigation.
   */
  page<C extends PageContract = PageContract>(selector?: PageSelector, options?: PageBindOptions): Promise<TestPage<C>>;
  /** The View of whichever page is current at each call. */
  readonly view: TestView;
  /** Native app-window input, independent of pages and DOM focus. */
  readonly window: TestWindow;
  /** The app's Logic runtime: `eval(fn)`. */
  readonly logic: TestLogic;
  /** The page stack; actions wait for the landed page's `onReady` unless you pass `waitUntil`. */
  readonly nav: TestNav;
  /** Spec-scoped test routing of Logic `fetch`. */
  readonly network: TestNetwork;
  /** Spec-scoped test clock for the app's Logic. */
  readonly clock: TestClock;
  /** Toasts, modals and action sheets the app's Logic opens during the spec. */
  readonly dialogs: TestDialogs;
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

/**
 * Awaiting `expect(…)` or `expect.poll(…)` itself checks nothing: this member
 * makes `await expect(x)` without a matcher a type error, and it rejects at
 * run time naming the line.
 */
export interface NeedsMatcher<Hint extends string> {
  then(needsAMatcher: Hint): never;
}

/** `expect.poll(read)` matchers: they call `read` until the matcher passes. */
export interface RetryMatchers<T> extends NeedsMatcher<"expect.poll(read) checks nothing until a matcher is called: await expect.poll(read).toBe(expected)"> {
  readonly not: RetryMatchers<T>;
  toBe(expected: unknown): Promise<void>;
  toEqual(expected: unknown): Promise<void>;
  toContain(expected: unknown): Promise<void>;
  /** An array has an element equal to `expected` (as `toEqual` compares). */
  toContainEqual(expected: unknown): Promise<void>;
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
export interface LocatorMatchers extends NeedsMatcher<"expect(locator) checks nothing until a matcher is called: await expect(locator).toBeVisible()"> {
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

/**
 * What `expect(fn)` gives: `toThrow`, which calls it once. A function is not
 * a read to retry; that is `expect.poll(read)`.
 */
export interface FunctionMatchers extends NeedsMatcher<"expect(fn) checks nothing until a matcher is called: expect(fn).toThrow()"> {
  readonly not: FunctionMatchers;
  toThrow(expected?: unknown): void;
  readonly "expect(fn)": "a function is only called by toThrow; to retry a read until it passes, use expect.poll(read)";
}

/** The matchers `expect(subject)` gives for a subject of type `T`. */
export type ExpectResult<T> =
  0 extends 1 & T ? Matchers<any>
    : [T] extends [Locator] ? LocatorMatchers
      : [T] extends [PromiseLike<unknown>] ? PromiseNotAllowed
        : [T] extends [(...args: never[]) => unknown] ? FunctionMatchers
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
  /**
   * Equal (in `toEqual`, `toContainEqual` and friends) to any object with
   * every key of `sample` equal; other keys are ignored.
   */
  objectContaining(sample: Record<string, unknown>): unknown;
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
  /** One product scenario per spec; replacing it replaces HTTP and Function rules together. */
  readonly scenario: TestScenarios;
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
export interface Matchers<T> extends NeedsMatcher<"expect(value) checks nothing until a matcher is called: expect(value).toBe(expected)"> {
  readonly not: Matchers<T>;
  toBe(expected: unknown): void;
  toEqual(expected: unknown): void;
  toContain(expected: unknown): void;
  /** An array has an element equal to `expected` (as `toEqual` compares). */
  toContainEqual(expected: unknown): void;
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
