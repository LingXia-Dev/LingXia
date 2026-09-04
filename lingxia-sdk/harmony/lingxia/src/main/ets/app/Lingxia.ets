import { window } from '@kit.ArkUI';
import { webview } from '@kit.ArkWeb';
import { i18n } from '@kit.LocalizationKit';
import { Want, Configuration, ConfigurationConstant, EnvironmentCallback } from '@kit.AbilityKit';
import common from '@ohos.app.ability.common';
import { hilog } from '@kit.PerformanceAnalysisKit';
import { LxApp } from '../lxapp/LxApp';
import { LxAppPush } from '../lxapp/APIs/LxAppPush';
import { PermissionManager } from './PermissionManager';
import { initNativeBridge, getNativeCallbackFunction } from './NativeBridge';
import { LxLog } from './LxLog';
import {
  lingxiaInit,
  registerCustomSchemes,
  getCurrentLxapp,
  getPageOrientation,
  getAppCapabilities,
  getDisplayLanguage as getRuntimeDisplayLanguage,
  shouldEnableWebViewDebugging,
  onHostAppearanceChanged,
  onHostLocaleChanged,
  onApplinkReceived,
  CurrentLxApp
} from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.Lingxia';

/**
 * Top-level entry point for the LingXia host SDK. Operations scoped to the
 * host application (process, window, capabilities, runtime bootstrap, external
 * URL handoff, applink) live here. LxApp-scope operations (a single lxapp)
 * live in [LxApp].
 *
 * Counterpart of Android's `com.lingxia.app.Lingxia`.
 */
export class Lingxia {
  // Host capabilities, resolved from the native runtime during initializeRuntime.
  public static capabilities: number = 0;
  public static readonly CAP_BROWSER: number = 0x1;
  public static readonly CAP_NOTIFICATIONS: number = 0x2;
  public static readonly CAP_TERMINAL: number = 0x4;
  public static readonly CAP_PROXY: number = 0x8;

  /** Effective display language selected by the initialized runtime. */
  public static getDisplayLanguage(): string {
    if (LxApp.getInstance().getHomeAppId() === null) {
      return Lingxia.getSystemLocale();
    }
    return getRuntimeDisplayLanguage();
  }

  // Host UIAbility context + window stage, registered once during
  // initializeRuntime. Shared with LxApp-scope APIs that need a host context
  // (file/share/scan/media/push) — lxapp code reads these, never owns them.
  public static context: common.UIAbilityContext | null = null;
  public static windowStage: window.WindowStage | null = null;
  private static environmentCallback: EnvironmentCallback | null = null;

  /**
   * Product-app entry point, paired with `LxAppNavigation`. Only stages the
   * cheap, frame-independent pieces here; the heavy runtime boot is driven
   * by `LxAppNavigation` after its first frame — the splash cover — is on
   * screen, so the OS start window hands off to the cover in one beat
   * instead of sitting through initialization.
   */
  public static quickStart(context: common.UIAbilityContext, windowStage: window.WindowStage): void {
    if (LxApp.getInstance().getHomeAppId() !== null) {
      return;
    }
    Lingxia.context = context;
    PermissionManager.initialize(context);
    Lingxia.windowStage = windowStage;
    // Set window to full screen layout. This runs before loadContent and can
    // be dropped by the window manager; LxAppContainer re-asserts it once
    // content is up, so here we only log the failure.
    windowStage.getMainWindowSync().setWindowLayoutFullScreen(true).catch((error: Error) => {
      hilog.warn(DOMAIN, TAG, `setWindowLayoutFullScreen failed: ${error}`);
    });
    initNativeBridge();
  }

