import { hilog } from '@kit.PerformanceAnalysisKit';
import { webview } from '@kit.ArkWeb';
import {
  onDownloadStart as hostOnDownloadStart,
  onNavigationPolicy as hostOnNavigationPolicy,
  onWebviewControllerCreated as hostOnWebviewControllerCreated,
  onWebviewControllerDestroyed as hostOnWebviewControllerDestroyed
} from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.WebView';

export interface EffectiveWebViewCreateOptions {
  profile: string;
  hasDownloadHandler: boolean;
}

export interface WebViewDownloadRequest {
  webtag: string;
  url: string;
  userAgent: string;
  contentDisposition: string;
  mimetype: string;
  contentLength: number;
}

export interface WebViewHostHooks {
  onControllerCreated?: (webtag: string) => void;
  onControllerDestroyed?: (webtag: string) => void;
  onNavigationPolicy?: (webtag: string, url: string) => boolean;
  onDownloadStart?: (request: WebViewDownloadRequest) => void;
}

interface RawOptionsToken {
  profile?: string;
  has_download_handler?: boolean;
}

const PROFILE_STRICT_DEFAULT = 'strict_default';
const PROFILE_BROWSER_RELAXED = 'browser_relaxed';

const STRICT_DEFAULT_OPTIONS: EffectiveWebViewCreateOptions = {
  profile: PROFILE_STRICT_DEFAULT,
  hasDownloadHandler: false
};

let hostHooks: WebViewHostHooks = {};
let defaultHostHooksBound = false;

function hasHostHooksConfigured(): boolean {
  return !!(
    hostHooks.onControllerCreated ||
    hostHooks.onControllerDestroyed ||
    hostHooks.onNavigationPolicy ||
    hostHooks.onDownloadStart
  );
}

function bindDefaultHostHooksIfNeeded(): void {
  if (defaultHostHooksBound || hasHostHooksConfigured()) {
    defaultHostHooksBound = true;
    return;
  }

  hostHooks = {
    onControllerCreated: (webtag: string): void => {
      hostOnWebviewControllerCreated(webtag);
    },
    onControllerDestroyed: (webtag: string): void => {
      hostOnWebviewControllerDestroyed(webtag);
    },
    onNavigationPolicy: (webtag: string, url: string): boolean => {
      return hostOnNavigationPolicy(webtag, url);
    },
    onDownloadStart: (request: WebViewDownloadRequest): void => {
      hostOnDownloadStart(
        request.webtag,
        request.url,
        request.userAgent,
        request.contentDisposition,
        request.mimetype,
        request.contentLength
      );
    }
  };
  defaultHostHooksBound = true;
}

function strictDefaultOptions(): EffectiveWebViewCreateOptions {
  return {
    profile: STRICT_DEFAULT_OPTIONS.profile,
    hasDownloadHandler: STRICT_DEFAULT_OPTIONS.hasDownloadHandler
  };
}

function decodeBase64Url(token: string): string {
  const normalized: string = token.replace(/-/g, '+').replace(/_/g, '/');
  const padLen: number = normalized.length % 4 === 0 ? 0 : (4 - (normalized.length % 4));
  const padded: string = normalized + '='.repeat(padLen);
  const chars: string = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
  let buffer: number = 0;
  let bits: number = 0;
  let output: string = '';

  for (let i = 0; i < padded.length; i++) {
    const ch: string = padded.charAt(i);
    if (ch === '=') {
      break;
    }
    const val: number = chars.indexOf(ch);
    if (val < 0) {
      throw new Error(`invalid base64 character: ${ch}`);
    }
    buffer = (buffer << 6) | val;
    bits += 6;
    if (bits >= 8) {
      bits -= 8;
      output += String.fromCharCode((buffer >> bits) & 0xff);
    }
  }

  return output;
}

function normalizeDecodedOptions(raw: Object): EffectiveWebViewCreateOptions {
  const data = raw as RawOptionsToken;
  const normalized: EffectiveWebViewCreateOptions = strictDefaultOptions();

  if (typeof data.profile === 'string') {
    const profile = data.profile.toLowerCase();
    if (profile === PROFILE_STRICT_DEFAULT || profile === PROFILE_BROWSER_RELAXED) {
      normalized.profile = profile;
    } else {
      hilog.warn(DOMAIN, TAG, `Invalid profile=${profile}, fallback to strict_default`);
    }
  }

  normalized.hasDownloadHandler = data.has_download_handler === true;

  return normalized;
}

function decodeOptionsToken(optionsToken?: string): EffectiveWebViewCreateOptions {
  if (!optionsToken || optionsToken.trim().length === 0) {
    return strictDefaultOptions();
  }

  try {
    const json = decodeBase64Url(optionsToken.trim());
    const parsed = JSON.parse(json) as Object;
    return normalizeDecodedOptions(parsed);
  } catch (error) {
    hilog.warn(DOMAIN, TAG, `Failed to decode options token, fallback strict: ${error}`);
    return strictDefaultOptions();
  }
}

