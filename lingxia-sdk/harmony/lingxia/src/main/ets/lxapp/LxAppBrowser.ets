import { hilog } from '@kit.PerformanceAnalysisKit';
import { webview } from '@kit.ArkWeb';
import { display, window } from '@kit.ArkUI';
import { Lingxia } from '../app/Lingxia';
import { requestWebFileChooser } from './WebFileChooserBridge';
import {
  WebViewManager,
  notifyWebViewControllerAttached,
  WebViewInfo
} from './WebViewBridge';
import {
  resolveBrowserNavigationPolicy,
  type BrowserNavigationPolicyResponse
} from './BrowserNavigationPolicy';
import {
	  browserTabActivate,
	  browserTabClose,
	  browserTabIsAside,
	  browserTabNavigate,
	  browserTabPathForId,
  browserUrlIsHidden as nativeBrowserUrlIsHidden,
	  getBuiltinBrowserAppId,
	  getLxappSessionId,
	  openAsideBrowserTab,
	  openBrowserTab
	} from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.Browser';
const ATTACH_RETRY_DELAY_MS = 100;
const ATTACH_MAX_RETRIES = 8;
const KEYBOARD_MONITOR_INTERVAL_MS = 80;

const NEW_TAB_URL = 'lingxia://newtab';
const DOWNLOADS_URL = 'lingxia://downloads';
const SETTINGS_URL = 'lingxia://settings';

interface BrowserUrlDisplay {
  text: string;
  iconPath: Resource;
  iconColor: string;
  iconVisible: boolean;
}

interface DisplayMetrics {
  heightVp: number;
  density: number;
}

/**
 * A new/blank tab (lingxia://newtab) and a bare `lingxia://` carry no address
 * the user should edit: they read as an empty start page.
 */
function browserUrlIsHidden(rawUrl: string): boolean {
  const raw = String(rawUrl ?? '');
  try {
    return nativeBrowserUrlIsHidden(raw);
  } catch (error) {
    hilog.warn(DOMAIN, TAG, `browserUrlIsHidden native call failed: ${error}`);
  }
  const trimmed = raw.trim().toLowerCase();
  return !trimmed
    || trimmed === NEW_TAB_URL
    || trimmed === 'lingxia://'
    || trimmed.startsWith('lx:')
    || trimmed.startsWith('data:')
    || trimmed.startsWith('javascript:')
    || trimmed.startsWith('blob:')
    || trimmed === 'about:blank';
}

