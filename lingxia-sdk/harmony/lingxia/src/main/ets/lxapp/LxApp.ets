import { hilog } from '@kit.PerformanceAnalysisKit';
import { webview } from '@kit.ArkWeb';
import { window } from '@kit.ArkUI';
import { resourceManager, i18n } from '@kit.LocalizationKit';
import { Want } from '@kit.AbilityKit';
import common from '@ohos.app.ability.common';
import { LxAppContainer } from './LxAppContainer';
import { ToastManager } from './APIs/LxAppToast';
import { initNativeBridge, getNativeCallbackFunction } from './NativeBridge';
import { WebViewManager, toWebTag } from './WebView';
import { AnimationType } from './LxAppNavigation';
import { CameraCaptureOverlay } from './APIs/media/CameraCaptureOverlay';
import promptAction from '@kit.ArkUI';
import { abilityAccessCtrl, Permissions } from '@kit.AbilityKit';

// Import native functions from Rust
import { lxappInit, registerCustomSchemes, onLxappOpened, onLxappClosed, onPageShow, onApplinkReceived, getCurrentLxapp, CurrentLxApp } from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.LxApp';

interface NavigationInstance {
  openLxApp(appId: string, path: string): void;
  closeLxApp(appId: string): void;
  navigate(appId: string, path: string, animationType: AnimationType): boolean;
  isTabBarHidden(appId: string): boolean;
}

export class LxApp {
  private static instance: LxApp | null = null;
  public static managerInstance: LxAppManager | null = null;
  public static context: common.UIAbilityContext | null = null;
  public static windowStage: window.WindowStage | null = null;

  // Store navigation instance for Navigation-based architecture
  public static navigationInstance: NavigationInstance | null = null;

  private homeLxAppId: string | null = null;

  private constructor() {
    hilog.info(DOMAIN, TAG, 'LxApp instance created');
  }

  public static getInstance(): LxApp {
    if (!LxApp.instance) {
      LxApp.instance = new LxApp();
    }
    return LxApp.instance;
  }

  /**
   * Request location permissions
   * @param context - UIAbilityContext for permission requests
   */
  public static requestLocationPermissions(context: common.UIAbilityContext): void {
    hilog.info(DOMAIN, TAG, 'requestLocationPermissions called');

    const atManager = abilityAccessCtrl.createAtManager();
    const permissions: Permissions[] = [
      'ohos.permission.LOCATION' as Permissions,
      'ohos.permission.APPROXIMATELY_LOCATION' as Permissions,
      'ohos.permission.CAMERA' as Permissions,
      'ohos.permission.MICROPHONE' as Permissions,
      'ohos.permission.READ_IMAGEVIDEO' as Permissions,
      'ohos.permission.WRITE_IMAGEVIDEO' as Permissions
    ];

    hilog.info(DOMAIN, TAG, `Requesting permissions: ${JSON.stringify(permissions)}`);

    try {
      hilog.info(DOMAIN, TAG, 'Calling atManager.requestPermissionsFromUser...');
      atManager.requestPermissionsFromUser(context, permissions).then((data) => {
        hilog.info(DOMAIN, TAG, `Permission request result: ${JSON.stringify(data)}`);
        if (data && data.authResults) {
          for (let i = 0; i < data.authResults.length; i++) {
            const permission = permissions[i];
            const result: number = data.authResults[i];
            const isGranted = result === 0; // 0 means PERMISSION_GRANTED
            hilog.info(DOMAIN, TAG, `Permission ${permission}: ${isGranted ? 'GRANTED' : 'DENIED'}`);
          }
        }
      }).catch((err: Error) => {
        hilog.error(DOMAIN, TAG, `Permission request failed: ${err.message}`);
      });
    } catch (err) {
      hilog.error(DOMAIN, TAG, `Permission request error: ${err}`);
    }
  }

