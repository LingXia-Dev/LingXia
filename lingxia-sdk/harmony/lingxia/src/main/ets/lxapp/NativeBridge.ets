import { hilog } from '@kit.PerformanceAnalysisKit';
import { webview } from '@kit.ArkWeb';
import { LxApp } from './LxApp';
import {
  createWebViewController,
  destroyWebViewController,
  loadUrl,
  clearBrowsingData,
  setUserAgent,
  setScrollListenerEnabled
} from './WebView';

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
    // Core LxApp functions
    this.register('openLxApp', (appId: string, path: string) => {
      LxApp.openLxApp(appId, path);
      return true;
    });

    this.register('closeLxApp', (appId: string) => {
      LxApp.closeLxApp(appId);
      return true;
    });

    this.register('switchPage', (appId: string, path: string) => {
      return LxApp.switchToPage(appId, path);
    });

    // Launch external URL
    this.register('launchWithUrl', (url: string) => {
      LxApp.launchWithUrl(url);
      return true;
    });

    // WebView management
    this.register('createWebViewController', (webtag: string) => {
      return createWebViewController(webtag);
    });

    this.register('destroyWebViewController', (webtag: string) => {
      return destroyWebViewController(webtag);
    });

    // WebView URL loading
    this.register('loadUrl', (webtag: string, url: string) => {
      return loadUrl(webtag, url);
    });




    // WebView data management
    this.register('clearBrowsingData', (webtag: string) => {
      return clearBrowsingData(webtag);
    });

    // WebView user agent
    this.register('setUserAgent', (webtag: string, userAgent: string) => {
      return setUserAgent(webtag, userAgent);
    });

    // WebView scroll listener
    this.register('setScrollListenerEnabled', (webtag: string, enabled: boolean) => {
      return setScrollListenerEnabled(webtag, enabled);
    });

    hilog.info(DOMAIN, TAG, `Registered ${NativeBridge.callbacks.size} default callbacks`);
  }

  /**
   * Register a callback function
   */
  public register(name: string, callback: Function): void {
    NativeBridge.callbacks.set(name, callback);
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
}

/**
 * Get the callback function for native layer
 * Node-API ThreadSafe Function limitation: can only pass single string
 * Format: "function_name|arg1|arg2|arg3|..." (using | as separator to avoid conflicts with URLs)
 */
export function getNativeCallbackFunction(): (data: string) => Object | null {
  return (data: string) => {
    const parts = data.split('|');
    if (parts.length >= 1) {
      const functionName = parts[0];
      const rawArgs = parts.slice(1); // Get all arguments after function name

      // Convert arguments based on function type
      let args: Object[] = rawArgs;
      if (functionName === 'setScrollListenerEnabled' && rawArgs.length >= 2) {
        // Convert the second argument (enabled) from string to boolean
        args = [rawArgs[0], rawArgs[1] === 'true'];
      }

      return NativeBridge.call(functionName, ...args);
    }
    return null;
  };
}

/**
 * Initialize the native bridge
 * This should be called during LxApp initialization
 */
export function initNativeBridge(): void {
  NativeBridge.getInstance();
}
