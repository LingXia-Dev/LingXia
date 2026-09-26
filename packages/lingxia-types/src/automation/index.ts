/**
 * In-process UI/runtime automation — `lx.automation()`.
 *
 * Two roots share one runtime object:
 *
 * - {@link Automation} is what app Logic may call. Selecting the calling lxapp
 *   requires the `automation` security privilege; cross-lxapp and host
 *   surfaces require `host`. A session holds one only when the privilege
 *   class is allowed for the app (the app registry, or its unrestricted
 *   default) and the native host seals a matching session grant — from a
 *   product `HostAddon`, or a devtools host such as the Runner, which grants
 *   the allowed automation privileges and nothing beyond them. Neither
 *   `lingxia dev` nor the Runner widens what app Logic is allowed.
 * - {@link HostRunAutomation} is the root of a host automation run
 *   (`lxdev test`). That context carries the host's own authority, so every
 *   selector is available without a grant, and it adds the test-run-only
 *   members: `network`, nav `waitUntil: 'ready'`, and eval call tracing.
 *
 * This mirrors the devtool (`lxdev`) automation surface as a privilege-scoped,
 * product-side API.
 */

import type { TerminalSettingsValue } from '../generated/logic.js';

// ============================ factory ============================

// ============================ errors ============================

/**
 * Stable `code`s of automation driver rejections. Anything without a more
 * specific code rejects with `E_AUTOMATION`; desktop automation uses its own
 * `E_DESKTOP_<CODE>` family. Messages are unchanged, so match on the code.
 */
export const AUTOMATION_ERROR_CODES = [
  /** Fallback for a failure with no more specific code. */
  'E_AUTOMATION',
  /** The caller lacks the `automation`/`host` privilege the call needs. */
  'E_AUTOMATION_PRIVILEGE',
  /** The target page is not the active instance: not open, or replaced. `data: { page, instanceId?, current? }`. */
  'E_PAGE_NOT_ACTIVE',
  /** The page has no WebView or current page to act on yet. */
  'E_PAGE_NOT_READY',
  /** No element matched the selector at dispatch. */
  'E_ELEMENT_NOT_FOUND',
  /** The element matched but cannot take the input (disabled, not editable, …). */
  'E_ELEMENT_NOT_INTERACTABLE',
  /** A driver wait (`waitFor`, `waitUntil: 'ready'`, …) ran out of time. */
  'E_AUTOMATION_TIMEOUT',
  /** The evaluated script threw. */
  'E_EVAL_SCRIPT',
  /** The evaluation did not settle within its `timeoutMs`. */
  'E_EVAL_TIMEOUT',
  /** Profile rollback outside an isolated run (`lxdev test --profile`), or for an lxapp the run does not isolate. */
  'E_PROFILE_NOT_ISOLATED',
  /** A test clock call needs an installed clock and none is; an app that reopened is back on real time. */
  'E_CLOCK_NOT_INSTALLED',
  /** `clock.install()` while this lxapp's Logic already runs on a test clock. */
  'E_CLOCK_INSTALLED',
] as const;

export type AutomationErrorCode = (typeof AUTOMATION_ERROR_CODES)[number];

/** A page as automation error `data` names it. */
export interface AutomationPageRef {
  /** Configured page name, else its path. */
  page?: string;
  /** The page instance the call resolved to, when it still resolves. */
  instanceId?: string;
  /** The page that was current when the call failed. */
  current?: { name: string | null; path: string; instanceId: string };
}

/**
 * Automation root as app Logic sees it (`lx.automation()` in Logic). It grants
 * no capability until one is selected. Reading a driver property never throws:
 * each driver method checks the caller and rejects with
 * `E_AUTOMATION_PRIVILEGE` when it is not allowed.
 */
export interface Automation {
  /**
   * Drive the calling lxapp. Requires the `automation` privilege. Evaluating
   * the calling Logic runtime itself (`eval`) rejects.
   */
  lxapp(): LogicLxAppDriver;
  /** Drive a specific running lxapp. Requires the `host` privilege. */
  lxapp(appid: string): LogicLxAppDriver;
  /** Cross-lxapp lifecycle and host-window capture. Requires `host`. */
  readonly lxapps: LxAppManager;
  /** The host app's browser tabs. Requires `host`. */
  readonly browser: BrowserDriver;
  /** Persisted host-shell shortcuts for deterministic test setup/assertion. Requires `host`. */
  readonly shell: ShellDriver;
  /** Simulated-device selection in a host runner. Requires `host`. */
  readonly device: DeviceDriver;
  /**
   * Session-less local-OS desktop automation (`lxdev desktop`). Windows/macOS
   * only. Gated by the `host` privilege alone, and present only in hosts
   * built with desktop automation (the Runner, or a product that enables the
   * `desktop-automation` feature); elsewhere reading it throws. It drives the
   * whole OS, beyond the app sandbox — grant `host` only to lxapps you trust
   * with that.
   */
  readonly desktop: DesktopDriver;
  /** Native terminal workspace state and pane actions. Requires `host`. */
  readonly terminal: TerminalDriver;
}

/**
 * Automation root of a host automation run — the `lx` global of an
 * `lxdev test` program (`@lingxia/types/automation-test-globals`). The run
 * carries host authority, so no selector needs a privilege grant, and the
 * selected lxapp driver adds the test-run-only members.
 */
export interface HostRunAutomation extends Automation {
  /** Drive the host's current lxapp. */
  lxapp(): LxAppDriver;
  /** Drive a specific running lxapp. */
  lxapp(appid: string): LxAppDriver;
}

// ========================== terminal tier ==========================

export type TerminalSplitDirection = 'left' | 'right' | 'up' | 'down';

export interface TerminalSurfaceRef {
  /** Stable id returned by `lx.shell.openDeclared('terminal', { key })`. */
  surface: string;
}

export interface TerminalPaneSnapshot {
  paneId: string;
  active: boolean;
  visible: boolean;
  frame: { x: number; y: number; width: number; height: number };
  grid: {
    cols: number;
    rows: number;
    generation: number;
    imageGeneration: number;
    imageCount: number;
    imagePlacementCount: number;
    defaultForeground: number;
    defaultBackground: number;
    cursorRow: number;
    cursorCol: number;
    cursorVisible: boolean;
    cursorStyle: 'block' | 'bar' | 'underline' | 'block-hollow';
  };
}

export type TerminalPaneTree =
  | { kind: 'leaf'; pane: TerminalPaneSnapshot }
  | {
    kind: 'split';
    /** `horizontal` places children left/right; `vertical` stacks them. */
    axis: 'horizontal' | 'vertical';
    children: TerminalPaneTree[];
  };

export interface TerminalTabSnapshot {
  id: string;
  active: boolean;
  activePaneId?: string;
  paneCount: number;
  tree?: TerminalPaneTree;
}

/** Semantic state published by the native terminal host after layout. */
export interface TerminalWorkspaceSnapshot {
  surfaceId: string;
  presentation: 'main' | 'aside';
  visible: boolean;
  /** Expanded to the full content area rather than its docked size. */
  maximized: boolean;
  activeTabId?: string;
  tabCount: number;
  paneCount: number;
  configGeneration: number;
  visualGeneration: number;
  config: TerminalSettingsValue;
  chrome: {
    surface: string;
    header: string;
    separator: string;
    text: string;
    textMuted: string;
    cursor: string;
    selectionBackground: string;
    selectionForeground: string;
  };
  tabs: TerminalTabSnapshot[];
}

export interface TerminalSplitOptions extends TerminalSurfaceRef {
  direction: TerminalSplitDirection;
}

/**
 * Native terminal automation. Requires the `host` privilege from app Logic
 * (a host automation run needs none) and a host with a native terminal.
 */
export interface TerminalDriver {
  snapshot(options: TerminalSurfaceRef): Promise<TerminalWorkspaceSnapshot>;
  /** Send text to the focused pane, as if typed into its PTY. */
  input(options: TerminalInputOptions): Promise<TerminalWorkspaceSnapshot>;
  split(options: TerminalSplitOptions): Promise<TerminalWorkspaceSnapshot>;
  /** Open a tab and activate it. Resolves with the snapshot that follows. */
  newTab(options: TerminalSurfaceRef): Promise<TerminalWorkspaceSnapshot>;
  /** Expand to the full content area, or return to the docked size. */
  setMaximized(options: TerminalMaximizeOptions): Promise<TerminalWorkspaceSnapshot>;
}

export interface TerminalInputOptions extends TerminalSurfaceRef {
  text: string;
}

export interface TerminalMaximizeOptions extends TerminalSurfaceRef {
  maximized: boolean;
}

// ============================ shell tier ============================

/** One ordered shortcut in the host shell's persisted Pin collection. */
export type AutomationShellPin =
  | {
    /** A host workspace shortcut; activation opens/focuses the app as main. */
    kind: 'lxapp';
    /** Installed lxapp id. This is not a page name or route. */
    key: string;
  }
  | {
    /** A website shortcut opened/focused as a main browser tab. */
    kind: 'bookmark';
    /** Persisted bookmark id. */
    key: string;
  };

export type SetAutomationShellPinOptions = AutomationShellPin & {
  pinned: boolean;
};

/**
 * Host-shell state for test setup and end-to-end assertions. This does not
 * customize an lxapp's production shell; use `lx.shell` for app behavior.
 */
