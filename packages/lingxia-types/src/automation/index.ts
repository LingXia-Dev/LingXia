/**
 * In-process UI/runtime automation — `lx.automation()`.
 *
 * Returns a capability handle for driving the calling lxapp's own UI and
 * runtime. Gated by the `automation` security privilege (base tier) or `host`
 * (host tier); `lingxia dev` sessions and the Runner grant them implicitly.
 *
 * This mirrors the devtool (`lxdev`) automation surface as a privilege-scoped,
 * product-side API.
 */

// ============================ factory ============================

export interface AutomationOptions {
  /** Opt into the host tier: cross-lxapp management, browser tabs, and
   *  host-window input. Requires the `host` privilege. */
  host?: boolean;
}

/** Base tier: operate on the calling lxapp itself. */
export interface Automation {
  /** Element-level automation of the lxapp's own page WebViews. */
  readonly page: PageDriver;
  /** Page-stack navigation and runtime reads. */
  readonly nav: NavDriver;
  /** Read-only introspection of the calling lxapp. */
  readonly lxapp: LxAppSelfInfo;
}

/** Host tier: adds cross-lxapp, browser, and host-window control. */
export interface HostAutomation {
  readonly page: PageDriver;
  readonly nav: NavDriver;
  /** Cross-lxapp lifecycle + logic-runtime access. */
  readonly lxapp: LxAppManager;
  /** The host app's browser tabs. */
  readonly browser: BrowserDriver;
  /** The host app as a whole: window screenshot, mouse, and keyboard input. */
  readonly app: AppDriver;
}

// ============================ page tier ============================

/** Fields common to every page action; `page` defaults to the current page. */
export interface PageTarget {
  /** Configured page name (from lxapp.json); defaults to the current page. */
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
}

export interface PageSelectorOptions extends PageTarget {
  css: string;
  /** Target the nth match. */
  index?: number;
}

export interface PageTypeOptions extends PageSelectorOptions {
  text: string;
}

