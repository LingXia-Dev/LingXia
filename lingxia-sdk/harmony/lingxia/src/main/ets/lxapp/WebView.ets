import { hilog } from '@kit.PerformanceAnalysisKit';
import { webview } from '@kit.ArkWeb';
import { onWebviewControllerDestroyed } from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.WebView';

/**
 * WebView information for management
 */
export interface WebViewInfo {
  webtag: string;
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

  // Multiple callbacks for UI component management
  private static uiCallbacks: Set<(action: string, info: WebViewInfo) => void> = new Set();

  /**
   * Add UI callback for WebView management
   */
  static addUiCallback(callback: (action: string, info: WebViewInfo) => void) {
    WebViewManager.uiCallbacks.add(callback);
  }

  /**
   * Remove UI callback for WebView management
   */
  static removeUiCallback(callback: (action: string, info: WebViewInfo) => void) {
    WebViewManager.uiCallbacks.delete(callback);
  }

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

      // Notify all UI components to create component
      WebViewManager.uiCallbacks.forEach(callback => {
        try {
          callback('create', webViewInfo);
        } catch (error) {
          hilog.error(DOMAIN, TAG, `UI callback error: ${error}`);
        }
      });

      hilog.info(DOMAIN, TAG, `Created new WebView: ${webtag}`);
      return true;
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to create WebView ${webtag}: ${error}`);
      return false;
    }
  }

  /**
   * Find WebView by appId and path (only find, don't create)
   * @param appId - LxApp ID
   * @param path - Page path
   * @returns WebView controller or null if not found
   */
  static findWebview(appId: string, path: string): webview.WebviewController | null {
    const webtag = toWebTag(appId, path);
    const controller = WebViewManager.controllers.get(webtag);

    if (controller) {
      hilog.info(DOMAIN, TAG, `Found existing WebView: ${webtag}`);
      return controller;
    }

    hilog.info(DOMAIN, TAG, `WebView not found: ${webtag}`);
    return null;
  }

  /**
   * Destroy WebView by appId and path
   * Remove from map and release resources
   * @param webtag - WebView tag
   * @returns true if destroyed successfully, false if not found
   */
  static destroyWebview(webtag: string): boolean {
    const controller = WebViewManager.controllers.get(webtag);
    const webViewInfo = WebViewManager.webViewInfos.get(webtag);

    if (!controller || !webViewInfo) {
      hilog.warn(DOMAIN, TAG, `WebView not found for destroy: ${webtag}`);
      return false;
    }

    WebViewManager.controllers.delete(webtag);
    WebViewManager.webViewInfos.delete(webtag);
    WebViewManager.uiCallbacks.forEach(callback => {
      try {
        callback('destroy', webViewInfo);
      } catch (error) {
        hilog.error(DOMAIN, TAG, `UI callback error: ${error}`);
      }
    });

    onWebviewControllerDestroyed(webtag);
    hilog.info(DOMAIN, TAG, `WebViewManager.destroyWebview completed -> webtag=${webtag}`);
    return true;
  }

  /**
   * Load URL in WebView
   * @param webtag - WebView tag
   * @param url - URL to load
   * @returns true if URL loaded successfully, false if WebView not found or load failed
   */
  static loadUrl(webtag: string, url: string): boolean {
    const controller = WebViewManager.controllers.get(webtag);
    if (!controller) {
      hilog.warn(DOMAIN, TAG, `WebView not found for loadUrl: ${webtag}`);
      return false;
    }

    try {
      controller.loadUrl(url);
      hilog.info(DOMAIN, TAG, `Loaded URL in WebView ${webtag}: ${url}`);
      return true;
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to load URL in WebView ${webtag}: ${error}`);
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
    if (!controller) {
      hilog.warn(DOMAIN, TAG, `WebView not found for clearBrowsingData: ${webtag}`);
      return false;
    }

    try {
      controller.clearHistory();
      controller.removeCache(true);
      hilog.info(DOMAIN, TAG, `Cleared browsing data for WebView ${webtag}`);
      return true;
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to clear browsing data for WebView ${webtag}: ${error}`);
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
    if (!WebViewManager.controllers.has(webtag)) {
      hilog.warn(DOMAIN, TAG, `WebView not found for setUserAgent: ${webtag}`);
      return false;
    }

    hilog.info(DOMAIN, TAG, `Set user agent for WebView ${webtag}: ${userAgent} (not implemented)`);
    return true;
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
   * @param appId - LxApp ID
   * @param path - Page path
   * @param visible - Whether WebView should be visible
   */
  static setWebViewVisibility(appId: string, path: string, visible: boolean) {
    const webtag = toWebTag(appId, path);
    const webViewInfo = WebViewManager.webViewInfos.get(webtag);

    if (webViewInfo) {
      webViewInfo.visible = visible;
      WebViewManager.uiCallbacks.forEach(callback => {
        try {
          callback('visibility', webViewInfo);
        } catch (error) {
          hilog.error(DOMAIN, TAG, `UI callback error: ${error}`);
        }
      });
      hilog.info(DOMAIN, TAG, `Set WebView ${webtag} visibility: ${visible}`);
    }
  }

  /**
   * Notify UI that a WebView is being reused by the popup overlay.
   * When `active` is true the base container should hide the WebView; when false it should restore it.
   */
  static notifyPopupState(appId: string, path: string, active: boolean) {
    const webtag = toWebTag(appId, path);
    const webViewInfo = WebViewManager.webViewInfos.get(webtag);
    if (!webViewInfo) {
      hilog.warn(DOMAIN, TAG, `notifyPopupState: WebView not found for ${webtag}`);
      return;
    }

    WebViewManager.uiCallbacks.forEach(callback => {
      try {
        callback(active ? 'popup_show' : 'popup_hide', webViewInfo);
      } catch (error) {
        hilog.error(DOMAIN, TAG, `UI callback error: ${error}`);
      }
    });

    hilog.info(DOMAIN, TAG, `notifyPopupState -> ${active ? 'show' : 'hide'} for ${webtag}`);
  }

  /**
   * Get all WebView infos (for UI rendering)
   * @returns Array of WebView infos
   */
  static getAllWebViewInfos(): WebViewInfo[] {
    return Array.from(WebViewManager.webViewInfos.values());
  }

  /**
   * Enable WebView debugging globally
   * This affects all WebView instances created after this call
   */
  static enableDebugging(): void {
    webview.WebviewController.setWebDebuggingAccess(true);
    hilog.info(DOMAIN, TAG, 'WebView debugging enabled globally');
  }
}

/**
 * Create webtag from appId and path
 * @param appId - LxApp ID
 * @param path - Page path
 * @returns webtag in format "appId-path"
 */
export function toWebTag(appId: string, path: string): string {
  return `${appId}-${path}`;
}

/**
 * WebTag parts interface
 */
export interface WebTagParts {
  appId: string;
  path: string;
}

/**
 * Extract appId and path from webtag
 * @param webtag - WebView tag in format "appId-path"
 * @returns Object with appId and path, or null if invalid format
 */
export function extractWebTag(webtag: string): WebTagParts | null {
  const dashIndex = webtag.indexOf('-');
  if (dashIndex === -1) {
    return null;
  }

  const appId = webtag.substring(0, dashIndex);
  const path = webtag.substring(dashIndex + 1);
  return { appId: appId, path: path };
}

/**
 * Add UI callback for WebView management
 */
export function addWebViewUiCallback(callback: (action: string, info: WebViewInfo) => void) {
  WebViewManager.addUiCallback(callback);
}

/**
 * Remove UI callback for WebView management
 */
export function removeWebViewUiCallback(callback: (action: string, info: WebViewInfo) => void) {
  WebViewManager.removeUiCallback(callback);
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
 * @param appId - LxApp ID
 * @param path - Page path
 * @returns WebView controller or null if not found
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
  const result = WebViewManager.destroyWebview(webtag);
  return result;
}

/**
 * Get WebView info by webtag
 */
export function getWebViewInfo(webtag: string): WebViewInfo | null {
  return WebViewManager.getWebViewInfo(webtag);
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