export interface ShellDriver {
  /** Ordered shortcuts exactly as projected into the host sidebar. */
  pins(): Promise<AutomationShellPin[]>;
  /** Submit every current Pin exactly once, in the desired mixed order. */
  reorderPins(items: AutomationShellPin[]): Promise<AutomationShellPin[]>;
  /**
   * Idempotently persist or remove one shortcut. New Pins append to the
   * existing order; adding beyond the host limit rejects without mutation.
   * Returns the resulting complete order.
   */
  setPin(options: SetAutomationShellPinOptions): Promise<AutomationShellPin[]>;
}

// ============================ page tier ============================

/** Fields common to every page action; `page` defaults to the current page. */
export interface PageTarget {
  /** Configured page name or live instance_id; defaults to the current page. */
  page?: string;
}

export interface PageEvalOptions extends PageTarget {
  /** JavaScript expression or function body evaluated in the page WebView. */
  script: string;
  timeoutMs?: number;
}

export interface PageQueryOptions extends PageTarget {
  /** CSS selector. */
  css: string;
  /** Target the nth match (single-element mode). */
  index?: number;
  /** Return every match as `{ count, items }` instead of a single element. */
  all?: boolean;
  /** Cap text/value length (default 4096). */
  maxText?: number;
  /** Return untruncated text/value (ignores `maxText`). */
  full?: boolean;
}

export interface PageSelectorOptions extends PageTarget {
  css: string;
  /** Target the nth match. */
  index?: number;
}

export interface PageTypeOptions extends PageSelectorOptions {
  text: string;
}

export interface PageClickOptions extends PageSelectorOptions {
  /**
   * Dispatch to the element itself, without the in-viewport and hit-test
   * checks; it must still exist and be enabled. For content no scroll can
   * bring under the pointer, such as the lower part of an overflowing sheet.
   */
  force?: boolean;
}

export interface PageFillOptions extends PageTypeOptions {
  /** Write without the in-viewport check; see `PageClickOptions.force`. */
  force?: boolean;
}

export interface PagePressOptions extends PageTarget {
  /** Key name, e.g. `Enter`, `Escape`, `Tab`. */
  key: string;
  /** Focus this CSS selector before pressing; otherwise use the current focus. */
  css?: string;
  /** Target the nth selector match. Requires `css`. */
  index?: number;
}

export interface PageScrollOptions extends PageTarget {
  /** Horizontal delta in CSS pixels. */
  dx?: number;
  /** Vertical delta in CSS pixels (positive scrolls down). */
  dy?: number;
}

export interface PageScrollToOptions extends PageTarget {
  /** CSS selector of the element to reveal (first match). */
  css: string;
}

/**
 * Raw `page.waitFor` states check the first match of `css`:
 * - `attached`: at least one match.
 * - `detached`: no match (also while the page is not yet active).
 * - `visible`: the first match is rendered (a non-empty box, not
 *   `display:none`, `visibility:hidden` or `opacity:0`), in the viewport or
 *   scrolled out of it.
 * - `hidden`: the first match exists and is not rendered; no match does not
 *   satisfy it (wait for `detached`).
 * - `enabled` / `editable`: the first match exists and is enabled / editable.
 * `@lingxia/test` locators apply stricter, uniqueness-aware states.
 */
export type PageWaitState =
  | 'attached'
  | 'detached'
  | 'visible'
  | 'hidden'
  | 'enabled'
  | 'editable';

export interface PageWaitForOptions extends PageTarget {
  css: string;
  /** Condition to await (default `visible`). */
  state?: PageWaitState;
  /** Timeout in ms (default 30000, capped at 60000). */
  timeoutMs?: number;
}

/**
 * An element's viewport rectangle (viewport-relative CSS pixels).
 *  Windows LingXia WebViews use a 1:1 CSS-to-child-window rasterization scale,
 *  so combine this with that WebView's desktop bounds without multiplying by
 *  `DesktopWindowInfo.scale`.
 */
export interface ElementRect {
  left: number;
  top: number;
  width: number;
  height: number;
  right: number;
  bottom: number;
  center_x: number;
  center_y: number;
  viewport_width: number;
  viewport_height: number;
}

/** A matched element. Keys are the raw automation payload (snake_case). */
export interface PageElement {
  exists: true;
  index: number;
  /** Total number of matches for the selector. */
  count: number;
  tag: string;
  /** `<input>` type, else null. */
  type: string | null;
  id: string | null;
  name: string | null;
  role: string | null;
  aria_label: string | null;
  placeholder: string | null;
  /**
   * Rendered: a non-empty box that is not `display:none`,
   * `visibility:hidden` or `opacity:0`, wherever it is scrolled.
   */
  visible: boolean;
  /** Rendered and intersecting the viewport. */
  inViewport: boolean;
  enabled: boolean;
  editable: boolean;
  text: string;
  text_truncated: boolean;
  value: string | null;
  value_truncated: boolean;
  rect: ElementRect;
}

/** Returned when no element matches (single-element mode). */
export interface PageElementMiss {
  exists: false;
  index: number;
  count: number;
  visible: false;
  inViewport: false;
  enabled: false;
  editable: false;
}

export type PageQueryResult = PageElement | PageElementMiss;

/** Returned by `query` when `all: true`. */
export interface PageQueryAll {
  count: number;
  items: PageElement[];
}

export interface Screenshot {
  format: 'png';
  /** Base64-encoded PNG bytes. */
  base64: string;
  width: number;
  height: number;
}

/** Element-level automation of the selected lxapp's page WebViews. */
export interface PageDriver {
  /** Evaluate in the page WebView. T describes the expected JSON result; it is not runtime validation. */
  eval<T = unknown>(options: PageEvalOptions): Promise<T>;
  /** Query one element's info. */
  query(options: PageQueryOptions & { all?: false }): Promise<PageQueryResult>;
  /** Query every matching element. */
  query(options: PageQueryOptions & { all: true }): Promise<PageQueryAll>;
  query(options: PageQueryOptions): Promise<PageQueryResult | PageQueryAll>;
  /** Single dispatch; test locators provide actionability waiting. */
  click(options: PageClickOptions): Promise<void>;
  /** Type text into an element without clearing existing content. */
  type(options: PageTypeOptions): Promise<void>;
  /** Replace an element's current value. */
  fill(options: PageFillOptions): Promise<void>;
  press(options: PagePressOptions): Promise<void>;
  /** Scroll the first matching element into view. */
  scrollTo(options: PageScrollToOptions): Promise<void>;
  /** Scroll the page DOM by a pixel delta (nearest scrollable container). */
  scroll(options?: PageScrollOptions): Promise<void>;
  /** Poll until the selector reaches `state`, else reject on timeout. */
  waitFor(options: PageWaitForOptions): Promise<void>;
  screenshot(options?: PageTarget): Promise<Screenshot>;
  /** App-window pointer input at page coordinates (`lxdev lxapp page pointer`). */
  readonly pointer: PagePointer;
  /** App-window keyboard input (`lxdev lxapp page key`). */
  readonly key: PageKey;
}

// ============================ nav tier ============================

/**
 * When a nav action resolves. `'commit'` (default) resolves once the page
 * stack changed, before the landed page is ready. `'ready'` also waits for its
 * `onReady` and rejects if the page is disposed or replaced first (e.g. by the
 * app's own `lx.reLaunch`); it is available only in a host automation run
 * ({@link NavDriver}), since app Logic awaiting its own `onReady` would
 * deadlock. The `@lingxia/test` fixture's `t.app.nav` defaults to `'ready'`.
 */
export type NavWaitUntil = 'commit' | 'ready';

/** Nav wait options available to app Logic: only `'commit'`. */
export interface LogicNavWaitOptions {
  waitUntil?: 'commit';
}

/** Nav wait options of a host automation run. */
export interface NavWaitOptions {
  waitUntil?: NavWaitUntil;
  /** Bound for `waitUntil: 'ready'` in ms (default 15000, capped at 60000). */
  timeoutMs?: number;
}

interface NavTargetFields {
  /** Configured page name (from lxapp.json). */
  page: string;
  /** Query forwarded to the destination page. */
  query?: Record<string, unknown>;
}

interface NavBackFields {
  /** Number of pages to pop (default 1). */
  delta?: number;
}

export interface LogicNavOptions extends NavTargetFields, LogicNavWaitOptions {}
export interface LogicNavBackOptions extends NavBackFields, LogicNavWaitOptions {}
export interface NavOptions extends NavTargetFields, NavWaitOptions {}
export interface NavBackOptions extends NavBackFields, NavWaitOptions {}

/** A page's runtime position. */
export interface PageInfo {
  path: string;
  /** Configured page name, if the path maps to one. */
  name: string | null;
  /**
   * The page instance. Navigation that replaces a page (even with the same
   * path) gives it a new id. `null` when no live instance backs a stack
   * entry; absent on hosts that predate it.
   */
  instanceId?: string | null;
  current: boolean;
  inStack: boolean;
  /** Whether the page has dispatched `onReady` (what `waitUntil: 'ready'` awaits). */
  ready: boolean;
  /** Whether the page currently has an attached WebView; precedes `ready`. */
  webviewAttached: boolean;
}

/**
 * Page-stack navigation for the selected lxapp, as app Logic sees it. Action
 * verbs take a configured page name (`redirect` rejects a tab-bar page);
 * `back` pops; `current`/`stack` read. Unlike the JS `lx.navigateTo` family
 * this returns the landed page. It resolves once the stack changed, before the
 * destination is ready.
 */
