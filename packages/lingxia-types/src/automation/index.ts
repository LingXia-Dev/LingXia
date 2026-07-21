/**
 * In-process UI/runtime automation — `lx.automation()`.
 *
 * Returns a stable selector root. Selecting the calling lxapp requires the
 * `automation` security privilege; cross-lxapp and host surfaces require
 * `host`. `lingxia dev` sessions and the Runner grant both implicitly.
 *
 * This mirrors the devtool (`lxdev`) automation surface as a privilege-scoped,
 * product-side API.
 */

// ============================ factory ============================

/** Stable automation root; it grants no capability until one is selected. */
export interface Automation {
  /** Drive the calling/current lxapp. Requires `automation` outside dev. */
  lxapp(): LxAppDriver;
  /** Drive a specific running lxapp. Requires `host` outside dev. */
  lxapp(appid: string): LxAppDriver;
  /** Cross-lxapp lifecycle and host-window capture. */
  readonly lxapps: LxAppManager;
  /** The host app's browser tabs. */
  readonly browser: BrowserDriver;
  /** Simulated-device selection in a host runner. */
  readonly device: DeviceDriver;
  /**
   * Session-less local-OS desktop automation (`lxdev desktop`). Beyond the
   * app sandbox, so restricted to dev/test hosts (`lingxia dev` or the
   * Runner) on top of the `host` privilege. Windows/macOS only.
   */
  readonly desktop: DesktopDriver;
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

/** Element-level automation of the selected lxapp's page WebViews. */
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
  /** App-window pointer input at page coordinates (`lxdev lxapp page pointer`). */
  readonly pointer: PagePointer;
  /** App-window keyboard input (`lxdev lxapp page key`). */
  readonly key: PageKey;
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
 * Page-stack navigation for the selected lxapp. Action verbs take a configured
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
  /** Status of a configured page by name; omit `page` for the current page. */
  info(options?: PageTarget): Promise<PageInfo>;
  stack(): Promise<PageInfo[]>;
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

export interface LxAppEvalOptions {
  /** JavaScript expression or function body run in the selected Logic runtime. */
  script: string;
  timeoutMs?: number;
}

/** Capability for one selected running lxapp. */
export interface LxAppDriver {
  readonly page: PageDriver;
  readonly nav: NavDriver;
  /** Complete runtime snapshot of the selected lxapp. */
  info(): Promise<LxAppRuntimeInfo>;
  /** Configured pages of the selected lxapp. */
  pages(): Promise<LxAppPageConfig[]>;
  /** Logic-runtime eval; self-eval from that Logic runtime is rejected. */
  eval(options: LxAppEvalOptions): Promise<unknown>;
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

/**
 * Cross-lxapp lifecycle and host-window access. Requires `host` outside dev.
 *
 * `close`, `restart`, and `uninstall` reject when they target the calling app
 * itself. Use `lx.app.exit()` to self-exit.
 */
export interface LxAppManager {
  list(): Promise<LxAppRuntimeInfo[]>;
  current(): Promise<LxAppSummary>;
  open(options: LxAppOpenOptions): Promise<LxAppOpenResult>;
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
}

export interface DeviceSetOptions {
  /** Device preset id (see `list()`). */
  id: string;
  /** Force landscape (`true`) or portrait (`false`); omit to use the
   * runner's normal device-selection behavior. */
  landscape?: boolean;
}

/**
 * Simulated-device control (`lxdev lxapp device`). Only functional in a host
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
  /** With `navigation`: baseline URL to detect a change from (default: any
   *  navigation satisfies it). */
  fromUrl?: string;
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
  /** Evaluate JS; with `waitNavigation` resolves to `{ value, navigation }`. */
  eval(options: BrowserEvalOptions): Promise<unknown>;
  query(options: BrowserQueryOptions): Promise<PageElement | PageElementMiss>;
  /** Wait for a condition (pass exactly one condition field). */
  wait(options: BrowserWaitOptions): Promise<unknown>;
  /** Click; with `waitNavigation` resolves to the navigation payload else `null`. */
  click(options: BrowserClickOptions): Promise<unknown>;
  type(options: BrowserTypeOptions): Promise<void>;
  fill(options: BrowserTypeOptions): Promise<void>;
  /** Press; with `waitNavigation` resolves to the navigation payload else `null`. */
  press(options: BrowserPressOptions): Promise<unknown>;
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

// ======================= desktop (host, dev/test only) =======================

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
export interface DesktopWindowSel {
  window?: string;
  match?: string;
}

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
  key: string;
  modifiers?: KeyModifier[];
}

export interface DesktopKeyNameOptions extends DesktopInputTarget {
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

export interface DesktopWindowMoveOptions extends DesktopWindowSel {
  /** Target position as `[x, y]` in desktop coordinates. */
  to: Point;
}

export interface DesktopWindowResizeOptions extends DesktopWindowSel {
  width: number;
  height: number;
}

export interface DesktopWindowMoveDisplayOptions extends DesktopWindowSel {
  /** Display id from `displays()`. */
  display: string;
}

export interface DesktopWindowAlwaysOnTopOptions extends DesktopWindowSel {
  on: boolean;
}

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
export interface DesktopAppQuitOptions {
  match?: string;
  pid?: number;
  window?: string;
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
