import { hilog } from '@kit.PerformanceAnalysisKit';
import { webview } from '@kit.ArkWeb';
import { window } from '@kit.ArkUI';
import { common } from '@kit.AbilityKit';
import { LxApp } from './LxApp';
import { findWebview, handleScrollChanged, toWebTag, extractWebTag, WebTagParts, addWebViewUiCallback, removeWebViewUiCallback, WebViewInfo } from './WebView';
import { NavigationBar, type NavigationBarState, NAVIGATION_BUTTON_SIZE, BACK_BUTTON_LEFT_POSITION, HOME_BUTTON_LEFT_POSITION, calculateNavigationButtonYPosition } from './NavigationBar';
import { NavigationButton, CapsuleButton } from './LxAppDrawables';
import { TabBar } from './TabBar';
import { TabBarPosition, type TabBarItem, UiEventType } from 'liblingxia.so';
import liblingxia, { TabBarState, getNavigationBarState, onPageShow, onUiEvent } from 'liblingxia.so';
import { NAVIGATION_ACTION_BACK, NAVIGATION_ACTION_HOME, CAPSULE_ACTION_MORE, CAPSULE_ACTION_CLOSE } from './NativeBridge';
import { NavigationType } from './LxAppNavigation';

// Container-specific constants
const SYSTEM_STATUS_BAR_HEIGHT = 44;     // Default status bar height

const NAVIGATION_BUTTON_Z_INDEX = 1000;           // Navigation button z-index
const NAVIGATION_BUTTON_Y_POSITION = 45;          // Navigation button vertical position

// Calculated absolute positions
const NAVIGATION_BUTTON_POSITION_BACK_LEFT = 16;  // Back button left position
const NAVIGATION_BUTTON_POSITION_HOME_LEFT = 8;   // Home button left position

const COLOR_WHITE = '#ffffff';
const COLOR_BLACK = '#000000';
const COLOR_TRANSPARENT = Color.Transparent;

const ANIMATION_DURATION_STANDARD = 300;      // Standard animation duration
const ANIMATION_CURVE_EASE_IN_OUT = Curve.EaseInOut;  // Smooth ease in/out curve

// Frosted glass effect colors
const FROSTED_GLASS_LIGHT = 'rgba(255, 255, 255, 0.2)';  // For dark navbar backgrounds
const FROSTED_GLASS_DARK = 'rgba(0, 0, 0, 0.1)';          // For light navbar backgrounds

const DOMAIN = 0x0000;
const TAG = 'LingXia.Container';

/**
 * Translation options for WebView animations
 */
interface TranslationOptions {
  x: number | string;
  y: number | string;
}

/**
 * Navigation button style configuration
 */
interface NavigationButtonStyle {
  color: ResourceColor;
  bgColor: ResourceColor;
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

  // Static registry for navigation type information
  private static navigationTypes: Map<string, NavigationType> = new Map()

  @State pageConfig: NavigationBarState | null = null
  @State tabBarState: TabBarState | null = null
  @State selectedTabIndex: number = 0
  @State tabBarRefreshKey: number = 0  // Force TabBar refresh when data changes
  @State webViewComponents: Map<string, webview.WebviewController> = new Map() // Store Web components for each WebView
  @State currentWebTag: string = '' // Current active WebView tag
  @State webViewKeys: string[] = [] // Array of WebView keys for ForEach
  private pageShowTriggered: Set<string> = new Set() // Track which pages have triggered onPageShow
  @State private statusBarHeight: number = SYSTEM_STATUS_BAR_HEIGHT // Default status bar height

  // Animation state for WebView transitions
  @State private animationInProgress: boolean = false
  @State private previousWebTag: string = ''
  @State private currentNavigationType: NavigationType = NavigationType.Launch
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
  @Watch('onNavigationTrigger') @Prop navigationTrigger: number = 0

  // Manual animation control for new lxapps
  @State animationOffsetY: number = 0 // 0 = final position, positive = off-screen bottom