export interface LogicNavDriver {
  /** Push a page onto the stack. */
  to(options: LogicNavOptions): Promise<PageInfo>;
  /** Replace the current page (rejects tab-bar targets). */
  redirect(options: LogicNavOptions): Promise<PageInfo>;
  /** Switch to a configured tab page. */
  switchTab(options: LogicNavOptions): Promise<PageInfo>;
  /** Unload every page (cached tab pages included) and open a fresh instance of a page. */
  relaunch(options: LogicNavOptions): Promise<PageInfo>;
  back(options?: LogicNavBackOptions): Promise<PageInfo>;
  current(): Promise<PageInfo>;
  /** Status of a configured page by name; omit `page` for the current page. */
  info(options?: PageTarget): Promise<PageInfo>;
  stack(): Promise<PageInfo[]>;
}

/**
 * Page-stack navigation in a host automation run. Same verbs as
 * {@link LogicNavDriver}; pass `waitUntil: 'ready'` to also wait for the
 * landed page's `onReady`.
 */
export interface NavDriver extends LogicNavDriver {
  to(options: NavOptions): Promise<PageInfo>;
  redirect(options: NavOptions): Promise<PageInfo>;
  switchTab(options: NavOptions): Promise<PageInfo>;
  relaunch(options: NavOptions): Promise<PageInfo>;
  back(options?: NavBackOptions): Promise<PageInfo>;
}

// ============================ lxapp driver ============================

export interface LxAppSummary {
  appid: string;
  currentPage: string | null;
}

export interface LxAppPageConfig {
  name: string;
  path: string;
}

/**
 * Envelope `eval` resolves to with `captureCalls: true`.
 *
 * @internal Report plumbing for the test runner; the `@lingxia/test` fixture
 * unwraps it, and specs never see it.
 */
export interface LxAppEvalTrace<T = unknown> {
  __lxEval: 1;
  value: T;
  calls: string[];
}

/** Logic-runtime eval options available to app Logic. */
export interface LogicLxAppEvalOptions {
  /** JavaScript expression or function body run in the selected Logic runtime. */
  script: string;
  timeoutMs?: number;
}

/** Logic-runtime eval options of a host automation run. */
export interface LxAppEvalOptions extends LogicLxAppEvalOptions {
  /**
   * Resolve to `{ value, calls }`, where `calls` lists the `lx.*` members the
   * script reached, instead of the bare value.
   *
   * @internal The test runner sets this so a report can tell a capability a
   * spec exercised from one it merely declared. Only the evaluated script is
   * observed — the lxapp's own concurrent work is not.
   */
  captureCalls?: boolean;
}

/** Shell admission class. Content `lx.surface.watchContext` uses `compact` | `regular`. */
export type SurfaceLayoutSizeClass = 'compact' | 'medium' | 'expanded';
export type SurfaceLayoutSwitcherForm = 'none' | 'sidebar' | 'rail';
export type SurfaceLayoutSplitForm = 'none' | 'split' | 'collapsible' | 'fullScreen';
export type SurfaceLayoutEdge = 'left' | 'right' | 'top' | 'bottom';

export type SurfaceLayoutIcon =
  | { source: 'builtIn'; name: string }
  | { source: 'resource'; uri: string }
  | { source: 'providerAsset'; provider: string; key: string };

/** Resolved content identity carried by one host switcher item. */
export type SurfaceSwitcherContent =
  | { kind: 'lxapp'; appId: string }
  | { kind: 'page'; appId: string }
  | { kind: 'browser' }
  | { kind: 'native'; capability: string };

export interface SurfaceSwitcherItem {
  surfaceId: string;
  content: SurfaceSwitcherContent;
  title?: string;
  icon?: SurfaceLayoutIcon;
  active: boolean;
  root: boolean;
  closable: boolean;
  renameable: boolean;
  titleOverridden: boolean;
}

/** Ordered semantic model behind the host's main-surface switcher. */
export interface SurfaceSwitcherSnapshot {
  /** Monotonically changes when the host surface model changes. */
  revision: number;
  rootSurfaceId?: string;
  activeSurfaceId?: string;
  items: SurfaceSwitcherItem[];
}

export interface SurfaceLayoutAside {
  id: string;
  edge?: SurfaceLayoutEdge;
  preferredSize?: number;
}

export interface SurfaceLayoutAsideSlot {
  kind: 'lxapp' | 'browser' | 'native';
  edge?: SurfaceLayoutEdge;
  /** Stable tab order within this content-kind region. */
  children: string[];
  activeChild?: string;
  visible: boolean;
  overlay: boolean;
}

export type SurfaceLayoutFloatAnchor =
  | { to: 'screen' }
  | { to: 'surface'; surfaceId: string };

export interface SurfaceLayoutFloat {
  id: string;
  anchor: SurfaceLayoutFloatAnchor;
  dismiss: 'tapOutside' | 'manual';
  modal: boolean;
  closeButton: boolean;
}

/** Id-only surface tree emitted by the shared layout core. */
export type SurfaceLayoutTree =
  | { kind: 'leaf'; surfaceId: string }
  | {
    kind: 'split';
    axis: 'horizontal' | 'vertical';
    children: SurfaceLayoutTree[];
    weights: number[];
  }
  | { kind: 'tabs'; activeId: string; children: string[] }
  | { kind: 'freeform'; surfaceId: string };

/**
 * Read-only automation snapshot of the exact render plan consumed by the host
 * skin. Use this for end-to-end assertions; production lxapp behavior should
 * depend on `SurfaceHandle`, not host layout internals.
 */
export interface SurfaceLayoutSnapshot {
  sizeClass: SurfaceLayoutSizeClass;
  bottomOwner: 'app';
  switcherForm: SurfaceLayoutSwitcherForm;
  splitForm: SurfaceLayoutSplitForm;
  mains: string[];
  activeMainId?: string;
  mainSwitcher: SurfaceSwitcherSnapshot;
  asides: SurfaceLayoutAside[];
  asideSlots: SurfaceLayoutAsideSlot[];
  floats: SurfaceLayoutFloat[];
  tree?: SurfaceLayoutTree;
}

// ======================= test network routing =======================

/**
 * Which Logic `fetch` requests a route handles: a URL glob over the whole URL
 * (`**` any characters, `*` any except `/`, `{a,b}` alternatives; `?` is
 * literal), a `RegExp` searched like `RegExp.test` (Rust regex syntax: no
 * lookaround or backreferences), or either plus a method and a match budget.
 */
export type NetworkRoutePattern =
  | string
  | RegExp
  | {
    url: string | RegExp;
    /** Case-insensitive HTTP method; omit or `'*'` for any. */
    method?: string;
    /** Remove the route after this many matched requests. */
    times?: number;
  };

type NetworkRouteFulfillKey =
  | 'status'
  | 'statusText'
  | 'headers'
  | 'contentType'
  | 'body'
  | 'json'
  | 'delay';
type NetworkRouteForbid<K extends string> = Partial<Record<K, never>>;

interface NetworkRouteFulfillFields {
  /** 200..=599. Default 200. */
  status?: number;
  statusText?: string;
  headers?: Record<string, string>;
  /** Sets `content-type` unless `headers` already has one. */
  contentType?: string;
  /**
   * Milliseconds to wait before the response resolves, 0..=30000; an
   * `AbortSignal` passed to `fetch` still rejects it early.
   */
  delay?: number;
}

/**
 * Answer the request without touching the network. The body is either `body`
 * (text or bytes, sent verbatim) or `json` (serialized, implies
 * `content-type: application/json`), never both.
 */
export type NetworkRouteFulfill = NetworkRouteFulfillFields &
  NetworkRouteForbid<'abort' | 'continue' | 'patchJson' | 'hang' | 'sse' | 'sequence'> &
  (
    | { body?: string | ArrayBuffer | Uint8Array; json?: never }
    | { json: unknown; body?: never }
  );

/**
 * Reject the `fetch` like a transport failure (`TypeError: fetch failed`).
 * `'failed'` is the only failure the wrapper emulates.
 */
export type NetworkRouteAbort = { abort: 'failed' } &
  NetworkRouteForbid<NetworkRouteFulfillKey | 'continue' | 'patchJson' | 'hang' | 'sse' | 'sequence'>;

/**
 * Let the request reach the network, shadowing older matching routes. With
 * `patchJson`, the real response's JSON body is rewritten host-side by that
 * RFC 7396 merge patch: object keys merge, `null` deletes a key, and any
 * other value (arrays included) replaces what it patches. Status and headers
 * stay the real ones. A body that is not JSON rejects the `fetch` with a
 * `TypeError` naming the URL; an empty body passes through. The patched
 * body is re-serialized, so object keys come back sorted.
 */
export type NetworkRouteContinue = { continue: true; patchJson?: unknown } &
  NetworkRouteForbid<NetworkRouteFulfillKey | 'abort' | 'hang' | 'sse' | 'sequence'>;

/**
 * Never answer: the `fetch` stays pending until the route is removed (the
 * spec ends, `unroute()`, or the run ends), then rejects like a transport
 * failure. An `AbortSignal` passed to `fetch` still rejects it early. For
 * loading states and client-side timeouts. `times` limits which requests are
 * held, not how long.
 */
export type NetworkRouteHang = { hang: true } &
  NetworkRouteForbid<NetworkRouteFulfillKey | 'abort' | 'continue' | 'patchJson' | 'sse' | 'sequence'>;

/**
 * One item of an SSE answer, played in order: an event (`data` that is not a
 * string is sent as JSON; multi-line data becomes several `data:` lines), a
 * `comment` line, a pause of `delayMs` (0..=30000), or `drop`, which closes
 * the stream as a server dropping the connection would and must come last.
 */
export type NetworkSseItem =
  | { event?: string; data: unknown; id?: string; retry?: number }
  | { comment: string }
  | { delayMs: number }
  | { drop: true };

