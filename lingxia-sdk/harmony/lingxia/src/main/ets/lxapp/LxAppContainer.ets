import { hilog } from '@kit.PerformanceAnalysisKit';
import { webview } from '@kit.ArkWeb';
import { window, display } from '@kit.ArkUI';
import { common } from '@kit.AbilityKit';
import { LxApp } from './LxApp';
import { LxAppPopup } from './APIs/LxAppPopup';
import {
  findWebview,
  toWebTag,
  extractWebTag,
  addWebViewUiCallback,
  removeWebViewUiCallback,
  WebViewInfo,
  getAllWebViewInfos,
  getWebViewEmbedManager,
  pauseWebViewComponents,
  resumeWebViewComponents
} from './WebView';
import { NavigationBar, type NavigationBarState, NAVBAR_TOTAL_HEIGHT } from './NavigationBar';
import { NavigationButton, CapsuleButton } from './LxAppDrawables';
import { TabBar } from './TabBar';
import { TabBarPosition, type TabBarItem, UiEventType } from 'liblingxia.so';
import liblingxia, { TabBarState, getNavigationBarState, onPageShow, onUiEvent, onWebviewControllerCreated } from 'liblingxia.so';
import { NAVIGATION_ACTION_BACK, NAVIGATION_ACTION_HOME, CAPSULE_ACTION_MORE, CAPSULE_ACTION_CLOSE } from './NativeBridge';
import { AnimationType } from './LxAppNavigation';
import { SameLevelEmbedManager } from './SameLevel/SameLevelEmbed';
import { setEmbedController, teardownEmbedManager } from './SameLevel/SameLevelBridge';
import { LxNativeVideoPlayer } from './media/LxNativeVideoPlayer';
import { LxFullscreenVideoOverlay } from './media/LxFullscreenVideoOverlay';

// Container-specific constants
const SYSTEM_STATUS_BAR_HEIGHT = 44;     // Default status bar height
const TAB_BAR_HORIZONTAL_HEIGHT = 72;    // Match TabBar default height

const COLOR_WHITE = '#ffffff';
const COLOR_BLACK = '#000000';
const COLOR_TRANSPARENT = Color.Transparent;
const POPUP_MASK_COLOR = 'rgba(0,0,0,0.45)';
const OVERLAY_MASK_FADE_DURATION = 180;

const ANIMATION_DURATION_STANDARD = 300;      // Standard animation duration
const ANIMATION_CURVE_EASE_IN_OUT = Curve.EaseInOut;  // Smooth ease in/out curve

const DOMAIN = 0x0000;
const TAG = 'LingXia.Container';

/**
 * Translation options for WebView animations
 */
interface TranslationOptions {
  x: number | string;
  y: number | string;
}

interface MarginOptions {
  top?: number;
  bottom?: number;
  left?: number;
  right?: number;
}

interface CornerRadii {
  topLeft: number;
  topRight: number;
  bottomLeft: number;
  bottomRight: number;
}

interface RenderableWebEntry {
  webTag: string;
  controller: webview.WebviewController;
}

// Layout snapshot for same-level NodeContainer so we can update attributes without recreating the native view
interface NodeLayoutState {
  width: Length;
  height: Length;
  position?: Position;
  zIndex: number;
  cornerRadius: number;
  safeAreaTypes: SafeAreaType[];
  safeAreaEdges: SafeAreaEdge[];
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

  // Static registry for animation type information
  private static animationTypes: Map<string, AnimationType> = new Map()

  @State pageConfig: NavigationBarState | null = null
  @State tabBarState: TabBarState | null = null
  @State selectedTabIndex: number = 0
  @State tabBarRefreshKey: number = 0  // Force TabBar refresh when data changes
  @State navbarRefreshKey: number = 0  // Force NavigationBar refresh when data changes
  @State webViewComponents: Map<string, webview.WebviewController> = new Map() // Store Web components for each WebView
  @State currentWebTag: string = '' // Current active WebView tag
  @State webViewKeys: string[] = [] // Array of WebView keys for ForEach
  @State popupOverlayWebTag: string = '' // WebView currently displayed in popup
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

  // Animation state for WebView transitions
  @State private animationInProgress: boolean = false
  @State private previousWebTag: string = ''
  @State private currentAnimationType: AnimationType = AnimationType.None
  @State private currentWebViewTranslateX: number = 0
  @State private previousWebViewTranslateX: number = 0
  @State private navbarTranslateX: number = 0
  private controllerReadyAcked: Set<string> = new Set()

