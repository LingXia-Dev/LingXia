import { hilog } from '@kit.PerformanceAnalysisKit';
import { LxLog } from '../app/LxLog';
import { webview } from '@kit.ArkWeb';
import { window, display, FrameCallback } from '@kit.ArkUI';
import { common, ConfigurationConstant } from '@kit.AbilityKit';
import { LxApp } from './LxApp';
import { Lingxia } from '../app/Lingxia';
import { LxAppSurface } from './APIs/LxAppSurface';
import {
  findWebview,
  toWebTag,
  extractWebTag,
  addWebViewUiCallback,
  removeWebViewUiCallback,
  WebViewInfo,
  getAllWebViewInfos,
  pauseWebViewComponents,
  resumeWebViewComponents,
  getWebviewGeneration
} from './WebViewBridge';
import { LingXiaWebView } from './LingXiaWebView';
import { NavigationBar, NavigationBarState, NAVBAR_TOTAL_HEIGHT, isColorDark } from './NavigationBar';
import { NavigationButton, CapsuleButton } from './LxAppDrawables';
import { TabBar, tabBarExtraBottomHeight } from './TabBar';
import { TabBarPosition, TabBarItem, UiEventType } from 'liblingxia.so';
import liblingxia, { TabBarState, getNavigationBarState, isPullDownRefreshEnabled, onPageShow, onLxappEvent, setSurfaceViewport, setSurfaceWidth, pageBackgroundColor, syncLxappHostUI } from 'liblingxia.so';
import { notePageOrigin, setCapsuleGeometryListener } from './capsuleGeometry';
import { NAVIGATION_ACTION_BACK, NAVIGATION_ACTION_HOME, CAPSULE_ACTION_CLOSE } from '../app/NativeBridge';
import { AnimationType } from './LxAppNavigation';
import { LxNativeVideoPlayer } from './APIs/media/player/LxNativeVideoPlayer';
import { LxFullscreenVideoOverlay } from './APIs/media/player/LxFullscreenVideoOverlay';
import { RefreshHeader } from './RefreshHeader';
import { CapsuleMenuManager, CapsuleMenuBottomSheetComponent } from './CapsuleMenuBottomSheet';

// Container-specific constants
const SYSTEM_STATUS_BAR_HEIGHT = 44;     // Default status bar height
const TAB_BAR_HORIZONTAL_HEIGHT = 72;    // Match TabBar default height

const COLOR_BLACK = '#000000';
const COLOR_TRANSPARENT = Color.Transparent;

const canvasColorCache: Map<boolean, ResourceColor> = new Map<boolean, ResourceColor>();

/**
 * The colour the canvas behind the page should be, or transparent when the
 * host declares no page floor.
 *
 * The web stack is transparent all the way down, so anything the page does not
 * cover falls through to the ability window — which is the system default
 * white, in both appearances. That shows wherever native chrome borders the
 * page: the strip a pull-to-refresh opens above it most visibly. Painting the
 * host's declared page floor there makes the strip read as part of the page.
 */
function resolveCanvasColor(dark: boolean): ResourceColor {
  // Memoized per appearance: this is read from a @Builder, which ArkUI re-runs
  // on every state change around it, and the answer comes from app config that
  // cannot change while the process lives. "No colour declared" is memoized
  // too — it is the default, and the common case. Only the pre-runtime throw
  // stays unmemoized, so an early call cannot poison the slot.
  const cached: ResourceColor | undefined = canvasColorCache.get(dark);
  if (cached !== undefined) {
    return cached;
  }
  try {
    const declared: string = pageBackgroundColor(dark);
    const answer: ResourceColor = declared.length > 0 ? declared : COLOR_TRANSPARENT;
    canvasColorCache.set(dark, answer);
    return answer;
  } catch (_error) {
    // Runtime not up yet; the page paints over this almost immediately.
    return COLOR_TRANSPARENT;
  }
}

const POPUP_MASK_COLOR = 'rgba(0,0,0,0.45)';
const OVERLAY_MASK_FADE_DURATION = 180;

const ANIMATION_DURATION_STANDARD = 300;      // Standard animation duration
const ANIMATION_CURVE_EASE_IN_OUT = Curve.EaseInOut;  // Smooth ease in/out curve
const KEYBOARD_MONITOR_INTERVAL_MS = 80;

const DOMAIN = 0x0000;
const TAG = 'LingXia.Container';

interface RenderableWebEntry {
  webTag: string;
  controller: webview.WebviewController;
  // Native creation generation: part of the ForEach key so a same-tag
  // successor rebuilds the Web component. ArkUI would otherwise reuse the
  // component, whose Web keeps its original (destroyed) controller bound and
  // whose one-shot onAppear ack never fires for the new controller — the
  // runtime then waits forever and the page stays webview-less.
  generation: string;
}

@Component
export struct LxAppContainer {
  @Prop appId: string
  @Prop initialPath: string
  @Prop navigationMode: boolean = false
  @Prop closeCallback: string = ''
  @State path: string = ''

  // Static registry for TabBar updates (simple and direct)
  private static activeContainers: Map<string, LxAppContainer> = new Map()
  // Scheme of the lxapp currently on screen. Overlays that are mounted above
  // the containers (modal, action sheet) have no appId of their own, so they
  // read the foreground scheme from here.
  private static currentAppearanceDark: boolean = false
  private static appearanceByApp: Map<string, boolean> = new Map()

  // Static registry for animation type information
  private static animationTypes: Map<string, AnimationType> = new Map()

  @State pageConfig: NavigationBarState | null = null
  @State tabBarState: TabBarState | null = null
  @State selectedTabIndex: number = 0
  @State tabBarRefreshKey: number = 0  // Force TabBar refresh when data changes
  @State navbarRefreshKey: number = 0  // Force NavigationBar refresh when data changes
  @State appearanceDark: boolean = false
  @State webViewComponents: Map<string, webview.WebviewController> = new Map() // Store Web components for each WebView
  @State currentWebTag: string = '' // Current active WebView tag
  @State webViewKeys: string[] = [] // Array of WebView keys for ForEach
  @State presentationOverlayWebTag: string = '' // WebView tag currently displayed in presentation (may include #instance suffix)
  @State private presentationPreviousWebTag: string = '' // Previously focused WebView before presentation took focus
  @State overlayActive: boolean = false
  @State overlayWidthVp: number = 0
  @State overlayHeightVp: number = 0
  @State overlayPosition: number = 0 // 0 center, 1 bottom, 2 left, 3 right
  @State overlayBottomInset: number = 0
  @State overlayFullWidth: boolean = false
  @State overlayFullHeight: boolean = false
  @State private overlayMaskOpacity: number = 0
  @State private placeholderControllers: Map<string, webview.WebviewController> = new Map()
  @State private keepAliveTags: string[] = [] // Keep WebViews hidden (not removed) to avoid destroy
  private pageShowTriggered: Set<string> = new Set() // Track which pages have triggered onPageShow
  @State private statusBarHeight: number = SYSTEM_STATUS_BAR_HEIGHT // Default status bar height
  // Screen-space Y of the container root (vp); >0 when the window manager
  // already shifted the page below the system bar.
  @State private containerTopVp: number = 0
  // Home-indicator gesture inset (vp); lifts TabBar items out of the zone
  @State private bottomIndicatorInset: number = 0

  // Animation state for WebView transitions
  @State private animationInProgress: boolean = false
  @State private previousWebTag: string = ''
  @State private currentAnimationType: AnimationType = AnimationType.None
  @State private currentWebViewTranslateX: number = 0
  @State private previousWebViewTranslateX: number = 0
  @State private navbarTranslateX: number = 0
  // Fullscreen state - when set, only the fullscreen component is rendered
  @State private fullscreenWebTag: string = ''
  @State private fullscreenComponentId: string = ''
  // Pull-to-refresh state
  @State private isRefreshing: boolean = false
  @State private enablePullDownRefresh: boolean = false
  @State private refreshingPath: string = ''
  @State private keyboardOffsetVp: number = 0

  private webViewUiCallback = (action: string, info: WebViewInfo) => {
    this.handleWebViewUiEvent(action, info);
  }
	  private keyboardMonitorTimer: number | null = null;
	  private keyboardMonitorActive: boolean = false;
	  private keyboardWindow: window.Window | null = null;
  private lastReportedSurfaceWidthVp: number = 0;
  private lastReportedSurfaceHeightVp: number = 0;
  private lastReportedShellWidthVp: number = 0;
  // Raw web-area (Refresh) size in vp, kept so the surface viewport can be
  // re-reported when navbar visibility changes without an area change.
  private webAreaWidthVp: number = 0;
  private webAreaHeightVp: number = 0;
  // Per-page navbar visibility; navigationStyle is static page config, so a
  // small cache avoids an FFI query per layout pass.
  private navbarShownByPath: Map<string, boolean> = new Map();
  // Overflow tabs are created on first pick. Harmony WebView creation is a
  // queued ArkTS call, so navigate can land before the controller exists.
  private pendingNavigatePath: string = '';

