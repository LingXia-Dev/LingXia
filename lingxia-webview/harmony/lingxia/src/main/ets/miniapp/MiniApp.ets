import { hilog } from '@kit.PerformanceAnalysisKit';
import { webview } from '@kit.ArkWeb';
import { window } from '@kit.ArkUI';
import { resourceManager } from '@kit.LocalizationKit';
import { MiniAppContainer } from './MiniAppContainer';
import { initNativeBridge, getNativeCallbackFunction } from './NativeBridge';

// Import native functions from Rust
import { miniappInit } from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.MiniApp';

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

        //if (windowStage) {
          //instance.setSystemStatusBarTransparent(windowStage);
        //}
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

      MiniApp.openMiniApp(homeAppId, homeAppPath);
    } else {
      hilog.error(DOMAIN, TAG, 'Home MiniApp info not available for openHomeMiniApp');
    }
  }

  public static closeMiniApp(appId: string): void {
    hilog.info(DOMAIN, TAG, `Closing miniapp: ${appId}`);

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
   * Dummy implementation: MiniApp opened event
   */
  public on_miniapp_opened(appId: string, path: string): number {
    hilog.info(DOMAIN, TAG, `on_miniapp_opened called with appId: ${appId}, path: ${path}`);
    return 0; // Return success status code
  }

  /**
   * Dummy implementation: MiniApp closed event
   */
  public on_miniapp_closed(appId: string): number {
    hilog.info(DOMAIN, TAG, `on_miniapp_closed called with appId: ${appId}`);
    return 0; // Return success status code
  }



  // Store WebView controllers for reuse
  private webviewControllers: Map<string, webview.WebviewController> = new Map();

  public find_webview(appId: string, path: string): webview.WebviewController | null {
    hilog.info(DOMAIN, TAG, `find_webview called with appId: ${appId}, path: ${path}`);

    const key = `${appId}:${path}`;

    // Return existing controller if available
    if (this.webviewControllers.has(key)) {
      const controller = this.webviewControllers.get(key);
      if (controller) {
        hilog.info(DOMAIN, TAG, `Found existing WebView controller for ${key}`);
        return controller;
      }
    }

    try {
      hilog.info(DOMAIN, TAG, `Creating new WebView controller for ${key} (only place allowed to create)`);
      const webviewController = new webview.WebviewController();
      this.webviewControllers.set(key, webviewController);

      hilog.info(DOMAIN, TAG, `Created WebView controller for ${key} - ready for UI association`);
      return webviewController;
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to create WebView controller for ${key}: ${error}`);
      return null;
    }
  }

  /**
   * Dummy function: onPageShow - called when WebView is actually shown to user
   * This is where URL loading should happen (like iOS)
   * Loads different URLs based on the page path to simulate tab switching
   */
  public on_page_show(appId: string, path: string): void {
    hilog.info(DOMAIN, TAG, `onPageShow called: ${appId}:${path} - WebView is now visible to user`);

    // Find the WebView controller
    const controller = this.find_webview(appId, path);
    if (controller) {
      try {
        // Load different URLs based on path to simulate different pages
        let url = "https://www.bing.com"; // Default URL

        // More precise path matching to avoid conflicts
        if (path === 'pages/home/index.html' || path === '/pages/home/index.html') {
          url = "https://www.baidu.com";
          hilog.info(DOMAIN, TAG, `Matched HOME page: ${path}`);
        } else if (path === 'pages/API/index.html' || path === '/pages/API/index.html') {
          url = "https://www.bing.com";
          hilog.info(DOMAIN, TAG, `Matched API page: ${path}`);
        } else if (path === 'pages/todo/index.html' || path === '/pages/todo/index.html') {
          url = "https://www.ifeng.com";
          hilog.info(DOMAIN, TAG, `Matched TODO page: ${path}`);
        } else {
          hilog.warn(DOMAIN, TAG, `No specific URL mapping for path: ${path}, using default`);
        }

        hilog.info(DOMAIN, TAG, `Loading URL for path ${path}: ${url}`);

        // Force reload even if WebView already has content
        controller.loadUrl(url);
        hilog.info(DOMAIN, TAG, `onPageShow: URL loaded successfully for path ${path}: ${url}`);
      } catch (error) {
        hilog.error(DOMAIN, TAG, `onPageShow: Failed to load URL: ${error}`);
      }
    } else {
      hilog.error(DOMAIN, TAG, `onPageShow: WebView controller not found for ${appId}:${path}`);
    }
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
      // Simply trigger page show - UI layer will handle the rest
      this.on_page_show(appId, path);
      hilog.info(DOMAIN, TAG, `Successfully triggered page show for: ${path}`);
      return true;
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
          initialPath: this.homeAppCurrentPath || this.homeAppPath
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
        initialPath: this.modalMiniAppPath
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
      const result = MiniApp.getInstance().on_miniapp_opened(appId, path);
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

    // Update UI state immediately
    this.modalMiniAppVisible = false;
    this.modalMiniAppId = '';
    this.modalMiniAppPath = '';

    // Show home miniapp when modal is closed
    MiniApp.hideHomeMiniApp = false;
    this.hideHomeMiniApp = false;
  }
}
