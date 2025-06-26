import { hilog } from '@kit.PerformanceAnalysisKit';
import { webview } from '@kit.ArkWeb';
import { window } from '@kit.ArkUI';
import { resourceManager } from '@kit.LocalizationKit';
import { taskpool } from '@kit.ArkTS';
import { MiniAppContainer } from './MiniAppContainer';
import { initNativeBridge, getNativeCallbackFunction } from './NativeBridge';
import { WebViewManager, createWebViewController, toWebTag } from './WebView';

// Import native functions from Rust
import { miniappInit, onMiniappOpened, onMiniappClosed, onPageShow } from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.MiniApp';

// Task function for handling MiniApp close operation asynchronously
@Concurrent
function closeMiniAppTask(appId: string): number {
  return onMiniappClosed(appId);
}

interface NavigationInstance {
  openMiniApp(appId: string, path: string): void;
  closeMiniApp(appId: string): void;
}

export class MiniApp {
  private static instance: MiniApp | null = null;
  public static managerInstance: MiniAppManager | null = null;
  public static context: Context | null = null;
  public static windowStage: window.WindowStage | null = null;

  // Modal miniapp state (for non-home miniapps)
  public static modalMiniAppId: string | null = null;
  public static modalMiniAppPath: string | null = null;
  public static modalMiniAppVisible: boolean = false;

  // Control home miniapp visibility when modal is open
  public static hideHomeMiniApp: boolean = false;

  // Store navigation instance for Navigation-based architecture
  public static navigationInstance: NavigationInstance | null = null;

  private homeMiniAppId: string | null = null;
  private homeMiniAppInitialRoute: string | null = null;

  private constructor() {
    hilog.info(DOMAIN, TAG, 'MiniApp instance created');
  }

  public static getInstance(): MiniApp {
    if (!MiniApp.instance) {
      MiniApp.instance = new MiniApp();
    }
    return MiniApp.instance;
  }

  public static initialize(context?: Context, windowStage?: window.WindowStage): void {
    const instance = MiniApp.getInstance();
    if (instance.homeMiniAppId !== null && instance.homeMiniAppInitialRoute !== null) {
      hilog.info(DOMAIN, TAG, 'MiniApp already successfully initialized');
      return;
    }

    // Check if native layer has already been initialized by checking homeMiniAppId
    if (instance.homeMiniAppId !== null) {
      hilog.warn(DOMAIN, TAG, 'Native layer already initialized, skipping native_miniapp_init');
      return;
    }

    // Store context and windowStage for window operations
    if (context) {
      MiniApp.context = context;
    }
    if (windowStage) {
      MiniApp.windowStage = windowStage;
      // Set window to full screen layout
      const mainWindow = windowStage.getMainWindowSync();
      mainWindow.setWindowLayoutFullScreen(true);
    }

    // Initialize native bridge
    initNativeBridge();

    // Get application-level file system paths (not module-specific)
    const appContext = context!.getApplicationContext();
    const dataDir = appContext.filesDir;
    const cacheDir = appContext.cacheDir;
    const resourceManager = context!.resourceManager;

    hilog.info(DOMAIN, TAG, `Using application context - dataDir: ${dataDir}, cacheDir: ${cacheDir}`);

    const callbackFunction = getNativeCallbackFunction();

    hilog.info(DOMAIN, TAG, `Calling miniappInit with callback and file system paths (FIRST AND ONLY TIME)`);
    const result: string | null = miniappInit(callbackFunction, dataDir, cacheDir, resourceManager);

    if (result) {
      hilog.info(DOMAIN, TAG, `MiniApp initialized successfully: ${result}`);

      const parts: string[] = result.split(':', 2);
      if (parts.length === 2) {
        instance.homeMiniAppId = parts[0];
        instance.homeMiniAppInitialRoute = parts[1];

        // Initialize Web component core: initialize the Browser process and create BrowserContext.
        webview.WebviewController.initializeWebEngine();

        const webtag = toWebTag(instance.homeMiniAppId, instance.homeMiniAppInitialRoute);
        const success = createWebViewController(webtag);
        hilog.info(DOMAIN, TAG, `Initial WebView creation result: ${success}`);
      } else {
        hilog.error(DOMAIN, TAG, `Failed to parse home MiniApp details from native (expected 2 parts): ${result}`);
      }
    } else {
      hilog.error(DOMAIN, TAG, 'Failed to get home MiniApp details from native init.');
    }
  }

