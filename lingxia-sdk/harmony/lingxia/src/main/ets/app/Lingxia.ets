import { window } from '@kit.ArkUI';
import { webview } from '@kit.ArkWeb';
import { i18n } from '@kit.LocalizationKit';
import { Want } from '@kit.AbilityKit';
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
  shouldEnableWebViewDebugging,
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

  // Host UIAbility context + window stage, registered once during
  // initializeRuntime. Shared with LxApp-scope APIs that need a host context
  // (file/share/scan/media/push) — lxapp code reads these, never owns them.
  public static context: common.UIAbilityContext | null = null;
  public static windowStage: window.WindowStage | null = null;

  /**
   * Product-app entry point. Initializes the runtime for use with `LxAppNavigation`.
   */
  public static quickStart(context: common.UIAbilityContext, windowStage: window.WindowStage): void {
    Lingxia.initializeRuntime(context, windowStage);
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

    // Store context and windowStage for window/host operations
    if (context) {
      Lingxia.context = context;
      PermissionManager.initialize(context);
    } else {
      hilog.warn(DOMAIN, TAG, 'No context provided, skipping permission request');
    }
    if (windowStage) {
      Lingxia.windowStage = windowStage;
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
      LxLog.e(TAG, 'Failed to register custom schemes');
      return;
    }

    // Initialize WebEngine after schemes are registered
    webview.WebviewController.initializeWebEngine();

    hilog.info(DOMAIN, TAG, `Calling lingxiaInit with callback and file system paths (FIRST AND ONLY TIME)`);

    // Get system locale using HarmonyOS i18n API
    const systemLanguage = i18n.System.getSystemLanguage();
    const systemRegion = i18n.System.getSystemRegion();

    // Convert HarmonyOS locale format to standard format
    // HarmonyOS returns "zh-Hans" but we want "zh"
    const normalizedLanguage = systemLanguage.split('-')[0];
    const locale = `${normalizedLanguage}-${systemRegion}`;

    const appId: string | null = lingxiaInit(callbackFunction, dataDir, cacheDir, resourceManager, locale);

    if (appId) {
      hilog.info(DOMAIN, TAG, `Lingxia initialized successfully with home app: ${appId}`);
      LxApp.setHomeAppId(appId);
      Lingxia.capabilities = getAppCapabilities();
      if (shouldEnableWebViewDebugging()) {
        webview.WebviewController.setWebDebuggingAccess(true);
      }
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