function optionsEqual(a: EffectiveWebViewCreateOptions, b: EffectiveWebViewCreateOptions): boolean {
  return a.profile === b.profile && a.hasDownloadHandler === b.hasDownloadHandler;
}

function notifyControllerCreated(webtag: string): void {
  const callback = hostHooks.onControllerCreated;
  if (!callback) {
    return;
  }
  try {
    callback(webtag);
  } catch (error) {
    hilog.error(DOMAIN, TAG, `Host hook onControllerCreated failed for ${webtag}: ${error}`);
  }
}

function notifyControllerDestroyed(webtag: string): void {
  const callback = hostHooks.onControllerDestroyed;
  if (!callback) {
    return;
  }
  try {
    callback(webtag);
  } catch (error) {
    hilog.error(DOMAIN, TAG, `Host hook onControllerDestroyed failed for ${webtag}: ${error}`);
  }
}

/**
 * WebView information for management
 */
export interface WebViewInfo {
  webtag: string;
  controller: webview.WebviewController;
  visible: boolean;
  effectiveOptions: EffectiveWebViewCreateOptions;
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
  static createWebview(webtag: string, optionsToken?: string): boolean {
    // Normalize to logical tag for registry keys (appid-path)
    // Normalize to logical tag for registry keys (appid:path)
    const extracted = extractWebTag(webtag);
    const logicalTag = extracted ? toWebTag(extracted.appId, extracted.path) : webtag;
    const effectiveOptions = decodeOptionsToken(optionsToken);

    // Check if already exists for this logical page
    if (WebViewManager.controllers.has(logicalTag)) {
      const existingInfo = WebViewManager.webViewInfos.get(logicalTag);
      if (existingInfo && optionsEqual(existingInfo.effectiveOptions, effectiveOptions)) {
        hilog.info(DOMAIN, TAG, `Reuse existing WebView for logical tag: ${logicalTag}`);
        return true;
      }
      hilog.error(
        DOMAIN,
        TAG,
        `WebView options conflict for logical tag=${logicalTag}, reject recreate`
      );
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
        visible: false,
        effectiveOptions
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

  static getEffectiveOptions(webtag: string): EffectiveWebViewCreateOptions {
    const logicalTag = WebViewManager.resolveLogicalTag(webtag);
    const info = WebViewManager.webViewInfos.get(logicalTag);
    return info ? info.effectiveOptions : strictDefaultOptions();
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

    WebViewManager.controllers.delete(logicalTag);
    WebViewManager.webViewInfos.delete(logicalTag);
    WebViewManager.uiCallbacks.forEach(callback => {
      try {
        callback('destroy', webViewInfo);
      } catch (error) {
        hilog.error(DOMAIN, TAG, `UI callback error: ${error}`);
      }
    });

    notifyControllerDestroyed(webtag);
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
export function createWebViewController(webtag: string, optionsToken?: string): boolean {
  return WebViewManager.createWebview(webtag, optionsToken);
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

export function getWebViewCreateOptions(webtag: string): EffectiveWebViewCreateOptions {
  return WebViewManager.getEffectiveOptions(webtag);
}

export function isBrowserRelaxedProfile(webtag: string): boolean {
  return getWebViewCreateOptions(webtag).profile === PROFILE_BROWSER_RELAXED;
}

export function setWebViewHostHooks(hooks: WebViewHostHooks): void {
  hostHooks = {
    onControllerCreated: hooks.onControllerCreated,
    onControllerDestroyed: hooks.onControllerDestroyed,
    onNavigationPolicy: hooks.onNavigationPolicy,
    onDownloadStart: hooks.onDownloadStart
  };
  defaultHostHooksBound = true;
}

export function notifyWebViewControllerCreated(webtag: string): void {
  bindDefaultHostHooksIfNeeded();
  notifyControllerCreated(webtag);
}

export function handleWebViewNavigationPolicy(webtag: string, url: string): boolean {
  bindDefaultHostHooksIfNeeded();
  if (url.length === 0) {
    return false;
  }
  const callback = hostHooks.onNavigationPolicy;
  if (!callback) {
    return false;
  }
  try {
    return callback(webtag, url) === true;
  } catch (error) {
    hilog.error(DOMAIN, TAG, `Host hook onNavigationPolicy failed for ${webtag}: ${error}`);
    return false;
  }
}

export function handleWebViewDownloadStart(
  webtag: string,
  url: string,
  userAgent: string,
  contentDisposition: string,
  mimetype: string,
  contentLength: number
): void {
  bindDefaultHostHooksIfNeeded();
  const options = getWebViewCreateOptions(webtag);
  if (options.profile !== PROFILE_BROWSER_RELAXED || options.hasDownloadHandler !== true) {
    return;
  }

  const callback = hostHooks.onDownloadStart;
  if (!callback) {
    return;
  }

  const request: WebViewDownloadRequest = {
    webtag,
    url,
    userAgent,
    contentDisposition,
    mimetype,
    contentLength
  };

  try {
    callback(request);
  } catch (error) {
    hilog.error(DOMAIN, TAG, `Host hook onDownloadStart failed for ${webtag}: ${error}`);
  }
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
