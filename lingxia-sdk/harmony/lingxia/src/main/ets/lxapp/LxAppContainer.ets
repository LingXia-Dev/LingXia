import { hilog } from '@kit.PerformanceAnalysisKit';
import { webview } from '@kit.ArkWeb';
import { window } from '@kit.ArkUI';
import { common } from '@kit.AbilityKit';
import { LxApp } from './LxApp';
import { LxAppPopup } from './APIs/LxAppPopup';
import { findWebview, toWebTag, extractWebTag, WebTagParts, addWebViewUiCallback, removeWebViewUiCallback, WebViewInfo } from './WebView';
import { NavigationBar, type NavigationBarState, NAVBAR_TOTAL_HEIGHT } from './NavigationBar';
import { NavigationButton, CapsuleButton } from './LxAppDrawables';
import { TabBar } from './TabBar';
import { TabBarPosition, type TabBarItem, UiEventType } from 'liblingxia.so';
import liblingxia, { TabBarState, getNavigationBarState, onPageShow, onUiEvent } from 'liblingxia.so';
import { NAVIGATION_ACTION_BACK, NAVIGATION_ACTION_HOME, CAPSULE_ACTION_MORE, CAPSULE_ACTION_CLOSE } from './NativeBridge';
import { AnimationType } from './LxAppNavigation';

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

interface RenderableWebEntry {
  webTag: string;
  controller: webview.WebviewController;
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
  @State overlayPosition: number = 0 // 0 center, 1 bottom
  @State overlayBottomInset: number = 0
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

    // Always set currentWebTag for initial page to ensure visibility
    const initialWebTag = toWebTag(this.appId, this.path);
    this.currentWebTag = initialWebTag;
    hilog.info(DOMAIN, TAG, `Set initial currentWebTag to: ${initialWebTag} (will be visible when WebView is created)`);