  /**
   * Bootstrap the native runtime: register the native bridge, custom schemes,
   * WebEngine, then call into Rust to discover the home LxApp and host
   * capabilities. Idempotent — a second call after the home app is resolved is
   * a no-op.
   */
  public static initializeRuntime(context?: common.UIAbilityContext, windowStage?: window.WindowStage): void {
    if (LxApp.getInstance().getHomeAppId() !== null) {
      hilog.info(DOMAIN, TAG, 'Lingxia already successfully initialized');
      return;
    }

    hilog.info(DOMAIN, TAG, `Lingxia.initializeRuntime called with context: ${context ? 'provided' : 'null'}, windowStage: ${windowStage ? 'provided' : 'null'}`);

    // Store context and windowStage for window/host operations. quickStart
    // may already have staged them; only take what is newly provided.
    if (context && context !== Lingxia.context) {
      Lingxia.context = context;
      PermissionManager.initialize(context);
    }
    context = context ?? Lingxia.context ?? undefined;
    if (!context) {
      LxLog.e(TAG, 'initializeRuntime requires a context (directly or via quickStart)');
      return;
    }
    if (windowStage && windowStage !== Lingxia.windowStage) {
      Lingxia.windowStage = windowStage;
      const mainWindow = windowStage.getMainWindowSync();
      mainWindow.setWindowLayoutFullScreen(true).catch((error: Error) => {
        hilog.warn(DOMAIN, TAG, `setWindowLayoutFullScreen failed: ${error}`);
      });
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
      LxLog.e(TAG, 'Failed to register custom schemes');
      return;
    }

    // Initialize WebEngine after schemes are registered
    webview.WebviewController.initializeWebEngine();

    hilog.info(DOMAIN, TAG, `Calling lingxiaInit with callback and file system paths (FIRST AND ONLY TIME)`);

    const locale = Lingxia.getSystemLocale();

    const hostAppearanceDark = context?.config?.colorMode ===
      ConfigurationConstant.ColorMode.COLOR_MODE_DARK;
    const appId: string | null = lingxiaInit(
      callbackFunction,
      dataDir,
      cacheDir,
      resourceManager,
      locale,
      hostAppearanceDark
    );

    if (appId) {
      hilog.info(DOMAIN, TAG, `Lingxia initialized successfully with home app: ${appId}`);
      LxApp.setHomeAppId(appId);
      Lingxia.capabilities = getAppCapabilities();
      if (shouldEnableWebViewDebugging()) {
        webview.WebviewController.setWebDebuggingAccess(true);
      }
      Lingxia.observeHostAppearance(appContext);
    } else {
      LxLog.e(TAG, 'Failed to get home app details from native init.');
    }

    if ((Lingxia.capabilities & Lingxia.CAP_NOTIFICATIONS) !== 0) {
      try {
        LxAppPush.requestEnableNotification();
        LxAppPush.initToken();
      } catch (e) {
        LxLog.e(TAG, `Push init error: ${e}`);
      }
    }
  }

  /**
   * Persist the appearance the app draws in, so the *next* launch resolves the
   * same one everywhere it matters: the system-composed start window, the
   * launch face this SDK draws over it, and every resource in between.
   *
   * `auto` clears the override rather than storing today's answer — the launch
   * after it must follow the system as it is then, not as it was here.
   */
  public static setHostColorMode(mode: string): void {
    const context = Lingxia.context;
    if (!context) {
      return;
    }
    let colorMode: ConfigurationConstant.ColorMode = ConfigurationConstant.ColorMode.COLOR_MODE_NOT_SET;
    if (mode === 'dark') {
      colorMode = ConfigurationConstant.ColorMode.COLOR_MODE_DARK;
    } else if (mode === 'light') {
      colorMode = ConfigurationConstant.ColorMode.COLOR_MODE_LIGHT;
    }
    try {
      context.getApplicationContext().setColorMode(colorMode);
    } catch (error) {
      LxLog.e(TAG, `setColorMode(${mode}) failed: ${error}`);
    }
  }

  private static observeHostAppearance(appContext: common.ApplicationContext): void {
    if (Lingxia.environmentCallback !== null) {
      return;
    }
    const callback: EnvironmentCallback = {
      onConfigurationUpdated: (config: Configuration): void => {
        onHostLocaleChanged(Lingxia.getSystemLocale());
        const dark = config.colorMode === ConfigurationConstant.ColorMode.COLOR_MODE_DARK;
        onHostAppearanceChanged(dark);
      },
      onMemoryLevel: (_level: number): void => {}
    };
    Lingxia.environmentCallback = callback;
    appContext.on('environment', callback);
  }

