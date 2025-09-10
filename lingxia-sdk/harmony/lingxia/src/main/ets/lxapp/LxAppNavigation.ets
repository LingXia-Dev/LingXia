import { hilog } from '@kit.PerformanceAnalysisKit';
import { LxApp } from './LxApp';
import { LxAppContainer } from './LxAppContainer';
import { onLxappOpened, onLxappClosed, getTabBar, TabBarItem, TabBarState, getCurrentLxapp, CurrentLxApp } from 'liblingxia.so';
import { ToastManager } from './APIs/LxAppToast';
import { ModalManager } from './APIs/LxAppModal';

const DOMAIN = 0xA000;
const TAG = 'LingXia.Navigation';

/**
 * Internal NavigationType enum
 */
export enum NavigationType {
  Launch = 0,
  Forward = 1,
  Backward = 2,
  Replace = 3,
  SwitchTab = 4
}

/**
 * LxApp Navigation with pure Rust stack management
 * - Completely relies on Rust stack for LxApp management
 * - Only manages UI state for current visible LxApp
 */
@Component
export struct LxAppNavigation {
  @State isReady: boolean = false
  @State homeAppId: string = ''
  @State homeAppPath: string = ''
  @State homeAppExternalPath: string = '' // For triggering navigation changes
  @State errorMessage: string = ''

  // Current modal LxApp state (only one modal app can be shown at a time)
  @State private currentModalAppId: string = ''
  @State private currentModalAppPath: string = ''
  @State private modalAppVisible: boolean = false

  // Navigation state for triggering path changes
  @State private navigationTrigger: number = 0

  @Provide navPageInfos: NavPathStack = new NavPathStack()

  aboutToAppear() {
    hilog.info(DOMAIN, TAG, 'LxAppNavigation starting with WebView preservation');
    LxApp.setNavigationInstance(this);
    this.initializeLxApp();
  }

  private async initializeLxApp(): Promise<void> {
    try {
      const instance = LxApp.getInstance();
      const homeAppId = instance.getHomeLxAppId();

      if (homeAppId) {
        this.homeAppId = homeAppId;
        this.isReady = true;
        hilog.info(DOMAIN, TAG, `LxApp Navigation ready: ${this.homeAppId}`);

        // Auto-open home lxapp now that navigation instance is ready
        // Pass empty string for path to let on_lxapp_opened resolve the initial route
        hilog.info(DOMAIN, TAG, 'Auto-opening home lxapp');
        this.openLxApp(homeAppId, "");
      } else {
        this.errorMessage = 'LxApp not initialized. Please call LxApp.initialize() in EntryAbility first.';
        hilog.error(DOMAIN, TAG, 'LxApp not initialized');
      }
    } catch (error) {
      this.errorMessage = `Initialization error: ${error}`;
      hilog.error(DOMAIN, TAG, `Initialization error: ${error}`);
    }
  }