export interface PagePressOptions extends PageTarget {
  /** Key name, e.g. `Enter`, `Escape`, `Tab`. */
  key: string;
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

export type PageWaitState = 'exists' | 'visible' | 'gone';

export interface PageWaitForOptions extends PageTarget {
  css: string;
  /** Condition to await (default `visible`). */
  state?: PageWaitState;
  /** Timeout in ms (default 10000, capped at 60000). */
  timeoutMs?: number;
}

/** An element's viewport rectangle (viewport-relative CSS pixels). */
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
  /** Viewport-aware visibility (size, style, and in-viewport). */
  visible: boolean;
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

/** Element-level automation of the calling lxapp's own page WebViews. */
export interface PageDriver {
  /** Evaluate JavaScript in the page WebView; resolves to the returned value. */
  eval(options: PageEvalOptions): Promise<unknown>;
  /** Query one element's info. */
  query(options: PageQueryOptions & { all?: false }): Promise<PageQueryResult>;
  /** Query every matching element. */
  query(options: PageQueryOptions & { all: true }): Promise<PageQueryAll>;
  click(options: PageSelectorOptions): Promise<void>;
  /** Type text into an element without clearing existing content. */
  type(options: PageTypeOptions): Promise<void>;
  /** Replace an element's current value. */
  fill(options: PageTypeOptions): Promise<void>;
  press(options: PagePressOptions): Promise<void>;
  /** Scroll the first matching element into view. */
  scrollTo(options: PageScrollToOptions): Promise<void>;
  /** Scroll the page DOM by a pixel delta (nearest scrollable container). */
  scroll(options?: PageScrollOptions): Promise<void>;
  /** Poll until the selector reaches `state`, else reject on timeout. */
  waitFor(options: PageWaitForOptions): Promise<void>;
  screenshot(options?: PageTarget): Promise<Screenshot>;
}

// ============================ nav tier ============================

export interface NavOptions {
  /** Configured page name (from lxapp.json). */
  page: string;
  /** Query forwarded to the destination page. */
  query?: Record<string, unknown>;
}

export interface NavBackOptions {
  /** Number of pages to pop (default 1). */
  delta?: number;
}

/** A page's runtime position. */
export interface PageInfo {
  path: string;
  /** Configured page name, if the path maps to one. */
  name: string | null;
  current: boolean;
  inStack: boolean;
  /** Whether the page has an attached WebView. */
  ready: boolean;
}

/**
 * Page-stack navigation for the calling lxapp. Action verbs take a configured
 * page name (`redirect` rejects a tab-bar page); `back` pops; `current`/`stack`
 * read. Unlike the JS `lx.navigateTo` family this returns the landed page, but
 * like it does not wait for the destination WebView (awaiting in-process would
 * deadlock the logic thread).
 */
export interface NavDriver {
  /** Push a page onto the stack. */
  to(options: NavOptions): Promise<PageInfo>;
  /** Replace the current page (rejects tab-bar targets). */
  redirect(options: NavOptions): Promise<PageInfo>;
  /** Switch to a configured tab page. */
  switchTab(options: NavOptions): Promise<PageInfo>;
  /** Clear the stack and relaunch at a page. */
  relaunch(options: NavOptions): Promise<PageInfo>;
  back(options?: NavBackOptions): Promise<PageInfo>;
  current(): Promise<PageInfo>;
  stack(): Promise<PageInfo[]>;
}

// ============================ lxapp (base) ============================

export interface LxAppSummary {
  appid: string;
  currentPage: string | null;
}

export interface LxAppPageConfig {
  name: string;
  path: string;
}

/** Read-only introspection of the calling lxapp (base tier). */
export interface LxAppSelfInfo {
  current(): Promise<LxAppSummary>;
  /** Configured pages of the calling lxapp. */
  pages(): Promise<LxAppPageConfig[]>;
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
  is_home: boolean;
  current_page: string | null;
  initial_route: string;
  pages_count: number;
  page_entries: LxAppPageEntry[];
  page_stack: string[];
  lxapp_dir: string;
  data_dir: string;
  cache_dir: string;
}

/** Selects a running lxapp by id; defaults to the current app. */
export interface LxAppRef {
  /** LxApp id, or `"current"` (default). */
  app?: string;
}

export interface LxAppListOptions {
  /** Accepted for API shape; currently returns all instances regardless. */
  all?: boolean;
}

export interface LxAppOpenOptions {
  appid: string;
  /** Initial page/path. */
  path?: string;
  releaseType?: 'release' | 'preview' | 'developer';
}

export interface LxAppOpenResult {
  appid: string;
  path: string;
}

export interface LxAppEvalOptions extends LxAppRef {
  /** JavaScript expression or function body run in the target logic runtime. */
  script: string;
  timeoutMs?: number;
}

/**
 * Cross-lxapp lifecycle and logic-runtime access (host tier).
 *
 * `eval`, `close`, `restart`, and `uninstall` reject when they target the
 * calling app itself — running teardown or a re-entrant eval from inside the
 * app's own single-threaded logic runtime would deadlock. Use `lx.app.exit()`
 * to self-exit.
 */
export interface LxAppManager {
  list(options?: LxAppListOptions): Promise<LxAppRuntimeInfo[]>;
  current(): Promise<LxAppSummary>;
  info(options?: LxAppRef): Promise<LxAppRuntimeInfo>;
  pages(options?: LxAppRef): Promise<LxAppPageEntry[]>;
  open(options: LxAppOpenOptions): Promise<LxAppOpenResult>;
  close(options?: LxAppRef): Promise<void>;
  restart(options?: LxAppRef): Promise<void>;
  uninstall(options?: LxAppRef): Promise<void>;
  /** Logic-runtime eval in another app (cannot target the calling app). */
  eval(options: LxAppEvalOptions): Promise<unknown>;
  /** A base-tier handle (`page`/`nav`/`lxapp`) scoped to another app. */
  scope(options?: LxAppRef): Automation;
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
  timeoutMs?: number;
}

export interface BrowserQueryOptions extends BrowserTabRef {
  css: string;
  maxText?: number;
}

export interface BrowserSelectorOptions extends BrowserTabRef {
  css: string;
}

export interface BrowserTypeOptions extends BrowserSelectorOptions {
  text: string;
}

export interface BrowserPressOptions extends BrowserTabRef {
  key: string;
}

export interface BrowserScrollOptions extends BrowserTabRef {
  dx?: number;
  dy?: number;
}

/**
 * A browser wait condition — pass **exactly one** of the condition fields.
 * `navigation` may add `complete` to wait for load completion.
 */
export interface BrowserWaitOptions extends BrowserTabRef {
  /** Wait for page load. */
  loaded?: boolean;
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
  navigation?: boolean;
  /** With `navigation`: wait until the load completes. */
  complete?: boolean;
  /** Timeout in ms (default 10000, capped at 60000). */
  timeoutMs?: number;
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
  eval(options: BrowserEvalOptions): Promise<unknown>;
  query(options: BrowserQueryOptions): Promise<PageElement | PageElementMiss>;
  /** Wait for a condition (pass exactly one condition field). */
  wait(options: BrowserWaitOptions): Promise<unknown>;
  click(options: BrowserSelectorOptions): Promise<void>;
  type(options: BrowserTypeOptions): Promise<void>;
  fill(options: BrowserTypeOptions): Promise<void>;
  press(options: BrowserPressOptions): Promise<void>;
  scroll(options: BrowserScrollOptions): Promise<void>;
  scrollTo(options: BrowserSelectorOptions): Promise<void>;
  screenshot(options?: BrowserTabRef): Promise<Screenshot>;
  readonly cookies: BrowserCookies;
}

// ======================= app / input (host) =======================

/** Targets a host window; defaults to the focused/main window. */
export interface WindowRef {
  /** Window id from `app.windows()`. */
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

export interface MousePoint extends WindowRef {
  /** X in logical window content points. */
  x: number;
  /** Y in logical window content points. */
  y: number;
  button?: MouseButton;
}

export interface MouseDrag extends WindowRef {
  fromX: number;
  fromY: number;
  toX: number;
  toY: number;
  button?: MouseButton;
}

export interface MouseScrollOptions extends WindowRef {
  x: number;
  y: number;
  dx?: number;
  dy?: number;
}

/** Raw mouse input to a host window (logical content coordinates). */
export interface AppMouse {
  move(options: MousePoint): Promise<InputResult>;
  down(options: MousePoint): Promise<InputResult>;
  up(options: MousePoint): Promise<InputResult>;
  click(options: MousePoint): Promise<InputResult>;
  drag(options: MouseDrag): Promise<InputResult>;
  scroll(options: MouseScrollOptions): Promise<InputResult>;
}

export type KeyModifier = 'command' | 'shift' | 'option' | 'control';

export interface KeyTypeOptions extends WindowRef {
  text: string;
}

export interface KeyPressOptions extends WindowRef {
  /** Key name, e.g. `return`, `tab`, `escape`, arrows. */
  key: string;
  modifiers?: KeyModifier[];
}

/** Keyboard input to a host window's focused control. */
export interface AppKey {
  type(options: KeyTypeOptions): Promise<InputResult>;
  press(options: KeyPressOptions): Promise<InputResult>;
}

/** The host app as a whole: window screenshot, enumeration, and input. */
export interface AppDriver {
  /** PNG of the full host window (native controls + composited WebViews). */
  screenshot(options?: WindowRef): Promise<Screenshot>;
  /** Enumerate the host app's top-level windows. */
  windows(): Promise<AppWindowInfo[]>;
  readonly mouse: AppMouse;
  readonly key: AppKey;
}
