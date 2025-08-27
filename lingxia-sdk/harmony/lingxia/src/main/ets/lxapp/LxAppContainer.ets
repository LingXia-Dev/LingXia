import { hilog } from '@kit.PerformanceAnalysisKit';
import { webview } from '@kit.ArkWeb';
import { window } from '@kit.ArkUI';
import { common } from '@kit.AbilityKit';
import { LxApp } from './LxApp';
import { findWebview, handleScrollChanged, toWebTag, extractWebTag, WebTagParts, addWebViewUiCallback, removeWebViewUiCallback, WebViewInfo } from './WebView';
import { NavigationBar, type NavigationBarConfig } from './NavigationBar';
import { TabBar } from './TabBar';
import { TabBarPosition, type TabBarItem } from 'liblingxia.so';
import liblingxia, { TabBarState, getNavigationBarConfig, onPageShow } from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.Container';

@Component
export struct LxAppContainer {
  @Prop appId: string
  @Prop initialPath: string
  @Prop navigationMode: boolean = false
  @Prop closeCallback: string = ''
  @State path: string = ''

  // Static registry for TabBar updates (simple and direct)
  private static activeContainers: Map<string, LxAppContainer> = new Map()

  @State pageConfig: NavigationBarConfig | null = null
  @State tabBarState: TabBarState | null = null
  @State selectedTabIndex: number = 0
  @State tabBarRefreshKey: number = 0  // Force TabBar refresh when data changes
  @State webViewComponents: Map<string, webview.WebviewController> = new Map() // Store Web components for each WebView
  @State currentWebTag: string = '' // Current active WebView tag
  @State webViewKeys: string[] = [] // Array of webView keys for ForEach
  private pageShowTriggered: Set<string> = new Set() // Track which pages have triggered onPageShow


  private webViewUiCallback = (action: string, info: WebViewInfo) => {
    this.handleWebViewUiEvent(action, info);
  }

  /**
   * Trigger onPageShow for a WebView if it hasn't been triggered yet
   */
  private triggerOnPageShowIfNeeded(webTag: string) {
    if (!this.pageShowTriggered.has(webTag)) {
      const extracted = extractWebTag(webTag);
      if (extracted) {
        const appId = extracted.appId;
        const path = extracted.path;
        const actualPath = path.replace(/-/g, '/');

        // Check if this is the first page (LxApp runtime might not be ready)
        const isFirstPage = this.pageShowTriggered.size === 0;
        this.pageShowTriggered.add(webTag);

        if (isFirstPage) {
          // Delay first page to ensure LxApp runtime is ready
          setTimeout(() => {
            hilog.info(DOMAIN, TAG, `Triggering onPageShow for first page: ${appId}:${actualPath}`);
            onPageShow(appId, actualPath);
          }, 100);
        } else {
          // No delay for subsequent pages
          hilog.info(DOMAIN, TAG, `Triggering onPageShow for: ${appId}:${actualPath}`);
          onPageShow(appId, actualPath);
        }
      }
    } else {
      hilog.info(DOMAIN, TAG, `onPageShow already triggered for: ${webTag}, skipping`);
    }
  }

  // Watch for external path changes from LxApp.switchPage API
  @Watch('onExternalPathChange') @Prop externalPath: string = ''

  // Manual animation control for new lxapps
  @State animationOffsetY: number = 0 // 0 = final position, positive = off-screen bottom

