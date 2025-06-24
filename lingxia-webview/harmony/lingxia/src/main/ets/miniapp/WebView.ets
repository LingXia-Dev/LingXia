import { hilog } from '@kit.PerformanceAnalysisKit';
import { webview } from '@kit.ArkWeb';
import { onPageShow, onScrollChanged } from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.WebView';

/**
 * WebView information for management
 */
interface WebViewInfo {
  webtag: string;
  controller: webview.WebviewController;
  visible: boolean;
  scrollListenerEnabled?: boolean;
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
   * Create WebView by webtag
   * @param webtag - WebView tag
   * @returns true if created successfully, false if already exists or creation failed
   */
  static createWebview(webtag: string): boolean {
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
        webtag,
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
    const webtag = toWebTag(appId, path);
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
   * @param webtag - WebView tag
   * @returns true if destroyed successfully, false if not found
   */
  static destroyWebview(webtag: string): boolean {
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
   * Load URL in WebView
   * @param webtag - WebView tag
   * @param url - URL to load
   * @returns true if URL loaded successfully, false if WebView not found or load failed
   */
  static loadUrl(webtag: string, url: string): boolean {
    const controller = WebViewManager.controllers.get(webtag);

    if (controller) {
      try {
        controller.loadUrl(url);
        hilog.info(DOMAIN, TAG, `Loaded URL in WebView ${webtag}: ${url}`);
        return true;
      } catch (error) {
        hilog.error(DOMAIN, TAG, `Failed to load URL in WebView ${webtag}: ${error}`);
        return false;
      }
    } else {
      hilog.warn(DOMAIN, TAG, `WebView not found for loadUrl: ${webtag}`);
      return false;
    }
  }

  /**
   * Set web debugging access for WebView
   * @param webtag - WebView tag
   * @param enable - Whether to enable web debugging
   * @returns true if setting applied successfully, false if WebView not found or setting failed
   */
  static setWebDebuggingAccess(webtag: string, enable: boolean): boolean {
    const controller = WebViewManager.controllers.get(webtag);

    if (controller) {
      try {
        webview.WebviewController.setWebDebuggingAccess(enable);
        hilog.info(DOMAIN, TAG, `Set web debugging access for WebView ${webtag}: ${enable}`);
        return true;
      } catch (error) {
        hilog.error(DOMAIN, TAG, `Failed to set web debugging access for WebView ${webtag}: ${error}`);
        return false;
      }
    } else {
      hilog.warn(DOMAIN, TAG, `WebView not found for setWebDebuggingAccess: ${webtag}`);
      return false;
    }
  }

  /**
   * Clear browsing data for WebView
   * @param webtag - WebView tag
   * @returns true if clearing succeeded, false if WebView not found or clearing failed
   */
  static clearBrowsingData(webtag: string): boolean {
    const controller = WebViewManager.controllers.get(webtag);

    if (controller) {
      try {
        controller.clearHistory();
        controller.removeCache(true);
        hilog.info(DOMAIN, TAG, `Cleared browsing data for WebView ${webtag}`);
        return true;
      } catch (error) {
        hilog.error(DOMAIN, TAG, `Failed to clear browsing data for WebView ${webtag}: ${error}`);
        return false;
      }
    } else {
      hilog.warn(DOMAIN, TAG, `WebView not found for clearBrowsingData: ${webtag}`);
      return false;
    }
  }

  /**
   * Set user agent for WebView
   * @param webtag - WebView tag
   * @param userAgent - User agent string
   * @returns true if setting succeeded, false if WebView not found or setting failed
   */
  static setUserAgent(webtag: string, userAgent: string): boolean {
    const controller = WebViewManager.controllers.get(webtag);

    if (controller) {
      try {
        // Note: HarmonyOS WebView may not support setUserAgent directly
        // This is a placeholder implementation
        hilog.info(DOMAIN, TAG, `Set user agent for WebView ${webtag}: ${userAgent} (not implemented)`);
        return true;
      } catch (error) {
        hilog.error(DOMAIN, TAG, `Failed to set user agent for WebView ${webtag}: ${error}`);
        return false;
      }
    } else {
      hilog.warn(DOMAIN, TAG, `WebView not found for setUserAgent: ${webtag}`);
      return false;
    }
  }

  /**
   * Set scroll listener enabled for WebView
   * @param webtag - WebView tag
   * @param enabled - Whether to enable scroll listener
   * @returns true if setting succeeded, false if WebView not found or setting failed
   */
  static setScrollListenerEnabled(webtag: string, enabled: boolean): boolean {
    const controller = WebViewManager.controllers.get(webtag);

    if (controller) {
      try {
        // Note: HarmonyOS WebView scroll events are typically handled through onScroll callback
        // This method stores the scroll listener state for the WebView
        const webViewInfo = WebViewManager.webViewInfos.get(webtag);
        if (webViewInfo) {
          // Store scroll listener state in webViewInfo for later use
          webViewInfo.scrollListenerEnabled = enabled;
          hilog.info(DOMAIN, TAG, `Set scroll listener enabled for WebView ${webtag}: ${enabled}`);
          return true;
        } else {
          hilog.warn(DOMAIN, TAG, `WebView info not found for setScrollListenerEnabled: ${webtag}`);
          return false;
        }
      } catch (error) {
        hilog.error(DOMAIN, TAG, `Failed to set scroll listener enabled for WebView ${webtag}: ${error}`);
        return false;
      }
    } else {
      hilog.warn(DOMAIN, TAG, `WebView not found for setScrollListenerEnabled: ${webtag}`);
      return false;
    }
  }

  /**
   * Handle scroll change event for WebView
   * This should be called from the UI component when scroll occurs
   * @param webtag - WebView tag
   * @param scrollX - Horizontal scroll position
   * @param scrollY - Vertical scroll position
   */
  static handleScrollChanged(webtag: string, scrollX: number, scrollY: number): void {
    const webViewInfo = WebViewManager.webViewInfos.get(webtag);

    if (webViewInfo && webViewInfo.scrollListenerEnabled) {
      try {
        // Parse appId and path from webtag (format: appId-path)
        const parts = webtag.split('-');
        if (parts.length >= 2) {
          const appId = parts[0];
          const path = parts.slice(1).join('-'); // In case path contains '-'

          // Call native function to notify scroll change
          const result: number = onScrollChanged(appId, path, scrollX, scrollY);
          if (result === 0) {
            hilog.debug(DOMAIN, TAG, `Scroll change notified for ${webtag}: (${scrollX}, ${scrollY})`);
          } else {
            hilog.warn(DOMAIN, TAG, `Failed to notify scroll change for ${webtag}, result: ${result}`);
          }
        } else {
          hilog.error(DOMAIN, TAG, `Invalid webtag format for scroll change: ${webtag}`);
        }
      } catch (error) {
        hilog.error(DOMAIN, TAG, `Error handling scroll change for ${webtag}: ${error}`);
      }
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
    const webtag = toWebTag(appId, path);
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
 * Create webtag from appId and path
 * @param appId - MiniApp ID
 * @param path - Page path
 * @returns webtag in format "appId-path"
 */
export function toWebTag(appId: string, path: string): string {
  return `${appId}-${path}`;
}

/**
 * Create WebView controller
 * @param webtag - WebView tag
 * @returns true if created successfully, false otherwise
 */
export function createWebViewController(webtag: string): boolean {
  return WebViewManager.createWebview(webtag);
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
 * @param webtag - WebView tag
 * @returns true if destroyed successfully
 */
export function destroyWebViewController(webtag: string): boolean {
  return WebViewManager.destroyWebview(webtag);
}

/**
 * Load URL in WebView
 * @param webtag - WebView tag
 * @param url - URL to load
 * @returns true if URL loaded successfully
 */
export function loadUrl(webtag: string, url: string): boolean {
  return WebViewManager.loadUrl(webtag, url);
}

/**
 * Set web debugging access for WebView
 * @param webtag - WebView tag
 * @param enable - Whether to enable web debugging
 * @returns true if setting applied successfully
 */
export function setWebDebuggingAccess(webtag: string, enable: boolean): boolean {
  return WebViewManager.setWebDebuggingAccess(webtag, enable);
}

/**
 * Clear browsing data for WebView
 * @param webtag - WebView tag
 * @returns true if clearing succeeded
 */
export function clearBrowsingData(webtag: string): boolean {
  return WebViewManager.clearBrowsingData(webtag);
}

/**
 * Set user agent for WebView
 * @param webtag - WebView tag
 * @param userAgent - User agent string
 * @returns true if setting succeeded
 */
export function setUserAgent(webtag: string, userAgent: string): boolean {
  return WebViewManager.setUserAgent(webtag, userAgent);
}

/**
 * Set scroll listener enabled for WebView
 * @param webtag - WebView tag
 * @param enabled - Whether to enable scroll listener
 * @returns true if setting succeeded
 */
export function setScrollListenerEnabled(webtag: string, enabled: boolean): boolean {
  return WebViewManager.setScrollListenerEnabled(webtag, enabled);
}

/**
 * Handle scroll change event for WebView
 * This should be called from UI components when scroll occurs
 * @param webtag - WebView tag
 * @param scrollX - Horizontal scroll position
 * @param scrollY - Vertical scroll position
 */
export function handleScrollChanged(webtag: string, scrollX: number, scrollY: number): void {
  return WebViewManager.handleScrollChanged(webtag, scrollX, scrollY);
}
