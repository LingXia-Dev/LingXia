import { hilog } from '@kit.PerformanceAnalysisKit';
import { webview } from '@kit.ArkWeb';
import { window } from '@kit.ArkUI';
import { resourceManager } from '@kit.LocalizationKit';
import { taskpool } from '@kit.ArkTS';
import { LxAppContainer } from './LxAppContainer';
import { initNativeBridge, getNativeCallbackFunction } from './NativeBridge';
import { WebViewManager, toWebTag } from './WebView';

// Import native functions from Rust
import { lxappInit, onLxappOpened, onLxappClosed, onPageShow } from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.LxApp';

// Task function for handling LxApp close operation asynchronously
@Concurrent
function closeLxAppTask(appId: string): number {
  return onLxappClosed(appId);
}

interface NavigationInstance {
  openLxApp(appId: string, path: string): void;
  closeLxApp(appId: string): void;
}

export class LxApp {
  private static instance: LxApp | null = null;
  public static managerInstance: LxAppManager | null = null;
  public static context: Context | null = null;
  public static windowStage: window.WindowStage | null = null;

  // Modal miniapp state (for non-home miniapps)
  public static modalLxAppId: string | null = null;
  public static modalLxAppPath: string | null = null;
  public static modalLxAppVisible: boolean = false;

  // Control home miniapp visibility when modal is open
  public static hideHomeLxApp: boolean = false;

  // Store navigation instance for Navigation-based architecture
  public static navigationInstance: NavigationInstance | null = null;

  private homeLxAppId: string | null = null;
  private homeLxAppInitialRoute: string | null = null;

  private constructor() {
    hilog.info(DOMAIN, TAG, 'LxApp instance created');
  }

  public static getInstance(): LxApp {
    if (!LxApp.instance) {
      LxApp.instance = new LxApp();
    }
    return LxApp.instance;
  }