/**
 * Answer with a `text/event-stream` (status 200) that plays `sse`. Without a
 * final `{ drop: true }` the stream stays open after its last item, like a
 * live server, until the route is removed. `Rong.SSE` in Logic receives the
 * events and, after a drop, reconnects with `Last-Event-ID`, which the next
 * answer of a `sequence` can serve. `delay` (0..=30000 ms) holds the response
 * before it opens.
 */
export type NetworkRouteSse = {
  sse: NetworkSseItem[];
  headers?: Record<string, string>;
  delay?: number;
} & NetworkRouteForbid<
  'status' | 'statusText' | 'contentType' | 'body' | 'json' | 'abort' | 'continue' | 'patchJson' | 'hang' | 'sequence'
>;

/** Exactly one of fulfill, abort, continue, hang, or sse. */
export type NetworkRouteAnswer =
  | NetworkRouteFulfill
  | NetworkRouteAbort
  | NetworkRouteContinue
  | NetworkRouteHang
  | NetworkRouteSse;

/**
 * Answers served in call order: the first matched request gets the first
 * answer, and the last answer repeats. Answers take text or `json` bodies,
 * not bytes. Combine with a pattern's `times` to stop matching.
 */
export type NetworkRouteSequence = { sequence: NetworkRouteAnswer[] } &
  NetworkRouteForbid<NetworkRouteFulfillKey | 'abort' | 'continue' | 'patchJson' | 'hang' | 'sse'>;

/**
 * One answer, or a `sequence` of them. String values may contain
 * relative-time templates rendered each time the answer is served:
 * `{{now}}`, `{{now-2h}}`, `{{now+30m}}` (ISO-8601 UTC, like
 * `Date.prototype.toISOString`; units `ms`, `s`, `m`, `h`, `d`) and
 * `{{nowMs}}` (epoch milliseconds, as text).
 */
export type NetworkRouteHandler = NetworkRouteAnswer | NetworkRouteSequence;

/** `bodyBase64` answers: bytes in a file, instead of `body` or `json`. */
export type ScenarioHttpBinary = NetworkRouteFulfillFields & { bodyBase64: string } &
  NetworkRouteForbid<'body' | 'json'>;

/**
 * A `match` value: objects match when every listed key is present and
 * matches (other keys are ignored), arrays match element by element with the
 * same length, a `"/regex/flags"` string (flags among `imsu`) matches a
 * scalar whose text it finds, and any other value matches an equal value.
 */
export type ScenarioMatchValue = unknown;

/** Fields every rule may have. */
type ScenarioRuleCommon = {
  /** Answer this many matching calls, then stand aside. */
  times?: number;
  /** Free text, ignored. */
  note?: string;
};

/**
 * A rule for the app's Logic `fetch` / `Rong.SSE`: `http` is
 * `"METHOD url-glob"` (`*` for any method; the URL may be `/regex/flags`),
 * and the answer is a route handler (or `bodyBase64`).
 */
export type ScenarioHttpRule = ScenarioRuleCommon & {
  http: string;
  function?: never;
  /** Match the request's JSON body. */
  match?: { json: ScenarioMatchValue };
} & (NetworkRouteHandler | ScenarioHttpBinary);

/** How a `function` rule answers one call. */
export type ScenarioFunctionAnswer = { delay?: number } & (
  | { result: unknown; error?: never; fault?: never }
  | { error: { code: string; [key: string]: unknown }; result?: never; fault?: never }
  | { fault: 'notRun' | 'unknown'; result?: never; error?: never }
);

/**
 * A rule for a Worker Function call, answered by the dev session's
 * companion: `result`, a declared `error`, or a transport `fault`.
 */
export type ScenarioFunctionRule = ScenarioRuleCommon & {
  function: string;
  http?: never;
  /** Match the call's arguments. */
  match?: { args: ScenarioMatchValue };
} & (
  | (ScenarioFunctionAnswer & { sequence?: never })
  | { sequence: ScenarioFunctionAnswer[]; result?: never; error?: never; fault?: never; delay?: never }
);

export type ScenarioRule = ScenarioHttpRule | ScenarioFunctionRule;

/**
 * A scenario file: rules tried in file order (the first match answers;
 * nothing matched goes to the real backend), and named variants whose rules
 * go before the shared ones. Unknown fields are rejected.
 */
export type ScenarioDefinition = {
  $schema?: string;
  name?: string;
  description?: string;
  rules?: ScenarioRule[];
  variants?: Record<string, { description?: string; rules: ScenarioRule[] }>;
};

/**
 * What `scenario()` accepts: a typed definition, or an imported JSON file,
 * whose literal types TypeScript widens. Either is validated by the host.
 */
export type ScenarioInput =
  | ScenarioDefinition
  | { readonly rules: readonly object[]; readonly [key: string]: unknown }
  | { readonly variants: { readonly [variant: string]: { readonly rules: readonly object[] } }; readonly [key: string]: unknown };

/** One rule of an installed scenario. */
export interface ScenarioRuleInfo {
  /** 1-based, in precedence order (the variant's rules first). */
  index: number;
  /** `GET **\/wifi/main`, `function orders.submit`. */
  target: string;
  kind: 'http' | 'function';
  /** Calls it answered (`http` rules); `null` for `function` rules. */
  hits: number | null;
}

/** One call that reached a scenario. */
export interface ScenarioCall {
  /** Epoch milliseconds. */
  time: number;
  kind: 'http' | 'function';
  /** `http`: upper-case method and URL. */
  method?: string;
  url?: string;
  /** `http`: the request body, parsed when it is JSON. */
  body?: unknown;
  /** `http`: the answered status, `null` for abort/continue/hang or no answer. */
  status?: number | null;
  /** `function`: its name, arguments, and `result`/`error`/`fault`/`default`. */
  function?: string;
  args?: unknown;
  outcome?: string;
  /** The rule that answered, or `null` when none did. */
  rule: number | null;
  /** `rule 2 (name:variant)`, `real`, or `companion default`. */
  answeredBy: string;
  /** Why no rule matched, when rules targeted the call. */
  noMatch?: string;
}

/** `calls()` filter: one rule's target, or its number. */
export type ScenarioCallFilter = { http: string } | { function: string } | { rule: number };

/** Handle returned by `scenario()`. */
export interface Scenario {
  readonly name: string | null;
  readonly variant: string | null;
  /** Its rules with their hit counts, read when accessed. */
  readonly rules: ScenarioRuleInfo[];
  /** Calls that reached the scenario since it was installed, oldest first. */
  calls(filter?: ScenarioCallFilter): Promise<ScenarioCall[]>;
  /** Remove the scenario; resolves how many rules were still installed. */
  unroute(): Promise<number>;
}

/** One request a route handled, as the app sent it. */
export interface NetworkRouteRequest {
  routeId: number;
  /** The route's glob or `/source/flags`. */
  pattern: string;
  /** Upper-case method. */
  method: string;
  url: string;
  /** Request headers with lower-case names. */
  headers: Record<string, string>;
  /**
   * Request body as UTF-8 text, cut to 64 KiB. `null` when there is none or
   * it cannot be read without consuming it: a stream, `Blob`, `FormData`, or
   * the body of a `Request` object passed as `fetch`'s first argument.
   */
  body: string | null;
  /** `body` was cut to the 64 KiB (65536-byte) limit. */
  bodyTruncated: boolean;
  /**
   * `continue` also covers a `patchJson` pass-through; `fulfill` also covers
   * an `sse` answer.
   */
  action: 'fulfill' | 'abort' | 'continue' | 'hang';
  /** Fulfilled status (200 for `sse`); `null` for abort/continue/hang. */
  status: number | null;
  /** Epoch milliseconds. */
  timestamp: number;
}

export interface NetworkRoute {
  readonly id: number;
  readonly pattern: string;
  /** Resolves `false` when the route already expired or was removed. */
  unroute(): Promise<boolean>;
  /** Requests this route handled, oldest first. */
  requests(): Promise<NetworkRouteRequest[]>;
}

/**
 * One Logic `fetch` response recorded by `NetworkDriver.captureResponses()`.
 * Headers are never recorded, and `url` keeps only scheme, host and path.
 *
 * @internal Test-runner plumbing for `lxdev test --openapi`.
 */
export interface NetworkResponseRecord {
  /** Increasing across the run; pass it as `since` to read only newer ones. */
  seq: number;
  /** Upper-case method. */
  method: string;
  /** `scheme://host[:port]/path`: userinfo, query and fragment removed. */
  url: string;
  /**
   * `route`: a route fulfilled it. `patch`: the real server answered and a
   * route merge-patched the body. `network`: the real server answered.
   */
  source: 'route' | 'patch' | 'network';
  /** The route's pattern for `route` and `patch`. */
  pattern: string | null;
  status: number;
  contentType: string | null;
  /** JSON body text, cut to `maxBodyBytes`; `null` for other content types. */
  body: string | null;
  bodyTruncated: boolean;
  /** Epoch milliseconds. */
  timestamp: number;
}

/** @internal `NetworkDriver.captureResponses()` options. */
export interface NetworkCaptureOptions {
  /** Body bytes kept per response, 1..=1048576. Default 262144. */
  maxBodyBytes?: number;
}

/**
 * Test-only routing of the selected lxapp's Logic `fetch` and `Rong.SSE`.
 * Available only in a host automation run (`lxdev test`); every route is
 * removed when that run ends. The newest matching route handles a request; unmatched requests are
 * untouched. A fulfillment never answers a host the app's network policy
 * refuses — the request goes to the real `fetch`, which rejects it.
 * WebView page requests are not routed.
 */