  public static openHomeMiniApp(): void {
    hilog.info(DOMAIN, TAG, 'openHomeMiniApp called');

    const instance = MiniApp.getInstance();
    const homeAppId = instance.getHomeMiniAppId();
    const homeAppPath = instance.getHomeMiniAppInitialRoute();

    if (homeAppId && homeAppPath) {
      // Home MiniApp has special handling - it's displayed in the base layer, not in stack
      hilog.info(DOMAIN, TAG, `Opening home MiniApp directly: ${homeAppId}:${homeAppPath}`);

      try {
        // Just call the native function for home MiniApp
        const result: number = onMiniappOpened(homeAppId, homeAppPath);
        hilog.info(DOMAIN, TAG, `onMiniappOpened result for home: ${result}`);
      } catch (error) {
        hilog.error(DOMAIN, TAG, `Failed to open home MiniApp: ${error}`);
      }
    } else {
      hilog.error(DOMAIN, TAG, 'Home MiniApp info not available for openHomeMiniApp');
    }
  }

  public static closeMiniApp(appId: string): void {
    // Call native onMiniappClosed asynchronously (fire and forget)
    taskpool.execute(new taskpool.Task(closeMiniAppTask, appId));

    if (MiniApp.navigationInstance) {
      hilog.info(DOMAIN, TAG, 'Using MiniAppNavigation for closeMiniApp');
      MiniApp.navigationInstance.closeMiniApp(appId);
      return;
    }

    if (MiniApp.modalMiniAppId === appId) {
      MiniApp.modalMiniAppId = null;
      MiniApp.modalMiniAppPath = null;
      MiniApp.modalMiniAppVisible = false;
      MiniApp.hideHomeMiniApp = false;

      if (MiniApp.managerInstance) {
        MiniApp.managerInstance.closeModalMiniApp();
      } else {
        hilog.warn(DOMAIN, TAG, ' No manager instance available!');
      }
    } else {
      hilog.warn(DOMAIN, TAG, `Cannot close miniapp ${appId} - not currently displayed modal miniapp (current: ${MiniApp.modalMiniAppId})`);
    }
  }

  public static openMiniApp(appId: string, path: string): void {
    hilog.info(DOMAIN, TAG, `openMiniApp called: ${appId}:${path}`);
    if (MiniApp.navigationInstance) {
      hilog.info(DOMAIN, TAG, 'Using MiniAppNavigation for openMiniApp');
      MiniApp.navigationInstance.openMiniApp(appId, path);
      return;
    }

    if (!MiniApp.managerInstance) {
      hilog.info(DOMAIN, TAG, 'No MiniApp instance ready, waiting...');
      setTimeout(() => {
        MiniApp.openMiniApp(appId, path);
      }, 50);
      return;
    }

    const instance = MiniApp.getInstance();
    const isHomeMiniApp = (appId === instance.getHomeMiniAppId() && path === instance.getHomeMiniAppInitialRoute());

    if (isHomeMiniApp && MiniApp.managerInstance.isReady) {
      hilog.info(DOMAIN, TAG, 'Home MiniApp already loaded');
      return;
    }

    hilog.info(DOMAIN, TAG, 'Using MiniAppManager for openMiniApp (legacy)');
    MiniApp.managerInstance.doOpenMiniApp(appId, path, isHomeMiniApp);
  }

  /**
   * Switch to a specific page within a MiniApp
   * @param appId - MiniApp ID
   * @param path - Page path to switch to
   */
  public static switchPage(appId: string, path: string): boolean {
    hilog.info(DOMAIN, TAG, `switchPage API called: ${appId}:${path}`);

    // The manager finds the right container and handles the switch
    if (MiniApp.managerInstance) {
      return MiniApp.managerInstance.switchToPage(appId, path);
    }

    hilog.warn(DOMAIN, TAG, 'No MiniApp manager ready for switchPage');
    return false;
  }

  /**
   * Get Home MiniApp ID
   */
  public getHomeMiniAppId(): string | null {
    return this.homeMiniAppId;
  }