  /**
   * Trigger onPageShow if WebView hasn't triggered it yet
   */
  private triggerOnPageShowIfNeeded(webTag: string) {
    if (!this.pageShowTriggered.has(webTag)) {
      const extracted = extractWebTag(webTag);
      if (extracted) {
        const appId = extracted.appId;
        const path = extracted.path;
        // extractWebTag already returns the route; hyphens are literal path characters.
        const actualPath = path;

        // Check if this is the first page (LxApp runtime might not be ready yet)
        const isFirstPage = this.pageShowTriggered.size === 0;
        this.pageShowTriggered.add(webTag);

        if (isFirstPage) {
          // Delay first page to ensure LxApp runtime is ready
          setTimeout(() => {
            hilog.info(DOMAIN, TAG, `Triggering onPageShow for first page: ${appId}:${actualPath}`);
            onPageShow(appId, actualPath);
            Lingxia.updateOrientationUI(appId, actualPath);
          }, 100);
        } else {
          // Subsequent pages without delay
          hilog.info(DOMAIN, TAG, `Triggering onPageShow for: ${appId}:${actualPath}`);
          onPageShow(appId, actualPath);
          Lingxia.updateOrientationUI(appId, actualPath);
        }
      }
    } else {
      hilog.info(DOMAIN, TAG, `onPageShow already triggered for: ${webTag}, skipping`);
    }
  }

  // Listen for external path changes from LxApp.navigate API
  @Watch('onExternalPathChange') @Prop externalPath: string = ''

  // Listen for navigation trigger changes (unified method for all LxApps)
  @Watch('onNavigationTriggerChange') @Prop navigationTrigger: number = 0

  // Manual animation control for new lxapps
  @State animationOffsetY: number = 0 // 0 = final position, positive = off-screen bottom

  aboutToAppear() {
    this.initStatusBarHeight();

    const context = this.getUIContext().getHostContext() as common.UIAbilityContext;
    const hostDark = context?.config?.colorMode === ConfigurationConstant.ColorMode.COLOR_MODE_DARK;
    this.appearanceDark = LxAppContainer.appearanceByApp.get(this.appId) ?? hostDark;
    LxAppContainer.currentAppearanceDark = this.appearanceDark;

    this.path = this.initialPath;
    hilog.info(DOMAIN, TAG, `Container creating for ${this.appId}:${this.path}, isHome=${this.isHomeApp()}`);
    hilog.info(DOMAIN, TAG, `Layout info: statusBarHeight=${this.statusBarHeight}, showCapsule=${this.shouldShowCapsuleButton()}`);

    // Register this container for TabBar updates
    LxAppContainer.activeContainers.set(this.appId, this);
    setCapsuleGeometryListener(() => {
      try {
        syncLxappHostUI(this.appId);
      } catch (error) {
        hilog.warn(DOMAIN, TAG, `syncLxappHostUI failed: ${error}`);
      }
    });

    // Initialize animation type from static registry
    const storedAnimationType: AnimationType | undefined = LxAppContainer.animationTypes.get(this.appId);
    if (storedAnimationType !== undefined) {
      this.currentAnimationType = storedAnimationType;
    }

    // Add WebView UI callback to handle component creation (supports multiple containers)
    addWebViewUiCallback(this.webViewUiCallback);

    this.pageConfig = getNavigationBarState(this.appId, this.path);
    this.navbarShownByPath.set(this.path, this.pageConfig?.showNavbar === true);
    hilog.info(DOMAIN, TAG, `Initial navbar state for ${this.appId}:${this.path}: showNavbar=${this.pageConfig?.showNavbar}, showBack=${this.pageConfig?.showBackButton}, showHome=${this.pageConfig?.showHomeButton}, bgColor=0x${this.pageConfig?.navigationBarBackgroundColor?.toString(16)}, textStyle=${this.pageConfig?.navigationBarTextStyle}`);
    this.applyStatusBarContentColor();

    this.loadPullDownRefreshState(this.path);

    // Load TabBar data using new data-driven API
    this.loadTabBarState();
    if (this.tabBarState) {
      this.selectedTabIndex = this.tabBarState.selectedIndex;
      hilog.info(DOMAIN, TAG, `Initial tab index set from Rust state: ${this.selectedTabIndex}`);
    }

    // If this page has tabs, set animation type to None (no animation for tab switches)
    if (this.tabBarState && this.tabBarState.items && this.tabBarState.items.length > 0) {
      this.currentAnimationType = AnimationType.None;
      hilog.info(DOMAIN, TAG, `Set animation type to None for page with ${this.tabBarState.items.length} tabs`);
    }

    // Initial currentWebTag will be promoted from the first 'create' event;
    // do not assume a specific tag format here to keep instance IDs opaque.
    this.currentWebTag = '';
    const initialWebTag = toWebTag(this.appId, this.path);

    // Explicitly replay existing WebViews for this appId so containers created
    // after WebViews still receive 'create' events and can mark currentWebTag.
    const allInfos: WebViewInfo[] = getAllWebViewInfos();
    allInfos.forEach((info: WebViewInfo) => {
      const extracted = extractWebTag(info.webtag);
      if (extracted && extracted.appId === this.appId) {
        this.handleWebViewUiEvent('create', info);
      }
    });

    this.startKeyboardMonitor();
  }

  aboutToDisappear() {
    // Unregister this container
    LxAppContainer.activeContainers.delete(this.appId);

    // Remove WebView UI callback when container is destroyed
    removeWebViewUiCallback(this.webViewUiCallback);
    this.stopKeyboardMonitor();
    hilog.info(DOMAIN, TAG, `Container destroyed for ${this.appId}:${this.path}`);
  }