export interface NetworkDriver {
  route(pattern: NetworkRoutePattern, handler: NetworkRouteHandler): Promise<NetworkRoute>;
  /** Remove every route this run installed for the app; resolves the count. */
  unrouteAll(): Promise<number>;
  /**
   * Run-scoped: requests any route of this automation run handled for the
   * app, oldest first, across every spec. `t.app.network.requests()` narrows
   * this to the current spec.
   */
  requests(): Promise<NetworkRouteRequest[]>;
  /**
   * Record the app's Logic `fetch` responses — status, content type and
   * JSON body, never headers — until the run ends. A JSON response is read
   * once and handed to the app as an equivalent buffered `Response`.
   *
   * @internal Test-runner plumbing for `lxdev test --openapi`.
   */
  captureResponses(options?: NetworkCaptureOptions): Promise<void>;
  /**
   * Responses captured for the app in this run, oldest first; `since` skips
   * records up to that `seq`. At most 500 records and 8 MiB of bodies are
   * kept; the oldest go first.
   *
   * @internal Test-runner plumbing for `lxdev test --openapi`.
   */
  responses(options?: { since?: number }): Promise<NetworkResponseRecord[]>;
}

/**
 * Checkpoint and roll back the isolated data profile of a host automation run
 * (`lxdev test --profile`). Each call rejects with `E_PROFILE_NOT_ISOLATED`
 * unless the selected lxapp runs on this run's profile, so it can never touch
 * the app's real data.
 *
 * `checkpoint` and `restore` close the app, copy or swap its closed data and
 * reopen it at its initial page: a driver selected before the call is bound to
 * the closed instance, so select the lxapp again afterwards.
 */
export interface ProfileDriver {
  /** Snapshot the profile; resolves the checkpoint id. */
  checkpoint(): Promise<string>;
  /**
   * Replace the profile with checkpoint `id`. With `keep`, the storage keys
   * those globs match keep their current values across the rollback.
   */
  restore(id: string, options?: ProfileRestoreOptions): Promise<ProfileRestoreResult>;
  /** Discard checkpoint `id`; the app keeps running. */
  drop(id: string): Promise<void>;
}

/** `ProfileDriver.restore` options. */
export interface ProfileRestoreOptions {
  /**
   * `lx.getStorage()` keys whose current state survives the rollback: globs
   * over the whole key, `*` any run of characters (dots included), `?` one
   * character, anything else literal (at most 64 patterns). A matching key
   * keeps its current value, one added since the checkpoint stays, and one
   * deleted since stays deleted. Files (`lx://userdata`, …) always roll back.
   */
  keep?: string[];
}

export interface ProfileRestoreResult {
  /** Kept keys that currently exist and were carried into the restored data. */
  kept: string[];
}

// ============================ test clock ============================

/** A time for the test clock: epoch milliseconds, a `Date.parse` string, or a `Date`. */
export type ClockTime = number | string | Date;

export interface ClockInstallOptions {
  /** What Logic's `Date` reads at install. Default: the real current time. */
  now?: ClockTime;
}

export interface ClockRunAllOptions {
  /** Reject once this many timers fired and more are pending. Default 1000. */
  maxTimers?: number;
}

/** Logic's test time after a clock call. */
export interface ClockState {
  /** Logic's `Date.now()` afterwards. */
  now: number;
  /** Timers still scheduled on the test clock. */
  pending: number;
}

/** What `tick` / `runAll` did. */
export interface ClockAdvance extends ClockState {
  /** Timers fired by this call. */
  fired: number;
}

export interface ClockUninstallResult {
  /** `false` when no clock was installed (never, or the app reopened since). */
  uninstalled: boolean;
  /** Timers still pending on the test clock; they are discarded, never fired. */
  dropped: number;
}

/**
 * Test clock for the selected lxapp's Logic context, scoped to the host
 * automation run (`lxdev test`); outside one every call rejects with
 * `E_AUTOMATION`. While installed, Logic's `Date` (no-argument
 * construction, `Date()`, `Date.now()`), `setTimeout` / `setInterval` and
 * their `clear*`, and `performance.now()` read test time, and those timers
 * fire only from `tick` / `runAll`, in time order. The page WebView, native
 * work, real network requests, test route `delay` / `hang` and timers started
 * before `install` keep real time. The run's end, or the app reopening,
 * returns the app to real time.
 */
export interface ClockDriver {
  /**
   * Put Logic on test time; resolves its state (`pending` is 0: timers
   * started before `install` keep real time). Rejects with
   * `E_CLOCK_INSTALLED` when a clock is already installed.
   */
  install(options?: ClockInstallOptions): Promise<ClockState>;
  /**
   * Advance by `ms`, firing each timer due on the way at its own time. After
   * every firing, promise chains the callback started settle before the next
   * timer fires. Rejects with `E_CLOCK_NOT_INSTALLED` without a clock.
   */
  tick(ms: number): Promise<ClockAdvance>;
  /**
   * Fire timers, including those they schedule, until none is left; rejects
   * once `maxTimers` fired (an interval never lets the queue empty).
   */
  runAll(options?: ClockRunAllOptions): Promise<ClockAdvance>;
  /**
   * Change what `Date` reads without firing timers; timer due times and
   * `performance.now()` are unaffected. Resolves the new state.
   */
  setSystemTime(time: ClockTime): Promise<ClockState>;
  /** Return Logic to real time; pending test timers are dropped. */
  uninstall(): Promise<ClockUninstallResult>;
}

/** Capability for one selected running lxapp, as app Logic sees it. */
export interface LogicLxAppDriver {
  readonly page: PageDriver;
  readonly nav: LogicNavDriver;
  /** Complete runtime snapshot of the selected lxapp. */
  info(): Promise<LxAppRuntimeInfo>;
  /** Configured pages of the selected lxapp. */
  pages(): Promise<LxAppPageConfig[]>;
  /** Authoritative host surface render plan, for end-to-end assertions. */
  surfaceLayout(): Promise<SurfaceLayoutSnapshot>;
  /**
   * Logic-runtime eval of another lxapp; evaluating the calling Logic runtime
   * itself rejects. `T` describes the expected JSON result; it is not
   * runtime validation.
   */
  eval<T = unknown>(options: LogicLxAppEvalOptions): Promise<T>;
}

/**
 * Capability for one selected running lxapp in a host automation run
 * (`HostRunAutomation.lxapp()`): the Logic driver plus test-run-only members.
 */
export interface LxAppDriver extends LogicLxAppDriver {
  readonly nav: NavDriver;
  /**
   * Test-only Logic `fetch` routing, scoped to the host automation run.
   *
   * @remarks Reading the property always works, but every call rejects with
   * `E_AUTOMATION` outside a host test run (`lxdev test`) — from app Logic,
   * or in a host built without the automation runtime.
   */
  readonly network: NetworkDriver;
  /**
   * Install a scenario file (with one of its variants) for the host run:
   * `http` rules answer Logic `fetch`, `function` rules go to the dev
   * session's companion. Validated as a whole; it replaces the scenario the
   * run installed for this app before, and the run's end removes it. Routes
   * added with `network.route()` take precedence over it.
   *
   * @remarks Rejects with `E_AUTOMATION` outside a host test run.
   */
  scenario(definition: ScenarioInput, variant?: string): Promise<Scenario>;
  /**
   * Isolated data profile rollback, scoped to the host automation run.
   *
   * @remarks Reading the property always works; every call rejects with
   * `E_PROFILE_NOT_ISOLATED` outside an isolated run.
   */
  readonly profile: ProfileDriver;
  /**
   * Test clock for the selected lxapp's Logic, scoped to the host automation
   * run.
   *
   * @remarks Reading the property always works, but every call rejects with
   * `E_AUTOMATION` outside a host test run (`lxdev test`).
   */
  readonly clock: ClockDriver;
  /** @internal Test-runner plumbing: resolves to the call-trace envelope. */
  eval<T = unknown>(options: LxAppEvalOptions & { captureCalls: true }): Promise<LxAppEvalTrace<T>>;
  /**
   * Logic-runtime eval; evaluating the calling Logic runtime itself rejects.
   * `T` describes the expected JSON result; it is not runtime validation.
   */
  eval<T = unknown>(options: LxAppEvalOptions & { captureCalls?: false }): Promise<T>;
  eval<T = unknown>(options: LxAppEvalOptions): Promise<T | LxAppEvalTrace<T>>;
}

// ======================= lxapp manager (host) =======================

/** One configured page in a runtime info payload. */
export interface LxAppPageEntry {
  name: string;
  path: string;
}

/** Runtime snapshot of a running lxapp (raw payload, snake_case keys). */
export interface LxAppRuntimeInfo {
  appid: string;
  app_name: string;
  version: string;
  release_type: string;
  session_id: number;
  status: string;
  /** True while the lxapp holds a place in the host's page stack. */
  in_stack: boolean;
  is_home: boolean;
  current_page: string | null;
  initial_route: string;
  pages_count: number;
  page_entries: LxAppPageEntry[];
  page_stack: string[];
  tab_bar: LxAppRuntimeTabBarInfo | null;
  navigation_bar: LxAppRuntimeNavigationBarInfo | null;
  lxapp_dir: string;
  data_dir: string;
  cache_dir: string;
}

/** Runtime NavigationBar state exposed for deterministic host-level assertions. */
export interface LxAppRuntimeNavigationBarInfo {
  title: string;
  home_button: 'auto' | 'hidden';
  home_button_visible: boolean;
  runtime_style: {
    background_color: string | null;
    foreground_color: string | null;
    divider_color: string | null;
  };
}