  public openLxApp(appId: string, path: string): void {
    hilog.info(DOMAIN, TAG, `Opening lxapp: ${appId}:${path}`);

    try {
      const resolvedPath: string = onLxappOpened(appId, path);

      // UI layer distinction: Home LxApp vs Modal LxApp
      if (appId === this.homeAppId) {
        hilog.info(DOMAIN, TAG, `Showing home LxApp: ${appId} with path: ${resolvedPath}`);
        this.modalAppVisible = false;
        this.homeAppPath = resolvedPath;
        // Trigger navigation to show the home app
        this.navigationTrigger++;
        return;
      }

      // For modal apps, show modal over home
      hilog.info(DOMAIN, TAG, `Showing modal LxApp: ${appId}:${resolvedPath}`);
      this.currentModalAppId = appId;
      this.currentModalAppPath = resolvedPath;
      this.modalAppVisible = true;
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to open LxApp ${appId}: ${error}`);
    }
  }

  public closeLxApp(appId: string): void {
    hilog.info(DOMAIN, TAG, `Closing lxapp: ${appId}`);

    try {
      // Call native close handler first - this removes the app from Rust stack
      onLxappClosed(appId);

      // UI cleanup: hide the closed app
      if (appId === this.homeAppId) {
        hilog.info(DOMAIN, TAG, `Hiding home LxApp: ${appId}`);
        // Home app should never be closed, but if it happens, don't hide the container
      } else if (appId === this.currentModalAppId) {
        // Don't clear currentModalAppId to preserve the container and WebView
        hilog.info(DOMAIN, TAG, `Hiding modal LxApp: ${appId}`);
        this.modalAppVisible = false;
      }

      // Get next LxApp from Rust stack and open it
      const currentLxApp: CurrentLxApp = getCurrentLxapp();
      if (currentLxApp.appid && currentLxApp.appid.length > 0) {
        this.openLxApp(currentLxApp.appid, currentLxApp.path);
      } else {
        hilog.info(DOMAIN, TAG, `No more LxApps in Rust stack - showing empty state`);
        // No apps left in stack - hide modal but keep home visible
        this.modalAppVisible = false;
      }
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to close LxApp ${appId}: ${error}`);
    }
  }

  /**
   * Navigate to page with navigation type behavior
   * @param appId - LxApp ID
   * @param path - Page path to navigate to
   * @param navigationType - Type of navigation
   * @returns boolean - True if navigation was successful, false otherwise
   */
  public navigate(appId: string, path: string, navigationType: NavigationType): boolean {
    const typeName = NavigationType[navigationType];
    hilog.info(DOMAIN, TAG, `navigate: ${appId}:${path} (${typeName})`);

    try {
      // Resolve type: LAUNCH → SWITCH_TAB for tab pages
      const actualType = this.resolveNavigationType(navigationType, appId, path);

      // Apply navigation type specific updates
      this.applyNavigationTypeSpecificUpdates(appId, path, actualType);

      // Execute navigation with type information
      return this.navigateToPage(appId, path, actualType);
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Navigation failed: ${error}`);
      return false;
    }
  }

  private resolveNavigationType(navigationType: NavigationType, appId: string, path: string): NavigationType {
    if (navigationType === NavigationType.Launch) {
      // Only LAUNCH converts to SWITCH_TAB for tab pages
      return this.isTabBarItem(appId, path) ? NavigationType.SwitchTab : NavigationType.Launch;
    }
    // REPLACE, FORWARD, BACKWARD should never convert to SWITCH_TAB
    return navigationType;
  }

  /**
   * Apply navigation type specific updates
   */
  private applyNavigationTypeSpecificUpdates(appId: string, path: string, navigationType: NavigationType): void {
    switch (navigationType) {
      case NavigationType.SwitchTab:
        // Tab switch: update tab selection
        LxAppContainer.updateTabSelection(appId, path);
        break;

      case NavigationType.Launch:
      case NavigationType.Forward:
      case NavigationType.Backward:
      case NavigationType.Replace:
        // No special updates needed
        break;
    }
  }

  /**
   * Required by NavigationInstance interface
   * Tabbar visibility is handled by navigation type in LxAppContainer
   */
  public isTabBarHidden(appId: string): boolean {
    return false;
  }

  /**
   * Check if a path is a tabbar item - public for reuse
   */
  public isTabBarItem(appId: string, path: string): boolean {
    try {
      const tabBarState: TabBarState | null = getTabBar(appId);
      if (!tabBarState || !tabBarState.items || tabBarState.items.length === 0) {
        return false;
      }

      const tabIndex: number = tabBarState.items.findIndex((item: TabBarItem): boolean =>
        item.pagePath === path || item.pagePath.includes(path)
      );

      return tabIndex >= 0;
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Error checking if path is tabbar item: ${error}`);
      return false;
    }
  }

  /**
   * Navigate to a specific page within an LxApp
   * @param appId - LxApp ID
   * @param path - Page path to navigate to
   * @param navigationType - Navigation type for animation control
   * @returns boolean - True if navigation was successful, false otherwise
   */
  private navigateToPage(appId: string, path: string, navigationType: NavigationType = NavigationType.Launch): boolean {
    hilog.info(DOMAIN, TAG, `Navigating to page: ${appId}:${path}`);

    // For home app, update the path and trigger navigation
    if (appId === this.homeAppId) {
      hilog.info(DOMAIN, TAG, `Navigating page in home app ${appId} to ${path} with type: ${NavigationType[navigationType]}`);
      this.homeAppPath = path;
      // Pass navigation type to container for animation control
      LxAppContainer.setNavigationType(appId, navigationType);
      // Increment trigger to force UI update
      this.navigationTrigger++;
      hilog.info(DOMAIN, TAG, `Updated homeAppPath to: ${path}, trigger: ${this.navigationTrigger}`);
      return true;
    }

    // For modal apps, check if it's the current modal app
    if (appId === this.currentModalAppId && this.modalAppVisible) {
      hilog.info(DOMAIN, TAG, `Navigating page in modal app ${appId} to ${path} with type: ${NavigationType[navigationType]}`);
      this.currentModalAppPath = path;
      // Pass navigation type to container for animation control
      LxAppContainer.setNavigationType(appId, navigationType);
      // Trigger UI update by updating the path
      this.currentModalAppPath = path;
      hilog.info(DOMAIN, TAG, `Updated modal app ${appId} path to: ${path}`);
      return true;
    }

    hilog.warn(DOMAIN, TAG, `LxApp ${appId} not found or not currently visible for page navigation`);
    return false;
  }



  /**
   * Build modal LxApp container with common styling
   */
  @Builder
  buildModalLxApp(): void {
    LxAppContainer({
      appId: this.currentModalAppId,
      initialPath: this.currentModalAppPath,
      externalPath: this.currentModalAppPath,
      navigationMode: true,
      closeCallback: 'navigation',
      navigationTrigger: 0
    })
    .width('100%')
    .height('100%')
    .zIndex(1000)
    .backgroundColor(Color.White)
    .expandSafeArea([SafeAreaType.SYSTEM], [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM])
    .visibility(this.modalAppVisible ? Visibility.Visible : Visibility.Hidden)
    .onAppear(() => {
      hilog.info(DOMAIN, TAG, `Modal LxApp appeared: ${this.currentModalAppId}`);
    })
  }

  build() {
    Stack() {
      // Always show home LxApp container to preserve WebViews, but hide when modal is visible
      if (this.isReady && this.homeAppId && this.homeAppPath) {
        LxAppContainer({
          appId: this.homeAppId,
          initialPath: this.homeAppPath,
          externalPath: this.homeAppPath,
          navigationMode: false,
          closeCallback: undefined,
          navigationTrigger: this.navigationTrigger
        })
        .zIndex(0)
        .visibility(this.modalAppVisible ? Visibility.Hidden : Visibility.Visible)
      } else if (this.errorMessage) {
        this.buildErrorPage()
      } else {
        this.buildLoadingPage()
      }

      // Always show modal LxApp container to preserve WebViews, but hide when not visible
      if (this.currentModalAppId) {
        this.buildModalLxApp();
      }

      ToastManager()
        .zIndex(10000)

      ModalManager()
        .zIndex(10001)
    }
    .width('100%')
    .height('100%')
  }

  @Builder
  buildFixedCapsuleButton(appId: string): void {
    if (appId !== LxApp.getInstance().getHomeLxAppId()) {
      Stack() {
        Row() {
          Button() {
            this.buildThreeDots()
          }
          .width(44)
          .height(36)
          .backgroundColor(Color.Transparent)
          .onClick(() => {
            hilog.info(DOMAIN, TAG, 'Capsule more button pressed');
          })

          Column()
            .width(0.5)
            .height(18)
            .backgroundColor('#DDDDDD')

          Button() {
            this.buildCloseIcon()
          }
          .width(44)
          .height(36)
          .backgroundColor(Color.Transparent)
          .onClick(() => {
            hilog.info(DOMAIN, TAG, `Capsule close button pressed for appId: ${appId}`);
            this.closeLxApp(appId);
          })
        }
        .backgroundColor('rgba(255,255,255,0.9)')
        .borderRadius(20)
        .border({ width: 0.5, color: '#DDDDDD' })
        .margin({ top: 52, right: 12 })
      }
      .width('100%')
      .height('100%')
      .backgroundColor(Color.Transparent)
      .zIndex(9999)
      .alignContent(Alignment.TopEnd)
    }
  }

  /**
   * Build three dots icon
   */
  @Builder
  buildThreeDots(): void {
    Row() {
      Circle().width(3).height(3).fill('#000000')
      Circle().width(5).height(5).fill('#000000').margin({ left: 4 })
      Circle().width(3).height(3).fill('#000000').margin({ left: 4 })
    }
    .justifyContent(FlexAlign.Center)
    .alignItems(VerticalAlign.Center)
  }

  /**
   * Build close icon
   */
  @Builder
  buildCloseIcon(): void {
    Stack() {
      Circle()
        .width(16)
        .height(16)
        .fillOpacity(0)
        .stroke('#000000')
        .strokeWidth(2.5)
      Circle()
        .width(6.5)
        .height(6.5)
        .fill('#000000')
    }
  }

  @Builder
  buildErrorPage() {
    Column() {
      Text('⚠️')
        .fontSize(32)
    }
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
    .width('100%')
    .height('100%')
  }

  /**
   * Build clean loading page
   */
  @Builder
  buildLoadingPage() {
    Column() {
      LoadingProgress()
        .width(24)
        .height(24)
        .color('#1677FF')
    }
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
    .width('100%')
    .height('100%')
  }
}