  /**
   * Get Home MiniApp initial route
   */
  public getHomeMiniAppInitialRoute(): string | null {
    return this.homeMiniAppInitialRoute;
  }

  /**
   * Set manager instance (called by MiniAppManager)
   */
  public static setManagerInstance(manager: MiniAppManager): void {
    MiniApp.managerInstance = manager;
  }

  /**
   * Set navigation instance (called by MiniAppNavigation)
   */
  public static setNavigationInstance(navigation: NavigationInstance): void {
    MiniApp.navigationInstance = navigation;
  }

  /**
   * Setup system status bar transparency (call after window is created)
   * Should be called in onWindowStageCreate
   */
  public static setupSystemStatusBar(windowStage: window.WindowStage): void {
    //const instance = MiniApp.getInstance();
    //instance.setSystemStatusBarTransparent(windowStage);
  }

  /**
   * Switch to specific page in current MiniApp
   * @param appId MiniApp ID
   * @param path Page path to switch to
   * @returns Promise<boolean> Success status
   */
  public static async switchToPage(appId: string, path: string): Promise<boolean> {
    const instance = MiniApp.getInstance();
    return await instance.switchToPageInternal(appId, path);
  }

  /**
   * Switch to specific page in current MiniApp (internal implementation)
   * @param appId MiniApp ID
   * @param path Page path to switch to
   * @returns Promise<boolean> Success status
   */
  public async switchToPageInternal(appId: string, path: string): Promise<boolean> {
    hilog.info(DOMAIN, TAG, `switchToPageInternal called: appId=${appId}, path=${path}`);

    try {
      // Find the container and switch to the page
      if (MiniApp.managerInstance) {
        const success: boolean = MiniApp.managerInstance.switchToPage(appId, path);
        if (success) {
          hilog.info(DOMAIN, TAG, `Successfully switched to page: ${path}`);
          return true;
        } else {
          hilog.warn(DOMAIN, TAG, `Failed to switch to page: ${path} - page not found in TabBar`);
          return false;
        }
      } else {
        hilog.error(DOMAIN, TAG, `No MiniApp manager instance available for page switch`);
        return false;
      }
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Error switching to page ${path}: ${error}`);
      return false;
    }
  }
}

/**
 * MiniApp Manager Component - UI management for MiniApps
 * Internal component
 */
@Component
struct MiniAppManager {
  @State isReady: boolean = false
  @State homeAppId: string = ''
  @State homeAppPath: string = ''
  @State homeAppCurrentPath: string = ''
  @State errorMessage: string = ''
  @State modalMiniAppVisible: boolean = false
  @State modalMiniAppId: string = ''
  @State modalMiniAppPath: string = ''
  @State hideHomeMiniApp: boolean = false

  // References to containers for direct method calls
  private homeContainerRef: MiniAppContainer | null = null
  private modalContainerRef: MiniAppContainer | null = null

  aboutToAppear() {
    hilog.info(DOMAIN, TAG, 'MiniAppManager starting - intelligent initialization');

    // Set static instance in MiniApp
    MiniApp.setManagerInstance(this);

    // Manager is ready for openHomeMiniApp() calls
    hilog.info(DOMAIN, TAG, 'MiniAppManager ready, waiting for openHomeMiniApp() call');
  }

  build() {
    Stack() {
      if (this.isReady && this.homeAppId && this.homeAppPath && !this.hideHomeMiniApp) {
        // Display Home MiniApp (hidden when modal miniapp is open)
        // Use current path if available, otherwise use initial path
        MiniAppContainer({
          appId: this.homeAppId,
          initialPath: this.homeAppCurrentPath || this.homeAppPath,
          externalPath: this.homeAppCurrentPath || this.homeAppPath
        })
      } else if (this.errorMessage) {
      // Minimalist error display
      Column() {
        Text('⚠️')
          .fontSize(32)
          .margin({ bottom: 8 })
      }
      .justifyContent(FlexAlign.Center)
      .alignItems(HorizontalAlign.Center)
      .width('100%')
      .height('100%')
      .padding(20)
    } else {
      // Clean loading state
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

    // Display modal miniapp on top if visible
    if (this.modalMiniAppVisible && this.modalMiniAppId && this.modalMiniAppPath) {
      MiniAppContainer({
        appId: this.modalMiniAppId,
        initialPath: this.modalMiniAppPath,
        externalPath: this.modalMiniAppPath
      })
      .zIndex(999) // Display on top but below system UI
    }
    }
    .width('100%')
    .height('100%')
    .expandSafeArea(this.shouldExpandSafeArea() ? [SafeAreaType.SYSTEM] : [])
  }

  /**
   * Open MiniApp (unified method for both home and other miniapps)
   */
  public doOpenMiniApp(appId: string, path: string, isHomeMiniApp: boolean = false): void {
    hilog.info(DOMAIN, TAG, `Opening MiniApp: ${appId}:${path}, isHome: ${isHomeMiniApp}`);

    try {
      const result: number = onMiniappOpened(appId, path);
      hilog.info(DOMAIN, TAG, `Native onMiniappOpened result: ${result}`);

      if (isHomeMiniApp) {
        this.homeAppId = appId;
        this.homeAppPath = path;
        this.homeAppCurrentPath = path; // Initialize current page
        this.isReady = true;
        hilog.info(DOMAIN, TAG, 'Home MiniApp loaded successfully');
      } else {
        // Modal miniapp: create new MiniAppContainer and display it
        hilog.info(DOMAIN, TAG, `Opening modal miniapp: ${appId}:${path}`);

        // Store modal miniapp info in both static and instance state
        MiniApp.modalMiniAppId = appId;
        MiniApp.modalMiniAppPath = path;
        MiniApp.modalMiniAppVisible = true;

        // Hide home miniapp when modal is open
        MiniApp.hideHomeMiniApp = true;
        this.hideHomeMiniApp = true;
        hilog.info(DOMAIN, TAG, `HIDING home miniapp - set to: ${this.hideHomeMiniApp}`);

        // Update UI state immediately with priority
        this.modalMiniAppId = appId;
        this.modalMiniAppPath = path;
        this.modalMiniAppVisible = true;
      }
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to open MiniApp ${appId}: ${error}`);
      if (isHomeMiniApp) {
        this.errorMessage = `Failed to load Home MiniApp: ${error}`;
      }
    }
  }

  /**
   * Determine if safe area should be expanded for status bar transparency
   */
  private shouldExpandSafeArea(): boolean {
    // Always expand safe area for status bar transparency
    // This applies to both home miniapp and modal miniapp
    return true;
  }

  /**
   * Update home miniapp current path (called when user switches tabs)
   */
  public updateHomeMiniAppCurrentPath(path: string): void {
    if (this.homeAppId) {
      this.homeAppCurrentPath = path;
    }
  }

  /**
   * Close modal miniapp
   */
  public closeModalMiniApp(): void {
    hilog.info(DOMAIN, TAG, `Closing modal miniapp: visible=${this.modalMiniAppVisible}, id=${this.modalMiniAppId}`);

    // Update UI state immediately (onMiniappClosed already called in static closeMiniApp)
    this.modalMiniAppVisible = false;
    this.modalMiniAppId = '';
    this.modalMiniAppPath = '';

    // Show home miniapp when modal is closed
    MiniApp.hideHomeMiniApp = false;
    this.hideHomeMiniApp = false;
  }

  /**
   * Switch to specific page in any MiniApp (External API for Rust)
   * @param appId MiniApp ID
   * @param path Page path to switch to
   * @returns true if successful, false otherwise
   */
  public switchToPage(appId: string, path: string): boolean {
    hilog.info(DOMAIN, TAG, `switchToPage API called: ${appId}:${path}`);

    // Handle home miniapp
    if (appId === this.homeAppId) {
      hilog.info(DOMAIN, TAG, `Switching home miniapp page to: ${path}`);
      this.homeAppCurrentPath = path;
      return true;
    }

    // Handle modal miniapp
    if (appId === this.modalMiniAppId) {
      hilog.info(DOMAIN, TAG, `Switching modal miniapp page to: ${path}`);
      this.modalMiniAppPath = path;
      return true;
    }

    hilog.warn(DOMAIN, TAG, `MiniApp ${appId} not currently active`);
    return false;
  }
}