/** Runtime TabBar state exposed for deterministic host-level assertions. */
export interface LxAppRuntimeTabBarInfo {
  presentation: 'standard' | 'immersive';
  visibility: 'auto' | 'visible' | 'hidden';
  route_visible: boolean;
  effective_visible: boolean;
  selected_index: number;
  items: Array<{
    index: number;
    text: string | null;
    icon_path: string | null;
    badge: string | null;
    red_dot: boolean;
  }>;
}

/** Selects a running lxapp by id; defaults to the current app. */
export interface LxAppRef {
  /** LxApp id, or `"current"` (default). */
  app?: string;
}

export interface LxAppOpenOptions {
  appid: string;
  /** Initial page/path. */
  path?: string;
  channel?: 'release' | 'draft';
}

export interface LxAppOpenResult {
  appid: string;
  path: string;
}

export interface ApplinkOptions {
  /** `https://` AppLink URL. */
  url: string;
}

export interface ApplinkResult {
  accepted: boolean;
  code: number;
}

/**
 * Cross-lxapp lifecycle and host-window access. Requires the `host` privilege
 * from app Logic; a host automation run needs no grant.
 *
 * `close`, `restart`, and `uninstall` reject when they target the calling app
 * itself. Use `lx.host.exit()` to self-exit.
 */
export interface LxAppManager {
  list(): Promise<LxAppRuntimeInfo[]>;
  current(): Promise<LxAppSummary>;
  open(options: LxAppOpenOptions): Promise<LxAppOpenResult>;
  /**
   * Inject an App Link (`lxdev host applink`). Warm `onShow`, `scene === 8003`.
   * Host must match `appLinks.hosts`. Resolves when accepted, not when
   * navigation finishes.
   */
  applink(options: ApplinkOptions): Promise<ApplinkResult>;
  close(options?: LxAppRef): Promise<void>;
  restart(options?: LxAppRef): Promise<void>;
  uninstall(options?: LxAppRef): Promise<void>;
  /** Enumerate the host app's top-level windows (`lxdev lxapp windows`). */
  windows(): Promise<AppWindowInfo[]>;
  /** PNG of a host app window (`lxdev lxapp screenshot`); defaults to the
   *  session's focused/main window. */
  screenshot(options?: WindowRef): Promise<Screenshot>;
}

// ======================= device (host) =======================

/** A device preset the runner can simulate (raw payload, snake_case-free). */
export interface DeviceEntry {
  id: string;
  name: string;
  /** Form-factor group: `phone` | `tablet` | `desktop`. */
  group: string;
  /** Logical width in points. */
  width: number;
  /** Logical height in points. */
  height: number;
  /** True for the currently selected device. */
  current: boolean;
}

/** The active device selection. */
export interface DeviceState {
  id: string;
  name: string;
  group: string;
  /** Logical width in points (accounts for orientation). */
  width: number;
  height: number;
  /** True when rotated to landscape. */
  landscape: boolean;
  /** Simulated system appearance of the device screen. */
  appearance: "system" | "light" | "dark";
  /** Whether the simulated host capsule is enabled (the setting, not
   * per-device visibility: desktop presets draw no phone chrome either way). */
  capsule: boolean;
}

export interface DeviceSetOptions {
  /** Device preset id (see `list()`); omit to keep the current device. */
  id?: string;
  /** Force landscape (`true`) or portrait (`false`); omit to use the
   * runner's normal device-selection behavior. */
  landscape?: boolean;
  /** Simulated appearance; omit to keep. */
  appearance?: "system" | "light" | "dark";
  /** Show (`true`) or hide (`false`) the simulated host capsule; omit to keep. */
  capsule?: boolean;
}

/**
 * Simulated-device control (`lxdev runner`). Only functional in a host
 * runner that registered a device controller; otherwise every call rejects.
 */
export interface DeviceDriver {
  list(): Promise<DeviceEntry[]>;
  get(): Promise<DeviceState>;
  set(options: DeviceSetOptions): Promise<DeviceState>;
}

// ======================= browser (host) =======================

/** Selects a browser tab by id; defaults to the current tab. */
export interface BrowserTabRef {
  /** Tab id, or `"current"` (default). */
  tab?: string;
}

export interface BrowserOpenOptions {
  url: string;
  /** Reuse an existing tab id instead of opening a new one. */
  tab?: string;
}

export interface BrowserEvalOptions extends BrowserTabRef {
  js: string;
  /** After the eval, wait for a navigation it triggers. */
  waitNavigation?: boolean;
  /** With `waitNavigation`: wait until the load completes. */
  complete?: boolean;
  timeoutMs?: number;
}

export interface BrowserQueryOptions extends BrowserTabRef {
  css: string;
  maxText?: number;
  /** Return untruncated text/value (ignores `maxText`). */
  full?: boolean;
}

export interface BrowserSelectorOptions extends BrowserTabRef {
  css: string;
}

/** `click` / `press` also carry the navigation-sync flags. */
export interface BrowserClickOptions extends BrowserTabRef {
  css: string;
  /** After the click, wait for a navigation it triggers. */
  waitNavigation?: boolean;
  complete?: boolean;
  timeoutMs?: number;
}

export interface BrowserTypeOptions extends BrowserSelectorOptions {
  text: string;
}

export interface BrowserPressOptions extends BrowserTabRef {
  key: string;
  /** After the press, wait for a navigation it triggers. */
  waitNavigation?: boolean;
  complete?: boolean;
  timeoutMs?: number;
}

export interface BrowserScrollOptions extends BrowserTabRef {
  dx?: number;
  dy?: number;
}

/**
 * A browser wait condition — pass **exactly one** of the condition fields.
 * `navigation` may add `complete` to wait for load completion.
 */
interface BrowserWaitFields extends BrowserTabRef {
  /** Wait for page load. */
  loaded?: true;
  /** Wait for a selector to exist. */
  exists?: string;
  /** Wait for a selector to be visible. */
  visible?: string;
  /** Wait for a selector to be hidden. */
  hidden?: string;
  /** Wait for a selector to be editable. */
  editable?: string;
  /** Wait until a JS expression is truthy. */
  js?: string;
  /** Wait for the URL to equal this. */
  url?: string;
  /** Wait for the URL to contain this. */
  urlContains?: string;
  /** Wait for a navigation. */
  navigation?: true;
  /** With `navigation`: baseline URL to detect a change from (default: any
   *  navigation satisfies it). */
  fromUrl?: string;
  /** With `navigation`: wait until the load completes. */
  complete?: boolean;
  /** Timeout in ms (default 10000, capped at 60000). */
  timeoutMs?: number;
}

type BrowserWaitConditionKey = 'loaded' | 'exists' | 'visible' | 'hidden' | 'editable' | 'js' | 'url' | 'urlContains' | 'navigation';

/** Exactly one condition; ambiguous and empty waits are rejected by the runtime. */
export type BrowserWaitOptions = Pick<BrowserWaitFields, 'tab' | 'timeoutMs' | 'fromUrl' | 'complete'> & {
  [K in BrowserWaitConditionKey]: Required<Pick<BrowserWaitFields, K>> &
    Partial<Record<Exclude<BrowserWaitConditionKey, K>, never>>;
}[BrowserWaitConditionKey];

/** Browser query payload; it does not carry lxapp-only identity/index fields. */
export interface BrowserElementInfo {
  exists: boolean;
  visible: boolean;
  enabled: boolean;
  editable: boolean;
  text?: string;
  text_truncated?: boolean;
  value?: string;
  value_truncated?: boolean;
  rect?: ElementRect;
}

export interface BrowserWaitResult {
  elapsed_ms: number;
  current_url?: string;
  element?: BrowserElementInfo;
  value?: unknown;
}

export interface BrowserEvalResult<T = unknown> {
  value: T;
  navigation: BrowserWaitResult;
}

export interface BrowserTab {
  tab_id: string;
  path: string;
  session_id: number;
  current_url?: string;
  title?: string;
  can_go_back: boolean;
  can_go_forward: boolean;
}

export interface BrowserOpenResult {
  tab: string;
}

export type CookieSameSite = 'Lax' | 'Strict' | 'None';

export interface CookieSetOptions extends BrowserTabRef {
  name: string;
  value: string;
  url?: string;
  domain?: string;
  /** Cookie path (default `/`). */
  path?: string;
  secure?: boolean;
  httpOnly?: boolean;
  expiresUnixMs?: number;
  sameSite?: CookieSameSite;
}

export interface CookieDeleteOptions extends BrowserTabRef {
  name: string;
  domain: string;
  /** Cookie path (default `/`). */
  path?: string;
}

export interface CookieListOptions extends BrowserTabRef {
  /** List cookies for every domain, not just the tab's URL. */
  all?: boolean;
}

/** A cookie from the WebView store (raw payload, snake_case keys). */
export interface BrowserCookie {
  name: string;
  value: string;
  domain: string;
  path: string;
  host_only?: boolean;
  secure: boolean;
  http_only: boolean;
  session: boolean;
  expires_unix_ms?: number;
  same_site?: CookieSameSite;
}

export interface BrowserCookies {
  list(options?: CookieListOptions): Promise<BrowserCookie[]>;
  set(options: CookieSetOptions): Promise<void>;
  delete(options: CookieDeleteOptions): Promise<void>;
  clear(options?: BrowserTabRef): Promise<void>;
}

