import { hilog } from '@kit.PerformanceAnalysisKit';
import { LxApp } from './LxApp';
import { LxAppContainer } from './LxAppContainer';
import { onLxappOpened, onLxappClosed, getTabBar, TabBarItem, TabBarState, getLxAppInfo, LxAppInfo } from 'liblingxia.so';
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
 * LxApp stack item for manual layer management
 */
interface LxAppStackItem {
  appId: string;
  path: string;
  offsetY: number; // For manual animation
  zIndex: number;  // Layer index
  navigationTrigger: number; // For triggering navigation updates
}

/**
 * LxApp Navigation with WebView preservation
 * - Hides LxApps instead of destroying them to preserve WebViews
 * - Uses independent hiddenApps Set to avoid miniAppStack mutations
 */
@Component
export struct LxAppNavigation {
  @State isReady: boolean = false
  @State homeAppId: string = ''
  @State homeAppPath: string = ''
  @State homeAppExternalPath: string = '' // For triggering navigation changes
  @State errorMessage: string = ''
  @State isHomeAppVisible: boolean = false // Control Home LxApp visibility

  @State private miniAppStack: LxAppStackItem[] = []
  @State private hiddenApps: Set<string> = new Set<string>()

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
      // Get initial route directly from native
      let homeAppPath: string | null = null;
      if (homeAppId) {
        const appInfo: LxAppInfo | null = getLxAppInfo(homeAppId);
        homeAppPath = appInfo?.initialRoute || null;
      }

      if (homeAppId && homeAppPath) {
        this.homeAppId = homeAppId;
        this.homeAppPath = homeAppPath;
        this.isReady = true;
        hilog.info(DOMAIN, TAG, `LxApp Navigation ready: ${this.homeAppId}:${this.homeAppPath}`);

        // Auto-open home lxapp now that navigation instance is ready
        hilog.info(DOMAIN, TAG, 'Auto-opening home lxapp');
        this.openLxApp(homeAppId, homeAppPath);
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
      const result: number = onLxappOpened(appId, path);
      hilog.info(DOMAIN, TAG, `onLxappOpened result: ${result}`);

      // Handle Home LxApp specially
      if (appId === this.homeAppId) {
        hilog.info(DOMAIN, TAG, `Showing home LxApp: ${appId}`);
        this.isHomeAppVisible = true;
        return;
      }

      const existingIndex = this.miniAppStack.findIndex(item => item.appId === appId);

      if (existingIndex >= 0) {
        hilog.info(DOMAIN, TAG, `LxApp ${appId} already exists, showing and bringing to front`);
        this.showLxApp(appId, existingIndex);
      } else {
        this.createNewLxApp(appId, path);
      }
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to open LxApp ${appId}: ${error}`);
    }
  }

  public closeLxApp(appId: string): void {
    hilog.info(DOMAIN, TAG, `Hiding lxapp (preserving WebViews): ${appId}`);

    try {
      const stackIndex = this.miniAppStack.findIndex(item => item.appId === appId);
      if (stackIndex >= 0) {
        this.hideLxAppWithAnimation(appId, stackIndex);
      } else {
        hilog.warn(DOMAIN, TAG, `LxApp not found in stack: ${appId}`);
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

    // For modal apps in stack, find the container and navigate page
    const stackItem = this.miniAppStack.find(item => item.appId === appId);
    if (stackItem) {
      hilog.info(DOMAIN, TAG, `Navigating page in modal app ${appId} to ${path} with type: ${NavigationType[navigationType]}`);
      stackItem.path = path;
      // Pass navigation type to container for animation control
      LxAppContainer.setNavigationType(appId, navigationType);
      stackItem.navigationTrigger++;
      // Trigger UI update for the modal container
      this.miniAppStack = [...this.miniAppStack];
      hilog.info(DOMAIN, TAG, `Updated modal app ${appId} path to: ${path}, trigger: ${stackItem.navigationTrigger}`);
      return true;
    }

    hilog.warn(DOMAIN, TAG, `LxApp ${appId} not found for page navigation`);
    return false;
  }

  private showLxApp(appId: string, stackIndex: number): void {
    const newHiddenApps = new Set<string>();
    this.hiddenApps.forEach(id => {
      if (id !== appId) {
        newHiddenApps.add(id);
      }
    });
    this.hiddenApps = newHiddenApps;

    this.miniAppStack[stackIndex].offsetY = 0;
    this.miniAppStack[stackIndex].zIndex = 1000 + this.miniAppStack.length;
  }

  private createNewLxApp(appId: string, path: string): void {
    const newStackItem: LxAppStackItem = {
      appId: appId,
      path: path,
      offsetY: 0,
      zIndex: 1000 + this.miniAppStack.length,
      navigationTrigger: 0
    };

    this.miniAppStack.push(newStackItem);
    hilog.info(DOMAIN, TAG, `LxApp created: ${appId}:${path}, stack size: ${this.miniAppStack.length}`);
  }

  private hideLxAppWithAnimation(appId: string, stackIndex: number): void {
    animateTo({
      duration: 250,
      curve: Curve.EaseIn,
      playMode: PlayMode.Normal
    }, () => {
      if (stackIndex < this.miniAppStack.length) {
        this.miniAppStack[stackIndex].offsetY = 800;
      }
    });

    setTimeout(() => {
      if (stackIndex < this.miniAppStack.length) {
        this.hiddenApps = new Set(this.hiddenApps).add(appId);
        hilog.info(DOMAIN, TAG, `LxApp ${appId} hidden, WebViews preserved for reuse`);
      }
      onLxappClosed(appId);
    }, 300);
  }

  private isLxAppVisible(appId: string): boolean {
    return !this.hiddenApps.has(appId);
  }

  /**
   * Build modal LxApp container with common styling
   */
  @Builder
  buildModalLxApp(stackItem: LxAppStackItem): void {
    LxAppContainer({
      appId: stackItem.appId,
      initialPath: stackItem.path,
      externalPath: stackItem.path,
      navigationMode: true,
      closeCallback: 'navigation',
      navigationTrigger: stackItem.navigationTrigger
    })
    .width('100%')
    .height('100%')
    .zIndex(this.isLxAppVisible(stackItem.appId) ? stackItem.zIndex : -1)
    .backgroundColor(Color.White)
    .expandSafeArea([SafeAreaType.SYSTEM], [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM])
    .translate({ y: stackItem.offsetY })
    .visibility(this.isLxAppVisible(stackItem.appId) ? Visibility.Visible : Visibility.Hidden)
    .enabled(this.isLxAppVisible(stackItem.appId))
    .onAppear(() => {
      hilog.info(DOMAIN, TAG, `Modal LxApp appeared: ${stackItem.appId}`);
    })
  }

  build() {
    Stack() {
      if (this.isReady && this.homeAppId && this.homeAppPath && this.isHomeAppVisible) {
        LxAppContainer({
          appId: this.homeAppId,
          initialPath: this.homeAppPath,
          externalPath: this.homeAppPath,
          navigationMode: false,
          closeCallback: undefined,
          navigationTrigger: this.navigationTrigger
        })
        .zIndex(0)
      } else if (this.errorMessage) {
        this.buildErrorPage()
      } else {
        this.buildLoadingPage()
      }

      // Only non-home apps go in the stack to avoid duplication
      ForEach(this.miniAppStack, (stackItem: LxAppStackItem, index: number) => {
        if (stackItem.appId !== this.homeAppId) {
          this.buildModalLxApp(stackItem);
        }
      })

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