    // Try to find existing WebView for the initial page
    const initialController = findWebview(this.appId, this.path);
    if (initialController) {
      this.storeWebViewComponent(initialWebTag, initialController);
      hilog.info(DOMAIN, TAG, `Found existing WebView for initial page: ${initialWebTag}`);
    } else {
      hilog.info(DOMAIN, TAG, `No existing WebView found for initial page: ${initialWebTag}, will create when needed`);
    }
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
      container.updateTabSelection(path);
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
    bottomInset?: number
  ): boolean {
    const container = LxAppContainer.activeContainers.get(appId);
    if (!container) {
      hilog.warn(DOMAIN, TAG, `setOverlay: no active container for ${appId}`);
      return false;
    }
    const webTag = toWebTag(appId, path);
    container.applyOverlayState(webTag, active, widthVp, heightVp, position, bottomInset);
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
        if (activePopup && info.webtag === toWebTag(activePopup.appId, activePopup.path)) {
          hilog.info(DOMAIN, TAG, `Skip base attach for popup webtag: ${info.webtag}`);
          break;
        }
        this.webViewComponents.set(info.webtag, info.controller);
        this.webViewKeys = Array.from(this.webViewComponents.keys());
        hilog.info(DOMAIN, TAG, `Stored WebView component: ${info.webtag}, total components: ${this.webViewComponents.size}`);
        break;
      case 'destroy':
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
        break;
      case 'popup_show':
        if (info.webtag.startsWith(`${this.appId}-`)) {
          this.webViewComponents.set(info.webtag, info.controller);
          this.webViewKeys = Array.from(this.webViewComponents.keys());
          const layout = LxAppPopup.getActiveLayout && LxAppPopup.getActiveLayout();
          if (layout && layout.appId === this.appId) {
            this.applyOverlayState(info.webtag, true, layout.widthVp, layout.heightVp, layout.position as number, layout.bottomInset);
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

  storeWebViewComponent(webTag: string, controller: webview.WebviewController) {
    hilog.info(DOMAIN, TAG, `Storing Web component for webTag: ${webTag}`);
    this.webViewComponents.set(webTag, controller);
    this.webViewKeys = Array.from(this.webViewComponents.keys());

    // If this is the current tab, ensure it's set as current
    if (webTag === toWebTag(this.appId, this.path)) {
      this.currentWebTag = webTag;
      hilog.info(DOMAIN, TAG, `Confirmed as current WebView: ${webTag}, will be visible immediately`);
    }
  }

  build() {
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
      .expandSafeArea(this.shouldExpandSafeAreaForContainer() ? [SafeAreaType.SYSTEM] : [], this.shouldExpandSafeAreaForContainer() ? [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM] : [])
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
      .expandSafeArea(this.shouldExpandSafeAreaForContainer() ? [SafeAreaType.SYSTEM] : [], this.shouldExpandSafeAreaForContainer() ? [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM] : [])
    }
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
    hilog.info(DOMAIN, TAG, `Safe area expansion for ${this.appId}: true`);
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
    bottomInset?: number
  ): void {
    if (active) {
      this.popupOverlayWebTag = webTag;
      this.overlayActive = true;
      if (widthVp !== undefined) this.overlayWidthVp = widthVp;
      if (heightVp !== undefined) this.overlayHeightVp = heightVp;
      if (position !== undefined) this.overlayPosition = position;
      if (bottomInset !== undefined) this.overlayBottomInset = bottomInset;
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
    const shouldShow = !this.isHomeLxApp();
    hilog.info(DOMAIN, TAG, `shouldShowCapsuleButton: appId=${this.appId}, shouldShow=${shouldShow}`);
    return shouldShow;
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
    // Reserve space for the navigation bar at the top and for an opaque TabBar elsewhere
    .padding(this.getWebViewAreaPadding())
    // Keep the layout fixed so the WebView bounds never change, removing the root cause of flicker
    .hitTestBehavior(HitTestMode.Transparent)
  }

  /**
   * Reserve TabBar space on the container (not inside the Web content):
   * - Only reserve it when the TabBar is opaque to avoid white gaps while the bar is transparent.
   */
  private getWebViewAreaPadding(): Padding {
    // No flicker: always reserve NAVBAR_TOTAL_HEIGHT at the top; let the Web content handle the bottom padding
    const top = NAVBAR_TOTAL_HEIGHT;
    const pad: Padding = { top } as Padding;
    hilog.info(DOMAIN, TAG, `containerPadding -> top=${top}, showNavbar=${this.pageConfig?.showNavbar}`);
    return pad;
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

    // Find WebView for the new page
    const newWebTag = toWebTag(this.appId, path);
    const webViewController = findWebview(this.appId, path);

    if (!webViewController) {
      hilog.warn(DOMAIN, TAG, `WebView not found for page: ${newWebTag}`);
      return false;
    }

    // Store previous WebView for animation
    const oldWebTag = this.currentWebTag;

    // Store WebView component first
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
    this.updateTabSelection(path);

    this.loadNavbarState(path);
    this.loadTabBarState();

    // Start slide animation
    animateTo({
      duration: ANIMATION_DURATION_STANDARD,
      curve: ANIMATION_CURVE_EASE_IN_OUT,
      playMode: PlayMode.Normal,
      onFinish: () => {
        this.finishTransition(newWebTag, path);
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

    // Update current WebView and path immediately
    this.currentWebTag = newWebTag;
    this.path = path;

    // Update tab selection if this is a tab page
    this.updateTabSelection(path);

    // Load navbar state for instant transitions
    this.loadNavbarState(path);
    this.loadTabBarState();

    this.finishTransition(newWebTag, path);
  }

  /**
   * Finish transition and trigger necessary callbacks
   */
  private finishTransition(newWebTag: string, path: string): void {
    // Reset animation state
    this.animationInProgress = false;
    this.previousWebTag = '';

    // Reset translation values
    this.currentWebViewTranslateX = 0;
    this.previousWebViewTranslateX = 0;
    this.navbarTranslateX = 0;

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
   * Update tab selection for a path (called by navigation layer)
   * Simply refresh TabBar data from Rust - no local state management needed
   */
  public updateTabSelection(path: string): void {
    if (!this.isTabBarItem(path)) {
      return;
    }

    // Just refresh TabBar data from Rust - Rust manages the selected_index
    this.refreshTabBarData();
    hilog.info(DOMAIN, TAG, `Updated tab selection for path: ${path}`);
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
  */
  @Builder
  buildWebViewComponent(webTag: string, controller: webview.WebviewController): void {
    Column() {
      Web({ src: '', controller: controller })
        .visibility(this.getRenderVisibility(webTag))
        .translate(this.getRenderTranslation(webTag))
        .margin(this.getRenderMargin(webTag))
        .onAppear(() => {
          const overlay = this.isOverlayWebView(webTag);
          hilog.info(DOMAIN, TAG, `WebView component appeared for webTag: ${webTag}, overlay: ${overlay}`);
          if (this.shouldTriggerOnAppear(webTag)) {
            this.triggerOnPageShowIfNeeded(webTag);
          }
        })
        .width(this.getRenderWidth(webTag))
        .height(this.getRenderHeight(webTag))
        .padding(this.getRenderPadding(webTag))
        .align(this.getRenderAlignment(webTag))
        .borderRadius(this.getRenderBorderRadius(webTag))
        .clip(this.getRenderClip(webTag))
        .zIndex(this.getRenderZIndex(webTag))
        .hitTestBehavior(this.getRenderHitTest(webTag))
        .backgroundColor(COLOR_TRANSPARENT);
    }
    .width('100%')
    .height('100%')
    .justifyContent(this.getWrapperJustify(webTag))
    .alignItems(this.getWrapperAlign(webTag))
    .hitTestBehavior(HitTestMode.Transparent)
    .zIndex(this.getRenderZIndex(webTag));
  }

  private getWrapperJustify(webTag: string): FlexAlign {
    if (this.isOverlayWebView(webTag)) {
      return this.overlayPosition === 1 ? FlexAlign.End : FlexAlign.Center;
    }
    return FlexAlign.Start;
  }

  private getWrapperAlign(webTag: string): HorizontalAlign {
    return this.isOverlayWebView(webTag) ? HorizontalAlign.Center : HorizontalAlign.Start;
  }

  private getOverlayWidth(): string {
    return this.overlayWidthVp > 0 ? `${this.overlayWidthVp}vp` : '90%';
  }

  private getOverlayHeight(): string {
    return this.overlayHeightVp > 0 ? `${this.overlayHeightVp}vp` : '80%';
  }

  private getOverlayAlignment(): Alignment {
    return this.overlayPosition === 1 ? Alignment.Bottom : Alignment.Center;
  }

  private getOverlayMargin(): MarginOptions {
    if (this.overlayPosition === 1 && this.overlayBottomInset > 0) {
      return { bottom: this.overlayBottomInset } as MarginOptions;
    }
    return { top: 0, left: 0, right: 0, bottom: 0 } as MarginOptions;
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

  private getRenderBorderRadius(webTag: string): number {
    return this.isOverlayWebView(webTag) ? 16 : 0;
  }

  private getRenderClip(webTag: string): boolean {
    return this.isOverlayWebView(webTag);
  }

  private getRenderZIndex(webTag: string): number {
    return this.isOverlayWebView(webTag) ? 10004 : 0;
  }

  private getRenderHitTest(webTag: string): HitTestMode {
    return this.isOverlayWebView(webTag) ? HitTestMode.Block : HitTestMode.Transparent;
  }

  private shouldTriggerOnAppear(webTag: string): boolean {
    if (this.isOverlayWebView(webTag)) {
      return true;
    }
    return webTag === this.currentWebTag;
  }

  /**
   * Get WebView visibility based on current state and animation
   */
  private getWebViewVisibility(webTag: string): Visibility {
    if (webTag === this.currentWebTag) {
      return Visibility.Visible;
    }

    // Show previous WebView during animation
    if (this.animationInProgress && webTag === this.previousWebTag) {
      return Visibility.Visible;
    }

    // Keep-alive pages remain Visible (moved offscreen) to preserve ArkWeb host
    if (this.keepAliveTags.indexOf(webTag) >= 0) {
      return Visibility.Visible;
    }

    return Visibility.None;
  }

  /**
   * Get WebView translation for slide animation
   */
  private getWebViewTranslation(webTag: string): TranslationOptions {
    // Translate only on the X axis during transitions; margins handle vertical adjustments to avoid hit-test quirks
    // If this page is kept alive (not current), move it offscreen vertically to avoid paint while keeping host
    if (this.keepAliveTags.indexOf(webTag) >= 0 && webTag !== this.currentWebTag && webTag !== this.previousWebTag) {
      return { x: 0, y: '200%' } as TranslationOptions;
    }

    if (!this.animationInProgress) {
      return { x: 0, y: 0 } as TranslationOptions;
    }

    if (webTag === this.currentWebTag) {
      // Current WebView uses animated translation value
      return { x: `${this.currentWebViewTranslateX}%`, y: 0 } as TranslationOptions;
    }

    if (webTag === this.previousWebTag) {
      // Previous WebView uses animated translation value
      return { x: `${this.previousWebViewTranslateX}%`, y: 0 } as TranslationOptions;
    }

    return { x: 0, y: 0 } as TranslationOptions;
  }

  private getRenderableWebKeys(): string[] {
    // Always return all keys; overlay branch will restyle the popup webtag,
    // default branch will handle others and skip overlay webtag via the if-branch in buildWebViewComponent
    return this.webViewKeys;
  }

  private getRenderableWebEntries(): RenderableWebEntry[] {
    const entries: RenderableWebEntry[] = [];
    const keys = this.getRenderableWebKeys();

    keys.forEach((webTag: string) => {
      const controller = this.getRenderController(webTag);
      if (controller) {
        entries.push({ webTag, controller });
      }
    });

    return entries;
  }

  private getRenderController(webTag: string): webview.WebviewController | null {
    // Always return the controller; overlay branch will restyle the popup webTag
    return this.webViewComponents.get(webTag) || null;
  }


  /**
   * When the navigation bar is hidden, move the WebView up by NAVBAR_TOTAL_HEIGHT so it visually touches the system top.
   * Keep the vertical offset at zero when the navigation bar is visible.
   * This keeps the fixed top padding (no flicker) while preventing pages without a nav bar from showing a white gap.
   */
  private getWebViewMargin(): MarginOptions {
    // Use a negative margin to lift the WebView when the nav bar is hidden; otherwise apply no offset
    const top = (this.pageConfig?.showNavbar === true) ? 0 : -NAVBAR_TOTAL_HEIGHT;
    return { top } as MarginOptions;
  }

  /**
   * Extend the WebView height when the navigation bar is hidden so the upward shift does not leave a gap; keep 100% height otherwise.
   */
  private getWebContentHeight(): string {
    // Add extra height only when the nav bar is hidden; the Web content handles the bottom padding internally
    const extraTop = (this.pageConfig?.showNavbar === true) ? 0 : NAVBAR_TOTAL_HEIGHT;
    const result = extraTop > 0 ? `calc(100% + ${extraTop}vp)` : '100%';
    return result;
  }

}