/** The host app's browser tabs (Playwright-like WebView automation). */
export interface BrowserDriver {
  open(options: BrowserOpenOptions): Promise<BrowserOpenResult>;
  tabs(): Promise<BrowserTab[]>;
  current(): Promise<BrowserTab | null>;
  activate(options?: BrowserTabRef): Promise<BrowserTab>;
  close(options?: BrowserTabRef): Promise<void>;
  reload(options?: BrowserTabRef): Promise<void>;
  back(options?: BrowserTabRef): Promise<void>;
  forward(options?: BrowserTabRef): Promise<void>;
  /** Evaluate JS; with `waitNavigation` resolves to `{ value, navigation }`. */
  eval<T = unknown>(options: BrowserEvalOptions & {waitNavigation: true}): Promise<BrowserEvalResult<T>>;
  eval<T = unknown>(options: BrowserEvalOptions & {waitNavigation?: false}): Promise<T>;
  eval<T = unknown>(options: BrowserEvalOptions): Promise<T | BrowserEvalResult<T>>;
  query(options: BrowserQueryOptions): Promise<BrowserElementInfo>;
  /** Wait for a condition (pass exactly one condition field). */
  wait(options: BrowserWaitOptions): Promise<BrowserWaitResult>;
  /** Click; with `waitNavigation` resolves to the navigation payload else `null`. */
  click(options: BrowserClickOptions & { waitNavigation: true }): Promise<BrowserWaitResult>;
  click(options: BrowserClickOptions & { waitNavigation?: false }): Promise<null>;
  click(options: BrowserClickOptions): Promise<BrowserWaitResult | null>;
  type(options: BrowserTypeOptions): Promise<void>;
  fill(options: BrowserTypeOptions): Promise<void>;
  /** Press; with `waitNavigation` resolves to the navigation payload else `null`. */
  press(options: BrowserPressOptions & { waitNavigation: true }): Promise<BrowserWaitResult>;
  press(options: BrowserPressOptions & { waitNavigation?: false }): Promise<null>;
  press(options: BrowserPressOptions): Promise<BrowserWaitResult | null>;
  scroll(options: BrowserScrollOptions): Promise<void>;
  scrollTo(options: BrowserSelectorOptions): Promise<void>;
  screenshot(options?: BrowserTabRef): Promise<Screenshot>;
  readonly cookies: BrowserCookies;
}

// ======================= page input (app window) =======================

/** Targets a host window; defaults to the session's focused/main window. */
export interface WindowRef {
  /** Window id from `lxapp.windows()`. */
  window?: string;
}

export interface AppWindowInfo {
  id: string;
  title?: string;
  width?: number;
  height?: number;
  focused?: boolean;
  main?: boolean;
  visible?: boolean;
}

/** Result of a dispatched input action. */
export interface InputResult {
  window_id: string;
  /** The action kind that was dispatched. */
  action: string;
}

export type MouseButton = 'left' | 'right' | 'middle';

/** A coordinate as `[x, y]` (the `--at X,Y` flag form). */
export type Point = [number, number];

export interface PointerAtOptions extends WindowRef {
  /** Target coordinate in page (CSS) pixels. */
  at: Point;
}

export interface PointerButtonOptions extends PointerAtOptions {
  button?: MouseButton;
}

export interface PointerClickOptions extends PointerButtonOptions {
  /** Number of clicks to report in the event (default 1). */
  count?: number;
}

export interface PointerDragOptions extends WindowRef {
  from: Point;
  to: Point;
  button?: MouseButton;
}

export interface PointerScrollOptions extends PointerAtOptions {
  /** Horizontal scroll delta in page pixels. */
  dx?: number;
  /** Vertical scroll delta in page pixels. */
  dy?: number;
}

/** App-window pointer input at page coordinates (`lxdev lxapp page pointer`). */
export interface PagePointer {
  move(options: PointerAtOptions): Promise<InputResult>;
  down(options: PointerButtonOptions): Promise<InputResult>;
  up(options: PointerButtonOptions): Promise<InputResult>;
  click(options: PointerClickOptions): Promise<InputResult>;
  drag(options: PointerDragOptions): Promise<InputResult>;
  scroll(options: PointerScrollOptions): Promise<InputResult>;
}

/** Canonical cross-platform modifier vocabulary; `meta` maps to the platform
 *  meta key (Command on macOS, Windows key on Windows). */
export type KeyModifier = 'ctrl' | 'shift' | 'alt' | 'meta';

export interface KeyTypeOptions extends WindowRef {
  text: string;
}

export interface KeyPressOptions extends WindowRef {
  /** Key name: `return`, `tab`, `escape`, `delete`, `space`, arrows. */
  key: string;
  modifiers?: KeyModifier[];
}

/** App-window keyboard input to the focused control (`lxdev lxapp page key`). */
export interface PageKey {
  type(options: KeyTypeOptions): Promise<InputResult>;
  press(options: KeyPressOptions): Promise<InputResult>;
}

// ======================= desktop (host) =======================

/** A rectangle in backend-native global desktop coordinates. */
export interface DesktopRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** A monitor/display (raw contract payload, snake_case keys). */
export interface DesktopDisplay {
  id: string;
  primary: boolean;
  bounds: DesktopRect;
  work_area: DesktopRect;
  scale: number;
  dpi: number;
}

/** A top-level OS window (raw contract payload, snake_case keys). */
export interface DesktopWindowInfo {
  id: string;
  title: string;
  process: string;
  pid: number;
  bounds: DesktopRect;
  display_id: string;
  /**
   * Display DIP-to-physical scale. `bounds` are already backend-native; do not
   * multiply them by this value.
   */
  scale: number;
  dpi: number;
  visible: boolean;
  focused: boolean;
  minimized: boolean;
  maximized: boolean;
  always_on_top: boolean;
  /** Front-to-back z index (0 = frontmost). */
  z: number;
}

/** Generic acknowledgement for input/mutation commands. */
export interface DesktopAck {
  ok: boolean;
  action: string;
}

export interface DesktopPermissions {
  accessibility: boolean;
  screen_recording: boolean;
  input: boolean;
}

export interface DesktopCapabilities {
  displays: boolean;
  windows: boolean;
  screenshot: boolean;
  window_screenshot_occlusion_independent: boolean;
  pixel: boolean;
  pointer: boolean;
  key: boolean;
  window_management: boolean;
  clipboard: boolean;
  ax_tree: boolean;
  ocr: boolean;
  image_match: boolean;
}

export interface DesktopDoctor {
  backend: string;
  os: string;
  os_version: string;
  capabilities: DesktopCapabilities;
  permissions: DesktopPermissions;
}

export interface DesktopPixel {
  x: number;
  y: number;
  hex: string;
  r: number;
  g: number;
  b: number;
}

export interface DesktopCapture extends Screenshot {
  /** True when the capture ignored occlusion (window PrintWindow path). */
  occlusionIndependent: boolean;
  backend: string;
}

export interface DesktopClipboardContent {
  available_formats: string[];
  text: string | null;
}

/** A node in the native accessibility tree. */
export interface DesktopAxNode {
  id: string;
  role: string;
  name: string;
  value?: string;
  enabled: boolean;
  focused: boolean;
  rect: DesktopRect;
  children?: DesktopAxNode[];
}

export interface DesktopProcessInfo {
  pid: number;
  name: string;
}

export interface DesktopSnapshotOptions {
  /** Window id from `windows()`. */
  window: string;
  /** Skip the accessibility tree. */
  noAx?: boolean;
  /** Limit ax tree depth. */
  depth?: number;
}

export interface DesktopSnapshot {
  window: DesktopWindowInfo;
  /** PNG capture, or `null` when unavailable. */
  screenshot: (Screenshot & { occlusionIndependent: boolean }) | null;
  /** AX tree, `null` when `noAx` or unavailable. */
  ax: DesktopAxNode | null;
}

export interface DesktopLaunchResult {
  /** Durable target pid — prefer this for follow-up quit/kill. */
  pid: number;
  launcher_pid: number;
  window?: DesktopWindowInfo;
}

/**
 * Selects a desktop window: exactly one of `window` (id from `windows()`) or
 * `match` (query `text | title: | class: | process: | pid:`, must resolve to
 * exactly one window).
 */
export type DesktopWindowSel =
  | { window: string; match?: never }
  | { window?: never; match: string };

export interface DesktopWindowsOptions {
  /** Match query (`text | title: | class: | process: | pid:`). */
  match?: string;
}

/** Capture target — at most one of `display` / `window` / `region`;
 *  omit all to capture the whole virtual screen. */
export interface DesktopScreenshotOptions {
  /** Monitor by 1-based index (as listed by `displays()`). */
  display?: number;
  /** Window by id (occlusion-independent capture). */
  window?: string;
  /** Region as `[x, y, w, h]` in desktop coordinates. */
  region?: [number, number, number, number];
}

export interface DesktopAtOptions {
  /** Coordinate in backend-native desktop pixels. */
  at: Point;
}

/** Optional background-input target: a `window` id (resolved to its owning
 *  process) or an explicit `pid`. Omit both for foreground input. */
export interface DesktopInputTarget {
  window?: string;
  pid?: number;
}

export interface DesktopPointerAtOptions extends DesktopInputTarget {
  at: Point;
}

export interface DesktopPointerButtonOptions extends DesktopPointerAtOptions {
  button?: MouseButton;
}

export interface DesktopPointerClickOptions extends DesktopPointerButtonOptions {
  count?: number;
}

export interface DesktopPointerDragOptions extends DesktopInputTarget {
  from: Point;
  to: Point;
  button?: MouseButton;
}

