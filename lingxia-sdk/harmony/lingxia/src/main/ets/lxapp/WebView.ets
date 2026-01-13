import { hilog } from '@kit.PerformanceAnalysisKit';
import { webview } from '@kit.ArkWeb';
import { onWebviewControllerDestroyed } from 'liblingxia.so';
import {
  getOrCreateEmbedManager,
  getEmbedManager,
  teardownEmbedManager,
  setEmbedController,
  pauseAllComponents,
  resumeAllComponents
} from './NativeComponents/NativeBridgeInternal';
import { NativeComponentEmbedManager } from './NativeComponents/NativeComponentEmbed';

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

  private static resolveLogicalTag(webtag: string): string {
    const extracted = extractWebTag(webtag);
    return extracted ? toWebTag(extracted.appId, extracted.path) : webtag;
  }

  /**
   * Get controller for a logical tag
   */
  static getController(logicalTag: string): webview.WebviewController | undefined {
    return WebViewManager.controllers.get(logicalTag);
  }

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
    // Normalize to logical tag for registry keys (appid-path)
    // Normalize to logical tag for registry keys (appid:path)
    const extracted = extractWebTag(webtag);
    const logicalTag = extracted ? toWebTag(extracted.appId, extracted.path) : webtag;

    // Check if already exists for this logical page
    if (WebViewManager.controllers.has(logicalTag)) {
      hilog.warn(DOMAIN, TAG, `WebView already exists for logical tag: ${logicalTag}`);
      return false;
    }

    // Create new WebView
    try {
      const controller = new webview.WebviewController(webtag);

      // Store in map
      WebViewManager.controllers.set(logicalTag, controller);

      // Create WebView info
      const webViewInfo: WebViewInfo = {
        webtag,
        controller,
        visible: false
      };
      WebViewManager.webViewInfos.set(logicalTag, webViewInfo);

      // Notify all UI components to create component
      WebViewManager.uiCallbacks.forEach(callback => {
        try {
          callback('create', webViewInfo);
        } catch (error) {
          hilog.error(DOMAIN, TAG, `UI callback error: ${error}`);
        }
      });

      hilog.info(DOMAIN, TAG, `Created new WebView: ${webtag} (logical: ${logicalTag})`);
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
    const logicalTag = toWebTag(appId, path); // appId:path

    // Prefer exact match first
    const direct = WebViewManager.controllers.get(logicalTag);
    if (direct) {
      hilog.info(DOMAIN, TAG, `Found existing WebView (direct): ${logicalTag}`);
      return direct;
    }

    // Fallback: match any controller whose tag shares the logical prefix
    const entries = WebViewManager.controllers.entries();
    for (let entry of entries) {
      const tag = entry[0];
      const controller = entry[1];
      if (tag === logicalTag || tag.startsWith(`${logicalTag}#`)) {
        hilog.info(DOMAIN, TAG, `Found existing WebView (instance): ${tag} for logical ${logicalTag}`);
        return controller;
      }
    }

    hilog.info(DOMAIN, TAG, `WebView not found for logical tag: ${logicalTag}`);
    return null;
  }

  /**
   * Destroy WebView by appId and path
   * Remove from map and release resources
   * @param webtag - WebView tag
   * @returns true if destroyed successfully, false if not found
   */
  static destroyWebview(webtag: string): boolean {
    // Normalize to logical tag for registry keys (appid:path)
    const extracted = extractWebTag(webtag);
    const logicalTag = extracted ? toWebTag(extracted.appId, extracted.path) : webtag;

    const controller = WebViewManager.controllers.get(logicalTag);
    const webViewInfo = WebViewManager.webViewInfos.get(logicalTag);

    if (!controller || !webViewInfo) {
      hilog.warn(DOMAIN, TAG, `WebView not found for destroy: ${webtag} (logical: ${logicalTag})`);
      return false;
    }

    // Teardown same-level native components before destroying WebView
    teardownEmbedManager(logicalTag);

    WebViewManager.controllers.delete(logicalTag);
    WebViewManager.webViewInfos.delete(logicalTag);
    WebViewManager.uiCallbacks.forEach(callback => {
      try {
        callback('destroy', webViewInfo);
      } catch (error) {
        hilog.error(DOMAIN, TAG, `UI callback error: ${error}`);
      }
    });

    // Notify Rust with the full tag used when the controller was created
    onWebviewControllerDestroyed(webtag);
    hilog.info(DOMAIN, TAG, `WebViewManager.destroyWebview completed -> webtag=${webtag} (logical: ${logicalTag})`);
    return true;
  }

  /**
   * Load URL in WebView
   * @param webtag - WebView tag
   * @param url - URL to load
   * @returns true if URL loaded successfully, false if WebView not found or load failed
   */
  static loadUrl(webtag: string, url: string): boolean {
    const logicalTag = WebViewManager.resolveLogicalTag(webtag);
    const controller = WebViewManager.controllers.get(logicalTag);
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
    const logicalTag = WebViewManager.resolveLogicalTag(webtag);
    const controller = WebViewManager.controllers.get(logicalTag);
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
    const logicalTag = WebViewManager.resolveLogicalTag(webtag);
    if (!WebViewManager.controllers.has(logicalTag)) {
      hilog.warn(DOMAIN, TAG, `WebView not found for setUserAgent: ${webtag}`);
      return false;
    }

    hilog.info(DOMAIN, TAG, `Set user agent for WebView ${webtag}: ${userAgent} (not implemented)`);
    return true;
  }

  /**
   * Get WebView info by webtag (internal use)
   * @param webtag - WebView tag
   * @returns WebView info or null
   */
  static getWebViewInfo(webtag: string): WebViewInfo | null {
    const logicalTag = WebViewManager.resolveLogicalTag(webtag);
    return WebViewManager.webViewInfos.get(logicalTag) || null;
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

      // Pause/resume same-level native components on visibility change
      if (visible) {
        resumeAllComponents(webtag);
      } else {
        pauseAllComponents(webtag);
      }

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
   * Pause all same-level native components for all WebViews of an appId
   * Call this when lxapp goes to background (e.g., closeLxApp)
   */
  static pauseAllComponentsByAppId(appId: string): void {
    hilog.info(DOMAIN, TAG, `pauseAllComponentsByAppId: ${appId}`);
    WebViewManager.webViewInfos.forEach((info: WebViewInfo, webtag: string) => {
      const extracted = extractWebTag(webtag);
      if (extracted && extracted.appId === appId) {
        hilog.info(DOMAIN, TAG, `Pausing components for webtag: ${webtag}`);
        pauseAllComponents(webtag);
      }
    });
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
 * @returns webtag in format "appId:path" or "appId:path#sessionId"
 */
export function toWebTag(appId: string, path: string, sessionId?: string | number): string {
  const base = `${appId}:${path}`;
  if (sessionId === undefined || sessionId === null) {
    return base;
  }
  return `${base}#${sessionId.toString()}`;
}

/**
 * WebTag parts interface
 */
export interface WebTagParts {
  appId: string;
  path: string;
  sessionId?: string;
}

/**
 * Extract appId and path from webtag
 * @param webtag - WebView tag in format "appId:path" or "appId:path#instance"
 * @returns Object with appId, path, and optional sessionId, or null if invalid format
 */
export function extractWebTag(webtag: string): WebTagParts | null {
  const separatorIndex = webtag.indexOf(':');
  if (separatorIndex === -1) {
    return null;
  }

  const appId = webtag.substring(0, separatorIndex);
  let pathWithInstance = webtag.substring(separatorIndex + 1);
  const hashIndex = pathWithInstance.indexOf('#');
  const path = hashIndex === -1 ? pathWithInstance : pathWithInstance.substring(0, hashIndex);
  const sessionId = hashIndex === -1 ? undefined : pathWithInstance.substring(hashIndex + 1) || undefined;
  return { appId: appId, path: path, sessionId: sessionId };
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
 * Get all WebView infos (for UI containers that need to replay existing instances)
 * @returns Array of WebView infos
 */
export function getAllWebViewInfos(): WebViewInfo[] {
  return WebViewManager.getAllWebViewInfos();
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

/**
 * Get or create EmbedManager for a WebView
 * Call this to enable same-level rendering for a WebView
 * @param webtag - WebView tag
 * @returns NativeComponentEmbedManager instance
 */
export function getWebViewEmbedManager(webtag: string): NativeComponentEmbedManager {
  const extracted = extractWebTag(webtag);
  const logicalTag = extracted ? toWebTag(extracted.appId, extracted.path) : webtag;
  const manager = getOrCreateEmbedManager(logicalTag);

  // Set controller for event communication
  const controller = WebViewManager.getController(logicalTag);
  if (controller) {
    setEmbedController(logicalTag, controller);
  }

  return manager;
}

/**
 * Pause all same-level native components for a WebView
 * Call this on page navigate away
 * @param webtag - WebView tag
 */
export function pauseWebViewComponents(webtag: string): void {
  const extracted = extractWebTag(webtag);
  const logicalTag = extracted ? toWebTag(extracted.appId, extracted.path) : webtag;
  pauseAllComponents(logicalTag);
}

/**
 * Resume all same-level native components for a WebView
 * Call this on page navigate back
 * @param webtag - WebView tag
 */
export function resumeWebViewComponents(webtag: string): void {
  const extracted = extractWebTag(webtag);
  const logicalTag = extracted ? toWebTag(extracted.appId, extracted.path) : webtag;
  resumeAllComponents(logicalTag);
}

/**
 * Pause all same-level native components for all WebViews of an appId
 * Call this when lxapp goes to background (e.g., closeLxApp)
 * @param appId - App ID
 */
export function pauseAllComponentsByAppId(appId: string): void {
  WebViewManager.pauseAllComponentsByAppId(appId);
}
