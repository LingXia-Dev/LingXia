import { hilog } from '@kit.PerformanceAnalysisKit';
import { webview } from '@kit.ArkWeb';
import { MiniApp } from './MiniApp';
import { createWebViewController, destroyWebViewController } from './WebView';

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
    // Core MiniApp functions
    this.register('openMiniApp', (appId: string, path: string) => {
      MiniApp.openMiniApp(appId, path);
      return true;
    });

    this.register('closeMiniApp', (appId: string) => {
      MiniApp.closeMiniApp(appId);
      return true;
    });

    this.register('switchToPage', (appId: string, path: string) => {
      return MiniApp.switchToPage(appId, path);
    });

    // WebView management
    this.register('createWebViewController', (appId: string, path: string) => {
      return createWebViewController(appId, path);
    });

    this.register('destroyWebViewController', (appId: string, path: string) => {
      return destroyWebViewController(appId, path);
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
 * Format: "function_name:arg1:arg2:..."
 */
export function getNativeCallbackFunction(): (data: string) => Object | null {
  return (data: string) => {
    const parts = data.split(':');
    return parts.length >= 3 ? NativeBridge.call(parts[0], parts[1], parts[2]) : null;
  };
}

/**
 * Initialize the native bridge
 * This should be called during MiniApp initialization
 */
export function initNativeBridge(): void {
  NativeBridge.getInstance();
}