export interface DesktopPointerScrollOptions extends DesktopPointerAtOptions {
  /** Horizontal scroll delta in notches. */
  dx?: number;
  /** Vertical scroll delta in notches. */
  dy?: number;
}

/** Synthetic physical mouse input at desktop coordinates. */
export interface DesktopPointer {
  move(options: DesktopPointerAtOptions): Promise<DesktopAck>;
  down(options: DesktopPointerButtonOptions): Promise<DesktopAck>;
  up(options: DesktopPointerButtonOptions): Promise<DesktopAck>;
  click(options: DesktopPointerClickOptions): Promise<DesktopAck>;
  drag(options: DesktopPointerDragOptions): Promise<DesktopAck>;
  scroll(options: DesktopPointerScrollOptions): Promise<DesktopAck>;
}

export interface DesktopKeyTypeOptions extends DesktopInputTarget {
  text: string;
}

export interface DesktopKeyPressOptions extends DesktopInputTarget {
  /** Case-insensitive named key (`Enter`, `ArrowDown`, `Down`, etc.) or one
   * printable character. */
  key: string;
  modifiers?: KeyModifier[];
}

export interface DesktopKeyNameOptions extends DesktopInputTarget {
  /** Case-insensitive named key (`Enter`, `ArrowDown`, `Down`, etc.) or one
   * printable character. */
  key: string;
}

/** Synthetic physical keyboard input. */
export interface DesktopKey {
  /** Type literal text into the focused control. */
  type(options: DesktopKeyTypeOptions): Promise<DesktopAck>;
  press(options: DesktopKeyPressOptions): Promise<DesktopAck>;
  down(options: DesktopKeyNameOptions): Promise<DesktopAck>;
  up(options: DesktopKeyNameOptions): Promise<DesktopAck>;
}

export type DesktopWindowMoveOptions = DesktopWindowSel & {
  /** Target position as `[x, y]` in desktop coordinates. */
  to: Point;
};

export type DesktopWindowResizeOptions = DesktopWindowSel & {
  width: number;
  height: number;
};

export type DesktopWindowMoveDisplayOptions = DesktopWindowSel & {
  /** Display id from `displays()`. */
  display: string;
};

export type DesktopWindowAlwaysOnTopOptions = DesktopWindowSel & {
  on: boolean;
};

/** Window management; every verb resolves to the resulting window state. */
export interface DesktopWindowDriver {
  status(options: DesktopWindowSel): Promise<DesktopWindowInfo>;
  focus(options: DesktopWindowSel): Promise<DesktopWindowInfo>;
  activate(options: DesktopWindowSel): Promise<DesktopWindowInfo>;
  raise(options: DesktopWindowSel): Promise<DesktopWindowInfo>;
  minimize(options: DesktopWindowSel): Promise<DesktopWindowInfo>;
  maximize(options: DesktopWindowSel): Promise<DesktopWindowInfo>;
  restore(options: DesktopWindowSel): Promise<DesktopWindowInfo>;
  /** Close a window. Destructive. */
  close(options: DesktopWindowSel): Promise<DesktopWindowInfo>;
  moveTo(options: DesktopWindowMoveOptions): Promise<DesktopWindowInfo>;
  moveToDisplay(options: DesktopWindowMoveDisplayOptions): Promise<DesktopWindowInfo>;
  resize(options: DesktopWindowResizeOptions): Promise<DesktopWindowInfo>;
  setAlwaysOnTop(options: DesktopWindowAlwaysOnTopOptions): Promise<DesktopWindowInfo>;
}

export interface DesktopClipboardSetOptions {
  text: string;
}

/** System clipboard access (Unicode text). */
export interface DesktopClipboard {
  get(): Promise<DesktopClipboardContent>;
  set(options: DesktopClipboardSetOptions): Promise<DesktopAck>;
  clear(): Promise<DesktopAck>;
  /** Paste into the focused control (Ctrl/Cmd+V). */
  paste(): Promise<DesktopAck>;
}

export interface DesktopAxTreeOptions {
  /** Window id from `windows()`. */
  window: string;
  /** Limit tree depth. */
  depth?: number;
  /** Cap the number of nodes. */
  maxNodes?: number;
}

/** Node match query: `text | name: | role: | value: | id:`. */
export interface DesktopAxSel {
  window: string;
  match: string;
}

export interface DesktopAxQueryOptions extends DesktopAxSel {
  /** Return every match instead of exactly one. */
  all?: boolean;
  /** Target the nth match. */
  index?: number;
}

export interface DesktopAxSetValueOptions extends DesktopAxSel {
  value: string;
}

/** Native accessibility tree inspection and atomic actions — never falls back
 *  to physical input silently. */
export interface DesktopAx {
  tree(options: DesktopAxTreeOptions): Promise<DesktopAxNode>;
  query(options: DesktopAxQueryOptions): Promise<DesktopAxNode[]>;
  /** Atomically match exactly one node and invoke it. */
  invoke(options: DesktopAxSel): Promise<DesktopAck>;
  focus(options: DesktopAxSel): Promise<DesktopAck>;
  setValue(options: DesktopAxSetValueOptions): Promise<DesktopAck>;
  select(options: DesktopAxSel): Promise<DesktopAck>;
  expand(options: DesktopAxSel): Promise<DesktopAck>;
  collapse(options: DesktopAxSel): Promise<DesktopAck>;
  scrollIntoView(options: DesktopAxSel): Promise<DesktopAck>;
  /** The accessible element at a screen point. */
  hitTest(options: DesktopAtOptions): Promise<DesktopAxNode>;
}

export interface DesktopWaitWindowOptions {
  match: string;
  /** `visible` (default) | `hidden`. */
  state?: 'visible' | 'hidden';
  /** Timeout in ms (default 5000). */
  timeoutMs?: number;
}

export interface DesktopWaitAxOptions extends DesktopAxSel {
  /** `exists` (default) | `gone` | `enabled` | `focused`. */
  state?: 'exists' | 'gone' | 'enabled' | 'focused';
  timeoutMs?: number;
}

export interface DesktopWaitPixelOptions extends DesktopAtOptions {
  /** Expected color as `#rrggbb`. */
  color: string;
  /** Per-channel tolerance (default 0). */
  tolerance?: number;
  timeoutMs?: number;
}

/** Wait for a condition; rejects with `E_DESKTOP_TIMEOUT` when it never holds. */
export interface DesktopWait {
  window(options: DesktopWaitWindowOptions): Promise<DesktopWindowInfo>;
  ax(options: DesktopWaitAxOptions): Promise<DesktopAck>;
  pixel(options: DesktopWaitPixelOptions): Promise<DesktopPixel>;
}

export interface DesktopAppLaunchOptions {
  /** Path or PATH-resolved command. */
  app: string;
  args?: string[];
  /** Wait for a window matching this query before resolving. */
  waitWindow?: string;
  timeoutMs?: number;
}

/** Quit target — exactly one of `match` / `pid` / `window`. */
export type DesktopAppQuitOptions = (
  | { match: string; pid?: never; window?: never }
  | { match?: never; pid: number; window?: never }
  | { match?: never; pid?: never; window: string }
) & {
  /** Terminate instead of a graceful close. */
  force?: boolean;
}

/** App lifecycle. */
export interface DesktopApp {
  launch(options: DesktopAppLaunchOptions): Promise<DesktopLaunchResult>;
  /** Quit an app. Destructive. */
  quit(options: DesktopAppQuitOptions): Promise<DesktopAck>;
}

export interface DesktopProcessListOptions {
  /** Case-insensitive name substring filter. */
  filter?: string;
}

export interface DesktopProcessKillOptions {
  pid: number;
  force?: boolean;
}

/** Process inspection/control. */
export interface DesktopProcess {
  list(options?: DesktopProcessListOptions): Promise<DesktopProcessInfo[]>;
  /** Terminate a process. Destructive. */
  kill(options: DesktopProcessKillOptions): Promise<DesktopAck>;
}

/**
 * Session-less local-OS desktop automation — the in-process mapping of
 * `lxdev desktop` over the same backend, DTOs, and error taxonomy
 * (errors carry stable `E_DESKTOP_<CODE>` codes). Windows and macOS;
 * other platforms reject with `E_DESKTOP_UNSUPPORTED`.
 *
 * Coordinates are backend-native global desktop coordinates: physical pixels
 * on Windows, display points (top-left origin) on macOS.
 */
export interface DesktopDriver {
  /** Backend, capability, and permission report. */
  doctor(): Promise<DesktopDoctor>;
  /** OS-permission grants; `{ request: true }` triggers the OS prompts. */
  permissions(options?: { request?: boolean }): Promise<DesktopPermissions>;
  displays(): Promise<DesktopDisplay[]>;
  windows(options?: DesktopWindowsOptions): Promise<DesktopWindowInfo[]>;
  /** Capture the screen (default), a display, a window, or a region. */
  screenshot(options?: DesktopScreenshotOptions): Promise<DesktopCapture>;
  /** Read one pixel's color. */
  pixel(options: DesktopAtOptions): Promise<DesktopPixel>;
  /** One-shot window info + screenshot + ax tree. */
  snapshot(options: DesktopSnapshotOptions): Promise<DesktopSnapshot>;
  readonly window: DesktopWindowDriver;
  readonly pointer: DesktopPointer;
  readonly key: DesktopKey;
  readonly clipboard: DesktopClipboard;
  readonly ax: DesktopAx;
  readonly wait: DesktopWait;
  readonly app: DesktopApp;
  readonly process: DesktopProcess;
}