  /**
   * Initialize status bar height with proper error handling
   */
  private async initStatusBarHeight(): Promise<void> {
    try {
      const context = this.getUIContext().getHostContext() as common.UIAbilityContext;
      if (!context) {
        throw new Error('Context not available');
      }

      const mainWindow = await window.getLastWindow(context);
      if (!mainWindow) {
        throw new Error('Main window not available');
      }
      this.keyboardWindow = mainWindow;

      // Re-assert full-screen layout: the quickStart-time call runs before
      // loadContent and can be silently dropped, which shifts the whole page
      // root below the status bar (a status-bar-height white band).
      try {
        await mainWindow.setWindowLayoutFullScreen(true);
      } catch (error) {
        LxLog.e(TAG, `setWindowLayoutFullScreen failed: ${error}`);
      }
      // Some window managers honor only the immersive-state API; harmless
      // where setWindowLayoutFullScreen already worked.
      try {
        mainWindow.setImmersiveModeEnabledState(true);
      } catch (error) {
        LxLog.e(TAG, `setImmersiveModeEnabledState failed: ${error}`);
      }

      const safeArea = await mainWindow.getWindowAvoidArea(window.AvoidAreaType.TYPE_SYSTEM);
      const cutout = await mainWindow.getWindowAvoidArea(window.AvoidAreaType.TYPE_CUTOUT);
      // Avoid-area rects are in PX; layout runs in vp. The chrome inset must
      // clear both the system bar and the camera cutout.
      const topPx = Math.max(safeArea.topRect.height, cutout.topRect.height);
      hilog.info(DOMAIN, TAG,
        `Avoid areas: system=${safeArea.topRect.height}px cutout=${cutout.topRect.height}px -> ${Math.round(px2vp(topPx))}vp`);
      this.statusBarHeight = Math.round(px2vp(topPx)) || SYSTEM_STATUS_BAR_HEIGHT;

      const indicator = await mainWindow.getWindowAvoidArea(
        window.AvoidAreaType.TYPE_NAVIGATION_INDICATOR);
      this.bottomIndicatorInset = Math.max(0, Math.round(px2vp(indicator.bottomRect.height)));
    } catch (error) {
      LxLog.e(TAG, `Failed to get status bar height: ${error}`);
      this.statusBarHeight = SYSTEM_STATUS_BAR_HEIGHT; // Safe fallback value for all devices
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
    if (this.overlayActive || this.hasFullscreenTarget()) {
      if (this.keyboardOffsetVp !== 0) {
        this.keyboardOffsetVp = 0;
      }
      return;
    }
    try {
      let targetWindow = this.keyboardWindow;
      if (!targetWindow) {
        const context = this.getUIContext().getHostContext() as common.UIAbilityContext;
        if (!context) {
          return;
        }
        targetWindow = await window.getLastWindow(context);
        if (!targetWindow) {
          return;
        }
        this.keyboardWindow = targetWindow;
      }
      const avoidArea = await targetWindow.getWindowAvoidArea(window.AvoidAreaType.TYPE_KEYBOARD);
      const bottomHeightPx = avoidArea.bottomRect?.height ?? 0;
      const nextOffset = Math.max(0, Math.round(Number(px2vp(bottomHeightPx))));
      if (Math.abs(nextOffset - this.keyboardOffsetVp) >= 1) {
        this.keyboardOffsetVp = nextOffset;
      }
    } catch (_error) {
      if (this.keyboardOffsetVp !== 0) {
        this.keyboardOffsetVp = 0;
      }
    }
  }

  /**
   * Load TabBar state from Rust - simplified with complete state
   */
  private loadTabBarState() {
    try {
      this.tabBarState = liblingxia.getTabBar(this.appId);

      if (this.tabBarState) {
        hilog.info(DOMAIN, TAG, `TabBar data loaded: ${this.tabBarState.items.length} items, position=${this.tabBarState.position}`);
      } else {
        hilog.warn(DOMAIN, TAG, `No TabBar data found for appId: ${this.appId}`);
      }
    } catch (error) {
      LxLog.e(TAG, `Failed to load TabBar data: ${error}`);
      this.tabBarState = null;
    }
  }

  /**
   * Refresh TabBar data from Rust (called when Rust triggers UI update)
   * This is the data-driven update mechanism
   */
  public refreshTabBarData() {
    this.tabBarState = null;
    this.loadTabBarState();

    // Rust manages selected_index, just use it directly
    if (this.tabBarState) {
      this.selectedTabIndex = this.tabBarState.selectedIndex;
    }

    this.tabBarRefreshKey++;
    const bg: number | undefined = this.tabBarState?.backgroundColor;
    const dim: number | undefined = this.tabBarState?.dimension;
    const pos: TabBarPosition | undefined = this.tabBarState?.position;
    const items: number = this.tabBarState?.items?.length ?? 0;
    const alpha: number = bg !== undefined ? ((bg >>> 24) & 0xFF) : -1;
  }

  /**
   * Static method to update TabBar UI for a specific appId
   * Called by NativeBridge when Rust triggers updateTabBarUI
   */
  public static updateTabBarUI(appId: string): boolean {
    hilog.info(DOMAIN, TAG, `updateTabBarUI called for appId: ${appId}`);

    const container = LxAppContainer.activeContainers.get(appId);
    if (container) {
      container.refreshTabBarData();
      return true;
    } else {
      hilog.warn(DOMAIN, TAG, `No active container found for appId: ${appId}`);
      return false;
    }
  }

  /**
   * TabBar update with a real completion signal for lx.tabBar.update().
   * The @State change renders on the next frame, so `done` is chained through
   * two frame callbacks: the second fires at the start of the frame after the
   * one that applied the state, i.e. once the hide/show has actually committed.
   */
  public static updateTabBarUIAsync(appId: string, done: (ok: boolean) => void): void {
    const container = LxAppContainer.activeContainers.get(appId);
    if (!container) {
      hilog.warn(DOMAIN, TAG, `No active container found for appId: ${appId}`);
      done(false);
      return;
    }
    container.refreshTabBarData();
    try {
      const ui = container.getUIContext();
      ui.postFrameCallback(new RunOnFrame(() => {
        ui.postFrameCallback(new RunOnFrame(() => {
          hilog.info(DOMAIN, TAG, `TabBar update committed for ${appId}`);
          done(true);
        }));
      }));
    } catch (error) {
      // No UI context (container tearing down): the state change is in;
      // report completion without the frame gate rather than hanging.
      hilog.warn(DOMAIN, TAG, `TabBar frame gate unavailable for ${appId}: ${error}`);
      done(true);
    }
  }

  /**
   * Static method to refresh NavigationBar for a specific appId (uses current path)
   */
  public static refreshNavBar(appId: string): boolean {
    const container = LxAppContainer.activeContainers.get(appId);
    if (container) {
      // Use current path from container
      container.pageConfig = getNavigationBarState(appId, container.path);
      container.navbarShownByPath.set(container.path, container.pageConfig?.showNavbar === true);
      // Force UI update by incrementing refresh key
      container.navbarRefreshKey++;
      container.applyStatusBarContentColor();
      container.reportCurrentPageViewport();
      return true;
    }
    return false;
  }

  /** Scheme of the lxapp currently on screen, for overlays without an appId. */
  public static appearanceDarkNow(): boolean {
    return LxAppContainer.currentAppearanceDark;
  }

  public static applyAppearance(appId: string, dark: boolean): boolean {
    LxAppContainer.appearanceByApp.set(appId, dark);
    LxAppSurface.applyAppearance(appId, dark);
    const container = LxAppContainer.activeContainers.get(appId);
    if (container) {
      container.appearanceDark = dark;
      LxAppContainer.currentAppearanceDark = dark;
      container.pageConfig = getNavigationBarState(appId, container.path);
      container.navbarRefreshKey++;
      container.tabBarRefreshKey++;
      container.applyStatusBarContentColor();
    }
    return true;
  }

  /**
   * Static method to update tab selection for a specific appId and path
   */
  public static updateTabSelection(appId: string, path: string): boolean {
    const container = LxAppContainer.activeContainers.get(appId);
    if (container) {
      // Check if it's a tab bar item before refreshing, if not, do nothing
      if (!container.isTabBarItem(path)) {
        return false;
      }
      container.refreshTabBarData();
      return true;
    }
    return false;
  }

  /**
   * Static method to set animation type for animation control
   */
  public static setAnimationType(appId: string, animationType: AnimationType): void {
    LxAppContainer.animationTypes.set(appId, animationType);
    const container = LxAppContainer.activeContainers.get(appId);
    if (container) {
      container.currentAnimationType = animationType;
    }
  }

  /**
   * Static method to set overlay (presentation) state for an appId + path
   */
  public static setOverlay(
    appId: string,
    path: string,
    active: boolean,
    widthVp?: number,
    heightVp?: number,
    position?: number,
    bottomInset?: number,
    fullWidth?: boolean,
    fullHeight?: boolean
  ): boolean {
    const container = LxAppContainer.activeContainers.get(appId);
    if (!container) {
      hilog.warn(DOMAIN, TAG, `setOverlay: no active container for ${appId}`);
      return false;
    }
    const webTag = toWebTag(appId, path);
    container.applyOverlayState(webTag, active, widthVp, heightVp, position, bottomInset, fullWidth, fullHeight);
    return true;
  }

  /**
   * Stop pull-to-refresh animation programmatically
   * Called from NativeBridge via static entry
   */
  public static stopPullRefresh(appId: string, path?: string): void {
    const container = LxAppContainer.activeContainers.get(appId);
    if (container) {
      container.stopPullDownRefresh(path);
    } else {
      hilog.warn(DOMAIN, TAG, `stopPullRefresh: no active container for ${appId}`);
    }
  }

  /**
   * Start pull-to-refresh animation programmatically
   * Called from NativeBridge via static entry
   */
  public static startPullRefresh(appId: string, path: string): void {
    const container = LxAppContainer.activeContainers.get(appId);
    if (container) {
      container.startPullDownRefresh(path);
    } else {
      hilog.warn(DOMAIN, TAG, `startPullRefresh: no active container for ${appId}`);
    }
  }

  /**
   * Handle external path changes from LxApp.navigate API
   */
  onExternalPathChange() {
    if (this.externalPath) {
      hilog.info(DOMAIN, TAG, `External path change: ${this.externalPath}`);
      this.navigateToPage(this.externalPath);
    }
  }

  private onNavigationTriggerChange() {
    if (!this.externalPath) {
      hilog.info(DOMAIN, TAG, 'navigationTrigger changed but externalPath is empty, skipping navigate');
      return;
    }

    hilog.info(DOMAIN, TAG, `navigationTrigger updated -> navigating to ${this.externalPath}`);
    this.navigateToPage(this.externalPath);
  }

  /**
   * Handle WebView UI events from WebViewManager
   */
  handleWebViewUiEvent(action: string, info: WebViewInfo) {
    // Extract appId from WebView tag to check if this event is for this container
    const extracted = extractWebTag(info.webtag);
    if (!extracted || extracted.appId !== this.appId) {
      // This WebView event is not for this container, ignore it
      return;
    }

    hilog.info(DOMAIN, TAG, `WebView UI event: ${action} for ${info.webtag} (container: ${this.appId})`);

    switch (action) {
      case 'create':
        if (this.isActiveSurfaceWebView(info)) {
          hilog.info(DOMAIN, TAG, `Surface WebView is owned by SurfaceManager: ${info.webtag}`);
          break;
        }
        const createdPath: string = extracted.path;
        // Mark current BEFORE inserting into the ForEach. Harmony ArkWeb
        // never paints a Web that first lays out translated off-screen;
        // overflow tabs then stay blank after switchTab.
        const wantCurrent: boolean = !this.currentWebTag
          || this.pathsEqual(createdPath, this.pendingNavigatePath)
          || this.pathsEqual(createdPath, this.path)
          || this.pathsEqual(createdPath, this.externalPath);
        if (wantCurrent) {
          this.currentWebTag = info.webtag;
        }
        this.webViewComponents.set(info.webtag, info.controller);
        this.webViewKeys = Array.from(this.webViewComponents.keys());
        hilog.info(DOMAIN, TAG, `Stored WebView component: ${info.webtag}, total components: ${this.webViewComponents.size}`);

        if (this.pendingNavigatePath && this.pathsEqual(createdPath, this.pendingNavigatePath)) {
          const pendingPath: string = this.pendingNavigatePath;
          this.pendingNavigatePath = '';
          hilog.info(DOMAIN, TAG, `Pending navigate now that WebView exists: ${pendingPath}`);
          this.navigateToPage(pendingPath);
        } else if (wantCurrent) {
          hilog.info(DOMAIN, TAG, `Promoting ${info.webtag} as current after create`);
          this.triggerOnPageShowIfNeeded(info.webtag);
        }
        break;
      case 'destroy':
        if (this.isActiveSurfaceWebView(info) && this.webViewComponents.get(info.webtag) === undefined) {
          hilog.info(DOMAIN, TAG, `Surface WebView destroyed outside container: ${info.webtag}`);
          break;
        }
        this.webViewComponents.delete(info.webtag);
        this.webViewKeys = Array.from(this.webViewComponents.keys());

        // If the destroyed WebView was currently displayed, switch to another one
          if (this.currentWebTag === info.webtag) {
            if (this.webViewKeys.length > 0) {
              // Switch to the first available WebView
              this.currentWebTag = this.webViewKeys[0];
              hilog.info(DOMAIN, TAG, `Destroyed current WebView ${info.webtag}, switched to: ${this.currentWebTag}`);
            } else {
              // No WebViews left
              this.currentWebTag = '';
              hilog.info(DOMAIN, TAG, `Destroyed last WebView: ${info.webtag}`);
            }
          }

          // When all WebViews for this LxApp have been destroyed, reset per-container
          // lifecycle/animation state so the next open behaves like a fresh instance.
          if (this.webViewKeys.length === 0) {
            this.previousWebTag = '';
            this.animationInProgress = false;
            this.keepAliveTags = [];
            this.pageShowTriggered.clear();
          }
          break;
      case 'surface_show':
        hilog.info(DOMAIN, TAG, `Ignoring legacy surface_show for ${info.webtag}; SurfaceManager owns page surface overlay`);
        break;
      case 'surface_hide':
        hilog.info(DOMAIN, TAG, `Ignoring legacy surface_hide for ${info.webtag}; SurfaceManager owns page surface overlay`);
        break;
    }
  }

  build() {
    Stack() {
      // Main content layer
      if (this.isTabBarVertical()) {
        Row() {
          if (this.tabBarState && this.tabBarState.position === TabBarPosition.Left) {
            this.buildTabBar()
          }

          Stack() {
            Column() {
              if (this.shouldShowCapsuleButton()) {
                CapsuleButton({
                  statusBarHeight: this.getChromeStatusInset(),
                  standalone: false,
                  capsuleBackgroundColor: this.pageConfig?.capsuleBackgroundColor ?? 0xFFFFFFFF,
                  capsuleForegroundColor: this.pageConfig?.capsuleForegroundColor ?? 0xFF000000,
                  capsuleDividerColor: this.pageConfig?.capsuleDividerColor ?? 0xFFD1D1D6,
                  capsuleInteractionColor: this.pageConfig?.capsuleInteractionColor ?? 0xFFE5E5EA,
                  onMorePressed: () => this.handleCapsuleMore(),
                  onClosePressed: () => this.handleCapsuleClose()
                })
              }
              // Show NavigationBar only when needed
              if (this.shouldRenderNavigationBar()) {
                this.buildInlineNavigationBar()
              }
              this.buildWebViewArea()
            }
            .layoutWeight(1)
            .height('100%')
          }

          if (this.tabBarState && this.tabBarState.position === TabBarPosition.Right) {
            this.buildTabBar()
          }
        }
        .width('100%')
        .height('100%')
        .backgroundColor(Color.Transparent)
      } else {
        Stack({ alignContent: this.getTabBarAlignment() }) {
          this.buildWebViewArea()
          this.buildNavigationBar()
          this.buildTabBar()

          if (this.shouldShowCapsuleButton()) {
            CapsuleButton({
              statusBarHeight: this.getChromeStatusInset(),
              standalone: true,
              capsuleBackgroundColor: this.pageConfig?.capsuleBackgroundColor ?? 0xFFFFFFFF,
              capsuleForegroundColor: this.pageConfig?.capsuleForegroundColor ?? 0xFF000000,
              capsuleDividerColor: this.pageConfig?.capsuleDividerColor ?? 0xFFD1D1D6,
              capsuleInteractionColor: this.pageConfig?.capsuleInteractionColor ?? 0xFFE5E5EA,
              onMorePressed: () => this.handleCapsuleMore(),
              onClosePressed: () => this.handleCapsuleClose()
            })
          }
        }
        .width('100%')
        .height('100%')
        .backgroundColor(Color.Transparent)
        .hitTestBehavior(HitTestMode.Transparent)
      }

      // Fullscreen overlay layer - rendered above all other UI
      if (this.hasFullscreenTarget()) {
        LxFullscreenVideoOverlay({
          componentId: this.fullscreenComponentId,
          webTag: this.fullscreenWebTag,
          onClose: () => {
            // When overlay closes, cleanup state if needed
            // Usually native player exitFullscreen trigger will clear state via handleFullscreenChange
          }
        })
      }

      // Capsule menu bottom sheet - renders on top of everything
      CapsuleMenuBottomSheetComponent()

	    }
	    .width('100%')
	    .height('100%')
    .onAreaChange((_, newValue) => {
      try {
        this.reportShellWidth(Number(newValue.width) || 0);
        // Track where the container actually sits: when the window manager
        // already insets the page root below the system bar, the chrome must
        // not add its own status spacer on top of that.
        const globalY = Number((newValue.globalPosition?.y as number) ?? 0);
        this.containerTopVp = Math.max(0, Math.round(globalY));
      } catch (error) {
        hilog.warn(DOMAIN, TAG, `container onAreaChange failed: ${error}`);
      }
    })
	    .expandSafeArea(this.shouldExpandSafeAreaForContainer() || this.fullscreenComponentId.length > 0 ? [SafeAreaType.SYSTEM, SafeAreaType.CUTOUT] : [], this.shouldExpandSafeAreaForContainer() || this.fullscreenComponentId.length > 0 ? [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM] : [])
	  }

  private isHomeApp(): boolean {
    const miniApp = LxApp.getInstance();
    const homeAppId = miniApp.getHomeAppId();
    return this.appId === homeAppId;
  }

  /**
   * Check if container should expand safe area
   */
  private shouldExpandSafeAreaForContainer(): boolean {
    // Enable safe area expansion for full screen coverage, but use padding to avoid content overlap
    return true;
  }

  /**
   * Check if TabBar is transparent using bit operations
   * When transparent, TabBar overlays WebView; when opaque, they are separate
   */
  private isTabBarTransparent(): boolean {
    // Default to transparent when Rust does not provide a background color so we match the existing behavior and avoid a white flash
    const bgColor: number = this.tabBarState?.backgroundColor ?? 0x00000000;
    const alpha = (bgColor >>> 24) & 0xFF;
    const transparent = alpha === 0;
    return transparent;
  }

  /**
   * Check if TabBar is vertical (left/right position)
   */
  private isTabBarVertical(): boolean {
    return this.tabBarState?.position === TabBarPosition.Left || this.tabBarState?.position === TabBarPosition.Right;
  }

  /**
   * Get TabBar alignment for Stack layout
   */
  private getTabBarAlignment(): Alignment {
    const position: number = this.tabBarState?.position ?? TabBarPosition.Bottom;
    switch (position) {
      case TabBarPosition.Left:
        return Alignment.Start;
      case TabBarPosition.Right:
        return Alignment.End;
      case TabBarPosition.Bottom:
      default:
        return Alignment.Bottom;
    }
  }

  /**
   * Build TabBar component
   */
  @Builder
  buildTabBar(): void {
    if (this.shouldShowTabBar()) {
      TabBar({
        tabbar: this.tabBarState,
        appId: this.appId,
        refreshKey: this.tabBarRefreshKey,
        pageCanvasColor: resolveCanvasColor(this.appearanceDark),
        bottomInset: this.isTabBarVertical() ? 0 : this.bottomIndicatorInset,
        onTabSelected: (appId: string, index: number, item: TabBarItem): void => this.handleTabSelected(index)
      })
    }
  }

  /**
   * Check if TabBar should be shown
   * Only SwitchTab navigation type shows tabbar
   */
  private shouldShowTabBar(): boolean {
    // Reflect visibility from Rust state only
    const isVisibleFromRust = this.tabBarState?.isVisible === true;
    return isVisibleFromRust && (this.tabBarState?.items?.length > 0);
  }

  /**
   * Check if two paths match (exact match or contains relationship)
   */
  private isPathMatch(tabPath: string, currentPath: string): boolean {
    if (!tabPath || !currentPath) {
      return false;
    }

    // Exact match
    if (tabPath === currentPath) {
      return true;
    }

    // Check if current path starts with tab path (for nested routes)
    return currentPath.startsWith(tabPath + '/') || tabPath.includes(currentPath);
  }

  @Builder
  buildNavigationBar(): void {
    if (this.pageConfig && this.pageConfig.showNavbar) {
      Stack() {
        this.renderNavigationBarCore()
        if (this.shouldRenderOverlayMask()) {
          this.renderNavigationMaskLayer()
        }
      }
      .width('100%')
      .position({ x: 0, y: 0 })
      .height(this.getNavigationBarHeight())
      .zIndex(this.shouldRenderOverlayMask() ? 10002 : 10)
      .hitTestBehavior(HitTestMode.Transparent)
    }
  }

  @Builder
  private buildInlineNavigationBar(): void {
    if (this.pageConfig && this.pageConfig.showNavbar) {
      Stack() {
        this.renderNavigationBarCore()
        if (this.shouldRenderOverlayMask()) {
          this.renderNavigationMaskLayer()
        }
      }
      .width('100%')
      .height(this.getNavigationBarHeight())
      .zIndex(this.shouldRenderOverlayMask() ? 10002 : 10)
      .hitTestBehavior(HitTestMode.Transparent)
    }
  }

  @Builder
  private renderNavigationBarCore(): void {
    NavigationBar({
      config: this.pageConfig as NavigationBarState,
      appId: this.appId,
      currentPath: this.path,
      translateX: this.navbarTranslateX,
      statusBarHeight: this.getChromeStatusInset(),
      onNavigationClick: (appId: string, isHomeButton: boolean): boolean => {
        this.handleNavigationBack(isHomeButton);
        return true;
      }
    })
    .width('100%')
    .position({ x: 0, y: 0 })
    .zIndex(10)
  }

  @Builder
  private renderNavigationMaskLayer(): void {
    Column()
      .width('100%')
      .height(this.getNavigationBarHeight())
      .backgroundColor(POPUP_MASK_COLOR)
      .opacity(this.overlayMaskOpacity)
      .hitTestBehavior(HitTestMode.None)
      .zIndex(10003)
  }

  /**
   * Status inset the chrome still has to cover itself: the measured system
   * avoid area minus whatever part the window manager already applied by
   * shifting the page root down.
   */
  private getChromeStatusInset(): number {
    return Math.max(0, this.statusBarHeight - this.containerTopVp);
  }

  private getNavigationBarHeight(): number {
    const contentHeight = NAVBAR_TOTAL_HEIGHT - SYSTEM_STATUS_BAR_HEIGHT;
    return this.getChromeStatusInset() + contentHeight;
  }

  private shouldRenderOverlayMask(): boolean {
    return this.overlayActive || this.overlayMaskOpacity > 0;
  }

  private shouldBlockOverlayInteractions(): boolean {
    return this.overlayActive || this.overlayMaskOpacity > 0;
  }

  private applyOverlayState(
    webTag: string,
    active: boolean,
    widthVp?: number,
    heightVp?: number,
    position?: number,
    bottomInset?: number,
    fullWidth?: boolean,
    fullHeight?: boolean
  ): void {
    if (active) {
      this.presentationOverlayWebTag = webTag;
      this.overlayActive = true;
      if (widthVp !== undefined) this.overlayWidthVp = widthVp;
      if (heightVp !== undefined) this.overlayHeightVp = heightVp;
      if (position !== undefined) this.overlayPosition = position;
      if (bottomInset !== undefined) this.overlayBottomInset = bottomInset;
      if (fullWidth !== undefined) this.overlayFullWidth = fullWidth;
      if (fullHeight !== undefined) this.overlayFullHeight = fullHeight;
      if (this.keepAliveTags.indexOf(webTag) < 0) {
        this.keepAliveTags = [...this.keepAliveTags, webTag];
      }
      this.animateOverlayMask(true);
      hilog.info(DOMAIN, TAG, `Overlay enabled for ${webTag} w=${this.overlayWidthVp} h=${this.overlayHeightVp}`);
    } else {
      if (this.presentationOverlayWebTag && this.getLogicalWebTag(this.presentationOverlayWebTag) === this.getLogicalWebTag(webTag)) {
        this.presentationOverlayWebTag = '';
      }
      this.overlayActive = false;
      if (this.presentationPreviousWebTag && this.currentWebTag === webTag) {
        this.currentWebTag = this.presentationPreviousWebTag;
      }
      this.presentationPreviousWebTag = '';
      this.animateOverlayMask(false);
      this.overlayFullWidth = false;
      this.overlayFullHeight = false;
      hilog.info(DOMAIN, TAG, `Overlay disabled for ${webTag}`);
    }
  }

  private animateOverlayMask(visible: boolean): void {
    const target = visible ? 1 : 0;
    if (this.overlayMaskOpacity === target) {
      return;
    }
    animateTo({
      duration: OVERLAY_MASK_FADE_DURATION,
      curve: ANIMATION_CURVE_EASE_IN_OUT,
      playMode: PlayMode.Normal
    }, () => {
      this.overlayMaskOpacity = target;
    });
  }

  /**
   * Check if capsule button should be shown
   * Home app doesn't show capsule button, other apps do
   */
  private shouldShowCapsuleButton(): boolean {
    return !this.isHomeApp();
  }

  /**
   * Render the WebStack content (WebViews + Overlay)
   * Moved to Builder to avoid closure creation in build()
   */
  @Builder
  private renderWebStackBuilder(): void {
    Stack() {
      // Render all WebView components so they can be associated with controllers
      ForEach(this.getRenderableWebEntries(), (entry: RenderableWebEntry) => {
        this.buildWebViewComponent(entry.webTag, entry.controller)
      }, (entry: RenderableWebEntry) => `${entry.webTag}@${entry.generation}`)
      // Overlay mask on top
      if (this.shouldRenderOverlayMask()) {
        Column()
          .width('100%')
          .height('100%')
          .backgroundColor(POPUP_MASK_COLOR)
          .opacity(this.overlayMaskOpacity)
          .hitTestBehavior(HitTestMode.None)
          .zIndex(10003)
      }
    }
    .width('100%')
    .height('100%')
    .backgroundColor(COLOR_TRANSPARENT)
    .clip(this.navigationMode)
  }

  /**
   * Custom Refresh Header Builder
   */
  @Builder
  RefreshHeaderBuilder() {
    // The container no longer reserves the navbar area, so the pull header
    // must clear the current page's navbar itself.
    RefreshHeader({ isRefreshing: this.isRefreshing })
      .margin({ top: this.getCurrentNavTop() } as Margin)
  }

  /**
   * Build WebView area
   */
  @Builder
  buildWebViewArea(): void {
    // Keep a stable component tree for same-level WebViews. Popup mode only changes
    // overlay styling/state; it should not swap out the WebView host structure.
    Refresh({ refreshing: $$this.isRefreshing, builder: this.RefreshHeaderBuilder() }) {
      this.renderWebStackBuilder()
    }
    .onRefreshing(() => {
      if (this.enablePullDownRefresh) {
        this.refreshingPath = this.path;
        this.handlePullToRefresh()
      } else {
        this.isRefreshing = false
      }
    })
    .pullDownRatio(this.enablePullDownRefresh ? 1 : 0) // Disable pull effect if not enabled
    // The strip a pull opens above the web stack is this component's own
    // background: the stack inside is transparent, so without a colour here
    // the pull bares the ability window, which is system white in both
    // appearances.
    .backgroundColor(resolveCanvasColor(this.appearanceDark))
    .width('100%')
    .height('100%')
    // No shared navbar padding here: each webview wrapper reserves its own
    // page's inset (getWrapperPadding), so navigating between pages with
    // different navbar visibility never resizes ArkWeb surfaces.
    .onAreaChange((_, newValue) => {
      try {
        this.webAreaWidthVp = Number(newValue.width) || 0;
        this.webAreaHeightVp = Number(newValue.height) || 0;
        notePageOrigin(
          Number((newValue.globalPosition?.x as number) ?? 0),
          Number((newValue.globalPosition?.y as number) ?? 0)
        );
        this.reportCurrentPageViewport();
      } catch (error) {
        hilog.warn(DOMAIN, TAG, `web area onAreaChange failed: ${error}`);
      }
    })
    .hitTestBehavior(HitTestMode.Transparent)
    .zIndex(this.shouldRenderOverlayMask() ? 10004 : 0)
    // CUTOUT included so the web area reaches y=0 on cutout devices like the
    // other mobile hosts; the camera zone is covered by the status-bar inset.
    .expandSafeArea([SafeAreaType.SYSTEM, SafeAreaType.CUTOUT], [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM])
  }
  
  /**
   * Handle pull-to-refresh action
   */
  private handlePullToRefresh(path?: string): void {
    // Notify Rust core about the event
    const eventPath = path || this.path;
    let _: boolean = onLxappEvent(this.appId, UiEventType.PullDownRefresh, eventPath);

    // Set a safety timeout to stop refreshing if the app doesn't respond
    setTimeout(() => {
      if (this.isRefreshing) {
        hilog.warn(DOMAIN, TAG, 'Pull-to-refresh timed out, stopping automatically')
        this.isRefreshing = false
        this.refreshingPath = ''
        // Restore the correct state for the current page
        this.loadPullDownRefreshState(this.path);
      }
    }, 10000)
  }

  /**
   * Stop pull-to-refresh animation programmatically
   * Called from Rust via FFI
   */
  public stopPullDownRefresh(path?: string): void {
    if (path && this.refreshingPath && path !== this.refreshingPath) {
      hilog.warn(DOMAIN, TAG, `Ignoring stop for ${path}, current refreshing is ${this.refreshingPath}`);
      return;
    }
    this.isRefreshing = false
    this.refreshingPath = ''
    // Restore the correct state for the current page
    this.loadPullDownRefreshState(this.path);
  }

  /**
   * Start pull-to-refresh animation programmatically
   * Called from Rust via FFI
   */
  public startPullDownRefresh(targetPath?: string): void {
    // If targetPath is provided and differs from current path, check config for targetPath
    let shouldRefresh = this.enablePullDownRefresh;

    if (targetPath && targetPath !== this.path) {
      hilog.info(DOMAIN, TAG, `startPullDownRefresh: target path ${targetPath} differs from current ${this.path}`);
      // Check if target path has refresh enabled
      shouldRefresh = isPullDownRefreshEnabled(this.appId, targetPath);
    }

    if (shouldRefresh) {
      // Force enable immediately to ensure Refresh component accepts the state change
      // This handles the race condition where navigation from a disabled page to an enabled page
      // hasn't completed yet, but we need the Refresh component to be active.
      if (!this.enablePullDownRefresh) {
        hilog.info(DOMAIN, TAG, 'Forcing enablePullDownRefresh for programmatic start');
        this.enablePullDownRefresh = true;
      }

      this.refreshingPath = targetPath || this.path;
      this.isRefreshing = true
      this.handlePullToRefresh(targetPath)
    } else {
      hilog.warn(DOMAIN, TAG, 'startPullDownRefresh ignored: refresh not enabled for this page')
    }
  }

  /** Navbar inset of the page currently on screen (vp). */
  private getCurrentNavTop(): number {
    // Fullscreen video removes all chrome reservations.
    if (this.hasFullscreenTarget()) {
      return 0;
    }
    return this.pageConfig?.showNavbar === true ? this.getNavigationBarHeight() : 0;
  }

  /** Navbar inset owned by the page rendered under webTag (0 for custom navigation). */
  private getPageNavTop(webTag: string): number {
    if (this.hasFullscreenTarget()) {
      return 0;
    }
    const extracted = extractWebTag(webTag);
    const path = extracted ? extracted.path : this.path;
    return this.isNavbarShownForPage(path) ? this.getNavigationBarHeight() : 0;
  }

  private isNavbarShownForPage(path: string): boolean {
    const cached: boolean | undefined = this.navbarShownByPath.get(path);
    if (cached !== undefined) {
      return cached;
    }
    let shown: boolean = false;
    try {
      const state: NavigationBarState | null = getNavigationBarState(this.appId, path);
      shown = state?.showNavbar === true;
    } catch (_error) {
      // Unknown page (e.g. surface-only tag): fall back to the current page.
      shown = this.pageConfig?.showNavbar === true;
    }
    this.navbarShownByPath.set(path, shown);
    return shown;
  }

  /**
   * Each webview wrapper reserves ITS OWN page's navbar inset instead of the
   * shared container reserving the current page's. A shared reservation
   * resized every ArkWeb surface on the same frame a tab switch revealed a
   * page with different navbar visibility, and the engine repaints at the new
   * size a frame late — flashing a blank band (≈ navbar height) right above
   * an opaque tabbar. Per-wrapper insets keep every surface at its final size
   * while parked, so switching only translates, never resizes.
   * Overlay webviews follow the current page so presentation geometry keeps
   * matching the on-screen chrome.
   */
  private getWrapperPadding(webTag: string): Padding {
    // Touch navbarRefreshKey so ArkUI re-evaluates this attribute when navbar
    // state is refreshed at runtime (the per-page cache itself is not @State).
    void this.navbarRefreshKey;
    const top = this.isOverlayWebView(webTag) ? this.getCurrentNavTop() : this.getPageNavTop(webTag);
    return { top: top, bottom: 0, left: 0, right: 0 } as Padding;
  }

  /** Report the current page's content viewport (web area minus its navbar). */
  private reportCurrentPageViewport(): void {
    if (this.webAreaWidthVp <= 0 || this.webAreaHeightVp <= 0) {
      return;
    }
    this.reportSurfaceViewport(this.webAreaWidthVp, this.webAreaHeightVp - this.getCurrentNavTop());
  }

  private getWebBottomPadding(): number {
    const hasTabBar = (this.tabBarState?.items?.length ?? 0) > 0;
    if (!hasTabBar) return 0;
    // Pushed (non-tab) pages hide the tabbar; reserving there opens a gap
    // between the navbar and the web area.
    if (this.tabBarState?.isVisible !== true) return 0;
    if (this.isTabBarVertical()) return 0; // Side TabBars do not affect vertical spacing
    // A transparent TabBar overlays the WebView, so allow content underneath (padding = 0)
    const transparent = this.isTabBarTransparent();
    if (transparent) {
      return 0;
    }
    // Reserve the TabBar's RENDERED height: configured dimension plus the
    // home-indicator growth the bar itself applies.
    const tabBarSize: number = (this.tabBarState?.dimension as number) ?? TAB_BAR_HORIZONTAL_HEIGHT;
    return tabBarSize + tabBarExtraBottomHeight(this.bottomIndicatorInset, transparent);
  }

  private shouldRenderNavigationBar(): boolean {
    // Render the navigation bar only when showNavbar is true so the home page does not show a transparent bar
    return this.pageConfig?.showNavbar === true;
  }

  private handleTabSelected(index: number) {
    hilog.info(DOMAIN, TAG, `Tab selected: ${index}, current: ${this.selectedTabIndex}`);
    let _: boolean = onLxappEvent(this.appId, UiEventType.TabBarClick, index.toString());
  }

  /** Compare lxapp page paths ignoring slash, /index, and view suffix. */
  private pathsEqual(left: string, right: string): boolean {
    if (!left || !right) {
      return false;
    }
    return LxAppContainer.normalizePagePath(left) === LxAppContainer.normalizePagePath(right);
  }

  private static normalizePagePath(path: string): string {
    let normalized: string = path.trim();
    if (normalized.startsWith('/')) {
      normalized = normalized.substring(1);
    }
    const suffixes: string[] = ['.tsx', '.ts', '.jsx', '.js', '.vue', '.html'];
    for (let i = 0; i < suffixes.length; i++) {
      const suffix: string = suffixes[i];
      if (normalized.toLowerCase().endsWith(suffix)) {
        normalized = normalized.substring(0, normalized.length - suffix.length);
        break;
      }
    }
    if (normalized.endsWith('/index')) {
      normalized = normalized.substring(0, normalized.length - '/index'.length);
    }
    return normalized;
  }

  /**
   * Load navbar state for a page (common for all navigation types)
   */
  private loadNavbarState(path: string): void {
    this.pageConfig = getNavigationBarState(this.appId, path);
    this.navbarShownByPath.set(path, this.pageConfig?.showNavbar === true);
    this.applyStatusBarContentColor();
    // The web area no longer resizes with navbar visibility, so the surface
    // viewport must be re-reported explicitly on page changes.
    this.reportCurrentPageViewport();
  }

  /** Keep status bar glyphs legible against native chrome or custom content. */
  private applyStatusBarContentColor(): void {
    const config = this.pageConfig;
    let lightGlyphs: boolean;
    if (config && config.showNavbar) {
      lightGlyphs = !isColorDark(config.navigationBarForegroundColor || 0xFF000000);
    } else {
      lightGlyphs = this.appearanceDark;
    }
    this.setStatusBarContentColor(lightGlyphs ? '#FFFFFF' : '#000000');
  }

  private async setStatusBarContentColor(color: string): Promise<void> {
    try {
      const context = this.getUIContext().getHostContext() as common.UIAbilityContext;
      const mainWindow = await window.getLastWindow(context);
      await mainWindow.setWindowSystemBarProperties({ statusBarContentColor: color });
    } catch (error) {
      LxLog.e(TAG, `Failed to set status bar content color: ${error}`);
    }
  }

  /**
   * Load pull-down refresh state for a page
   */
  private loadPullDownRefreshState(path: string): void {
    this.enablePullDownRefresh = isPullDownRefreshEnabled(this.appId, path);
    hilog.info(DOMAIN, TAG, `Pull-down refresh enabled for ${this.appId}:${path}: ${this.enablePullDownRefresh}`);
  }

  /**
   * Navigate to any page with animation support
   */
  private navigateToPage(path: string): boolean {
    hilog.info(DOMAIN, TAG, `navigate: ${this.appId}:${path} with type: ${AnimationType[this.currentAnimationType]}`);

    // Stop any active refresh when navigating away, unless we are navigating TO the refreshing page
    if (this.isRefreshing && this.refreshingPath !== path) {
      hilog.info(DOMAIN, TAG, `Navigating away from ${this.refreshingPath} to ${path}, stopping refresh`);
      this.isRefreshing = false;
      this.refreshingPath = '';
    }

    // Find WebView controller for the new page (logical lookup)
    const webViewController = findWebview(this.appId, path);

    if (!webViewController) {
      const logicalTag = toWebTag(this.appId, path);
      this.pendingNavigatePath = path;
      hilog.warn(DOMAIN, TAG, `WebView not found for page: ${logicalTag}; waiting for create`);
      return false;
    }
    this.pendingNavigatePath = '';

    // Determine the concrete webTag (Ark-facing) for this controller.
    // Prefer an existing entry from webViewComponents to keep instance IDs opaque.
    let newWebTag: string = '';
    const entries = this.webViewComponents.entries();
    for (let entry of entries) {
      const tag = entry[0];
      const controller = entry[1];
      if (controller === webViewController) {
        newWebTag = tag;
        break;
      }
    }

    if (!newWebTag) {
      // Fallback: use logical tag when no prior UI entry exists.
      newWebTag = toWebTag(this.appId, path);
    }

    // Store previous WebView for animation
    const oldWebTag = this.currentWebTag;

    // Ensure WebView component is tracked under the resolved tag
    this.webViewComponents.set(newWebTag, webViewController);
    this.webViewKeys = Array.from(this.webViewComponents.keys());
    this.pageShowTriggered.delete(newWebTag);

    // Determine if animation should be used
    const shouldAnimate = this.shouldAnimateTransition();

    if (shouldAnimate && oldWebTag) {
      this.performAnimatedTransition(oldWebTag, newWebTag, path);
    } else {
      this.performInstantTransition(newWebTag, path);
    }

    return true;
  }

  /**
   * Determine if transition should be animated based on animation type
   */
  private shouldAnimateTransition(): boolean {
    return this.currentAnimationType === AnimationType.Forward ||
           this.currentAnimationType === AnimationType.Backward;
  }

  /**
   * Perform animated transition between WebViews
   */
  private performAnimatedTransition(oldWebTag: string, newWebTag: string, path: string): void {
    hilog.info(DOMAIN, TAG, `Performing animated transition from ${oldWebTag} to ${newWebTag}`);

    // Notify old page becoming inactive (pause native components)
    if (oldWebTag) {
      pauseWebViewComponents(oldWebTag);
    }

    this.animationInProgress = true;
    this.previousWebTag = oldWebTag;

    // Determine animation direction
    const isBackward = this.currentAnimationType === AnimationType.Backward;

    // Set initial positions for slide animation
    if (isBackward) {
      this.currentWebViewTranslateX = -100;
      this.previousWebViewTranslateX = 0;
      this.navbarTranslateX = -100;
    } else {
      this.currentWebViewTranslateX = 100;
      this.previousWebViewTranslateX = 0;
      this.navbarTranslateX = 100;
    }

    // Update current WebView and path before animation
    this.currentWebTag = newWebTag;
    this.path = path;

    // Update tab selection if this is a tab page
    LxAppContainer.updateTabSelection(this.appId, path);

    this.loadNavbarState(path);
    this.loadPullDownRefreshState(path);
    this.loadTabBarState();

    // Start slide animation
    animateTo({
      duration: ANIMATION_DURATION_STANDARD,
      curve: ANIMATION_CURVE_EASE_IN_OUT,
      playMode: PlayMode.Normal,
      onFinish: () => {
        this.finishTransition(newWebTag, path, oldWebTag);
      }
    }, () => {
      // Animate to final position
      this.currentWebViewTranslateX = 0;
      this.navbarTranslateX = 0;
      this.previousWebViewTranslateX = isBackward ? 100 : -100;
    });
  }

  /**
   * Perform instant transition between WebViews
   */
  private performInstantTransition(newWebTag: string, path: string): void {
    hilog.info(DOMAIN, TAG, `Performing instant transition to ${newWebTag}`);

    // Notify old page becoming inactive if switching pages
    const oldWebTag = this.currentWebTag;
    if (oldWebTag && oldWebTag !== newWebTag) {
      pauseWebViewComponents(oldWebTag);
    }

    // Update current WebView and path immediately
    this.currentWebTag = newWebTag;
    this.path = path;

    // Update tab selection if this is a tab page
    LxAppContainer.updateTabSelection(this.appId, path);

    // Load navbar state for instant transitions
    this.loadNavbarState(path);
    this.loadPullDownRefreshState(path);
    this.loadTabBarState();

    this.finishTransition(newWebTag, path, oldWebTag);
  }

  /**
   * Finish transition and trigger necessary callbacks
   */
  private finishTransition(newWebTag: string, path: string, oldWebTag?: string): void {
    // Reset animation state
    this.animationInProgress = false;
    this.previousWebTag = '';

    // Reset translation values
    this.currentWebViewTranslateX = 0;
    this.previousWebViewTranslateX = 0;
    this.navbarTranslateX = 0;

    // Notify new page becoming active (resume native components)
    resumeWebViewComponents(newWebTag);

    // Trigger onPageShow
    this.triggerOnPageShowIfNeeded(newWebTag);

    hilog.info(DOMAIN, TAG, `Transition completed to: ${newWebTag}`);
  }

  /**
   * Check if a path is a tab bar item
   */
  private isTabBarItem(path: string): boolean {
    return this.tabBarState?.items?.some((item: TabBarItem) => item.pagePath === path) || false;
  }

  /**
   * Get tab index for a path
   */
  private getTabIndex(path: string): number {
    if (!this.tabBarState?.items) return -1;
    const index: number = this.tabBarState.items.findIndex((item: TabBarItem) => item.pagePath === path);
    return index >= 0 ? index : -1;
  }

  /**
   * Trigger onPageShow for direct navigation
   */
  private triggerOnPageShow(appId: string, path: string): void {
    hilog.info(DOMAIN, TAG, `Triggering onPageShow for direct navigation: ${appId}:${path}`);

    // Prevent duplicate onPageShow calls
    const pageKey = `${appId}:${path}`;
    if (this.pageShowTriggered.has(pageKey)) {
      hilog.info(DOMAIN, TAG, `onPageShow already triggered for ${pageKey}, skipping`);
      return;
    }

    this.pageShowTriggered.add(pageKey);
    onPageShow(appId, path);
    Lingxia.updateOrientationUI(appId, path);
  }

  private handleNavigationBack(isHomeButton: boolean = false) {
    if (isHomeButton) {
      hilog.info(DOMAIN, TAG, 'Navigation home button pressed');
      let _: boolean = onLxappEvent(this.appId, UiEventType.NavigationClick, NAVIGATION_ACTION_HOME);
    } else {
      hilog.info(DOMAIN, TAG, 'Navigation back button pressed');
      let _: boolean = onLxappEvent(this.appId, UiEventType.NavigationClick, NAVIGATION_ACTION_BACK);
    }
  }

  private handleCapsuleMore(): void {
    hilog.info(DOMAIN, TAG, 'Capsule more button pressed');
    const shown: boolean = CapsuleMenuManager.show(this.appId);
    if (!shown) {
      LxLog.e(TAG, `Failed to show capsule menu for app: ${this.appId}`);
    }
  }

  private handleCapsuleClose(): void {
    let _: boolean = onLxappEvent(this.appId, UiEventType.CapsuleClick, CAPSULE_ACTION_CLOSE);
  }

  // The previous height/position logic is deprecated; always rely on getWebViewPadding to keep the layout stable

  /**
   * Build individual WebView component for a specific webTag with animation support
   * Uses official ArkWeb same-level rendering with NodeController
  */
  @Builder
  buildWebViewComponent(webTag: string, controller: webview.WebviewController): void {
    Column() {
      Stack() {
        LingXiaWebView({
          webTag: webTag,
          controller: controller,
          appearanceDark: this.appearanceDark,
          renderVisibility: this.getRenderVisibility(webTag),
          renderTranslate: this.getRenderTranslation(webTag),
          renderMargin: this.getRenderMargin(webTag),
          renderWidth: this.getRenderWidth(webTag),
          renderHeight: this.getRenderHeight(webTag),
          renderPadding: this.getRenderPadding(webTag),
          renderBorderRadius: this.getRenderBorderRadius(webTag),
          renderClip: this.getRenderClip(webTag),
          renderZIndex: this.getRenderZIndex(webTag),
          renderHitTest: this.getRenderHitTest(webTag),
          onFullscreenChange: (tag: string, componentId: string, isFullscreen: boolean) => {
            this.handleFullscreenChange(tag, componentId, isFullscreen);
          }
        })
      }
      .width(this.getStackWidth(webTag))
      .height(this.getStackHeight(webTag))
      .alignContent(this.getRenderAlignment(webTag))
    }
    .width('100%')
    .height('100%')
    .padding(this.getWrapperPadding(webTag))
    .justifyContent(this.getWrapperJustify(webTag))
    .alignItems(this.getWrapperAlign(webTag))
    .hitTestBehavior(HitTestMode.Transparent)
    .zIndex(this.getRenderZIndex(webTag))
  }

  /**
   * Handle fullscreen state change from LingXiaWebView
   */
  private handleFullscreenChange(webTag: string, componentId: string, isFullscreen: boolean): void {
    if (isFullscreen) {
      this.fullscreenWebTag = webTag;
      this.fullscreenComponentId = componentId;
    } else if (this.fullscreenWebTag === webTag) {
      // Switch player back to original surface before clearing fullscreen state
      LxNativeVideoPlayer.savePlaybackState(this.fullscreenComponentId);
      LxNativeVideoPlayer.switchToOriginalSurface(this.fullscreenComponentId);
      this.fullscreenWebTag = '';
      this.fullscreenComponentId = '';
    }
  }

  private hasFullscreenTarget(): boolean {
    return this.fullscreenComponentId.length > 0 && this.fullscreenWebTag.length > 0;
  }

  private getWrapperJustify(webTag: string): FlexAlign {
    if (this.isOverlayWebView(webTag)) {
      switch (this.overlayPosition) {
        case 1:
          return FlexAlign.End;
        case 2:
        case 3:
          return FlexAlign.Center;
        default:
          return FlexAlign.Center;
      }
    }
    return FlexAlign.Start;
  }

  private getWrapperAlign(webTag: string): HorizontalAlign {
    if (!this.isOverlayWebView(webTag)) {
      return HorizontalAlign.Start;
    }
    switch (this.overlayPosition) {
      case 2:
        return HorizontalAlign.Start;
      case 3:
        return HorizontalAlign.End;
      default:
        return HorizontalAlign.Center;
    }
  }

  private getOverlayWidth(): string {
    if (this.overlayFullWidth && this.overlayWidthVp > 0) {
      return `${this.overlayWidthVp}vp`;
    }
    return this.overlayWidthVp > 0 ? `${this.overlayWidthVp}vp` : '90%';
  }

  private getOverlayHeight(): string {
    if (this.overlayFullHeight && this.overlayHeightVp > 0) {
      return `${this.overlayHeightVp}vp`;
    }
    return this.overlayHeightVp > 0 ? `${this.overlayHeightVp}vp` : '80%';
  }

  private getOverlayAlignment(): Alignment {
    switch (this.overlayPosition) {
      case 1:
        return Alignment.Bottom;
      case 2:
        return Alignment.Start;
      case 3:
        return Alignment.End;
      default:
        return Alignment.Center;
    }
  }

  private getOverlayMargin(): Margin {
    const base: Margin = { top: 0, left: 0, right: 0, bottom: 0 };
    const horizontalGap = this.overlayFullWidth ? 0 : 16;
    const verticalGap = this.overlayFullHeight ? 0 : 16;
    switch (this.overlayPosition) {
      case 1:
        if (this.overlayBottomInset > 0) {
          base.bottom = this.overlayBottomInset;
        }
        break;
      case 2:
        base.left = 0;
        base.right = 0;
        base.top = verticalGap;
        base.bottom = verticalGap;
        break;
      case 3:
        base.right = 0;
        base.left = 0;
        base.top = verticalGap;
        base.bottom = verticalGap;
        break;
      default:
        base.top = verticalGap;
        base.bottom = verticalGap;
        break;
    }
    return base;
  }

  private getLogicalWebTag(webTag: string): string {
    const extracted = extractWebTag(webTag);
    if (extracted) {
      return toWebTag(extracted.appId, extracted.path);
    }
    return webTag.split('#')[0] || webTag;
  }

  private isOverlayWebView(webTag: string): boolean {
    if (!this.overlayActive || !this.presentationOverlayWebTag) {
      return false;
    }
    return this.getLogicalWebTag(webTag) === this.getLogicalWebTag(this.presentationOverlayWebTag);
  }

  private isActiveSurfaceWebView(info: WebViewInfo): boolean {
    // Surface ownership is declared at creation; every page webview carries a
    // per-instance tag, so the tag shape says nothing about who presents it.
    if (info.effectiveOptions.surfaceOwned) {
      return true;
    }
    return LxAppSurface.isSurfacePageWebView(this.appId, info.webtag);
  }

  private reportSurfaceViewport(widthVp: number, heightVp: number): void {
    if (!Number.isFinite(widthVp) || !Number.isFinite(heightVp) || widthVp <= 0 || heightVp <= 0) {
      return;
    }
    const roundedWidth = Math.round(widthVp);
    const roundedHeight = Math.round(heightVp);
    if (Math.abs(roundedWidth - this.lastReportedSurfaceWidthVp) < 1 &&
      Math.abs(roundedHeight - this.lastReportedSurfaceHeightVp) < 1) {
      return;
    }
    this.lastReportedSurfaceWidthVp = roundedWidth;
    this.lastReportedSurfaceHeightVp = roundedHeight;
    try {
      setSurfaceViewport(this.appId, roundedWidth, roundedHeight);
    } catch (error) {
      hilog.warn(DOMAIN, TAG, `setSurfaceViewport failed appId=${this.appId}: ${error}`);
    }
  }

  private reportShellWidth(widthVp: number): void {
    if (!Number.isFinite(widthVp) || widthVp <= 0) {
      return;
    }
    const roundedWidth = Math.round(widthVp);
    if (Math.abs(roundedWidth - this.lastReportedShellWidthVp) < 1) {
      return;
    }
    this.lastReportedShellWidthVp = roundedWidth;
    try {
      setSurfaceWidth(this.appId, roundedWidth);
    } catch (error) {
      hilog.warn(DOMAIN, TAG, `setSurfaceWidth failed appId=${this.appId}: ${error}`);
    }
  }

  private getRenderVisibility(webTag: string): Visibility {
    if (this.isOverlayWebView(webTag)) {
      return Visibility.Visible;
    }
    // Keep the page behind the presentation visible so it shows through the semi-transparent mask
    if (this.overlayActive && this.presentationPreviousWebTag && webTag === this.presentationPreviousWebTag) {
      return Visibility.Visible;
    }
    return this.getWebViewVisibility(webTag);
  }

  private getRenderTranslation(webTag: string): TranslateOptions {
    if (this.isOverlayWebView(webTag)) {
      return { x: 0, y: 0 } as TranslateOptions;
    }
    return this.getWebViewTranslation(webTag);
  }

  private getRenderMargin(webTag: string): Margin {
    if (this.isOverlayWebView(webTag)) {
      return this.getOverlayMargin();
    }
    return this.getWebViewMargin();
  }

  private getRenderWidth(webTag: string): string {
    return this.isOverlayWebView(webTag) ? this.getOverlayWidth() : '100%';
  }

  private getRenderHeight(webTag: string): string {
    return this.isOverlayWebView(webTag) ? this.getOverlayHeight() : this.getWebContentHeight();
  }

  private getStackHeight(webTag: string): string {
    // For overlay WebViews, Stack height should match content height
    // so that Column's justifyContent can properly align it (e.g., to bottom)
    if (this.isOverlayWebView(webTag)) {
      return this.getOverlayHeight();
    }
    return '100%';
  }

  private getStackWidth(webTag: string): string {
    // For left/right overlay WebViews, Stack width should match content width
    // so the presentation can be flush with screen edges
    if (this.isOverlayWebView(webTag)) {
      if (this.overlayPosition === 2 || this.overlayPosition === 3) {
        return this.getOverlayWidth();
      }
    }
    return '100%';
  }

  private getRenderPadding(webTag: string): Padding {
    // ArkWeb ignores component padding; the tabbar reservation is applied
    // through getWebContentHeight instead.
    return { top: 0, bottom: 0, left: 0, right: 0 } as Padding;
  }

  private getRenderAlignment(webTag: string): Alignment {
    return this.isOverlayWebView(webTag) ? this.getOverlayAlignment() : Alignment.TopStart;
  }

  // Helper type for BorderRadiuses
  private getRenderBorderRadius(webTag: string): BorderRadiuses {
    if (this.isOverlayWebView(webTag)) {
      // Adjust corner radius based on overlay position so edges that touch screen edges are not rounded
      switch (this.overlayPosition) {
        case 1: // bottom - only top corners rounded
          return { topLeft: 16, topRight: 16, bottomLeft: 0, bottomRight: 0 };
        case 2: // left - only right corners rounded
          return { topLeft: 0, topRight: 16, bottomLeft: 0, bottomRight: 16 };
        case 3: // right - only left corners rounded
          return { topLeft: 16, topRight: 0, bottomLeft: 16, bottomRight: 0 };
        default: // center - all corners rounded
          return { topLeft: 16, topRight: 16, bottomLeft: 16, bottomRight: 16 };
      }
    }
    return { topLeft: 0, topRight: 0, bottomLeft: 0, bottomRight: 0 };
  }

  private getRenderClip(webTag: string): boolean {
    return this.isOverlayWebView(webTag);
  }

  private getRenderHitTest(webTag: string): HitTestMode {
    // Overlay WebView uses Block to prevent events from passing through to underlying content
    // Regular WebViews use Default for normal interaction
    if (this.isOverlayWebView(webTag)) {
      return HitTestMode.Block;
    }
    return HitTestMode.Default;
  }

  private getRenderZIndex(webTag: string): number {
    if (this.isOverlayWebView(webTag)) {
      return 10005;
    }
    return 0;
  }

  private getWebContentHeight(): string {
    // ArkWeb fills its border box and ignores component padding, so an
    // opaque visible tabbar must shrink the web height or it covers the
    // page's bottom content.
    const bottom = this.getWebBottomPadding();
    return bottom > 0 ? `calc(100% - ${bottom}vp)` : '100%';
  }

  private getWebViewVisibility(webTag: string): Visibility {
    // Animation logic
    if (this.animationInProgress) {
      if (webTag === this.currentWebTag) return Visibility.Visible;
      if (webTag === this.previousWebTag) return Visibility.Visible;
      return this.getHiddenVisibility(webTag);
    }

    // Standard visibility
    return webTag === this.currentWebTag ? Visibility.Visible : this.getHiddenVisibility(webTag);
  }

  private isParkedOffscreen(webTag: string): boolean {
    if (webTag === this.currentWebTag) return false;
    // The page behind a presentation overlay must stay in place so it shows
    // through the semi-transparent mask.
    if (this.overlayActive && webTag === this.presentationPreviousWebTag) return false;
    return true;
  }

  private getHiddenVisibility(webTag: string): Visibility {
    // Hidden/None blank the ArkWeb surface (and Hidden also blocks the
    // first paint of a page loaded before reveal), so inactive pages stay
    // Visible and are parked offscreen via translate instead.
    return this.isParkedOffscreen(webTag) ? Visibility.Visible : Visibility.Hidden;
  }

  private getWebViewTranslation(webTag: string): TranslateOptions {
    const keyboardTranslateY = this.getKeyboardTranslateY(webTag);
    if (!this.animationInProgress) {
      // Ensure active WebView is at 0, others are moved out if kept alive
      if (webTag === this.currentWebTag) return { x: 0, y: keyboardTranslateY };
      if (this.isParkedOffscreen(webTag)) return { x: '100%', y: 0 };
      return { x: 0, y: 0 };
    }

    if (webTag === this.currentWebTag) {
      return { x: `${this.currentWebViewTranslateX}%`, y: keyboardTranslateY };
    }
    if (webTag === this.previousWebTag) {
      return { x: `${this.previousWebViewTranslateX}%`, y: 0 };
    }
    if (this.isParkedOffscreen(webTag)) return { x: '100%', y: 0 };
    return { x: 0, y: 0 };
  }

  private getKeyboardTranslateY(webTag: string): number {
    if (this.keyboardOffsetVp <= 0) {
      return 0;
    }
    if (this.isOverlayWebView(webTag)) {
      return 0;
    }
    if (webTag !== this.currentWebTag) {
      return 0;
    }
    return 0;
  }

  private getWebViewMargin(): Margin {
    return { top: 0, bottom: 0, left: 0, right: 0 };
  }

  private getRenderableWebEntries(): RenderableWebEntry[] {
    const entries: RenderableWebEntry[] = [];
    this.webViewComponents.forEach((controller, tag) => {
      // Mount every webview: the runtime blocks page creation on the Web
      // component's onAppear ack, so an unmounted webview deadlocks
      // navigateTo. Inactive pages are parked offscreen, never unmounted —
      // remounting binds the controller to a fresh empty browser (src '')
      // that the runtime won't reload.
      entries.push({ webTag: tag, controller: controller, generation: getWebviewGeneration(tag) });
    });
    return entries;
  }
}

/** Adapter: run a closure on the next ArkUI frame (see updateTabBarUIAsync). */
class RunOnFrame extends FrameCallback {
  private readonly run: () => void;

  constructor(run: () => void) {
    super();
    this.run = run;
  }

  onFrame(_frameTimeInNano: number): void {
    this.run();
  }
}
