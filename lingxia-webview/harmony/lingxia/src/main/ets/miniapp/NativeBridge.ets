import { hilog } from '@kit.PerformanceAnalysisKit';
import { webview } from '@kit.ArkWeb';
import { MiniApp } from './MiniApp';

const DOMAIN = 0x0000;
const TAG = 'LingXia.NativeBridge';

/**
 * Native Bridge - Internal callback manager
 * Handles all native-to-ArkTS communication
 */
class NativeBridge {
  private static callbacks: Map<string, Function> = new Map();
  private static instance: NativeBridge | null = null;

  private constructor() {
    this.registerDefaultCallbacks();
  }

  public static getInstance(): NativeBridge {
    if (!NativeBridge.instance) {
      NativeBridge.instance = new NativeBridge();
    }
    return NativeBridge.instance;
  }

  /**
   * Register default callbacks that native layer can call
   */
  private registerDefaultCallbacks(): void {
    hilog.info(DOMAIN, TAG, 'Registering default native callbacks');

    // Core MiniApp functions
    this.register('openMiniApp', (appId: string, path: string) => {
      hilog.info(DOMAIN, TAG, `Native callback: openMiniApp(${appId}, ${path})`);
      MiniApp.openMiniApp(appId, path);
      return true;
    });

    this.register('closeMiniApp', (appId: string) => {
      hilog.info(DOMAIN, TAG, `Native callback: closeMiniApp(${appId})`);
      MiniApp.closeMiniApp(appId);
      return true;
    });

    this.register('switchToPage', (appId: string, path: string) => {
      hilog.info(DOMAIN, TAG, `Native callback: switchToPage(${appId}, ${path})`);
      return MiniApp.switchToPage(appId, path);
    });

    // WebView management
    this.register('createWebViewController', (key: string) => {
      hilog.info(DOMAIN, TAG, `Native callback: createWebViewController(${key})`);
      return this.createWebViewController(key);
    });

    hilog.info(DOMAIN, TAG, `Registered ${NativeBridge.callbacks.size} default callbacks`);
  }

  /**
   * Register a callback function
   */
  public register(name: string, callback: Function): void {
    NativeBridge.callbacks.set(name, callback);
    hilog.info(DOMAIN, TAG, `Registered callback: ${name}`);
  }

  /**
   * Call a registered callback function
   * This is the main entry point for native layer
   */
  public static call(name: string, ...args: Object[]): Object | null {
    const callback = NativeBridge.callbacks.get(name);
    if (callback) {
      try {
        hilog.info(DOMAIN, TAG, `Calling callback: ${name} with args: ${JSON.stringify(args)}`);
        return callback(...args);
      } catch (error) {
        hilog.error(DOMAIN, TAG, `Error calling callback ${name}: ${error}`);
        return null;
      }
    } else {
      hilog.warn(DOMAIN, TAG, `Callback not found: ${name}`);
      return null;
    }
  }

  /**
   * Get callback function for native layer
   * This returns the actual callback function that native can store and call
   */
  public static getCallbackFunction(): (name: string, ...args: Object[]) => Object | null {
    return NativeBridge.call;
  }

  /**
   * Create WebView controller for native layer
   * This creates a new controller, not finding existing one
   */
  private createWebViewController(key: string): webview.WebviewController | null {
    hilog.info(DOMAIN, TAG, `Creating new WebView controller for key: ${key}`);
    
    const parts = key.split(':', 2);
    if (parts.length !== 2) {
      hilog.error(DOMAIN, TAG, `Invalid key format: ${key}, expected "appId:path"`);
      return null;
    }

    try {
      const webviewController = new webview.WebviewController();
      hilog.info(DOMAIN, TAG, `Created new WebView controller for ${key}`);
      return webviewController;
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to create WebView controller for ${key}: ${error}`);
      return null;
    }
  }

}

/**
 * Global function for native layer to call ArkTS functions
 * This is the main entry point that Rust native code will call
 */
export function nativeCallback(functionName: string, ...args: Object[]): Object | null {
  return NativeBridge.call(functionName, ...args);
}

/**
 * Get the callback function for native layer
 * This should be called during native_miniapp_init to pass the callback to native
 */
export function getNativeCallbackFunction(): (name: string, ...args: Object[]) => Object | null {
  return NativeBridge.getCallbackFunction();
}

/**
 * Initialize the native bridge
 * This should be called during MiniApp initialization
 */
export function initNativeBridge(): void {
  NativeBridge.getInstance();
}