  aboutToAppear() {

    this.path = this.initialPath;
    hilog.info(DOMAIN, TAG, `Container creating for ${this.appId}:${this.path}, isHome=${this.isHomeLxApp()}`);

    // Register this container for TabBar updates
    LxAppContainer.activeContainers.set(this.appId, this);

    // Add WebView UI callback to handle component creation (supports multiple containers)
    addWebViewUiCallback(this.webViewUiCallback);

    this.pageConfig = getNavigationBarConfig(this.appId, this.path);

    // Load TabBar data using new data-driven API
    this.loadTabBarState();
    this.setInitialSelectedTab();

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
   * Handle external path changes from LxApp.switchPage API
   */
  onExternalPathChange() {
    if (this.externalPath && this.externalPath !== this.path) {
      hilog.info(DOMAIN, TAG, `External path change detected: ${this.externalPath}, switching from ${this.path}`);
      this.switchToPageInternal(this.externalPath);
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

        Column() {
          if (this.shouldShowCapsuleButton()) {
            this.buildCapsuleButton()
          }
          if (this.pageConfig && this.pageConfig.navigationStyle === 0 && !this.isInitialRoute()) {
            NavigationBar({
              config: this.pageConfig,
              appId: this.appId,
              currentPath: this.path,
              onBackPressed: (appId: string): boolean => {
                this.handleNavigationBack();
                return true;
              }
            })
          }
          this.buildWebViewArea()
        }
        .layoutWeight(1)
        .height('100%')

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
        this.buildStandaloneCapsuleButton()
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

        this.buildStandaloneCapsuleButton()
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

  private shouldExpandSafeAreaForContainer(): boolean {
    const shouldExpand = true;
    hilog.info(DOMAIN, TAG, `Always expand safe area for full screen coverage: ${this.appId}`);
    return shouldExpand;
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
    if (this.tabBarState && this.tabBarState.items.length > 0) {
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
   * Build NavigationBar component (shared by both branches)
   */
  @Builder
  buildNavigationBar(): void {
    if (this.pageConfig && this.pageConfig.navigationStyle === 0 && !this.isInitialRoute()) {
      NavigationBar({
        config: this.pageConfig,
        appId: this.appId,
        currentPath: this.path,
        showCapsuleButton: false,
        onBackPressed: (appId: string): boolean => {
          this.handleNavigationBack();
          return true;
        },
        onCapsuleMore: () => this.handleCapsuleMore(),
        onCapsuleClose: () => this.handleCapsuleClose()
      })
    }
  }

  @Builder
  buildStandaloneCapsuleButton(): void {
    if (this.shouldShowCapsuleButton()) {
      Stack() {
        Row() {
          Button() {
            this.buildThreeDots()
          }
          .width(44)
          .height(32)
          .backgroundColor(Color.Transparent)
          .onClick(() => {
            hilog.info(DOMAIN, TAG, 'Capsule more button pressed');
            this.handleCapsuleMore();
          })

          Column()
            .width(0.5)
            .height(16)
            .backgroundColor('#DDDDDD')

          Button() {
            this.buildCloseIcon()
          }
          .width(44)
          .height(32)
          .backgroundColor(Color.Transparent)
          .onClick(() => {
            hilog.info(DOMAIN, TAG, `Capsule close button pressed for appId: ${this.appId}`);
            this.handleCapsuleClose();
          })
        }
        .backgroundColor('rgba(255,255,255,0.9)')
        .borderRadius(16)
        .border({ width: 0.5, color: '#DDDDDD' })
      }
      .position({ x: '100%', y: 44 })
      .translate({ x: -102, y: 6 })
      .width(102) // 44+0.5+44+padding
      .height(32)
      .zIndex(9999)
    }
  }

  /**
   * Check if should show capsule button
   * homelxapp doesn't show capsule button, other lxapps do
   */
  private shouldShowCapsuleButton(): boolean {
    const isHome = this.isHomeLxApp();
    const shouldShow = !isHome;
    hilog.info(DOMAIN, TAG, `shouldShowCapsuleButton: appId=${this.appId}, isHome=${isHome}, shouldShow=${shouldShow}`);
    return shouldShow;
  }

  /**
   * Build capsule button
   */
  @Builder
  buildCapsuleButton(): void {
    Row() {
      Blank()

      Row() {
        Button() {
          this.buildThreeDots()
        }
        .width(44)
        .height(32)
        .backgroundColor(Color.Transparent)
        .onClick(() => this.handleCapsuleMore())

        Column()
          .width(0.5)
          .height(16)
          .backgroundColor('#DDDDDD')

        Button() {
          this.buildCloseIcon()
        }
        .width(44)
        .height(32)
        .backgroundColor(Color.Transparent)
        .enabled(true)
        .onClick(() => {
          this.handleCapsuleClose();
        })
        .onTouch((event) => {
          hilog.info(DOMAIN, TAG, `Close button touched: ${event.type}`);
        })
      }
      .backgroundColor('rgba(255,255,255,0.9)')
      .borderRadius(16)
      .border({ width: 0.5, color: '#DDDDDD' })
      .margin({ top: 8, right: 12 })
    }
    .width('100%')
    .height(56)
    .backgroundColor(Color.Transparent)
    .zIndex(9999)
  }

  /**
   * Build three dots icon
   */
  @Builder
  buildThreeDots(): void {
    Row() {
      // Three custom drawn dots - center dot larger, side dots smaller (matching Android)
      Circle()
        .width(3)
        .height(3)
        .fill('#000000')

      Circle()
        .width(5)
        .height(5)
        .fill('#000000')
        .margin({ left: 4 })

      Circle()
        .width(3)
        .height(3)
        .fill('#000000')
        .margin({ left: 4 })
    }
    .justifyContent(FlexAlign.Center)
    .alignItems(VerticalAlign.Center)
  }

  /**
   * Build close icon (outer circle LINE with inner dot, transparent middle)
   */
  @Builder
  buildCloseIcon(): void {
    Stack() {
      // Outer circle - use stroke instead of border to ensure circular shape
      Circle()
        .width(16)
        .height(16)
        .fillOpacity(0) // Transparent fill
        .stroke('#000000')
        .strokeWidth(2.5) // Thicker stroke matching Android

      // Inner dot (thick dot in center) - radius should be outer radius / 2.5
      Circle()
        .width(6.5)
        .height(6.5)
        .fill('#000000')
    }
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
    .backgroundColor(this.navigationMode ? Color.White : Color.Transparent)
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
    if (this.tabBarState && index < this.tabBarState.items.length) {
      this.switchToTabByIndex(index);
    }
  }

  /**
   * Switch to tab by index
   */
  private switchToTabByIndex(index: number): void {
    if (!this.tabBarState || index < 0 || index >= this.tabBarState.items.length) {
      hilog.warn(DOMAIN, TAG, `Invalid tab index: ${index}`);
      return;
    }

    const newPath: string = this.tabBarState.items[index].pagePath;

    // Skip if already on this tab
    if (this.selectedTabIndex === index) {
      hilog.info(DOMAIN, TAG, `Already on tab ${index}, skipping switch`);
      return;
    }

    hilog.info(DOMAIN, TAG, `Switching from tab ${this.selectedTabIndex} to tab ${index}, path: ${newPath}`);

    // Update selected index to trigger UI visibility change
    this.selectedTabIndex = index;

    // Update current path to the new tab path
    this.path = newPath;

    // Find WebView for the new tab (only when switching)
    hilog.info(DOMAIN, TAG, `Finding WebView for tab switch: ${this.appId}:${newPath}`);
    const newWebTag = toWebTag(this.appId, newPath);
    const newWebViewController = findWebview(this.appId, newPath);
    if (newWebViewController) {
      hilog.info(DOMAIN, TAG, `WebView for tab switch ${newPath}: found`);
      // Store the Web component for this WebView
      this.webViewComponents.set(newWebTag, newWebViewController);
      this.webViewKeys = Array.from(this.webViewComponents.keys()); // Update ForEach keys
      this.currentWebTag = newWebTag;
      // Clear the pageShow trigger state for this WebView so it can trigger again
      this.pageShowTriggered.delete(newWebTag);
      hilog.info(DOMAIN, TAG, `Updated currentWebTag to: ${newWebTag}`);

      // Trigger onPageShow immediately for tab switches
      this.triggerOnPageShowIfNeeded(newWebTag);
    } else {
      hilog.error(DOMAIN, TAG, `WebView for tab switch ${newPath}: not found`);
    }

    // Update page config
    this.pageConfig = getNavigationBarConfig(this.appId, newPath);

    // Update manager state if this is home lxapp
    if (this.isHomeLxApp()) {
      const manager = LxApp.managerInstance;
      if (manager) {
        manager.updateHomeLxAppCurrentPath(newPath);
      }
    }

    hilog.info(DOMAIN, TAG, `Tab switched to ${index}, onPageShow will be triggered automatically for path: ${newPath}`);
  }

  /**
   * Switch to page internally (handles both tab and non-tab pages)
   * Called by external API or tab clicks
   */
  private switchToPageInternal(path: string): void {
    hilog.info(DOMAIN, TAG, `switchToPageInternal called: ${this.appId}:${path}`);

    // Use TabBar data to find tab index
    if (this.tabBarState) {
      const tabIndex: number = this.tabBarState.items.findIndex((item: TabBarItem): boolean =>
        item.pagePath === path || item.pagePath.includes(path)
      );

      if (tabIndex >= 0) {
        hilog.info(DOMAIN, TAG, `Found tab ${tabIndex} for path ${path}, switching tab`);
        this.switchToTabByIndex(tabIndex);
        return;
      }
    }

    // For non-tabbar pages, directly switch
    hilog.info(DOMAIN, TAG, `Direct page switch for path ${path}`);
    this.path = path;
    const webTag = toWebTag(this.appId, path);
    const controller = findWebview(this.appId, path);
    if (controller) {
      this.storeWebViewComponent(webTag, controller);
    }
    hilog.info(DOMAIN, TAG, `WebView for page switch ${path}: ${controller ? 'found' : 'not found'}`);

    this.pageConfig = getNavigationBarConfig(this.appId, path);
  }

  private handleNavigationBack() {
    hilog.info(DOMAIN, TAG, 'Navigation back pressed');
    // TODO: Implement navigation back logic
  }

  private handleCapsuleMore(): void {
    hilog.info(DOMAIN, TAG, 'Capsule more button pressed');
    // TODO: Implement more menu
  }

  private handleCapsuleClose(): void {
    // Close second lxapp if this is not home lxapp
    if (!this.isHomeLxApp()) {
        LxApp.closeLxApp(this.appId);
    }
  }

  /**
   * Build individual WebView component for a specific webTag
   */
  @Builder
  buildWebViewComponent(webTag: string, controller: webview.WebviewController): void {
    Web({
      src: '',
      controller: controller
    })
      .visibility(webTag === this.currentWebTag ? Visibility.Visible : Visibility.Hidden)
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
      .backgroundColor(Color.Transparent)
  }

  /**
   * Check if current page is the initial route
   */
  private isInitialRoute(): boolean {
    const cachedInitialRoute = LxApp.initialRouteCache.get(this.appId);
    return cachedInitialRoute === this.path;
  }
}