  private static getSystemLocale(): string {
    const systemLanguage = i18n.System.getSystemLanguage();
    const systemRegion = i18n.System.getSystemRegion();
    // HarmonyOS may include a script (for example zh-Hans); the runtime
    // locale contract uses the primary language plus region.
    const normalizedLanguage = systemLanguage.split('-')[0];
    return `${normalizedLanguage}-${systemRegion}`;
  }

  /**
   * Apply the per-page preferred orientation to the host window. Window/display
   * state is host-scope; the page path that drives the choice comes from the
   * active LxApp.
   */
  public static updateOrientationUI(appId: string, pathOverride?: string): boolean {
    if (!Lingxia.windowStage) {
      hilog.warn(DOMAIN, TAG, `updateOrientationUI skipped: windowStage unavailable (${appId})`);
      return false;
    }

    let path: string = '';
    if (pathOverride && pathOverride.length > 0) {
      path = pathOverride;
    } else {
      try {
        const current: CurrentLxApp = getCurrentLxapp();
        if (current && current.appid === appId) {
          path = current.path;
        }
      } catch (error) {
        hilog.warn(DOMAIN, TAG, `updateOrientationUI getCurrentLxapp failed: ${error}`);
      }
    }

    const orientation: number = getPageOrientation(appId, path);
    let targetOrientation: window.Orientation = window.Orientation.UNSPECIFIED;
    if (orientation === 1 || orientation === 3) {
      targetOrientation = window.Orientation.PORTRAIT;
    } else if (orientation === 2 || orientation === 4) {
      targetOrientation = window.Orientation.LANDSCAPE;
    }

    try {
      const mainWindow = Lingxia.windowStage.getMainWindowSync();
      if (!mainWindow) {
        hilog.warn(DOMAIN, TAG, `updateOrientationUI skipped: mainWindow unavailable (${appId})`);
        return false;
      }
      mainWindow.setPreferredOrientation(targetOrientation)
        .catch((error: Error) => {
          const message: string = error?.message ?? String(error);
          LxLog.e(TAG, `updateOrientationUI failed: ${message}`);
        });
      return true;
    } catch (error) {
      LxLog.e(TAG, `updateOrientationUI failed: ${error}`);
      return false;
    }
  }

  /**
   * Launch external URL with the system default application.
   * @param url The URL to open
   */
  public static launchWithUrl(url: string): void {
    if (!Lingxia.context) {
      LxLog.e(TAG, `Context not available for launching URL: ${url}`);
      return;
    }

    Lingxia.context.openLink(url)
      .then(() => {
        hilog.info(DOMAIN, TAG, `Successfully launched URL: ${url}`);
      })
      .catch((err: Error) => {
        LxLog.e(TAG, `Failed to launch URL: ${url}, error: ${JSON.stringify(err)}`);
      });
  }

  /**
   * Internal applink handler - called by SDK infrastructure.
   * @param url The full applink URL (e.g., "https://www.lingxia.app/lxapp/shop/pages/detail?id=42")
   */
  private static handleAppLink(url: string): void {
    try {
      // Call native layer to handle the applink
      const result: number = onApplinkReceived(url);
      hilog.info(DOMAIN, TAG, `AppLink processed with result: ${result}`);
    } catch (error) {
      LxLog.e(TAG, `Error processing AppLink: ${url}, error: ${JSON.stringify(error)}`);
    }
  }

  /**
   * Handle Want from UIAbility - call this in your UIAbility onCreate/onNewWant methods.
   * This provides a simple one-line integration for applink handling.
   * @param want The Want object containing the applink URI
   */
  public static handleWant(want: Want): void {
    try {
      if (!want) { return; }
      // Treat HTTPS Want.uri as push click (trigger=1), do not call AppLink
      if (want.uri && typeof want.uri === 'string' && want.uri.startsWith('https://')) {
        LxAppPush.handlePushLink(want.uri, 1);
        return;
      }
    } catch (e) {
      const err: Error = e as Error;
      LxLog.e(TAG, `handleWant error: ${JSON.stringify(err)}`);
    }
  }
}
