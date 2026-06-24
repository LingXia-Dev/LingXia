import { hilog } from '@kit.PerformanceAnalysisKit';
import { LxAppContainer } from './LxAppContainer';
import { ToastManager } from './APIs/LxAppToast';
import { WebViewManager } from './WebViewBridge';
import { AnimationType } from './LxAppNavigation';
import { CameraCaptureOverlay } from './APIs/media/CameraCaptureOverlay';

// Import native functions from Rust
import { onLxappOpened, getLxappSessionId } from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.LxApp';

interface NavigationInstance {
  openLxApp(appId: string, path: string): void;
  closeLxApp(appId: string): void;
  openLxAppWithSession(appId: string, path: string, sessionId: number): void;
  closeLxAppWithSession(appId: string, sessionId: number): void;
  navigate(appId: string, path: string, animationType: AnimationType): boolean;
  isTabBarHidden(appId: string): boolean;
}

/**
 * LxApp-scope (single mini-program / tenant) facade. Owns the home app id,
 * open/close/navigate, and session lookup for one LxApp inside the host
 * window. Host-scope concerns (runtime bootstrap, context/window, capabilities,
 * external URL handoff, applink, orientation) live in [Lingxia].
 *
 * Counterpart of Android's `com.lingxia.lxapp.LxApp`.
 */
export class LxApp {
  private static instance: LxApp | null = null;
  public static managerInstance: LxAppManager | null = null;

  // Store navigation instance for Navigation-based architecture
  public static navigationInstance: NavigationInstance | null = null;

  private homeAppId: string | null = null;

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
   * Record the home LxApp id discovered by the native runtime. Called by
   * [Lingxia.initializeRuntime] once the host bootstrap resolves it.
   */
  public static setHomeAppId(appId: string): void {
    LxApp.getInstance().homeAppId = appId;
  }

  public static enableWebViewDebugging(): void {
    WebViewManager.enableDebugging();
  }

  private static openHomeApp(): void {
    hilog.info(DOMAIN, TAG, 'openHomeApp called');

    const instance = LxApp.getInstance();
    const homeAppId = instance.getHomeAppId();

    if (homeAppId && LxApp.navigationInstance) {
      const sessionId: number = getLxappSessionId(homeAppId);
      if (!Number.isFinite(sessionId) || sessionId <= 0) {
        hilog.error(DOMAIN, TAG, `Invalid home app session: ${homeAppId}`);
        return;
      }
      LxApp.openLxAppWithSession(homeAppId, '', sessionId);
    } else {
      hilog.error(DOMAIN, TAG, 'openHomeApp failed: homeAppId or navigationInstance not available');
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
  public getHomeAppId(): string | null {
    return this.homeAppId;
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

  public static getRuntimeSessionId(appId: string): number {
    const sessionId: number = getLxappSessionId(appId);
    if (Number.isFinite(sessionId) && sessionId > 0) {
      return sessionId;
    }
    return 0;
  }

  public static openLxAppWithSession(appId: string, path: string, sessionId: number): void {
    if (!Number.isFinite(sessionId) || sessionId <= 0) {
      hilog.error(DOMAIN, TAG, `openLxAppWithSession requires valid session for ${appId}`);
      return;
    }
    if (LxApp.navigationInstance) {
      LxApp.navigationInstance.openLxAppWithSession(appId, path, sessionId);
      return;
    }
    hilog.error(DOMAIN, TAG, 'LxAppNavigation instance not available for openLxAppWithSession');
  }

  public static closeLxAppWithSession(appId: string, sessionId: number): void {
    if (!Number.isFinite(sessionId) || sessionId <= 0) {
      hilog.error(DOMAIN, TAG, `closeLxAppWithSession requires valid session for ${appId}`);
      return;
    }
    if (LxApp.navigationInstance) {
      LxApp.navigationInstance.closeLxAppWithSession(appId, sessionId);
      return;
    }
    hilog.error(DOMAIN, TAG, 'LxAppNavigation instance not available for closeLxAppWithSession');
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
  @State hideHomeApp: boolean = false

  // References to containers for direct method calls
  private homeContainerRef: LxAppContainer | null = null
  private modalContainerRef: LxAppContainer | null = null

  aboutToAppear() {
    hilog.info(DOMAIN, TAG, 'LxAppManager starting - intelligent initialization');

    // Set static instance in LxApp for component compatibility
    LxApp.setManagerInstance(this);

    // Manager is ready for openHomeApp() calls
    hilog.info(DOMAIN, TAG, 'LxAppManager ready, waiting for openHomeApp() call');
  }

  build() {
    Stack() {
      if (this.isReady && this.homeAppId && this.homeAppPath && !this.hideHomeApp) {
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
  public doOpenLxApp(appId: string, path: string, sessionId: number, isHomeApp: boolean = false): void {
    if (!Number.isFinite(sessionId) || sessionId <= 0) {
      hilog.error(DOMAIN, TAG, `doOpenLxApp requires valid sessionId for ${appId}`);
      return;
    }
    hilog.info(DOMAIN, TAG, `Opening LxApp: ${appId}:${path}, isHome: ${isHomeApp}`);

    try {
      const resolvedPath: string = onLxappOpened(appId, path, sessionId);
      if (!resolvedPath || resolvedPath.length === 0) {
        hilog.warn(DOMAIN, TAG, `onLxappOpened rejected open request (stale session?): ${appId} (session=${sessionId})`);
        return;
      }

      if (isHomeApp) {
        this.homeAppId = appId;
        this.homeAppPath = resolvedPath;
        this.homeAppCurrentPath = resolvedPath; // Initialize current page
        this.isReady = true;
        hilog.info(DOMAIN, TAG, 'Home LxApp loaded successfully');
      } else {
        // Modal lxapp: create new LxAppContainer and display it
        hilog.info(DOMAIN, TAG, `Opening modal lxapp: ${appId}:${resolvedPath}`);

        // Hide home app when modal is open
        this.hideHomeApp = true;
        hilog.info(DOMAIN, TAG, `HIDING home app - set to: ${this.hideHomeApp}`);

        // Update UI state immediately with priority
        this.modalLxAppId = appId;
        this.modalLxAppPath = resolvedPath;
        this.modalLxAppVisible = true;
      }
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to open LxApp ${appId}: ${error}`);
      if (isHomeApp) {
        this.errorMessage = `Failed to load Home LxApp: ${error}`;
      }
    }
  }

  /**
   * Determine if safe area should be expanded for status bar transparency
   */
  private shouldExpandSafeArea(): boolean {
    // Always expand safe area for status bar transparency
    // This applies to both home app and modal lxapp
    return true;
  }

  /**
   * Update home app current path (called when user switches tabs)
   */
  public updateHomeAppCurrentPath(path: string): void {
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

    // Show home app when modal is closed
    this.hideHomeApp = false;
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
