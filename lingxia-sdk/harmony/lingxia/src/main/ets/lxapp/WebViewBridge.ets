import { hilog } from '@kit.PerformanceAnalysisKit';
import { webview } from '@kit.ArkWeb';
import {
  handleWebViewDownloadStart as coreHandleWebViewDownloadStart,
  handleWebViewLoadError as coreHandleWebViewLoadError,
  handleWebViewNavigationPolicy as coreHandleWebViewNavigationPolicy,
  notifyWebViewControllerCreated as coreNotifyWebViewControllerAttached,
  WebViewInfo,
  WebTagParts,
  WebViewManager as CoreWebViewManager,
  extractWebTag,
  isBrowserRelaxedProfile as coreIsBrowserRelaxedProfile,
  toWebTag
} from './WebViewCore';
import {
  getOrCreateEmbedManager,
  setEmbedController,
  teardownEmbedManager,
  pauseAllComponents,
  resumeAllComponents
} from './NativeComponents/NativeBridgeInternal';
import { NativeComponentEmbedManager } from './NativeComponents/NativeComponentEmbed';

const DOMAIN = 0x0000;
const TAG = 'LingXia.WebView';

export { WebViewInfo, WebTagParts, extractWebTag, toWebTag };

/**
 * SDK-side WebView manager adapter.
 * Core lifecycle/registry lives in WebViewCore (generated from lingxia-webview),
 * while embed manager wiring remains in lxapp SDK.
 */
export class WebViewManager {
  static getController(logicalTag: string): webview.WebviewController | undefined {
    return CoreWebViewManager.getController(logicalTag);
  }

  static addUiCallback(callback: (action: string, info: WebViewInfo) => void): void {
    CoreWebViewManager.addUiCallback(callback);
  }

  static removeUiCallback(callback: (action: string, info: WebViewInfo) => void): void {
    CoreWebViewManager.removeUiCallback(callback);
  }

  static createWebview(webtag: string, optionsToken?: string): boolean {
    return CoreWebViewManager.createWebview(webtag, optionsToken);
  }

  static findWebview(appId: string, path: string): webview.WebviewController | null {
    return CoreWebViewManager.findWebview(appId, path);
  }

  static destroyWebview(webtag: string): boolean {
    const extracted = extractWebTag(webtag);
    const logicalTag = extracted ? toWebTag(extracted.appId, extracted.path) : webtag;
    teardownEmbedManager(logicalTag);
    return CoreWebViewManager.destroyWebview(webtag);
  }

  static loadUrl(webtag: string, url: string): boolean {
    return CoreWebViewManager.loadUrl(webtag, url);
  }

  static clearBrowsingData(webtag: string): boolean {
    return CoreWebViewManager.clearBrowsingData(webtag);
  }

  static setUserAgent(webtag: string, userAgent: string): boolean {
    return CoreWebViewManager.setUserAgent(webtag, userAgent);
  }

  static getWebViewInfo(webtag: string): WebViewInfo | null {
    return CoreWebViewManager.getWebViewInfo(webtag);
  }

  static setWebViewVisibility(appId: string, path: string, visible: boolean): boolean {
    const webtag = toWebTag(appId, path);
    const info = CoreWebViewManager.getWebViewInfo(webtag);
    if (!info) {
      return false;
    }

    CoreWebViewManager.setWebViewVisibility(appId, path, visible);

    if (visible) {
      resumeAllComponents(webtag);
    } else {
      pauseAllComponents(webtag);
    }
    return true;
  }

  static notifySurfaceState(
    appId: string,
    path: string,
    active: boolean,
    pageInstanceId?: string
  ): boolean {
    return CoreWebViewManager.notifySurfaceState(appId, path, active, pageInstanceId);
  }

