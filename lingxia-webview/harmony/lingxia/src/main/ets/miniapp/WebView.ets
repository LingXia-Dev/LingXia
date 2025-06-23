import { hilog } from '@kit.PerformanceAnalysisKit';
import { webview } from '@kit.ArkWeb';
import { onPageShow } from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.WebView';

/**
 * WebView information for management
 */
interface WebViewInfo {
  appId: string;
  path: string;
  controller: webview.WebviewController;
  visible: boolean;
}

/**
 * WebView Manager - handles WebView lifecycle management
 * Provides create, find and destroy functionality for WebViews
 */
export class WebViewManager {
  // Map to store WebView controllers by appId:path
  private static controllers: Map<string, webview.WebviewController> = new Map();

  // Map to store WebView info for UI management
  private static webViewInfos: Map<string, WebViewInfo> = new Map();

  // Callback for UI component creation/destruction (if needed)
  private static uiCallback: ((action: string, info: WebViewInfo) => void) | null = null;

  /**
   * Create WebView by appId and path (called by Rust)
   * @param appId - MiniApp ID
   * @param path - Page path
   * @returns true if created successfully, false if already exists or creation failed
   */
  static createWebview(appId: string, path: string): boolean {
    const webtag = `${appId}:${path}`;

    // Check if already exists
    if (WebViewManager.controllers.has(webtag)) {
      hilog.warn(DOMAIN, TAG, `WebView already exists: ${webtag}`);
      return false;
    }

    // Create new WebView
    try {
      const controller = new webview.WebviewController(webtag);

      // Store in map
      WebViewManager.controllers.set(webtag, controller);

      // Create WebView info
      const webViewInfo: WebViewInfo = {
        appId,
        path,
        controller,
        visible: false
      };
      WebViewManager.webViewInfos.set(webtag, webViewInfo);

      // Notify UI to create component
      if (WebViewManager.uiCallback) {
        WebViewManager.uiCallback('create', webViewInfo);
      }

      hilog.info(DOMAIN, TAG, `Created new WebView: ${webtag}`);
      return true;
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to create WebView ${webtag}: ${error}`);
      return false;
    }
  }

  /**
   * Find WebView by appId and path (only find, don't create)
   * @param appId - MiniApp ID
   * @param path - Page path
   * @returns WebView controller or null if not found
   */
  static findWebview(appId: string, path: string): webview.WebviewController | null {
    const webtag = `${appId}:${path}`;
    const controller = WebViewManager.controllers.get(webtag);

    if (controller) {
      hilog.info(DOMAIN, TAG, `Found WebView: ${webtag}`);
      return controller;
    } else {
      hilog.info(DOMAIN, TAG, `WebView not found: ${webtag}`);
      return null;
    }
  }

  /**
   * Destroy WebView by appId and path
   * Remove from map and release resources
   * @param appId - MiniApp ID
   * @param path - Page path
   * @returns true if destroyed successfully, false if not found
   */
  static destroyWebview(appId: string, path: string): boolean {
    const webtag = `${appId}:${path}`;
    hilog.info(DOMAIN, TAG, `Destroying WebView: ${webtag}`);

    const controller = WebViewManager.controllers.get(webtag);
    const webViewInfo = WebViewManager.webViewInfos.get(webtag);

    if (controller && webViewInfo) {
      try {
        // Remove from maps
        WebViewManager.controllers.delete(webtag);
        WebViewManager.webViewInfos.delete(webtag);

        // Notify UI to remove component
        if (WebViewManager.uiCallback) {
          WebViewManager.uiCallback('destroy', webViewInfo);
        }

        // Release WebView resources if API supports it
        // controller.destroy(); // Uncomment if HarmonyOS provides destroy method

        hilog.info(DOMAIN, TAG, `Successfully destroyed WebView: ${webtag}`);
        return true;
      } catch (error) {
        hilog.error(DOMAIN, TAG, `Failed to destroy WebView ${webtag}: ${error}`);
        return false;
      }
    } else {
      hilog.warn(DOMAIN, TAG, `WebView not found for destroy: ${webtag}`);
      return false;
    }
  }

  /**
   * Get WebView controller by webtag (internal use)
   * @param webtag - WebView tag
   * @returns WebView controller or null
   */
  static getController(webtag: string): webview.WebviewController | null {
    return WebViewManager.controllers.get(webtag) || null;
  }

  /**
   * Get WebView info by webtag (internal use)
   * @param webtag - WebView tag
   * @returns WebView info or null
   */
  static getWebViewInfo(webtag: string): WebViewInfo | null {
    return WebViewManager.webViewInfos.get(webtag) || null;
  }

  /**
   * Set WebView visibility
   * @param appId - MiniApp ID
   * @param path - Page path
   * @param visible - Whether WebView should be visible
   */
  static setWebViewVisibility(appId: string, path: string, visible: boolean) {
    const webtag = `${appId}:${path}`;
    const webViewInfo = WebViewManager.webViewInfos.get(webtag);

    if (webViewInfo) {
      webViewInfo.visible = visible;
      if (WebViewManager.uiCallback) {
        WebViewManager.uiCallback('visibility', webViewInfo);
      }
      hilog.info(DOMAIN, TAG, `Set WebView ${webtag} visibility: ${visible}`);
    }
  }

  /**
   * Get all WebView infos (for UI rendering)
   * @returns Array of WebView infos
   */
  static getAllWebViewInfos(): WebViewInfo[] {
    return Array.from(WebViewManager.webViewInfos.values());
  }
}

/**
 * Export functions for external use
 */

/**
 * Create WebView controller (called by Rust)
 * @param appId - MiniApp ID
 * @param path - Page path
 * @returns true if created successfully, false otherwise
 */
export function createWebViewController(appId: string, path: string): boolean {
  return WebViewManager.createWebview(appId, path);
}

/**
 * Find WebView controller (only find, don't create)
 * @param appId - MiniApp ID
 * @param path - Page path
 * @returns WebView controller or null
 */
export function findWebview(appId: string, path: string): webview.WebviewController | null {
  return WebViewManager.findWebview(appId, path);
}

/**
 * Destroy WebView controller
 * @param appId - MiniApp ID
 * @param path - Page path
 * @returns true if destroyed successfully
 */
export function destroyWebViewController(appId: string, path: string): boolean {
  return WebViewManager.destroyWebview(appId, path);
}