  // Same-level rendering state
  private embedManagers: Map<string, SameLevelEmbedManager> = new Map()
  @State private embedComponentIds: Map<string, string[]> = new Map()
  @State private fullscreenRefreshKey: number = 0  // Force NodeContainer refresh when fullscreen changes

  // Fullscreen state - when set, only the fullscreen component is rendered
  @State private fullscreenWebTag: string = ''
  @State private fullscreenComponentId: string = ''

  // Layout cache keyed by webTag:componentId to drive NodeContainer attributes reactively
  @State private nodeLayoutCache: Map<string, NodeLayoutState> = new Map()

  private webViewUiCallback = (action: string, info: WebViewInfo) => {
    this.handleWebViewUiEvent(action, info);
  }

  /**
   * Trigger onPageShow if WebView hasn't triggered it yet
   */
  private triggerOnPageShowIfNeeded(webTag: string) {
    if (!this.pageShowTriggered.has(webTag)) {
      const extracted = extractWebTag(webTag);
      if (extracted) {
        const appId = extracted.appId;
        const path = extracted.path;
        const actualPath = path.replace(/-/g, '/');

        // Check if this is the first page (LxApp runtime might not be ready yet)
        const isFirstPage = this.pageShowTriggered.size === 0;
        this.pageShowTriggered.add(webTag);

        if (isFirstPage) {
          // Delay first page to ensure LxApp runtime is ready
          setTimeout(() => {
            hilog.info(DOMAIN, TAG, `Triggering onPageShow for first page: ${appId}:${actualPath}`);
            onPageShow(appId, actualPath);
          }, 100);
        } else {
          // Subsequent pages without delay
          hilog.info(DOMAIN, TAG, `Triggering onPageShow for: ${appId}:${actualPath}`);
          onPageShow(appId, actualPath);
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

    this.path = this.initialPath;
    hilog.info(DOMAIN, TAG, `Container creating for ${this.appId}:${this.path}, isHome=${this.isHomeLxApp()}`);
    hilog.info(DOMAIN, TAG, `Layout info: statusBarHeight=${this.statusBarHeight}, showCapsule=${this.shouldShowCapsuleButton()}`);

    // Register this container for TabBar updates
    LxAppContainer.activeContainers.set(this.appId, this);

    // Initialize animation type from static registry
    const storedAnimationType: AnimationType | undefined = LxAppContainer.animationTypes.get(this.appId);
    if (storedAnimationType !== undefined) {
      this.currentAnimationType = storedAnimationType;
    }

    // Add WebView UI callback to handle component creation (supports multiple containers)
    addWebViewUiCallback(this.webViewUiCallback);

    this.pageConfig = getNavigationBarState(this.appId, this.path);
    hilog.info(DOMAIN, TAG, `Initial navbar state for ${this.appId}:${this.path}: showNavbar=${this.pageConfig?.showNavbar}, showBack=${this.pageConfig?.showBackButton}, showHome=${this.pageConfig?.showHomeButton}, bgColor=0x${this.pageConfig?.navigationBarBackgroundColor?.toString(16)}, textStyle=${this.pageConfig?.navigationBarTextStyle}`);

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
  }

  aboutToDisappear() {
    // Unregister this container
    LxAppContainer.activeContainers.delete(this.appId);

    // Remove WebView UI callback when container is destroyed
    removeWebViewUiCallback(this.webViewUiCallback);
    hilog.info(DOMAIN, TAG, `Container destroyed for ${this.appId}:${this.path}`);
  }

  /**
   * Initialize status bar height with proper error handling
   */
  private async initStatusBarHeight(): Promise<void> {
    try {
      const context = getContext();
      if (!context) {
        throw new Error('Context not available');
      }

      const mainWindow = await window.getLastWindow(context);
      if (!mainWindow) {
        throw new Error('Main window not available');
      }

      const safeArea = await mainWindow.getWindowAvoidArea(window.AvoidAreaType.TYPE_SYSTEM);
      this.statusBarHeight = safeArea.topRect.height || SYSTEM_STATUS_BAR_HEIGHT; // Ensure fallback value
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to get status bar height: ${error}`);
      this.statusBarHeight = SYSTEM_STATUS_BAR_HEIGHT; // Safe fallback value for all devices
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
      hilog.error(DOMAIN, TAG, `Failed to load TabBar data: ${error}`);
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
   * Static method to refresh NavigationBar for a specific appId (uses current path)
   */
  public static refreshNavBar(appId: string): boolean {
    const container = LxAppContainer.activeContainers.get(appId);
    if (container) {
      // Use current path from container
      container.pageConfig = getNavigationBarState(appId, container.path);
      // Force UI update by incrementing refresh key
      container.navbarRefreshKey++;
      return true;
    }
    return false;
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
   * Static method to set overlay (popup) state for an appId + path
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
        // If popup overlay is currently using this webtag, skip attaching in base to avoid double-attach
        const activePopup = LxAppPopup.getActiveLayout && LxAppPopup.getActiveLayout();
        if (activePopup) {
          const popupLogical = toWebTag(activePopup.appId, activePopup.path);
          if (info.webtag === popupLogical || info.webtag.startsWith(`${popupLogical}#`)) {
            hilog.info(DOMAIN, TAG, `Skip base attach for popup webtag: ${info.webtag}`);
            break;
          }
        }
        this.webViewComponents.set(info.webtag, info.controller);
        this.webViewKeys = Array.from(this.webViewComponents.keys());
        hilog.info(DOMAIN, TAG, `Stored WebView component: ${info.webtag}, total components: ${this.webViewComponents.size}`);

        // If there is no active WebView for this container (e.g. after restart),
        // automatically promote the first created WebView to current and trigger page show.
        if (!this.currentWebTag) {
          this.currentWebTag = info.webtag;
          hilog.info(DOMAIN, TAG, `No currentWebTag set; promoting ${info.webtag} as current after create`);
          this.triggerOnPageShowIfNeeded(info.webtag);
        }
        break;
      case 'destroy':
        this.webViewComponents.delete(info.webtag);
        this.webViewKeys = Array.from(this.webViewComponents.keys());

        // Allow new WebView instances for this webtag to re-ack creation after destroy
        this.controllerReadyAcked.delete(info.webtag);

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
            this.controllerReadyAcked.clear();
          }
          break;
      case 'popup_show':
        if (info.webtag.startsWith(`${this.appId}-`)) {
          this.webViewComponents.set(info.webtag, info.controller);
          this.webViewKeys = Array.from(this.webViewComponents.keys());
          const layout = LxAppPopup.getActiveLayout && LxAppPopup.getActiveLayout();
      if (layout && layout.appId === this.appId) {
        this.applyOverlayState(info.webtag, true, layout.widthVp, layout.heightVp, layout.position as number, layout.bottomInset, layout.fullWidth, layout.fullHeight);
        hilog.info(DOMAIN, TAG, `Overlay style set for ${info.webtag}: w=${layout.widthVp} h=${layout.heightVp} pos=${layout.position}`);
      } else {
        this.applyOverlayState(info.webtag, true);
      }
          hilog.info(DOMAIN, TAG, `Popup overlay active for ${info.webtag}`);
        }
        break;
      case 'popup_hide':
        if (info.webtag.startsWith(`${this.appId}-`)) {
          this.webViewComponents.set(info.webtag, info.controller);
          this.webViewKeys = Array.from(this.webViewComponents.keys());
          if (this.keepAliveTags.indexOf(info.webtag) < 0) {
            this.keepAliveTags = [...this.keepAliveTags, info.webtag];
          }
          this.pageShowTriggered.delete(info.webtag);
          this.applyOverlayState(info.webtag, false);
          hilog.info(DOMAIN, TAG, `Popup overlay cleared for ${info.webtag}`);
        }
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
                  statusBarHeight: this.statusBarHeight,
                  standalone: false,
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
              statusBarHeight: this.statusBarHeight,
              standalone: true,
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
            // Usually native player exitFullscreen trigger will clear state via updateFullscreenState
          }
        })
      }
    }
    .width('100%')
    .height('100%')
    .expandSafeArea(this.shouldExpandSafeAreaForContainer() || this.fullscreenComponentId.length > 0 ? [SafeAreaType.SYSTEM] : [], this.shouldExpandSafeAreaForContainer() || this.fullscreenComponentId.length > 0 ? [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM] : [])
  }

  private isHomeLxApp(): boolean {
    const miniApp = LxApp.getInstance();
    const homeAppId = miniApp.getHomeLxAppId();
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
      .hitTestBehavior(this.shouldBlockOverlayInteractions() ? HitTestMode.Block : HitTestMode.Transparent)
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
      .hitTestBehavior(this.shouldBlockOverlayInteractions() ? HitTestMode.Block : HitTestMode.Transparent)
    }
  }

  @Builder
  private renderNavigationBarCore(): void {
    NavigationBar({
      config: this.pageConfig as NavigationBarState,
      appId: this.appId,
      currentPath: this.path,
      translateX: this.navbarTranslateX,
      statusBarHeight: this.statusBarHeight,
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
      .hitTestBehavior(this.shouldBlockOverlayInteractions() ? HitTestMode.Block : HitTestMode.Transparent)
      .onClick(() => {})
      .zIndex(10003)
  }

  private getNavigationBarHeight(): number {
    const contentHeight = NAVBAR_TOTAL_HEIGHT - SYSTEM_STATUS_BAR_HEIGHT;
    return this.statusBarHeight + contentHeight;
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
      this.popupOverlayWebTag = webTag;
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
      if (this.popupOverlayWebTag === webTag) {
        this.popupOverlayWebTag = '';
      }
      this.overlayActive = false;
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
    return !this.isHomeLxApp();
  }

  /**
   * Build WebView area
   */
  @Builder
  buildWebViewArea(): void {
    Stack() {
      // Render all WebView components so they can be associated with controllers
      ForEach(this.getRenderableWebEntries(), (entry: RenderableWebEntry) => {
        this.buildWebViewComponent(entry.webTag, entry.controller)
      }, (entry: RenderableWebEntry) => entry.webTag)
      // Overlay mask on top
      if (this.shouldRenderOverlayMask()) {
        Column()
          .width('100%')
          .height('100%')
          .backgroundColor(POPUP_MASK_COLOR)
          .opacity(this.overlayMaskOpacity)
          .hitTestBehavior(this.shouldBlockOverlayInteractions() ? HitTestMode.Block : HitTestMode.Transparent)
          // Do not close on mask click; hidePopup must be called explicitly
          .onClick(() => {})
          .zIndex(10003)
      }
    }
    .width('100%')
    .height('100%')
    .backgroundColor(COLOR_TRANSPARENT)
    .clip(this.navigationMode)
    // Reserve space for the navigation bar only when the overlay is hidden
    .padding(this.getContainerPadding())
    // Keep the layout fixed so the WebView bounds never change, removing the root cause of flicker
    .hitTestBehavior(HitTestMode.Transparent)
    .zIndex(this.shouldRenderOverlayMask() ? 10004 : 0)
    .expandSafeArea([SafeAreaType.SYSTEM], [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM])
  }

  /**
   * Reserve TabBar space on the container (not inside the Web content):
   * - Only reserve it when the TabBar is opaque to avoid white gaps while the bar is transparent.
   */
  private getWebViewAreaPadding(): Padding {
    // Only reserve NAVBAR_TOTAL_HEIGHT when the navigation bar is visible; otherwise, there would be a 44vp blank at the top
    const showNavbar = this.pageConfig?.showNavbar === true;
    const top = showNavbar ? NAVBAR_TOTAL_HEIGHT : 0;
    const pad: Padding = { top } as Padding;
    return pad;
  }

  private getContainerPadding(): Padding {
    // Remove padding when fullscreen or overlay is active
    if (this.overlayActive || this.overlayMaskOpacity > 0 || this.hasFullscreenTarget()) {
      return { top: 0, bottom: 0, left: 0, right: 0 } as Padding;
    }
    return this.getWebViewAreaPadding();
  }

  /**
   * Compute WebView padding centrally to keep the layout fixed and prevent overlap.
   * - Top: reserve the status or navigation bar height when needed.
   * - Bottom/sides: always reserve space for an opaque TabBar so content never jumps.
   */
  private getWebViewPadding(): Padding {
    // The container handles the top reservation; the bottom is fixed here so opaque TabBars never hide content or trigger flicker when transparency changes
    const bottomPad = this.getWebBottomPadding();
    return { top: 0, bottom: bottomPad } as Padding;
  }

  private getWebBottomPadding(): number {
    const hasTabBar = (this.tabBarState?.items?.length ?? 0) > 0;
    if (!hasTabBar) return 0;
    if (this.isTabBarVertical()) return 0; // Side TabBars do not affect vertical spacing
    // A transparent TabBar overlays the WebView, so allow content underneath (padding = 0)
    const transparent = this.isTabBarTransparent();
    if (transparent) {
      return 0;
    }
    // Reserve the TabBar height when it is opaque to avoid covering content
    const tabBarSize: number = (this.tabBarState?.dimension as number) ?? TAB_BAR_HORIZONTAL_HEIGHT;
    return tabBarSize;
  }

  private shouldRenderNavigationBar(): boolean {
    // Render the navigation bar only when showNavbar is true so the home page does not show a transparent bar
    return this.pageConfig?.showNavbar === true;
  }

  private handleTabSelected(index: number) {
    hilog.info(DOMAIN, TAG, `Tab selected: ${index}, current: ${this.selectedTabIndex}`);
    let _: boolean = onUiEvent(this.appId, UiEventType.TabBarClick, index.toString());
  }

  /**
   * Load navbar state for a page (common for all navigation types)
   */
  private loadNavbarState(path: string): void {
    this.pageConfig = getNavigationBarState(this.appId, path);
  }

  /**
   * Navigate to any page with animation support
   */
  private navigateToPage(path: string): boolean {
    hilog.info(DOMAIN, TAG, `navigate: ${this.appId}:${path} with type: ${AnimationType[this.currentAnimationType]}`);

    // Find WebView controller for the new page (logical lookup)
    const webViewController = findWebview(this.appId, path);

    if (!webViewController) {
      const logicalTag = toWebTag(this.appId, path);
      hilog.warn(DOMAIN, TAG, `WebView not found for page: ${logicalTag}`);
      return false;
    }

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

    // Notify old page becoming inactive (pause SameLevel components)
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

    // Notify new page becoming active (resume SameLevel components)
    resumeWebViewComponents(newWebTag);
    // Refresh NodeContainer layouts so properties like cornerRadius re-apply when returning
    this.embedManagers.forEach((manager: SameLevelEmbedManager, webTag: string) => {
      this.refreshNodeLayouts(manager, webTag);
    });
    // Force NodeContainer attribute refresh
    this.fullscreenRefreshKey++;

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
  }

  private handleNavigationBack(isHomeButton: boolean = false) {
    if (isHomeButton) {
      hilog.info(DOMAIN, TAG, 'Navigation home button pressed');
      let _: boolean = onUiEvent(this.appId, UiEventType.NavigationClick, NAVIGATION_ACTION_HOME);
    } else {
      hilog.info(DOMAIN, TAG, 'Navigation back button pressed');
      let _: boolean = onUiEvent(this.appId, UiEventType.NavigationClick, NAVIGATION_ACTION_BACK);
    }
  }

  private handleCapsuleMore(): void {
    hilog.info(DOMAIN, TAG, 'Capsule more button pressed');
    let _: boolean = onUiEvent(this.appId, UiEventType.CapsuleClick, CAPSULE_ACTION_MORE);
  }

  private handleCapsuleClose(): void {
    let _: boolean = onUiEvent(this.appId, UiEventType.CapsuleClick, CAPSULE_ACTION_CLOSE);
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
        // Same-level native components (rendered via NodeController, MUST be before Web)
        // Note: We use stable keys (componentId only) to avoid component recreation.
        // ArkUI doesn't allow reusing the same FrameNode when key changes.
        // Layout updates are handled through nodeLayoutCache @State variable.
        ForEach(this.getEmbedComponentIds(webTag), (componentId: string) => {
          this.buildNodeContainer(webTag, componentId, this.fullscreenRefreshKey)
        }, (componentId: string) => componentId)

        // Web component with native embed mode enabled
        Web({ src: '', controller: controller })
          .visibility(this.getRenderVisibility(webTag))
          .translate(this.getRenderTranslation(webTag))
          .margin(this.getRenderMargin(webTag))
          .onAppear(() => {
            const overlay = this.isOverlayWebView(webTag);
            hilog.info(DOMAIN, TAG, `WebView component appeared for webTag: ${webTag}, overlay: ${overlay}`);
            // Do not trigger onPageShow here to avoid duplicates; finishTransition will handle it once.
            // Notify native only when ArkUI Web component is actually attached
            if (!this.controllerReadyAcked.has(webTag)) {
              onWebviewControllerCreated(webTag);
              this.controllerReadyAcked.add(webTag);
            }
          })
          .onControllerAttached(() => {
            // Setup same-level embed manager for this WebView
            this.setupEmbedManager(webTag, controller);
            hilog.info(DOMAIN, TAG, `Same-level embed manager setup for webTag: ${webTag}`);
          })
          .onDisAppear(() => {
            // Cleanup same-level components when WebView is destroyed
            this.cleanupEmbedManager(webTag);
            hilog.info(DOMAIN, TAG, `Same-level cleanup for webTag: ${webTag}`);
          })
          // Enable native embed mode for same-level rendering
          .enableNativeEmbedMode(true)
          .onNativeEmbedLifecycleChange((embed) => {
            this.handleEmbedLifecycle(webTag, embed);
          })
          .onNativeEmbedGestureEvent((touch) => {
            this.handleEmbedGesture(webTag, touch);
          })
          .width(this.getRenderWidth(webTag))
          .height(this.getRenderHeight(webTag))
          .padding(this.getRenderPadding(webTag))
          .align(this.getRenderAlignment(webTag))
          .borderRadius(this.getRenderBorderRadius(webTag))
          .clip(this.getRenderClip(webTag))
          .zIndex(this.getRenderZIndex(webTag))
          .hitTestBehavior(this.getRenderHitTest(webTag))
          .backgroundColor(COLOR_TRANSPARENT)
          .domStorageAccess(false)
          .fileAccess(false)
          .databaseAccess(false)
          .mixedMode(MixedMode.None)
      }
      .width('100%')
      .height('100%')
    }
    .width('100%')
    .height('100%')
    .justifyContent(this.getWrapperJustify(webTag))
    .alignItems(this.getWrapperAlign(webTag))
    .hitTestBehavior(HitTestMode.Transparent)
    .zIndex(this.getRenderZIndex(webTag))
  }

  @Builder
  buildNodeContainer(webTag: string, componentId: string, refreshKey: number): void {
    // Fullscreen uses separate XComponent in renderFullscreenOverlay(); original stays at same-level position
    if (this.getEmbedManager(webTag) && this.getEmbedManager(webTag)!.getNodeController(componentId)) {
      NodeContainer(this.getEmbedManager(webTag)!.getNodeController(componentId)!)
        .width(this.getNodeContainerWidth(webTag, componentId))
        .height(this.getNodeContainerHeight(webTag, componentId))
        .borderRadius(this.getNodeContainerCornerRadius(webTag, componentId))
        .clip(true)
        .position(this.getNodeContainerPosition(webTag, componentId))
        .zIndex(this.getNodeContainerZIndex(webTag, componentId))
    }
  }

  private getNodeContainerCornerRadius(webTag: string, componentId: string): number {
    const controller = this.getEmbedManager(webTag)?.getNodeController(componentId);
    if (controller) {
      if (controller.getIsFullscreen()) {
        return 0;
      }
      const radius = controller.getCornerRadius();
      if (radius !== undefined) {
        return radius;
      }
    }

    const layout = this.getNodeLayout(webTag, componentId);
    return layout?.cornerRadius ?? 0;
  }

  private getNodeContainerWidth(webTag: string, componentId: string): Length {
    const layout = this.getNodeLayout(webTag, componentId);
    if (layout) {
      return layout.width;
    }
    const controller = this.getEmbedManager(webTag)?.getNodeController(componentId);
    return controller?.getWidth() ?? 0;
  }

  private getNodeContainerHeight(webTag: string, componentId: string): Length {
    const layout = this.getNodeLayout(webTag, componentId);
    if (layout) {
      return layout.height;
    }
    const controller = this.getEmbedManager(webTag)?.getNodeController(componentId);
    return controller?.getHeight() ?? 0;
  }

  private getNodeContainerPosition(webTag: string, componentId: string): Position | undefined {
    const layout = this.getNodeLayout(webTag, componentId);
    if (layout) {
      return layout.position;
    }
    return undefined;
  }

  private getNodeContainerZIndex(webTag: string, componentId: string): number {
    const layout = this.getNodeLayout(webTag, componentId);
    if (layout) {
      return layout.zIndex;
    }
    return 0;
  }

  // ============================================================================
  // Same-Level Rendering Helper Methods
  // ============================================================================

  /**
   * Setup embed manager for a WebView
   */
  private setupEmbedManager(webTag: string, controller: webview.WebviewController): void {
    const manager = getWebViewEmbedManager(webTag);
    manager.setUIContext(this.getUIContext());
    manager.setOnComponentsChanged(() => {
      // Update component IDs array to trigger ForEach re-render
      const newMap = new Map(this.embedComponentIds);
      newMap.set(webTag, manager.getComponentIds().slice());
      this.embedComponentIds = newMap;

      // Refresh layout cache so NodeContainer attributes react without recreating native view
      this.refreshNodeLayouts(manager, webTag);

      // Check for fullscreen state changes
      this.updateFullscreenState();

      // Increment refresh key to force NodeContainer attribute updates
      this.fullscreenRefreshKey++;
    });
    // Prime layout cache on setup
    this.refreshNodeLayouts(manager, webTag);
    this.embedManagers.set(webTag, manager);
    setEmbedController(webTag, controller);
  }

  /**
   * Check all embed managers for fullscreen state and update accordingly
   */
  private updateFullscreenState(): void {
    let foundFullscreen = false;

    this.embedManagers.forEach((manager: SameLevelEmbedManager, webTag: string) => {
      if (foundFullscreen) return;

      const componentIds = manager.getComponentIds();
      for (let i = 0; i < componentIds.length; i++) {
        const componentId = componentIds[i];
        const controller = manager.getNodeController(componentId);
        if (controller?.getIsFullscreen()) {
          if (this.fullscreenComponentId !== componentId) {
            this.fullscreenWebTag = webTag;
            this.fullscreenComponentId = componentId;
          }
          foundFullscreen = true;
          return;
        }
      }
    });

    if (!foundFullscreen && this.fullscreenComponentId.length > 0) {
      // Switch player back to original surface before clearing fullscreen state
      LxNativeVideoPlayer.savePlaybackState(this.fullscreenComponentId);
      const success = LxNativeVideoPlayer.switchToOriginalSurface(this.fullscreenComponentId);
      hilog.info(DOMAIN, TAG, `Switch to original surface result: ${success}`);

      this.fullscreenWebTag = '';
      this.fullscreenComponentId = '';
    }
  }

  /**
   * Cleanup embed manager for a WebView
   * Safe to call multiple times
   */
  private cleanupEmbedManager(webTag: string): void {
    if (!this.embedManagers.has(webTag)) {
      return; // Already cleaned up
    }
    teardownEmbedManager(webTag);
    this.embedManagers.delete(webTag);
    const newMap = new Map(this.embedComponentIds);
    newMap.delete(webTag);
    this.embedComponentIds = newMap;

    // Remove cached layouts for this WebView
    this.removeNodeLayoutsForWebTag(webTag);
  }

  /**
   * Compute and cache layout state for all components of a WebView
   * This drives NodeContainer attributes without recreating native nodes
   */
  private refreshNodeLayouts(manager: SameLevelEmbedManager, webTag: string): void {
    const next = new Map(this.nodeLayoutCache);
    const componentIds = manager.getComponentIds();
    const prefix = `${webTag}:`;

    // Remove stale entries for this webTag
    next.forEach((_, key: string) => {
      if (key.startsWith(prefix)) {
        const id = key.substring(prefix.length);
        if (!componentIds.includes(id)) {
          next.delete(key);
        }
      }
    });

    // Update current entries
    componentIds.forEach((componentId: string) => {
      const controller = manager.getNodeController(componentId);
      const isFullscreen = controller?.getIsFullscreen() ?? false;
      const layout: NodeLayoutState = {
        width: isFullscreen ? '100%' : (controller?.getWidth() ?? 0),
        height: isFullscreen ? '100%' : (controller?.getHeight() ?? 0),
        position: isFullscreen ? { x: 0, y: 0 } : undefined,
        // Must be higher than backdrop (49999) to appear on top
        zIndex: isFullscreen ? 50000 : 0,
        cornerRadius: isFullscreen ? 0 : (controller?.getCornerRadius() ?? 0),
        safeAreaTypes: isFullscreen ? [SafeAreaType.SYSTEM] : [],
        safeAreaEdges: isFullscreen ? [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM] : []
      };
      next.set(this.getLayoutKey(webTag, componentId), layout);
    });

    this.nodeLayoutCache = next;
  }

  /**
   * Remove cached layouts for a specific WebView
   */
  private removeNodeLayoutsForWebTag(webTag: string): void {
    const prefix = `${webTag}:`;
    const next = new Map(this.nodeLayoutCache);
    next.forEach((_, key: string) => {
      if (key.startsWith(prefix)) {
        next.delete(key);
      }
    });
    this.nodeLayoutCache = next;
  }

  private getLayoutKey(webTag: string, componentId: string): string {
    return `${webTag}:${componentId}`;
  }

  private getNodeLayout(webTag: string, componentId: string): NodeLayoutState | undefined {
    return this.nodeLayoutCache.get(this.getLayoutKey(webTag, componentId));
  }

  private isFullscreenTarget(webTag: string, componentId: string): boolean {
    return this.fullscreenComponentId.length > 0 &&
      this.fullscreenWebTag.length > 0 &&
      this.fullscreenComponentId === componentId &&
      this.fullscreenWebTag === webTag;
  }

  private hasFullscreenTarget(): boolean {
    return this.fullscreenComponentId.length > 0 && this.fullscreenWebTag.length > 0;
  }

  private getEmbedManager(webTag: string): SameLevelEmbedManager | undefined {
    return this.embedManagers.get(webTag);
  }
  private getEmbedComponentIds(webTag: string): string[] {
    return this.embedComponentIds.get(webTag) || [];
  }

  private handleEmbedLifecycle(webTag: string, embed: NativeEmbedDataInfo): void {
    const manager = this.embedManagers.get(webTag);
    if (manager) {
      manager.setUIContext(this.getUIContext());
      manager.handleLifecycleChange(embed);
    }
  }

  private handleEmbedGesture(webTag: string, touch: NativeEmbedTouchInfo): void {
    const manager = this.embedManagers.get(webTag);
    if (manager) manager.handleGestureEvent(touch);
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

  private getOverlayMargin(): MarginOptions {
    const base: MarginOptions = { top: 0, left: 0, right: 0, bottom: 0 };
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
        base.right = horizontalGap;
        base.top = verticalGap;
        base.bottom = verticalGap;
        break;
      case 3:
        base.right = 0;
        base.left = horizontalGap;
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

  private isOverlayWebView(webTag: string): boolean {
    return this.overlayActive && webTag === this.popupOverlayWebTag;
  }

  private getRenderVisibility(webTag: string): Visibility {
    if (this.isOverlayWebView(webTag)) {
      return Visibility.Visible;
    }
    return this.getWebViewVisibility(webTag);
  }

  private getRenderTranslation(webTag: string): TranslationOptions {
    if (this.isOverlayWebView(webTag)) {
      return { x: 0, y: 0 } as TranslationOptions;
    }
    return this.getWebViewTranslation(webTag);
  }

  private getRenderMargin(webTag: string): MarginOptions {
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

  private getRenderPadding(webTag: string): Padding {
    if (this.isOverlayWebView(webTag)) {
      return { top: 0, bottom: 0, left: 0, right: 0 } as Padding;
    }
    return this.getWebViewPadding();
  }

  private getRenderAlignment(webTag: string): Alignment {
    return this.isOverlayWebView(webTag) ? this.getOverlayAlignment() : Alignment.TopStart;
  }

  private getRenderBorderRadius(webTag: string): CornerRadii {
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
    // Web content always 100% height of the area allocated to it
    return '100%';
  }

  private getWebViewVisibility(webTag: string): Visibility {
    // Animation logic
    if (this.animationInProgress) {
      if (webTag === this.currentWebTag) return Visibility.Visible;
      if (webTag === this.previousWebTag) return Visibility.Visible;
      return Visibility.None;
    }

    // Standard visibility
    return webTag === this.currentWebTag ? Visibility.Visible : this.getHiddenVisibility(webTag);
  }

  private getHiddenVisibility(webTag: string): Visibility {
    // Keep alive logic
    if (this.keepAliveTags.includes(webTag)) {
      return Visibility.Visible;
    }
    // Pre-render logic could go here
    return Visibility.None;
  }

  private getWebViewTranslation(webTag: string): TranslationOptions {
    if (!this.animationInProgress) {
      // Ensure active WebView is at 0, others are moved out if kept alive
      if (webTag === this.currentWebTag) return { x: 0, y: 0 };
      if (this.keepAliveTags.includes(webTag)) return { x: '100%', y: 0 };
      return { x: 0, y: 0 };
    }

    if (webTag === this.currentWebTag) {
      return { x: `${this.currentWebViewTranslateX}%`, y: 0 };
    }
    if (webTag === this.previousWebTag) {
      return { x: `${this.previousWebViewTranslateX}%`, y: 0 };
    }
    return { x: 0, y: 0 };
  }

  private getWebViewMargin(): MarginOptions {
    return { top: 0, bottom: 0, left: 0, right: 0 };
  }

  private getRenderableWebEntries(): RenderableWebEntry[] {
    const entries: RenderableWebEntry[] = [];
    this.webViewComponents.forEach((controller, tag) => {
      entries.push({ webTag: tag, controller: controller });
    });
    return entries;
  }
}