  static pauseAllComponentsByAppId(appId: string): void {
    hilog.info(DOMAIN, TAG, `pauseAllComponentsByAppId: ${appId}`);
    const infos = CoreWebViewManager.getAllWebViewInfos();
    infos.forEach((info: WebViewInfo) => {
      const extracted = extractWebTag(info.webtag);
      if (extracted && extracted.appId === appId) {
        hilog.info(DOMAIN, TAG, `Pausing components for webtag: ${info.webtag}`);
        pauseAllComponents(info.webtag);
      }
    });
  }

  static getAllWebViewInfos(): WebViewInfo[] {
    return CoreWebViewManager.getAllWebViewInfos();
  }

  static enableDebugging(): void {
    CoreWebViewManager.enableDebugging();
  }
}

export function addWebViewUiCallback(callback: (action: string, info: WebViewInfo) => void): void {
  WebViewManager.addUiCallback(callback);
}

export function removeWebViewUiCallback(callback: (action: string, info: WebViewInfo) => void): void {
  WebViewManager.removeUiCallback(callback);
}

export function createWebViewController(webtag: string, optionsToken?: string): boolean {
  return WebViewManager.createWebview(webtag, optionsToken);
}

export function findWebview(appId: string, path: string): webview.WebviewController | null {
  return WebViewManager.findWebview(appId, path);
}

export function destroyWebViewController(webtag: string): boolean {
  return WebViewManager.destroyWebview(webtag);
}

export function getWebViewInfo(webtag: string): WebViewInfo | null {
  return WebViewManager.getWebViewInfo(webtag);
}

export function isBrowserRelaxed(webtag: string): boolean {
  return coreIsBrowserRelaxedProfile(webtag);
}

export function handleWebViewNavigationPolicy(webtag: string, url: string): boolean {
  return coreHandleWebViewNavigationPolicy(webtag, url);
}

export function handleWebViewLoadError(
  webtag: string,
  url: string,
  errorCode: number,
  description: string
): void {
  coreHandleWebViewLoadError(webtag, url, errorCode, description);
}

export function handleWebViewDownloadStart(
  webtag: string,
  url: string,
  userAgent: string,
  contentDisposition: string,
  mimetype: string,
  contentLength: number
): void {
  coreHandleWebViewDownloadStart(
    webtag,
    url,
    userAgent,
    contentDisposition,
    mimetype,
    contentLength
  );
}

export function notifyWebViewControllerAttached(webtag: string): void {
  coreNotifyWebViewControllerAttached(webtag);
}

export function getAllWebViewInfos(): WebViewInfo[] {
  return WebViewManager.getAllWebViewInfos();
}

export function loadUrl(webtag: string, url: string): boolean {
  return WebViewManager.loadUrl(webtag, url);
}

export function clearBrowsingData(webtag: string): boolean {
  return WebViewManager.clearBrowsingData(webtag);
}

export function setUserAgent(webtag: string, userAgent: string): boolean {
  return WebViewManager.setUserAgent(webtag, userAgent);
}

export function getWebViewEmbedManager(webtag: string): NativeComponentEmbedManager {
  const extracted = extractWebTag(webtag);
  const logicalTag = extracted ? toWebTag(extracted.appId, extracted.path) : webtag;
  const manager = getOrCreateEmbedManager(logicalTag);

  const controller = CoreWebViewManager.getController(logicalTag);
  if (controller) {
    setEmbedController(logicalTag, controller);
  }

  return manager;
}

export function pauseWebViewComponents(webtag: string): void {
  const extracted = extractWebTag(webtag);
  const logicalTag = extracted ? toWebTag(extracted.appId, extracted.path) : webtag;
  pauseAllComponents(logicalTag);
}

export function resumeWebViewComponents(webtag: string): void {
  const extracted = extractWebTag(webtag);
  const logicalTag = extracted ? toWebTag(extracted.appId, extracted.path) : webtag;
  resumeAllComponents(logicalTag);
}

export function pauseAllComponentsByAppId(appId: string): void {
  WebViewManager.pauseAllComponentsByAppId(appId);
}