  public static initialize(context?: Context, windowStage?: window.WindowStage): void {
    const instance = LxApp.getInstance();
    if (instance.homeLxAppId !== null && instance.homeLxAppInitialRoute !== null) {
      hilog.info(DOMAIN, TAG, 'LxApp already successfully initialized');
      return;
    }

    // Check if native layer has already been initialized by checking homeLxAppId
    if (instance.homeLxAppId !== null) {
      hilog.warn(DOMAIN, TAG, 'Native layer already initialized, skipping native_lxapp_init');
      return;
    }

    // Store context and windowStage for window operations
    if (context) {
      LxApp.context = context;
    }
    if (windowStage) {
      LxApp.windowStage = windowStage;
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

    hilog.info(DOMAIN, TAG, `Calling lxappInit with callback and file system paths (FIRST AND ONLY TIME)`);
    const result: string | null = lxappInit(callbackFunction, dataDir, cacheDir, resourceManager);

    if (result) {
      hilog.info(DOMAIN, TAG, `LxApp initialized successfully: ${result}`);

      const parts: string[] = result.split(':', 2);
      if (parts.length === 2) {
        instance.homeLxAppId = parts[0];
        instance.homeLxAppInitialRoute = parts[1];

        // Initialize Web component core: initialize the Browser process and create BrowserContext.
        webview.WebviewController.initializeWebEngine();

      } else {
        hilog.error(DOMAIN, TAG, `Failed to parse home LxApp details from native (expected 2 parts): ${result}`);
      }
    } else {
      hilog.error(DOMAIN, TAG, 'Failed to get home LxApp details from native init.');
    }
  }

  public static openHomeLxApp(): void {
    hilog.info(DOMAIN, TAG, 'openHomeLxApp called');

    const instance = LxApp.getInstance();
    const homeAppId = instance.getHomeLxAppId();
    const homeAppPath = instance.getHomeLxAppInitialRoute();

    if (homeAppId && homeAppPath) {
      // Home LxApp has special handling - it's displayed in the base layer, not in stack
      hilog.info(DOMAIN, TAG, `Opening home LxApp directly: ${homeAppId}:${homeAppPath}`);

      try {
        // Just call the native function for home LxApp
        const result: number = onLxappOpened(homeAppId, homeAppPath);
        hilog.info(DOMAIN, TAG, `onLxappOpened result for home: ${result}`);
      } catch (error) {
        hilog.error(DOMAIN, TAG, `Failed to open home LxApp: ${error}`);
      }
    } else {
      hilog.error(DOMAIN, TAG, 'Home LxApp info not available for openHomeLxApp');
    }
  }

  public static closeLxApp(appId: string): void {
    // Call native onLxappClosed asynchronously (fire and forget)
    taskpool.execute(new taskpool.Task(closeLxAppTask, appId));

    if (LxApp.navigationInstance) {
      hilog.info(DOMAIN, TAG, 'Using LxAppNavigation for closeLxApp');
      LxApp.navigationInstance.closeLxApp(appId);
      return;
    }

    if (LxApp.modalLxAppId === appId) {
      LxApp.modalLxAppId = null;
      LxApp.modalLxAppPath = null;
      LxApp.modalLxAppVisible = false;
      LxApp.hideHomeLxApp = false;

      if (LxApp.managerInstance) {
        LxApp.managerInstance.closeModalLxApp();
      } else {
        hilog.warn(DOMAIN, TAG, ' No manager instance available!');
      }
    } else {
      hilog.warn(DOMAIN, TAG, `Cannot close miniapp ${appId} - not currently displayed modal miniapp (current: ${LxApp.modalLxAppId})`);
    }
  }

  public static openLxApp(appId: string, path: string): void {
    hilog.info(DOMAIN, TAG, `openLxApp called: ${appId}:${path}`);
    if (LxApp.navigationInstance) {
      hilog.info(DOMAIN, TAG, 'Using LxAppNavigation for openLxApp');
      LxApp.navigationInstance.openLxApp(appId, path);
      return;
    }

    if (!LxApp.managerInstance) {
      hilog.info(DOMAIN, TAG, 'No LxApp instance ready, waiting...');
      setTimeout(() => {
        LxApp.openLxApp(appId, path);
      }, 50);
      return;
    }

    const instance = LxApp.getInstance();
    const isHomeLxApp = (appId === instance.getHomeLxAppId() && path === instance.getHomeLxAppInitialRoute());

    if (isHomeLxApp && LxApp.managerInstance.isReady) {
      hilog.info(DOMAIN, TAG, 'Home LxApp already loaded');
      return;
    }

    hilog.info(DOMAIN, TAG, 'Using LxAppManager for openLxApp (legacy)');
    LxApp.managerInstance.doOpenLxApp(appId, path, isHomeLxApp);
  }

  /**
   * Switch to a specific page within a LxApp
   * @param appId - LxApp ID
   * @param path - Page path to switch to
   */
  public static switchPage(appId: string, path: string): boolean {
    hilog.info(DOMAIN, TAG, `switchPage API called: ${appId}:${path}`);

    // The manager finds the right container and handles the switch
    if (LxApp.managerInstance) {
      return LxApp.managerInstance.switchToPage(appId, path);
    }

    hilog.warn(DOMAIN, TAG, 'No LxApp manager ready for switchPage');
    return false;
  }

  /**
   * Get Home LxApp ID
   */
  public getHomeLxAppId(): string | null {
    return this.homeLxAppId;
  }

  /**
   * Get Home LxApp initial route
   */
  public getHomeLxAppInitialRoute(): string | null {
    return this.homeLxAppInitialRoute;
  }

  /**
   * Set manager instance (called by LxAppManager)
   */
  public static setManagerInstance(manager: LxAppManager): void {
    LxApp.managerInstance = manager;
  }

  /**
   * Set navigation instance (called by LxAppNavigation)
   */
  public static setNavigationInstance(navigation: NavigationInstance): void {
    LxApp.navigationInstance = navigation;
  }

  /**
   * Setup system status bar transparency (call after window is created)
   * Should be called in onWindowStageCreate
   */
  public static setupSystemStatusBar(windowStage: window.WindowStage): void {
    //const instance = LxApp.getInstance();
    //instance.setSystemStatusBarTransparent(windowStage);
  }

  /**
   * Switch to specific page in current LxApp
   * @param appId LxApp ID
   * @param path Page path to switch to
   * @returns Promise<boolean> Success status
   */
  public static async switchToPage(appId: string, path: string): Promise<boolean> {
    const instance = LxApp.getInstance();
    return await instance.switchToPageInternal(appId, path);
  }

  /**
   * Switch to specific page in current LxApp (internal implementation)
   * @param appId LxApp ID
   * @param path Page path to switch to
   * @returns Promise<boolean> Success status
   */
  public async switchToPageInternal(appId: string, path: string): Promise<boolean> {
    hilog.info(DOMAIN, TAG, `switchToPageInternal called: appId=${appId}, path=${path}`);

    try {
      // Find the container and switch to the page
      if (LxApp.managerInstance) {
        const success: boolean = LxApp.managerInstance.switchToPage(appId, path);
        if (success) {
          hilog.info(DOMAIN, TAG, `Successfully switched to page: ${path}`);
          return true;
        } else {
          hilog.warn(DOMAIN, TAG, `Failed to switch to page: ${path} - page not found in TabBar`);
          return false;
        }
      } else {
        hilog.error(DOMAIN, TAG, `No LxApp manager instance available for page switch`);
        return false;
      }
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Error switching to page ${path}: ${error}`);
      return false;
    }
  }
}

/**
 * LxApp Manager Component - UI management for LxApps
 * Internal component
 */
@Component
struct LxAppManager {
  @State isReady: boolean = false
  @State homeAppId: string = ''
  @State homeAppPath: string = ''
  @State homeAppCurrentPath: string = ''
  @State errorMessage: string = ''
  @State modalLxAppVisible: boolean = false
  @State modalLxAppId: string = ''
  @State modalLxAppPath: string = ''
  @State hideHomeLxApp: boolean = false

  // References to containers for direct method calls
  private homeContainerRef: LxAppContainer | null = null
  private modalContainerRef: LxAppContainer | null = null

  aboutToAppear() {
    hilog.info(DOMAIN, TAG, 'LxAppManager starting - intelligent initialization');

    // Set static instance in LxApp
    LxApp.setManagerInstance(this);

    // Manager is ready for openHomeLxApp() calls
    hilog.info(DOMAIN, TAG, 'LxAppManager ready, waiting for openHomeLxApp() call');
  }

  build() {
    Stack() {
      if (this.isReady && this.homeAppId && this.homeAppPath && !this.hideHomeLxApp) {
        // Display Home LxApp (hidden when modal miniapp is open)
        // Use current path if available, otherwise use initial path
        LxAppContainer({
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
    if (this.modalLxAppVisible && this.modalLxAppId && this.modalLxAppPath) {
      LxAppContainer({
        appId: this.modalLxAppId,
        initialPath: this.modalLxAppPath,
        externalPath: this.modalLxAppPath
      })
      .zIndex(999) // Display on top but below system UI
    }
    }
    .width('100%')
    .height('100%')
    .expandSafeArea(this.shouldExpandSafeArea() ? [SafeAreaType.SYSTEM] : [])
  }

  /**
   * Open LxApp (unified method for both home and other miniapps)
   */
  public doOpenLxApp(appId: string, path: string, isHomeLxApp: boolean = false): void {
    hilog.info(DOMAIN, TAG, `Opening LxApp: ${appId}:${path}, isHome: ${isHomeLxApp}`);

    try {
      const result: number = onLxappOpened(appId, path);
      hilog.info(DOMAIN, TAG, `Native onLxappOpened result: ${result}`);

      if (isHomeLxApp) {
        this.homeAppId = appId;
        this.homeAppPath = path;
        this.homeAppCurrentPath = path; // Initialize current page
        this.isReady = true;
        hilog.info(DOMAIN, TAG, 'Home LxApp loaded successfully');
      } else {
        // Modal miniapp: create new LxAppContainer and display it
        hilog.info(DOMAIN, TAG, `Opening modal miniapp: ${appId}:${path}`);

        // Store modal miniapp info in both static and instance state
        LxApp.modalLxAppId = appId;
        LxApp.modalLxAppPath = path;
        LxApp.modalLxAppVisible = true;

        // Hide home miniapp when modal is open
        LxApp.hideHomeLxApp = true;
        this.hideHomeLxApp = true;
        hilog.info(DOMAIN, TAG, `HIDING home miniapp - set to: ${this.hideHomeLxApp}`);

        // Update UI state immediately with priority
        this.modalLxAppId = appId;
        this.modalLxAppPath = path;
        this.modalLxAppVisible = true;
      }
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to open LxApp ${appId}: ${error}`);
      if (isHomeLxApp) {
        this.errorMessage = `Failed to load Home LxApp: ${error}`;
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
  public updateHomeLxAppCurrentPath(path: string): void {
    if (this.homeAppId) {
      this.homeAppCurrentPath = path;
    }
  }

  /**
   * Close modal miniapp
   */
  public closeModalLxApp(): void {
    hilog.info(DOMAIN, TAG, `Closing modal miniapp: visible=${this.modalLxAppVisible}, id=${this.modalLxAppId}`);

    // Update UI state immediately (onLxappClosed already called in static closeLxApp)
    this.modalLxAppVisible = false;
    this.modalLxAppId = '';
    this.modalLxAppPath = '';

    // Show home miniapp when modal is closed
    LxApp.hideHomeLxApp = false;
    this.hideHomeLxApp = false;
  }

  /**
   * Switch to specific page in any LxApp (External API for Rust)
   * @param appId LxApp ID
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
    if (appId === this.modalLxAppId) {
      hilog.info(DOMAIN, TAG, `Switching modal miniapp page to: ${path}`);
      this.modalLxAppPath = path;
      return true;
    }

    hilog.warn(DOMAIN, TAG, `LxApp ${appId} not currently active`);
    return false;
  }
}