  public static initialize(context?: common.UIAbilityContext, windowStage?: window.WindowStage): void {
    const instance = LxApp.getInstance();
    if (instance.homeLxAppId !== null) {
      hilog.info(DOMAIN, TAG, 'LxApp already successfully initialized');
      return;
    }

    // Check if native layer has already been initialized by checking homeLxAppId
    if (instance.homeLxAppId !== null) {
      hilog.warn(DOMAIN, TAG, 'Native layer already initialized, skipping native_lxapp_init');
      return;
    }

    hilog.info(DOMAIN, TAG, `LxApp.initialize called with context: ${context ? 'provided' : 'null'}, windowStage: ${windowStage ? 'provided' : 'null'}`);

    // Store context and windowStage for window operations
    if (context) {
      LxApp.context = context;
      hilog.info(DOMAIN, TAG, 'Requesting location permissions...');
      LxApp.requestLocationPermissions(context);
    } else {
      hilog.warn(DOMAIN, TAG, 'No context provided, skipping permission request');
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

    // Register custom schemes BEFORE WebEngine initialization
    const schemesRegistered: boolean = registerCustomSchemes();
    if (!schemesRegistered) {
      hilog.error(DOMAIN, TAG, 'Failed to register custom schemes');
      return;
    }

    // Initialize WebEngine after schemes are registered
    webview.WebviewController.initializeWebEngine();
    webview.WebviewController.setWebDebuggingAccess(true);

    hilog.info(DOMAIN, TAG, `Calling lxappInit with callback and file system paths (FIRST AND ONLY TIME)`);

    // Get system locale using HarmonyOS i18n API
    const systemLanguage = i18n.System.getSystemLanguage();
    const systemRegion = i18n.System.getSystemRegion();

    // Convert HarmonyOS locale format to standard format
    // HarmonyOS returns "zh-Hans" but we want "zh"
    const normalizedLanguage = systemLanguage.split('-')[0];
    const locale = `${normalizedLanguage}-${systemRegion}`;

    const appId: string | null = lxappInit(callbackFunction, dataDir, cacheDir, resourceManager, locale);

    if (appId) {
      hilog.info(DOMAIN, TAG, `LxApp initialized successfully with home app: ${appId}`);
      instance.homeLxAppId = appId;
    } else {
      hilog.error(DOMAIN, TAG, 'Failed to get home LxApp details from native init.');
    }
  }

  public static enableWebViewDebugging(): void {
    WebViewManager.enableDebugging();
  }

  private static openHomeLxApp(): void {
    hilog.info(DOMAIN, TAG, 'openHomeLxApp called');

    const instance = LxApp.getInstance();
    const homeAppId = instance.getHomeLxAppId();

    if (homeAppId && LxApp.navigationInstance) {
      LxApp.openLxApp(homeAppId, '');
    } else {
      hilog.error(DOMAIN, TAG, 'openHomeLxApp failed: homeAppId or navigationInstance not available');
    }
  }

  /**
   * Close an LxApp by appId
   * @param appId - The ID of the LxApp to close
   */
  public static closeLxApp(appId: string): void {
    if (LxApp.navigationInstance) {
      LxApp.navigationInstance.closeLxApp(appId);
      return;
    }

    hilog.error(DOMAIN, TAG, 'LxAppNavigation instance not available for closeLxApp');
  }

  /**
   * Open an LxApp by appId and path
   * @param appId - The ID of the LxApp to open
   * @param path - The path to open within the LxApp (empty string will use initial route)
   */
  public static openLxApp(appId: string, path: string): void {
    hilog.info(DOMAIN, TAG, `openLxApp called: ${appId}:${path}`);

    // Use navigate with Launch type for unified architecture
    if (LxApp.navigationInstance) {
      hilog.info(DOMAIN, TAG, 'Using navigate for openLxApp with Launch type');
      LxApp.navigationInstance.openLxApp(appId, path);
      return;
    }

    hilog.error(DOMAIN, TAG, 'LxAppNavigation instance not available for openLxApp');
  }

  /**
   * Navigate to a specific page with animation type
   * @param appId - LxApp ID
   * @param path - Page path to navigate to
   * @param animationType - AnimationType enum
   * @returns boolean - True if navigation was successful, false otherwise
   */
  public static navigate(appId: string, path: string, animationType: AnimationType): boolean {
    const typeName = AnimationType[animationType];
    hilog.info(DOMAIN, TAG, `navigate API called: ${appId}:${path} with type: ${typeName}`);

    // Use LxAppNavigation for unified architecture
    if (LxApp.navigationInstance) {
      return LxApp.navigationInstance.navigate(appId, path, animationType);
    }

    hilog.warn(DOMAIN, TAG, 'LxAppNavigation not available for navigate');
    return false;
  }

  /**
   * Get Home LxApp ID
   * @returns string | null - Home LxApp ID or null if not initialized
   */
  public getHomeLxAppId(): string | null {
    return this.homeLxAppId;
  }

  /**
   * Set manager instance (used for component compatibility only)
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
   * Launch external URL with system default application
   * @param url The URL to open
   */
  public static launchWithUrl(url: string): void {
    if (!LxApp.context) {
      hilog.error(DOMAIN, TAG, `Context not available for launching URL: ${url}`);
      return;
    }

    LxApp.context.openLink(url)
      .then(() => {
        hilog.info(DOMAIN, TAG, `Successfully launched URL: ${url}`);
      })
      .catch((err: Error) => {
        hilog.error(DOMAIN, TAG, `Failed to launch URL: ${url}, error: ${JSON.stringify(err)}`);
      });
  }

  /**
   * Internal applinkhandler - called by SDK infrastructure
   * @param url The full applinkURL (e.g., "https://www.lingxia.app/12/3")
   */
  /**
   * Internal applink handler - called by SDK infrastructure
   * @param url The full applink URL (e.g., "https://www.lingxia.app/12/3")
   */
  private static handleAppLink(url: string): void {
    try {
      // Call native layer to handle the applink
      const result: number = onApplinkReceived(url);
      hilog.info(DOMAIN, TAG, `AppLink processed with result: ${result}`);
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Error processing AppLink: ${url}, error: ${JSON.stringify(error)}`);
    }
  }

  /**
   * Handle Want from UIAbility - call this in your UIAbility onCreate/onNewWant methods
   * This provides a simple one-line integration for applink handling
   * @param want The Want object containing the applink URI
   */
  public static handleWant(want: Want): void {
    if (want.uri && want.uri.startsWith('https://')) {
      hilog.info(DOMAIN, TAG, `Processing Want with URI: ${want.uri}`);
      LxApp.handleAppLink(want.uri);
    } else if (want.uri) {
      hilog.info(DOMAIN, TAG, `AppLink ignored - not HTTPS scheme: ${want.uri}`);
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

    // Set static instance in LxApp for component compatibility
    LxApp.setManagerInstance(this);

    // Manager is ready for openHomeLxApp() calls
    hilog.info(DOMAIN, TAG, 'LxAppManager ready, waiting for openHomeLxApp() call');
  }

  build() {
    Stack() {
      if (this.isReady && this.homeAppId && this.homeAppPath && !this.hideHomeLxApp) {
        // Display Home LxApp (hidden when modal lxapp is open)
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

    // Display modal lxapp on top if visible
    if (this.modalLxAppVisible && this.modalLxAppId && this.modalLxAppPath) {
      LxAppContainer({
        appId: this.modalLxAppId,
        initialPath: this.modalLxAppPath,
        externalPath: this.modalLxAppPath
      })
      .zIndex(999) // Display on top but below system UI
    }

    // Toast manager - always on top
    ToastManager()
      .zIndex(10000)

    // Camera capture overlay - always on top of everything
    CameraCaptureOverlay()
      .zIndex(20000)
    }
    .width('100%')
    .height('100%')
    .expandSafeArea(this.shouldExpandSafeArea() ? [SafeAreaType.SYSTEM] : [])
  }

  /**
   * Open LxApp (unified method for both home and other lxapps)
   */
  public doOpenLxApp(appId: string, path: string, isHomeLxApp: boolean = false): void {
    hilog.info(DOMAIN, TAG, `Opening LxApp: ${appId}:${path}, isHome: ${isHomeLxApp}`);

    try {
      const resolvedPath: string = onLxappOpened(appId, path);

      if (isHomeLxApp) {
        this.homeAppId = appId;
        this.homeAppPath = resolvedPath;
        this.homeAppCurrentPath = resolvedPath; // Initialize current page
        this.isReady = true;
        hilog.info(DOMAIN, TAG, 'Home LxApp loaded successfully');
      } else {
        // Modal lxapp: create new LxAppContainer and display it
        hilog.info(DOMAIN, TAG, `Opening modal lxapp: ${appId}:${resolvedPath}`);

        // Hide home lxapp when modal is open
        this.hideHomeLxApp = true;
        hilog.info(DOMAIN, TAG, `HIDING home lxapp - set to: ${this.hideHomeLxApp}`);

        // Update UI state immediately with priority
        this.modalLxAppId = appId;
        this.modalLxAppPath = resolvedPath;
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
    // This applies to both home lxapp and modal lxapp
    return true;
  }

  /**
   * Update home lxapp current path (called when user switches tabs)
   */
  public updateHomeLxAppCurrentPath(path: string): void {
    if (this.homeAppId) {
      this.homeAppCurrentPath = path;
    }
  }

  /**
   * Close modal lxapp
   */
  public closeModalLxApp(): void {
    hilog.info(DOMAIN, TAG, `Closing modal lxapp: visible=${this.modalLxAppVisible}, id=${this.modalLxAppId}`);

    // Update UI state immediately (onLxappClosed already called in static closeLxApp)
    this.modalLxAppVisible = false;
    this.modalLxAppId = '';
    this.modalLxAppPath = '';

    // Show home lxapp when modal is closed
    this.hideHomeLxApp = false;
  }

  /**
   * Navigate to specific page in any LxApp (External API for Rust)
   * @param appId - LxApp ID
   * @param path - Page path to navigate to
   * @param animationType - AnimationType enum
   * @returns boolean - True if successful, false otherwise
   */
  public navigate(appId: string, path: string, animationType: AnimationType): boolean {
    const typeName = AnimationType[animationType];
    hilog.info(DOMAIN, TAG, `LxAppManager.navigate called: ${appId}:${path} with type: ${typeName}`);
    return LxApp.navigate(appId, path, animationType);
  }

}