function browserUrlDisplay(rawUrl: string): BrowserUrlDisplay {
  const trimmed = String(rawUrl ?? '').trim();
  if (!trimmed || browserUrlIsHidden(trimmed)) {
    return {
      text: '',
      iconPath: $rawfile('icons/icon_lock.svg'),
      iconColor: '#666666',
      iconVisible: false,
    };
  }
  const fallback: BrowserUrlDisplay = {
    text: 'Web page',
    iconPath: $rawfile('icons/icon_warning.svg'),
    iconColor: '#A15C00',
    iconVisible: true,
  };
  const schemeEnd = trimmed.indexOf(':');
  if (schemeEnd <= 0) {
    return fallback;
  }
  const scheme = trimmed.substring(0, schemeEnd).toLowerCase();
  if (scheme === 'lingxia') {
    // The browser's own internal pages (settings / downloads): show the full
    // lingxia:// address, no security chrome.
    return {
      text: trimmed,
      iconPath: $rawfile('icons/icon_lock.svg'),
      iconColor: '#666666',
      iconVisible: true,
    };
  }
  if (scheme === 'http' || scheme === 'https') {
    const rest = trimmed.substring(schemeEnd + 1);
    if (!rest.startsWith('//')) {
      return fallback;
    }
    const authority = rest.substring(2).split(/[/?#]/)[0] ?? '';
    const hostPort = authority.split('@').pop() ?? authority;
    const host = hostPort.startsWith('[')
      ? (hostPort.split(']')[0] + ']')
      : hostPort.split(':')[0];
    if (!host) {
      return fallback;
    }
    if (scheme === 'http') {
      return {
        text: host,
        iconPath: $rawfile('icons/icon_warning.svg'),
        iconColor: '#A15C00',
        iconVisible: true,
      };
    }
    return {
      text: host,
      iconPath: $rawfile('icons/icon_lock.svg'),
      iconColor: '#666666',
      iconVisible: true,
    };
  }
  return fallback;
}

/**
 * Resolve a typed address: a full http(s)/lingxia URL loads as-is, a bare domain
 * gets https. Free-text search belongs to a configurable search provider or
 * the start page, not a hardcoded native fallback.
 * Returns the URL to navigate to, or '' when the input is not a navigable URL.
 */
function resolveAddressInput(input: string): string {
  const trimmed = String(input ?? '').trim();
  if (!trimmed) {
    return '';
  }
  const schemeEnd = trimmed.indexOf(':');
  if (schemeEnd > 0) {
    const scheme = trimmed.substring(0, schemeEnd).toLowerCase();
    if (scheme === 'http' || scheme === 'https' || scheme === 'lingxia') {
      return trimmed;
    }
  }
  if (trimmed.indexOf(' ') < 0 && trimmed.indexOf('.') >= 0) {
    return `https://${trimmed}`;
  }
  return '';
}

/**
 * Browser tab manager - owns the ordered set of open browser tabs and the
 * active tab. Switching tabs swaps the displayed managed webview in place
 * instead of opening another browser surface.
 */
export class LxAppBrowserManager {
  private static showing: boolean = false;
  private static openTabIds: Array<string> = [];
  private static activeTabId: string = '';
  private static initialUrls: Map<string, string> = new Map();
  private static listeners: Set<() => void> = new Set();

  /**
   * Show the browser displaying `tabId`. Registers + activates the tab, then
   * shows the browser (or swaps the displayed tab if already shown).
   */
  public static show(tabId: string, initialUrl: string = ''): boolean {
    const normalizedTabId = String(tabId ?? '').trim();
    if (!normalizedTabId) {
      hilog.error(DOMAIN, TAG, 'show failed: empty tabId');
      return false;
    }

    LxAppBrowserManager.register(normalizedTabId);
    const trimmedInitial = String(initialUrl ?? '').trim();
    if (trimmedInitial && !LxAppBrowserManager.initialUrls.has(normalizedTabId)) {
      LxAppBrowserManager.initialUrls.set(normalizedTabId, trimmedInitial);
    }

    LxAppBrowserManager.activeTabId = normalizedTabId;
    LxAppBrowserManager.showing = true;
    LxAppBrowserManager.markActive(normalizedTabId);
    LxAppBrowserManager.notifyListeners();
    return true;
  }

  /**
   * Open a fresh built-in start-page tab and switch to it.
   */
  public static openNewTab(): boolean {
    let appId = '';
    try {
      appId = String(getBuiltinBrowserAppId() ?? '').trim();
    } catch (error) {
      hilog.error(DOMAIN, TAG, `openNewTab failed: getBuiltinBrowserAppId threw: ${error}`);
      return false;
    }
    if (!appId) {
      hilog.error(DOMAIN, TAG, 'openNewTab failed: no builtin browser app id');
      return false;
    }

    const sessionId = Number(getLxappSessionId(appId));
    if (!Number.isFinite(sessionId) || sessionId <= 0) {
      hilog.error(DOMAIN, TAG, 'openNewTab failed: no session for builtin browser');
      return false;
    }

    let newTabId: string | null = null;
    try {
      newTabId = openBrowserTab(appId, Math.trunc(sessionId), NEW_TAB_URL);
    } catch (error) {
      hilog.error(DOMAIN, TAG, `openNewTab failed: openBrowserTab threw: ${error}`);
      return false;
    }
    if (!newTabId || newTabId.trim().length === 0) {
      hilog.error(DOMAIN, TAG, 'openNewTab failed: runtime returned no tab id');
      return false;
    }

    return LxAppBrowserManager.show(newTabId, NEW_TAB_URL);
  }

  /**
   * Make an already-open tab the active, displayed one.
   */
  public static activate(tabId: string): void {
    const normalizedTabId = String(tabId ?? '').trim();
    if (!LxAppBrowserManager.openTabIds.includes(normalizedTabId)) {
      return;
    }
    LxAppBrowserManager.activeTabId = normalizedTabId;
    LxAppBrowserManager.markActive(normalizedTabId);
    LxAppBrowserManager.notifyListeners();
  }

  /**
   * Close a single tab and move focus to a neighbor; exiting the browser when
   * the last tab goes away.
   */
  public static closeTab(tabId: string): void {
    const normalizedTabId = String(tabId ?? '').trim();
    const index = LxAppBrowserManager.openTabIds.indexOf(normalizedTabId);
    if (index < 0) {
      return;
    }

    LxAppBrowserManager.closeRuntimeTab(normalizedTabId);
    LxAppBrowserManager.openTabIds.splice(index, 1);
    LxAppBrowserManager.initialUrls.delete(normalizedTabId);

    const wasActive = LxAppBrowserManager.activeTabId === normalizedTabId;
    if (!wasActive) {
      LxAppBrowserManager.notifyListeners();
      return;
    }

    if (LxAppBrowserManager.openTabIds.length === 0) {
      LxAppBrowserManager.activeTabId = '';
      LxAppBrowserManager.showing = false;
      LxAppBrowserManager.notifyListeners();
      return;
    }

    const neighborIndex = index > 0 ? index - 1 : 0;
    LxAppBrowserManager.activate(LxAppBrowserManager.openTabIds[neighborIndex]);
  }

  /**
   * Navigate the active tab to a URL through the browser runtime.
   */
  public static navigateActiveTab(url: string): boolean {
    const tabId = LxAppBrowserManager.activeTabId;
    const target = String(url ?? '').trim();
    if (!tabId || !target) {
      return false;
    }
    try {
      return Boolean(browserTabNavigate(tabId, target));
    } catch (error) {
      hilog.error(DOMAIN, TAG, `browserTabNavigate failed for tabId=${tabId}: ${error}`);
      return false;
    }
  }

  public static hide(): void {
    LxAppBrowserManager.closeAllTabs();
    LxAppBrowserManager.showing = false;
    LxAppBrowserManager.activeTabId = '';
    LxAppBrowserManager.notifyListeners();
  }

  public static isShowing(): boolean {
    return LxAppBrowserManager.showing;
  }

  public static getActiveTabId(): string {
    return LxAppBrowserManager.activeTabId;
  }

  public static getOpenTabIds(): Array<string> {
    return LxAppBrowserManager.openTabIds.slice();
  }

  public static getTabCount(): number {
    return LxAppBrowserManager.openTabIds.length;
  }

  public static getInitialUrl(tabId: string): string {
    return LxAppBrowserManager.initialUrls.get(tabId) ?? '';
  }

  public static registerListener(listener: () => void): void {
    LxAppBrowserManager.listeners.add(listener);
  }

  public static unregisterListener(listener: () => void): void {
    LxAppBrowserManager.listeners.delete(listener);
  }

  private static register(tabId: string): void {
    if (!LxAppBrowserManager.openTabIds.includes(tabId)) {
      LxAppBrowserManager.openTabIds.push(tabId);
    }
  }

  private static markActive(tabId: string): void {
    try {
      browserTabActivate(tabId);
    } catch (error) {
      hilog.warn(DOMAIN, TAG, `browserTabActivate failed for tabId=${tabId}: ${error}`);
    }
  }

  private static notifyListeners(): void {
    LxAppBrowserManager.listeners.forEach((listener: () => void) => {
      try {
        listener();
      } catch (error) {
        hilog.error(DOMAIN, TAG, `Listener error: ${error}`);
      }
    });
  }

  private static closeRuntimeTab(tabId: string): void {
    if (!tabId) {
      return;
    }
    try {
      const closed: boolean = Boolean(browserTabClose(tabId));
      if (!closed) {
        hilog.warn(DOMAIN, TAG, `browserTabClose returned false for tabId=${tabId}`);
      }
    } catch (error) {
      hilog.error(DOMAIN, TAG, `browserTabClose failed for tabId=${tabId}: ${error}`);
    }
  }

  private static closeAllTabs(): void {
    LxAppBrowserManager.openTabIds.forEach((tabId: string) => {
      LxAppBrowserManager.closeRuntimeTab(tabId);
    });
    LxAppBrowserManager.openTabIds = [];
    LxAppBrowserManager.initialUrls.clear();
  }
}

interface BrowserTabRow {
  tabId: string;
  label: string;
  isActive: boolean;
}

/**
 * A live, displayable browser tab: its id plus the runtime-managed webview
 * controller and webtag that back it. Each entry renders its own `Web`
 * component; only the active entry is visible. Keeping every tab's `Web`
 * mounted (just hidden) preserves its page the way iOS keeps each tab's
 * WKWebView alive and only re-parents the active one.
 */
interface BrowserTabEntry {
  tabId: string;
  controller: webview.WebviewController;
  webTag: string;
}

/**
 * In-app browser component.
 *
 * Bottom two-row bar: row 1 = address pill (lock/warning icon + editable URL +
 * refresh); row 2 = back / forward / spacer / new-tab / tabs (with open count) /
 * overflow menu / close. A tab switcher and an overflow menu surface as in-place
 * layers.
 */
@Component
export struct LxAppBrowser {
  @State private isVisible: boolean = false;
  // The displayable tabs (one mounted `Web` each); only the active one shows.
  @State private tabEntries: Array<BrowserTabEntry> = [];
  @State private activeTabId: string = '';
  @State private visibleTabId: string = '';
  @State private url: string = '';
  @State private addressText: string = '';
  @State private addressEditing: boolean = false;
  @State private canGoBack: boolean = false;
  @State private canGoForward: boolean = false;
  // Aside chrome: the active tab was opened as an aside — hide the address
  // row and the new-tab/menu affordances; a row refresh appears instead.
  @State private isAsideActive: boolean = false;
  // Chrome-style history intervention: until the user interacts with a tab
  // (page touch or address navigation), auto-created history (SPA pushState
  // redirects) must not light back/forward.
  private interactedTabIds: Set<string> = new Set();
  @State private bottomSafeAreaHeight: number = 0;
  @State private topSafeAreaHeight: number = 0;
  @State private addressIconPath: Resource = $rawfile('icons/icon_lock.svg');
  @State private addressIconColor: string = '#666666';
  @State private addressIconVisible: boolean = false;
  @State private tabCount: number = 0;
	  @State private showTabSwitcher: boolean = false;
	  @State private showOverflowMenu: boolean = false;
	  @State private tabRows: Array<BrowserTabRow> = [];
  @State private keyboardOffsetVp: number = 0;

	  private listener: (() => void) | null = null;
	  private attachRetryTimerId: number | null = null;
	  private attachRetryCount: number = 0;
	  // Webtags whose `Web` component has already acked attachment to the runtime.
	  private ackedWebTags: Set<string> = new Set();
  private keyboardMonitorTimer: number | null = null;
  private keyboardMonitorActive: boolean = false;
  private keyboardWindow: window.Window | null = null;

  aboutToAppear() {
    this.listener = () => {
      const visible = LxAppBrowserManager.isShowing();
      this.isVisible = visible;
      this.tabCount = LxAppBrowserManager.getTabCount();
      if (!visible) {
        this.clearAttachRetryTimer();
        this.resetLocalState();
        return;
      }

      const nextActiveTabId = LxAppBrowserManager.getActiveTabId();
      const activeChanged = nextActiveTabId !== this.activeTabId;
      if (activeChanged) {
        // Keep the currently displayed WebView visible until the target tab's
        // managed WebView has resolved (mirrors iOS: no blank flash), but the
        // chrome state switches immediately — address and back/forward are
        // per-tab and must never show the previous tab's values.
        this.activeTabId = nextActiveTabId;
        this.isAsideActive = this.safeTabIsAside(nextActiveTabId);
        const initialUrl = LxAppBrowserManager.getInitialUrl(nextActiveTabId);
        this.url = initialUrl;
        if (!this.addressEditing) {
          this.updateAddressDisplay(initialUrl);
        }
        this.canGoBack = false;
        this.canGoForward = false;
      }

      // Re-resolve the open tabs against the runtime-managed webviews. New tabs
      // whose webview isn't ready yet are retried; the active tab is the one
      // whose `Web` becomes visible. On success this also refreshes the chrome
      // (address bar + nav) from the active tab's live controller.
      this.resolveTabEntriesWithRetry();
    };

	    LxAppBrowserManager.registerListener(this.listener!);
	    this.initSafeAreaInsets();
	    this.listener();
	  }

	  aboutToDisappear() {
	    this.clearAttachRetryTimer();
    this.stopKeyboardMonitor();
	    if (this.listener) {
	      LxAppBrowserManager.unregisterListener(this.listener!);
	      this.listener = null;
    }
  }

  private async initSafeAreaInsets(): Promise<void> {
    try {
      const windowClass: window.Window = await window.getLastWindow(getContext());
      const safeArea: window.AvoidArea = await windowClass.getWindowAvoidArea(window.AvoidAreaType.TYPE_SYSTEM);
      this.bottomSafeAreaHeight = safeArea.bottomRect.height || 0;
      this.topSafeAreaHeight = safeArea.topRect.height || 0;
      hilog.info(DOMAIN, TAG, `Safe area top=${this.topSafeAreaHeight} bottom=${this.bottomSafeAreaHeight}`);
    } catch (error) {
	      hilog.error(DOMAIN, TAG, `Failed to get safe area insets: ${error}`);
	    }
	  }

  private startKeyboardMonitor(): void {
    if (this.keyboardMonitorActive) {
      return;
    }
    this.keyboardMonitorActive = true;
    const tick = async () => {
      if (!this.keyboardMonitorActive) {
        return;
      }
      await this.refreshKeyboardOffset();
      if (!this.keyboardMonitorActive) {
        return;
      }
      this.keyboardMonitorTimer = setTimeout(() => {
        tick();
      }, KEYBOARD_MONITOR_INTERVAL_MS);
    };
    void tick();
  }

  private stopKeyboardMonitor(): void {
    this.keyboardMonitorActive = false;
    if (this.keyboardMonitorTimer !== null) {
      clearTimeout(this.keyboardMonitorTimer);
      this.keyboardMonitorTimer = null;
    }
    this.keyboardOffsetVp = 0;
  }

  private async refreshKeyboardOffset(): Promise<void> {
    if (!this.isVisible) {
      if (this.keyboardOffsetVp !== 0) {
        this.keyboardOffsetVp = 0;
      }
      return;
    }
    try {
      let targetWindow = this.keyboardWindow;
      if (!targetWindow) {
        targetWindow = await window.getLastWindow(getContext());
        this.keyboardWindow = targetWindow;
      }
      const avoidArea = await targetWindow.getWindowAvoidArea(window.AvoidAreaType.TYPE_KEYBOARD);
      const bottomHeight = Number(avoidArea.bottomRect?.height ?? 0);
      const nextOffset = this.normalizeKeyboardOffset(bottomHeight);
      if (Math.abs(nextOffset - this.keyboardOffsetVp) >= 1) {
        this.keyboardOffsetVp = nextOffset;
      }
    } catch (_error) {
      if (this.keyboardOffsetVp !== 0) {
        this.keyboardOffsetVp = 0;
      }
    }
  }

  private normalizeKeyboardOffset(rawHeight: number): number {
    const raw = Number(rawHeight) || 0;
    if (raw <= 0) {
      return 0;
    }
    const metrics = this.readDisplayMetrics();
    let offsetVp = raw;
    if (metrics.density > 0 && raw > metrics.heightVp * 0.65) {
      offsetVp = raw / metrics.density;
    }
    const maxOffset = metrics.heightVp > 0 ? metrics.heightVp * 0.48 : 420;
    return Math.max(0, Math.round(Math.min(offsetVp, maxOffset)));
  }

  private readDisplayMetrics(): DisplayMetrics {
    try {
      const displayInfo = display.getDefaultDisplaySync();
      const density = displayInfo.densityPixels || 1;
      return {
        heightVp: Math.max(1, displayInfo.height / density),
        density: density,
      };
    } catch (_error) {
      return {
        heightVp: 0,
        density: 1,
      };
    }
  }

  /**
   * Resolve a single open tab id to its runtime-managed webview controller and
   * webtag, or null when the webview hasn't been created yet.
   */
  private resolveTabEntry(tabId: string): BrowserTabEntry | null {
    if (!tabId) {
      return null;
    }

    let appId = '';
    let path = '';
    try {
      appId = String(getBuiltinBrowserAppId() ?? '').trim();
      path = String(browserTabPathForId(tabId) ?? '').trim();
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to resolve browser app/path for tabId=${tabId}: ${error}`);
      return null;
    }

    if (!appId || !path) {
      return null;
    }

    const managedController = WebViewManager.findWebview(appId, path);
    if (!managedController) {
      return null;
    }

    let resolvedWebTag = '';
    const infos: WebViewInfo[] = WebViewManager.getAllWebViewInfos();
    for (let i = 0; i < infos.length; i++) {
      if (infos[i].controller === managedController) {
        resolvedWebTag = infos[i].webtag;
        break;
      }
    }
    if (!resolvedWebTag) {
      hilog.warn(DOMAIN, TAG, `resolveTabEntry: cannot resolve webtag for tabId=${tabId}`);
      return null;
    }

    return { tabId, controller: managedController, webTag: resolvedWebTag };
  }

  /**
   * Rebuild the displayable tab entries from the manager's open tabs. Each
   * resolved tab keeps its own mounted `Web`; the active tab is the visible one.
   * If the active tab's managed webview isn't ready yet, retry until it is.
   */
  private resolveTabEntriesWithRetry(): void {
    if (!this.isVisible) {
      return;
    }

    const openTabIds = LxAppBrowserManager.getOpenTabIds();
    const entries: Array<BrowserTabEntry> = [];
    let activeResolved = false;
    for (let i = 0; i < openTabIds.length; i++) {
      const tabId = openTabIds[i];
      const entry = this.resolveTabEntry(tabId);
      if (entry) {
        entries.push(entry);
        if (tabId === this.activeTabId) {
          activeResolved = true;
        }
      }
    }

    // Only replace the bound array when the resolved set actually changed.
    // Reassigning an equivalent array would re-diff the `ForEach` of `Web`
    // components and repaint the whole Stack — including the bottom bar — which
    // reads as a flicker on a same-tab link navigation.
    if (!this.sameTabEntries(this.tabEntries, entries)) {
      this.tabEntries = entries;
    }

    if (activeResolved) {
      this.clearAttachRetryTimer();
      this.attachRetryCount = 0;
      if (this.visibleTabId !== this.activeTabId) {
        this.visibleTabId = this.activeTabId;
      }
      this.updateActiveState();
      return;
    }

    // The active tab's webview hasn't materialized yet: retry, and only give up
    // (dropping just that tab) after exhausting the budget.
    if (this.attachRetryCount >= ATTACH_MAX_RETRIES) {
      hilog.error(DOMAIN, TAG, `show failed: managed WebView not found for tabId=${this.activeTabId}`);
      this.clearAttachRetryTimer();
      this.attachRetryCount = 0;
      LxAppBrowserManager.closeTab(this.activeTabId);
      return;
    }

    this.attachRetryCount += 1;
    const expectedTabId = this.activeTabId;
    this.clearAttachRetryTimer();
    this.attachRetryTimerId = setTimeout(() => {
      this.attachRetryTimerId = null;
      if (!this.isVisible || this.activeTabId !== expectedTabId) {
        return;
      }
      this.resolveTabEntriesWithRetry();
    }, ATTACH_RETRY_DELAY_MS);
  }

  /**
   * Two resolved tab-entry sets are equivalent when they hold the same tab ids
   * (in order) each backed by the same managed controller and webtag. Used to
   * avoid reassigning the bound array — and so re-rendering the Stack — when a
   * navigation event re-resolves to the same live webviews.
   */
  private sameTabEntries(a: Array<BrowserTabEntry>, b: Array<BrowserTabEntry>): boolean {
    if (a.length !== b.length) {
      return false;
    }
    for (let i = 0; i < a.length; i++) {
      if (a[i].tabId !== b[i].tabId
        || a[i].controller !== b[i].controller
        || a[i].webTag !== b[i].webTag) {
        return false;
      }
    }
    return true;
  }

  /**
   * The runtime-managed controller backing the active tab, or null when the
   * active tab has no resolved webview yet.
   */
  private activeController(): webview.WebviewController | null {
    for (let i = 0; i < this.tabEntries.length; i++) {
      if (this.tabEntries[i].tabId === this.activeTabId) {
        return this.tabEntries[i].controller;
      }
    }
    return null;
  }

  private clearAttachRetryTimer(): void {
    if (this.attachRetryTimerId !== null) {
      clearTimeout(this.attachRetryTimerId);
      this.attachRetryTimerId = null;
    }
  }

  private resetLocalState(): void {
    this.tabEntries = [];
    this.activeTabId = '';
    this.visibleTabId = '';
    this.url = '';
    this.addressText = '';
    this.addressEditing = false;
    this.canGoBack = false;
    this.canGoForward = false;
    this.attachRetryCount = 0;
    this.ackedWebTags.clear();
    this.showTabSwitcher = false;
    this.showOverflowMenu = false;
    this.tabRows = [];
    this.addressIconPath = $rawfile('icons/icon_lock.svg');
    this.addressIconColor = '#666666';
    this.addressIconVisible = false;
    this.isAsideActive = false;
    this.interactedTabIds.clear();
  }

	  build() {
	    if (this.isVisible && this.tabEntries.length > 0) {
	      Stack({ alignContent: Alignment.Bottom }) {
	        // One `Web` per open tab, each bound to its own runtime-managed
	        // controller. Only the active tab is visible; the rest stay mounted
	        // (hidden) so their pages survive a switch, mirroring iOS keeping every
	        // tab's WKWebView alive and re-parenting only the active one.
        Column() {
          Stack() {
            ForEach(this.tabEntries, (entry: BrowserTabEntry) => {
              this.TabWeb(entry)
            }, (entry: BrowserTabEntry) => entry.tabId)
          }
          .width('100%')
          .layoutWeight(1)

          Blank()
            .width('100%')
            .height(this.contentBottomInset())
        }
        .width('100%')
        .height('100%')

	        // The tab switcher layer sits above the page, below the bar.
	        if (this.showTabSwitcher) {
          this.TabSwitcher();
        }

	        this.BottomBar();

        // The overflow menu is an overlay layer over the bar so opening it
        // never reflows or shifts the bottom bar.
	        if (this.showOverflowMenu) {
	          this.OverflowMenu();
	        }
      }
      .width('100%')
      .height('100%')
      .padding({ top: px2vp(this.topSafeAreaHeight) })
      .backgroundColor(Color.White)
      .expandSafeArea([SafeAreaType.SYSTEM], [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM])
    }
  }

  @Builder
  TabWeb(entry: BrowserTabEntry) {
    Web({ src: '', controller: entry.controller })
      .width('100%')
      .height('100%')
      .visibility(entry.tabId === (this.visibleTabId || this.activeTabId) ? Visibility.Visible : Visibility.Hidden)
      // Observe-only: the first touch marks the tab as user-interacted for
      // the back/forward history gate; the event passes through to the page.
      .onTouch((event: TouchEvent) => {
        if (event.type === TouchType.Down && entry.tabId === this.activeTabId) {
          this.markActiveTabInteracted();
        }
      })
      .javaScriptAccess(true)
      .domStorageAccess(true)
      .mixedMode(MixedMode.Compatible)
      // Surface target=_blank / window.open as our own managed tab.
      .multiWindowAccess(true)
      .allowWindowOpenMethod(true)
      .onWindowNew((event) => {
        if (!event) {
          return;
        }
        // Decline the native popup window: we don't host an ArkWeb child
        // window, the target is opened as one of our own managed tabs.
        try {
          event.handler.setWebController(null);
        } catch (error) {
          hilog.warn(DOMAIN, TAG, `onWindowNew: setWebController(null) failed: ${error}`);
        }
        this.openTargetInNewTab(event.targetUrl);
      })
      .onAppear(() => {
        if (!this.ackedWebTags.has(entry.webTag)) {
          notifyWebViewControllerAttached(entry.webTag);
          this.ackedWebTags.add(entry.webTag);
        }
      })
      .onLoadIntercept((event) => {
        if (!event || !event.data) {
          return false;
        }
        const request = event.data;
        const targetUrl = request.getRequestUrl();
        if (!targetUrl) {
          return false;
        }

        let hasUserGesture = false;
        let isMainFrame = true;
        try {
          hasUserGesture = request.isRequestGesture();
        } catch {}
        try {
          isMainFrame = request.isMainFrame();
        } catch {}

        const policy: BrowserNavigationPolicyResponse | null = resolveBrowserNavigationPolicy(
          targetUrl,
          hasUserGesture,
          isMainFrame
        );
        if (!policy) {
          return true;
        }
        if (policy.decision === 'in_webview') {
          return false;
        }
        if (policy.decision === 'open_external') {
          Lingxia.launchWithUrl(targetUrl);
        }
        return true;
      })
      .onShowFileSelector((event) => {
        if (!event) {
          return false;
        }
        return requestWebFileChooser(
          entry.webTag,
          (selectedPaths: Array<string>) => {
            event.result.handleFileList(selectedPaths);
          },
          event.fileSelector.getAcceptType() ?? [],
          event.fileSelector.getMode(),
          event.fileSelector.isCapture(),
          this.url
        );
      })
      .onPageEnd(() => {
        if (entry.tabId === this.activeTabId) {
          this.updateActiveState();
        }
        if (this.showTabSwitcher) {
          this.tabRows = this.buildTabRows();
        }
      })
      .onProgressChange((event) => {
        if (event && event.newProgress > 30 && entry.tabId === this.activeTabId) {
          this.updateActiveState();
        }
      })
  }

  @Builder
  ToolbarPlusIcon(color: string) {
    Stack({ alignContent: Alignment.Center }) {
      Row()
        .width(14)
        .height(2)
        .backgroundColor(color)
        .borderRadius(1)
      Row()
        .width(2)
        .height(14)
        .backgroundColor(color)
        .borderRadius(1)
    }
    .width(20)
    .height(20)
  }

  @Builder
  ToolbarMenuIcon(color: string) {
    Column({ space: 3 }) {
      Row()
        .width(16)
        .height(2)
        .backgroundColor(color)
        .borderRadius(1)
      Row()
        .width(16)
        .height(2)
        .backgroundColor(color)
        .borderRadius(1)
      Row()
        .width(16)
        .height(2)
        .backgroundColor(color)
        .borderRadius(1)
    }
    .width(20)
    .height(20)
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
  }

	  @Builder
	  BottomBar() {
    // Full-width translucent material panel pinned to the very bottom; its blur
    // extends behind the home indicator so the bottom edge blends instead of
    // exposing a strip of page content. A hairline top border separates it from
    // the page above.
    Column() {
      Divider().color('#E0E0E0').strokeWidth(0.5).width('100%')

      Column() {
        // Address row: editable URL pill. An aside tab hides it — asides are
        // API-opened content with no user navigation.
        if (!this.isAsideActive) {
        Row() {
          Image(this.addressIconPath)
            .width(16)
            .height(16)
            .fillColor(this.addressIconColor)
            .visibility((this.addressEditing || !this.addressIconVisible) ? Visibility.Hidden : Visibility.Visible)
            .margin({ left: 12, right: 6 })

          TextInput({ text: this.addressText, placeholder: 'Search or enter address' })
            .layoutWeight(1)
            .fontSize(13)
            .fontColor('#333333')
            .placeholderColor('#999999')
            .backgroundColor(Color.Transparent)
            .borderRadius(0)
            .padding({ left: 0, right: 4 })
            .type(InputType.URL)
            .enterKeyType(EnterKeyType.Go)
            .maxLines(1)
            .onChange((value: string) => {
              this.addressText = value;
            })
            .onFocus(() => {
              this.beginAddressEditing();
            })
            .onBlur(() => {
              this.endAddressEditing();
            })
            .onSubmit(() => {
              this.submitAddressInput();
            })

          Image($rawfile('icons/icon_browser_refresh.svg'))
            .width(16)
            .height(16)
            .fillColor('#666666')
            .padding(8)
            .width(32)
            .height(32)
            .margin({ right: 4 })
            .onClick(() => {
              const controller = this.activeController();
              if (!controller) {
                return;
              }
              try {
                controller.refresh();
              } catch (error) {
                hilog.warn(DOMAIN, TAG, `refresh failed: ${error}`);
              }
            })
        }
        .width('100%')
        .height(34)
        .backgroundColor('#F0F0F0')
        .borderRadius(18)
        .alignItems(VerticalAlign.Center)
        }

        // Action row: back / forward / spacer / new-tab / tabs / menu / close.
        Row() {
          Image($rawfile('icons/icon_back.svg'))
            .width(20)
            .height(20)
            .fillColor(this.canGoBack ? '#333333' : '#CCCCCC')
            .padding(8)
            .width(40)
            .height(36)
            .enabled(this.canGoBack)
            .onClick(() => {
              const controller = this.activeController();
              if (!controller) {
                return;
              }
              try {
                controller.backward();
              } catch (error) {
                hilog.warn(DOMAIN, TAG, `backward failed: ${error}`);
              }
            })

          Image($rawfile('icons/icon_forward.svg'))
            .width(20)
            .height(20)
            .fillColor(this.canGoForward ? '#333333' : '#CCCCCC')
            .padding(8)
            .width(40)
            .height(36)
            .enabled(this.canGoForward)
            .onClick(() => {
              const controller = this.activeController();
              if (!controller) {
                return;
              }
              try {
                controller.forward();
              } catch (error) {
                hilog.warn(DOMAIN, TAG, `forward failed: ${error}`);
              }
            })

          // Aside chrome has no address pill, so refresh moves into the row.
          if (this.isAsideActive) {
            Image($rawfile('icons/icon_browser_refresh.svg'))
              .width(18)
              .height(18)
              .fillColor('#333333')
              .padding(8)
              .width(40)
              .height(36)
              .onClick(() => {
                const controller = this.activeController();
                if (!controller) {
                  return;
                }
                try {
                  controller.refresh();
                } catch (error) {
                  hilog.warn(DOMAIN, TAG, `refresh failed: ${error}`);
                }
              })
          }

          Blank()

          // New-tab affordance (asides are API-opened; user tabs are self mode).
          if (!this.isAsideActive) {
            Stack({ alignContent: Alignment.Center }) {
              this.ToolbarPlusIcon('#333333')
            }
            .width(40)
            .height(36)
            .onClick(() => {
              this.closeMenus();
              LxAppBrowserManager.openNewTab();
            })
          }

          // Tabs button: draw the numbered overlapping-square icon directly.
          // Harmony can render stroke-only SVGs as solid masks when fillColor is
          // applied, so this mirrors the iOS icon shape without SVG.
          Stack() {
            Stack() {
              Text('')
                .width(14)
                .height(14)
                .border({ width: 1.8, color: '#333333' })
                .borderRadius(3)
                .translate({ x: 4.5, y: -4.5 })
              Text(this.tabCountLabel())
                .width(16)
                .height(16)
                .fontSize(10)
                .fontWeight(FontWeight.Medium)
                .fontColor('#333333')
                .textAlign(TextAlign.Center)
                .border({ width: 1.8, color: '#333333' })
                .borderRadius(3)
                .backgroundColor('#F5F5F7')
                .translate({ x: -1.5, y: 1.5 })
            }
            .width(24)
            .height(24)
          }
          .width(40)
          .height(36)
          .onClick(() => {
            this.toggleTabSwitcher();
          })

          // Overflow menu (downloads / settings) — self tabs only.
          if (!this.isAsideActive) {
            Stack({ alignContent: Alignment.Center }) {
              this.ToolbarMenuIcon('#333333')
            }
            .width(40)
            .height(36)
            .onClick(() => {
              this.toggleOverflowMenu();
            })
          }

          Image($rawfile('icons/icon_close_x.svg'))
            .width(20)
            .height(20)
            .fillColor('#333333')
            .padding(8)
            .width(40)
            .height(36)
            .onClick(() => {
              LxAppBrowserManager.hide();
            })
        }
        .width('100%')
        .height(36)
        .margin({ top: 4 })
        .alignItems(VerticalAlign.Center)
      }
      .width('100%')
      .padding({
        left: 8,
        right: 8,
        top: 6,
        bottom: px2vp(this.bottomSafeAreaHeight) + 10
      })
    }
    .width('100%')
    .backgroundColor('#F5F5F7')
	  }

  /**
   * Total height of the bottom bar (address row + action row + paddings),
   * so the overflow overlay can float its menu card just above the bar.
   */
	  private bottomBarHeight(): number {
    // hairline 0.5 + top padding 6 + address row 34 + action margin 4 +
    // action row 36 + bottom padding (safe area + 10).
	    return 0.5 + 6 + 34 + 4 + 36 + px2vp(this.bottomSafeAreaHeight) + 10;
	  }

  private contentBottomInset(): number {
    return this.bottomBarHeight();
  }

  private tabCountLabel(): string {
    if (this.tabCount > 99) {
      return '99+';
    }
    return `${Math.max(0, this.tabCount)}`;
  }

  @Builder
  OverflowMenu() {
    Column() {
      // A tap anywhere outside the menu card dismisses it.
      Column()
        .layoutWeight(1)
        .width('100%')
        .onClick(() => {
          this.closeMenus();
        })

      Row() {
        Blank()
        Column() {
          Row() {
            Image($rawfile('icons/icon_browser_download.svg'))
              .width(18)
              .height(18)
              .margin({ right: 10 })
            Text('Downloads')
              .fontSize(15)
              .fontColor('#333333')
          }
          .width('100%')
          .height(48)
          .padding({ left: 16, right: 16 })
          .alignItems(VerticalAlign.Center)
          .onClick(() => {
            this.closeMenus();
            LxAppBrowserManager.navigateActiveTab(DOWNLOADS_URL);
          })

          Divider().color('#11000000')

          Row() {
            Image($rawfile('icons/icon_browser_settings.svg'))
              .width(18)
              .height(18)
              .margin({ right: 10 })
            Text('Settings')
              .fontSize(15)
              .fontColor('#333333')
          }
          .width('100%')
          .height(48)
          .padding({ left: 16, right: 16 })
          .alignItems(VerticalAlign.Center)
          .onClick(() => {
            this.closeMenus();
            LxAppBrowserManager.navigateActiveTab(SETTINGS_URL);
          })
        }
        .width('60%')
        .backgroundColor(Color.White)
        .borderRadius(12)
        .border({ width: 0.5, color: '#14000000' })
        .shadow({ radius: 18, color: '#22000000', offsetX: 0, offsetY: 4 })
      }
      .width('100%')
	      .padding({ left: 12, right: 12, bottom: this.contentBottomInset() + 6 })
    }
    .width('100%')
    .height('100%')
  }

  @Builder
  TabSwitcher() {
    Column() {
      // A tap on the scrim dismisses the switcher.
      Column()
        .layoutWeight(1)
        .width('100%')
        .onClick(() => {
          this.toggleTabSwitcher();
        })

      Column() {
        Row() {
          Text('Tabs')
            .fontSize(17)
            .fontWeight(FontWeight.Medium)
            .fontColor('#1A1A1A')
            .layoutWeight(1)
          Stack({ alignContent: Alignment.Center }) {
            this.ToolbarPlusIcon('#333333')
          }
            .width(40)
            .height(36)
            .onClick(() => {
              this.toggleTabSwitcher();
              LxAppBrowserManager.openNewTab();
            })
        }
        .width('100%')
        .height(52)
        .padding({ left: 20, right: 12 })
        .alignItems(VerticalAlign.Center)

        Divider().color('#11000000')

        List() {
          ForEach(this.tabRows, (row: BrowserTabRow) => {
            ListItem() {
              Row() {
                Text(row.label)
                  .layoutWeight(1)
                  .fontSize(15)
                  .fontWeight(row.isActive ? FontWeight.Medium : FontWeight.Normal)
                  .fontColor(row.isActive ? '#1A1A1A' : '#4D4D4D')
                  .maxLines(1)
                  .textOverflow({ overflow: TextOverflow.Ellipsis })
                Image($rawfile('icons/icon_close_x.svg'))
                  .width(16)
                  .height(16)
                  .fillColor('#666666')
                  .padding(10)
                  .width(36)
                  .height(36)
                  .onClick(() => {
                    this.closeTabFromSwitcher(row.tabId);
                  })
              }
              .width('100%')
              .height(56)
              .padding({ left: 20, right: 16 })
              .alignItems(VerticalAlign.Center)
              .onClick(() => {
                this.activateTabFromSwitcher(row.tabId);
              })
            }
          }, (row: BrowserTabRow) => row.tabId)
        }
        .layoutWeight(1)
        .width('100%')
      }
      .width('100%')
      .height('60%')
      .backgroundColor(Color.White)
      .borderRadius({ topLeft: 16, topRight: 16 })
      .padding({ bottom: px2vp(this.bottomSafeAreaHeight) })
    }
    .width('100%')
    .height('100%')
    .backgroundColor('#66000000')
  }

  private beginAddressEditing(): void {
    this.addressEditing = true;
    this.closeMenus();
    // Reveal the full URL for editing, except on a blank new tab.
    if (this.url && !browserUrlIsHidden(this.url)) {
      this.addressText = this.url;
    } else {
      this.addressText = '';
    }
  }

  private endAddressEditing(): void {
    this.addressEditing = false;
    this.updateAddressDisplay(this.url);
  }

  private submitAddressInput(): void {
    const target = resolveAddressInput(this.addressText);
    if (target) {
      // An address-bar navigation is a user interaction.
      this.markActiveTabInteracted();
      LxAppBrowserManager.navigateActiveTab(target);
    }
    this.addressEditing = false;
    this.updateAddressDisplay(this.url);
  }

  private markActiveTabInteracted(): void {
    if (!this.activeTabId || this.interactedTabIds.has(this.activeTabId)) {
      return;
    }
    this.interactedTabIds.add(this.activeTabId);
    this.updateActiveState();
  }

  private safeTabIsAside(tabId: string): boolean {
    if (!tabId) {
      return false;
    }
    try {
      return browserTabIsAside(tabId);
    } catch (error) {
      hilog.warn(DOMAIN, TAG, `browserTabIsAside failed: ${error}`);
      return false;
    }
  }

  /**
   * Surface a target=_blank / window.open request as a new managed tab and
   * switch to it (matching iOS). The runtime opens our own browser tab; the
   * native popup the WebView wanted is declined by the caller.
   */
  private openTargetInNewTab(rawUrl: string): void {
    const targetUrl = String(rawUrl ?? '').trim();
    if (!targetUrl) {
      return;
    }

    let appId = '';
    try {
      appId = String(getBuiltinBrowserAppId() ?? '').trim();
    } catch (error) {
      hilog.error(DOMAIN, TAG, `onWindowNew: getBuiltinBrowserAppId threw: ${error}`);
      return;
    }
    if (!appId) {
      hilog.error(DOMAIN, TAG, 'onWindowNew: no builtin browser app id');
      return;
    }

    const sessionId = Number(getLxappSessionId(appId));
    if (!Number.isFinite(sessionId) || sessionId <= 0) {
      hilog.error(DOMAIN, TAG, 'onWindowNew: no session for builtin browser');
      return;
    }

    let newTabId: string | null = null;
    try {
      // A tab spawned from an aside page stays an aside (no address bar);
      // self tabs keep spawning self tabs.
      newTabId = this.isAsideActive
        ? openAsideBrowserTab(appId, Math.trunc(sessionId), targetUrl)
        : openBrowserTab(appId, Math.trunc(sessionId), targetUrl);
    } catch (error) {
      hilog.error(DOMAIN, TAG, `onWindowNew: openBrowserTab threw: ${error}`);
      return;
    }
    if (!newTabId || newTabId.trim().length === 0) {
      hilog.error(DOMAIN, TAG, 'onWindowNew: runtime returned no tab id');
      return;
    }

    this.closeMenus();
    LxAppBrowserManager.show(newTabId, targetUrl);
  }

  private toggleTabSwitcher(): void {
    this.showOverflowMenu = false;
    const next = !this.showTabSwitcher;
    if (next) {
      this.tabRows = this.buildTabRows();
    }
    this.showTabSwitcher = next;
  }

  private toggleOverflowMenu(): void {
    this.showTabSwitcher = false;
    this.showOverflowMenu = !this.showOverflowMenu;
  }

  private closeMenus(): void {
    this.showOverflowMenu = false;
    this.showTabSwitcher = false;
  }

  private activateTabFromSwitcher(tabId: string): void {
    this.showTabSwitcher = false;
    LxAppBrowserManager.activate(tabId);
  }

  private closeTabFromSwitcher(tabId: string): void {
    LxAppBrowserManager.closeTab(tabId);
    if (LxAppBrowserManager.getTabCount() === 0) {
      this.showTabSwitcher = false;
      return;
    }
    this.tabRows = this.buildTabRows();
  }

  /**
   * Resolve a display label for a tab from its managed webview title/host,
   * falling back to a generic label so the switcher always has something to show.
   */
  private buildTabRows(): Array<BrowserTabRow> {
    const activeTabId = LxAppBrowserManager.getActiveTabId();
    return LxAppBrowserManager.getOpenTabIds().map((tabId: string): BrowserTabRow => {
      return {
        tabId,
        label: this.resolveTabLabel(tabId),
        isActive: tabId === activeTabId,
      };
    });
  }

  private resolveTabLabel(tabId: string): string {
    try {
      const appId = String(getBuiltinBrowserAppId() ?? '').trim();
      const path = String(browserTabPathForId(tabId) ?? '').trim();
      if (appId && path) {
        const managedController = WebViewManager.findWebview(appId, path);
        if (managedController) {
          const title = String(managedController.getTitle() ?? '').trim();
          if (title) {
            return title;
          }
          const currentUrl = String(managedController.getUrl() ?? '').trim();
          if (currentUrl && !browserUrlIsHidden(currentUrl)) {
            const display = browserUrlDisplay(currentUrl);
            if (display.text) {
              return display.text;
            }
          }
        }
      }
    } catch (error) {
      hilog.warn(DOMAIN, TAG, `resolveTabLabel failed for tabId=${tabId}: ${error}`);
    }
    return 'New Tab';
  }

  /**
   * Pull url + nav state from the active tab's managed controller into the
   * chrome (address bar / back / forward), mirroring iOS updateAddressBar +
   * updateNavigationButtons after a swap or page load.
   */
  private updateActiveState(): void {
    const controller = this.activeController();
    if (!controller) {
      return;
    }
    try {
      // Refresh url + nav state in place, writing each @State only when the
      // value actually changed. onProgressChange fires this repeatedly while a
      // page loads; reassigning identical @State each time would repaint the
      // address bar and make it flicker on a same-tab link navigation.
      const currentUrl = controller.getUrl();
      if (currentUrl && currentUrl !== this.url) {
        this.url = currentUrl;
        if (!this.addressEditing) {
          this.updateAddressDisplay(currentUrl);
        }
      }
      // Pre-interaction history is auto-created (redirects/pushState) and
      // must not light the affordances.
      const interacted = this.interactedTabIds.has(this.activeTabId);
      const canGoBack = controller.accessBackward() && interacted;
      if (canGoBack !== this.canGoBack) {
        this.canGoBack = canGoBack;
      }
      const canGoForward = controller.accessForward() && interacted;
      if (canGoForward !== this.canGoForward) {
        this.canGoForward = canGoForward;
      }
    } catch (error) {
      hilog.warn(DOMAIN, TAG, `updateActiveState failed: ${error}`);
    }
  }

  private updateAddressDisplay(rawUrl: string): void {
    const display = browserUrlDisplay(rawUrl);
    // Only touch the @State when the resolved display changed: keeps repeated
    // calls during a load from repainting an unchanged address pill.
    if (display.text === this.addressText
      && display.iconColor === this.addressIconColor
      && display.iconVisible === this.addressIconVisible) {
      return;
    }
    this.addressText = display.text;
    this.addressIconPath = display.iconPath;
    this.addressIconColor = display.iconColor;
    this.addressIconVisible = display.iconVisible;
  }
}