  aboutToAppear() {
    this.initStatusBarHeight();

    this.path = this.initialPath;
    hilog.info(DOMAIN, TAG, `Container creating for ${this.appId}:${this.path}, isHome=${this.isHomeLxApp()}`);
    hilog.info(DOMAIN, TAG, `Layout info: statusBarHeight=${this.statusBarHeight}, showCapsule=${this.shouldShowCapsuleButton()}`);

    // Register this container for TabBar updates
    LxAppContainer.activeContainers.set(this.appId, this);

    // Initialize navigation type from static registry
    const storedNavigationType = LxAppContainer.navigationTypes.get(this.appId);
    if (storedNavigationType !== undefined) {
      this.currentNavigationType = storedNavigationType;
    }

    // Add WebView UI callback to handle component creation (supports multiple containers)
    addWebViewUiCallback(this.webViewUiCallback);

    this.pageConfig = getNavigationBarState(this.appId, this.path);
    hilog.info(DOMAIN, TAG, `Initial navbar state for ${this.appId}:${this.path}: showNavbar=${this.pageConfig?.showNavbar}, showBack=${this.pageConfig?.showBackButton}, showHome=${this.pageConfig?.showHomeButton}, bgColor=0x${this.pageConfig?.navigationBarBackgroundColor?.toString(16)}, textStyle=${this.pageConfig?.navigationBarTextStyle}`);

    // Load TabBar data using new data-driven API
    this.loadTabBarState();
    this.setInitialSelectedTab();

    // If this page has tabs, set navigation type to SwitchTab
    if (this.tabBarState && this.tabBarState.items && this.tabBarState.items.length > 0) {
      this.currentNavigationType = NavigationType.SwitchTab;
      hilog.info(DOMAIN, TAG, `Set navigation type to SwitchTab for page with ${this.tabBarState.items.length} tabs`);
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
   * Get absolute positioning for navigation buttons
   */
  private getNavigationButtonPosition(isBackButton: boolean): Position {
    return {
      x: isBackButton ? NAVIGATION_BUTTON_POSITION_BACK_LEFT : NAVIGATION_BUTTON_POSITION_HOME_LEFT,
      y: NAVIGATION_BUTTON_Y_POSITION
    };
  }

  /**
   * Get navigation button style based on navigation bar state
   */
  private getNavigationButtonStyle(isBackButton: boolean): NavigationButtonStyle {
    const hasNavbar = this.pageConfig?.showNavbar;
    const isWhiteText = this.pageConfig?.navigationBarTextStyle === 'white';
    const shouldUseWhiteStyle = hasNavbar === true && isWhiteText === true;

    return {
      color: shouldUseWhiteStyle ? COLOR_WHITE : COLOR_BLACK,
      bgColor: shouldUseWhiteStyle ? FROSTED_GLASS_LIGHT : FROSTED_GLASS_DARK
    };
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
    this.tabBarRefreshKey++;

    hilog.info(DOMAIN, TAG, `TabBar refresh completed, refreshKey: ${this.tabBarRefreshKey}`);
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
   * Static method to refresh NavigationBar for a specific appId and path
   */
  public static refreshNavBar(appId: string, path: string): boolean {
    const container = LxAppContainer.activeContainers.get(appId);
    if (container) {
      container.pageConfig = getNavigationBarState(appId, path);
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
   * Static method to set navigation type for animation control
   */
  public static setNavigationType(appId: string, navigationType: NavigationType): void {
    LxAppContainer.navigationTypes.set(appId, navigationType);
    const container = LxAppContainer.activeContainers.get(appId);
    if (container) {
      container.currentNavigationType = navigationType;
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

  /**
   * Handle navigation trigger changes (unified approach for all LxApps)
   */
  onNavigationTrigger() {
    if (this.externalPath) {
      hilog.info(DOMAIN, TAG, `Navigation trigger: ${this.externalPath}`);
      this.navigateToPage(this.externalPath);
    }
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
            if (this.pageConfig && this.pageConfig.showNavbar) {
              NavigationBar({
                config: this.pageConfig,
                appId: this.appId,
                currentPath: this.path,
                translateX: this.navbarTranslateX,
                statusBarHeight: this.statusBarHeight,
                showNavigationButtons: false, // Don't show buttons in navbar
                onNavigationClick: (appId: string, isHomeButton: boolean): boolean => {
                  this.handleNavigationBack(isHomeButton);
                  return true;
                }
              })
            }
            this.buildWebViewArea()
          }
          .layoutWeight(1)
          .height('100%')

          // Always show navigation button at absolute position
          this.buildAbsoluteNavigationButton()
        }

        if (this.tabBarState && this.tabBarState.position === TabBarPosition.Right) {
          this.buildTabBar()
        }
      }
      .width('100%')
      .height('100%')
      .backgroundColor(Color.Transparent)
      .expandSafeArea(this.shouldExpandSafeAreaForContainer() ? [SafeAreaType.SYSTEM] : [], this.shouldExpandSafeAreaForContainer() ? [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM] : [])
    } else if (this.isTabBarTransparent()) {

      Stack({ alignContent: this.getTabBarAlignment() }) {
        Column() {
          this.buildNavigationBar()
          this.buildWebViewArea()
        }
        .width('100%')
        .height('100%')

        this.buildTabBar()
        if (this.shouldShowCapsuleButton()) {
          CapsuleButton({
            statusBarHeight: this.statusBarHeight,
            standalone: true,
            onMorePressed: () => this.handleCapsuleMore(),
            onClosePressed: () => this.handleCapsuleClose()
          })
        }
        this.buildAbsoluteNavigationButton()
      }
      .width('100%')
      .height('100%')
      .backgroundColor(Color.Transparent)
      .expandSafeArea(this.shouldExpandSafeAreaForContainer() ? [SafeAreaType.SYSTEM] : [], this.shouldExpandSafeAreaForContainer() ? [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM] : [])
    } else {
      Stack() {
        Column() {
          this.buildNavigationBar()
          this.buildWebViewArea()

          if (this.tabBarState && (this.tabBarState.position === TabBarPosition.Bottom || !this.tabBarState.position)) {
            this.buildTabBar()
          }
        }
        .width('100%')
        .height('100%')

        if (this.shouldShowCapsuleButton()) {
          CapsuleButton({
            statusBarHeight: this.statusBarHeight,
            standalone: true,
            onMorePressed: () => this.handleCapsuleMore(),
            onClosePressed: () => this.handleCapsuleClose()
          })
        }
        this.buildAbsoluteNavigationButton()
      }
      .width('100%')
      .height('100%')
      .backgroundColor(Color.Transparent)
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
    // Always expand for full screen coverage
    hilog.info(DOMAIN, TAG, `Safe area expansion for ${this.appId}: true`);
    return true;
  }

  /**
   * Check if TabBar is transparent using bit operations
   * When transparent, TabBar overlays WebView; when opaque, they are separate
   */
  private isTabBarTransparent(): boolean {
    const bgColor: number = this.tabBarState?.backgroundColor ?? 0xFFFFFFFF;
    return ((bgColor >>> 24) & 0xFF) === 0;
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
        selectedIndex: this.selectedTabIndex,
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

  /**
   * Build NavigationBar component (shared by both branches)
   */
  @Builder
  buildNavigationBar(): void {
    if (this.pageConfig && this.pageConfig.showNavbar) {
      NavigationBar({
        config: this.pageConfig,
        appId: this.appId,
        currentPath: this.path,
        translateX: this.navbarTranslateX,
        statusBarHeight: this.statusBarHeight, // Pass status bar height
        showNavigationButtons: false, // Don't show buttons in navbar, use fixed position instead
        onNavigationClick: (appId: string, isHomeButton: boolean): boolean => {
          this.handleNavigationBack(isHomeButton);
          return true;
        }
      })
    }
  }

  /**
   * Build absolute positioned navigation button with consistent styling
   * Priority: Back button takes precedence over home button when both are true
   */
  @Builder
  buildAbsoluteNavigationButton(): void {
    if (this.pageConfig && this.pageConfig.showBackButton) {
      this.buildNavigationButtonInternal(true);
    } else if (this.pageConfig && this.pageConfig.showHomeButton) {
      this.buildNavigationButtonInternal(false);
    }
  }

  /**
   * Internal method to build navigation button with proper styling
   */
  @Builder
  buildNavigationButtonInternal(isBackButton: boolean): void {
    NavigationButton({
      isBackButton: isBackButton,
      color: this.getNavigationButtonStyle(isBackButton).color,
      bgColor: this.getNavigationButtonStyle(isBackButton).bgColor,
      onPressed: () => {
        hilog.info(DOMAIN, TAG, `Navigation ${isBackButton ? 'back' : 'home'} button pressed for appId: ${this.appId}`);
        this.handleNavigationBack(!isBackButton);
      }
    })
    .width(NAVIGATION_BUTTON_SIZE)
    .height(NAVIGATION_BUTTON_SIZE)
    .position(this.getNavigationButtonPosition(isBackButton))
    .zIndex(NAVIGATION_BUTTON_Z_INDEX)
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
      ForEach(this.webViewKeys, (webTag: string) => {
        this.buildWebViewComponent(webTag, this.webViewComponents.get(webTag)!)
      })
    }
    .width('100%')
    .height('100%')
    .layoutWeight(this.isTabBarTransparent() ? 0 : 1)
    .backgroundColor(this.navigationMode ? COLOR_WHITE : COLOR_TRANSPARENT)
    .clip(this.navigationMode)
  }

  private setInitialSelectedTab() {
    if (this.tabBarState && this.tabBarState.items.length > 0) {
      // Find corresponding tab based on current path
      const currentTabIndex: number = this.tabBarState.items.findIndex((item: TabBarItem): boolean =>
        this.path.includes(item.pagePath)
      );
      this.selectedTabIndex = currentTabIndex >= 0 ? currentTabIndex : 0;

      hilog.info(DOMAIN, TAG, `setInitialSelectedTab: appId=${this.appId}, path=${this.path}, selectedTabIndex=${this.selectedTabIndex}`);
      this.tabBarState.items.forEach((item: TabBarItem, index: number): void => {
        hilog.info(DOMAIN, TAG, `Tab ${index}: path=${item.pagePath}, selected=${index === this.selectedTabIndex} (appId: ${this.appId})`);
      });
    }
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
    hilog.info(DOMAIN, TAG, `Loaded navbar state for ${path}: showNavbar=${this.pageConfig?.showNavbar}, showBack=${this.pageConfig?.showBackButton}, showHome=${this.pageConfig?.showHomeButton}, bgColor=0x${this.pageConfig?.navigationBarBackgroundColor?.toString(16)}, textStyle=${this.pageConfig?.navigationBarTextStyle}`);
  }

  /**
   * Navigate to any page with animation support
   */
  private navigateToPage(path: string): boolean {
    hilog.info(DOMAIN, TAG, `navigate: ${this.appId}:${path} with type: ${NavigationType[this.currentNavigationType]}`);

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
   * Determine if transition should be animated based on navigation type
   */
  private shouldAnimateTransition(): boolean {
    return this.currentNavigationType === NavigationType.Forward ||
           this.currentNavigationType === NavigationType.Backward;
  }

  /**
   * Perform animated transition between WebViews
   */
  private performAnimatedTransition(oldWebTag: string, newWebTag: string, path: string): void {
    hilog.info(DOMAIN, TAG, `Performing animated transition from ${oldWebTag} to ${newWebTag}`);

    this.animationInProgress = true;
    this.previousWebTag = oldWebTag;

    // Determine animation direction
    const isBackward = this.currentNavigationType === NavigationType.Backward;

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
   */
  public updateTabSelection(path: string): void {
    if (!this.isTabBarItem(path)) {
      hilog.info(DOMAIN, TAG, `Path ${path} is not a tab item, skipping tab selection update`);
      return;
    }

    const tabIndex = this.getTabIndex(path);
    if (tabIndex >= 0) {
      this.selectedTabIndex = tabIndex;
      hilog.info(DOMAIN, TAG, `Updated tab selection to index: ${tabIndex} for path: ${path}`);
    } else {
      hilog.warn(DOMAIN, TAG, `Could not find tab index for path: ${path}`);
    }
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

  /**
   * Build individual WebView component for a specific webTag with animation support
   */
  @Builder
  buildWebViewComponent(webTag: string, controller: webview.WebviewController): void {
    Web({
      src: '',
      controller: controller
    })
      .visibility(this.getWebViewVisibility(webTag))
      .translate(this.getWebViewTranslation(webTag))
      .onAppear(() => {
        hilog.info(DOMAIN, TAG, `WebView component appeared for webTag: ${webTag}, visible: ${webTag === this.currentWebTag}`);

        // Trigger onPageShow if this is the current WebView
        if (webTag === this.currentWebTag) {
          this.triggerOnPageShowIfNeeded(webTag);
        }
      })
      .onScroll((event) => {
        // Handle scroll event when scroll listener is enabled
        handleScrollChanged(webTag, event.xOffset, event.yOffset);
      })
      .width('100%')
      .height('100%')
      .backgroundColor(COLOR_TRANSPARENT)
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

    return Visibility.Hidden;
  }

  /**
   * Get WebView translation for slide animation
   */
  private getWebViewTranslation(webTag: string): TranslationOptions {
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
}
