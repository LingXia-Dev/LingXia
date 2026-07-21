/**
 * Sub-window-per-float presentation for lxapp FLOAT surfaces.
 *
 * A kept-alive ArkWeb inside the main window's view tree intercepts touches
 * even while hidden (no ArkUI attribute stops it), so a hidden in-tree float
 * blocks the page underneath. Hosting each float in its own sub-window makes
 * hide() a window operation: `minimize()` hides the sub-window (content and
 * page instance stay alive), `showWindow()` restores it, and a hidden window
 * cannot intercept anything.
 *
 * The float's sole ArkWeb `Web` lives in this window's content page — the
 * main-tree SurfaceManager must never also render float-role surfaces, or the
 * WebviewController double-binds and the sub-window renders blank.
 */
import { hilog } from '@kit.PerformanceAnalysisKit';
import { LxLog } from '../../app/LxLog';
import { window } from '@kit.ArkUI';
import { display } from '@kit.ArkUI';
import { webview } from '@kit.ArkWeb';
import { Lingxia } from '../../app/Lingxia';
import { LingXiaWebView } from '../LingXiaWebView';
import { addWebViewUiCallback, removeWebViewUiCallback, WebViewInfo, WebViewManager, toWebTag } from '../WebViewBridge';
import { notifyPageInstanceMounted, notifyPageInstanceVisible, urlCallbackDispatch, webviewLoadErrorDocument } from 'liblingxia.so';
import { isFileUrl, isHttpUrl, isSameOrigin } from './LxAppSurface';
import type { SurfaceLayoutConfig } from './LxAppSurface';

const DOMAIN = 0x0000;
const TAG = 'LingXia.FloatWindow';
const FLOAT_ROUTE = 'LingXiaFloatSurface';
const FLOAT_CONTENT_PAGE = 0;
const FLOAT_CONTENT_URL = 1;
const FLOAT_MASK_COLOR = 'rgba(0,0,0,0.45)';
const FLOAT_CORNER_RADIUS = 16;

interface FloatEntry {
  win: window.Window;
  layout: SurfaceLayoutConfig;
}

/** Close request raised from float chrome (mask tap / back). */
type FloatCloseHandler = (id: string, appId: string, reason: string) => void;

export class FloatWindowManager {
  private static entries: Map<string, FloatEntry> = new Map();
  private static closeHandler: FloatCloseHandler | null = null;
  /**
   * Key handoff to the content page: `@Entry({ routeName })` does not receive
   * the LocalStorage passed to loadContentByName, so the page reads the key
   * here in aboutToAppear. Opens are serialized (loadContentByName is awaited
   * before the next open starts), so one slot suffices.
   */
  private static pendingKey: string = '';

  public static takePendingKey(): string {
    const key = FloatWindowManager.pendingKey;
    FloatWindowManager.pendingKey = '';
    return key;
  }

  public static setCloseHandler(handler: FloatCloseHandler): void {
    FloatWindowManager.closeHandler = handler;
  }

  public static requestClose(key: string, reason: string): void {
    const entry = FloatWindowManager.entries.get(key);
    if (!entry || !FloatWindowManager.closeHandler) {
      return;
    }
    FloatWindowManager.closeHandler(entry.layout.id, entry.layout.appId, reason);
  }

  public static layoutFor(key: string): SurfaceLayoutConfig | null {
    return FloatWindowManager.entries.get(key)?.layout ?? null;
  }

  public static has(key: string): boolean {
    return FloatWindowManager.entries.has(key);
  }

  /** Create (or re-present) the float's sub-window and show it. */
  public static async open(key: string, layout: SurfaceLayoutConfig): Promise<boolean> {
    const existing = FloatWindowManager.entries.get(key);
    if (existing) {
      existing.layout = layout;
      return FloatWindowManager.show(key);
    }
    const stage = Lingxia.windowStage;
    if (!stage) {
      LxLog.e(TAG, 'open failed: windowStage not initialized');
      return false;
    }
    try {
      const win = await stage.createSubWindow(FloatWindowManager.windowName(key));
      FloatWindowManager.entries.set(key, { win, layout });
      FloatWindowManager.pendingKey = key;
      await win.loadContentByName(FLOAT_ROUTE);
      // Full-screen: the content page draws the dim mask + the float rect, so
      // tap-outside-to-close matches the in-tree presentation. Window calls
      // must come AFTER loadContentByName or they throw "window state abnormal".
      const displayInfo = display.getDefaultDisplaySync();
      await win.resize(displayInfo.width, displayInfo.height);
      await win.moveWindowTo(0, 0);
      await win.setWindowBackgroundColor('#00000000');
      await win.showWindow();
      hilog.info(DOMAIN, TAG, `float window shown key=${key}`);
      return true;
    } catch (error) {
      LxLog.e(TAG, `open failed key=${key}: ${error}`);
      // Never leave a half-created window behind — even unshown it can hold
      // focus and block the app's input.
      FloatWindowManager.close(key);
      return false;
    }
  }

  public static async show(key: string): Promise<boolean> {
    const entry = FloatWindowManager.entries.get(key);
    if (!entry) {
      return false;
    }
    try {
      await entry.win.showWindow();
      return true;
    } catch (error) {
      LxLog.e(TAG, `show failed key=${key}: ${error}`);
      return false;
    }
  }

  /** Hide the sub-window; the ArkWeb content and page instance stay alive. */
  public static async hide(key: string): Promise<boolean> {
    const entry = FloatWindowManager.entries.get(key);
    if (!entry) {
      return false;
    }
    try {
      await entry.win.minimize();
      return true;
    } catch (error) {
      LxLog.e(TAG, `hide failed key=${key}: ${error}`);
      return false;
    }
  }

  public static async close(key: string): Promise<void> {
    const entry = FloatWindowManager.entries.get(key);
    if (!entry) {
      return;
    }
    FloatWindowManager.entries.delete(key);
    try {
      await entry.win.destroyWindow();
    } catch (error) {
      hilog.warn(DOMAIN, TAG, `destroy failed key=${key}: ${error}`);
    }
  }

  private static windowName(key: string): string {
    return `lx_float_${key.replace(/[^0-9A-Za-z]/g, '_')}`;
  }
}

/**
 * Sub-window content: full-screen dim mask (tap closes) + the float rect
 * hosting the surface's sole Web. Mirrors the in-tree float presentation.
 */
@Entry({ routeName: 'LingXiaFloatSurface' })
@Component
struct LingXiaFloatSurfacePage {
  @State lxFloatKey: string = '';
  @State private pageWebTag: string = '';
  @State private layout: SurfaceLayoutConfig | null = null;
  private pageController: webview.WebviewController = new webview.WebviewController();
  private urlController: webview.WebviewController = new webview.WebviewController();
  private webViewUiCallback = (action: string, info: WebViewInfo): void => {
    if (action === 'create') {
      this.tryAttachPage();
    }
  };

  aboutToAppear() {
    this.lxFloatKey = FloatWindowManager.takePendingKey();
    this.layout = FloatWindowManager.layoutFor(this.lxFloatKey);
    if (!this.layout) {
      LxLog.e(TAG, `float page has no layout key=${this.lxFloatKey}`);
    }
    if (this.layout?.content === FLOAT_CONTENT_URL && this.layout.ephemeralWebData) {
      // `incognitoMode` only stops on-disk persistence; the incognito
      // cookie/storage lives in a shared in-memory store, so a prior session
      // (e.g. an IdP SSO cookie) would carry into this surface. Wipe that
      // store before the Web builds and issues its first request.
      try {
        webview.WebCookieManager.clearAllCookiesSync(true);
        webview.WebStorage.deleteAllData(true);
      } catch (clearErr) {
        LxLog.e(TAG, `failed to clear incognito web data: ${clearErr}`);
      }
    }
    addWebViewUiCallback(this.webViewUiCallback);
    this.tryAttachPage();
  }

  aboutToDisappear() {
    removeWebViewUiCallback(this.webViewUiCallback);
  }

  onBackPress(): boolean {
    FloatWindowManager.requestClose(this.lxFloatKey, 'user');
    return true;
  }

  private tryAttachPage(): void {
    const layout = this.layout;
    if (!layout || layout.content !== FLOAT_CONTENT_PAGE || this.pageWebTag.length > 0) {
      return;
    }
    const exactTag = `${toWebTag(layout.appId, layout.path)}#${layout.pageInstanceId}`;
    const infos = WebViewManager.getAllWebViewInfos();
    for (let i = 0; i < infos.length; i++) {
      const info = infos[i];
      if (info.webtag === exactTag || info.webtag.startsWith(`${exactTag}#`)) {
        this.pageController = info.controller;
        this.pageWebTag = info.webtag;
        notifyPageInstanceMounted(layout.pageInstanceId);
        notifyPageInstanceVisible(layout.pageInstanceId);
        hilog.info(DOMAIN, TAG, `attached float page webtag=${info.webtag}`);
        return;
      }
    }
  }

  private isFullscreen(): boolean {
    return this.layout?.fullWidth === true && this.layout?.fullHeight === true;
  }

  build() {
    Stack() {
      if (this.layout) {
        if (!this.isFullscreen()) {
          // Dim scrim around the float rect; a tap outside closes.
          Column()
            .width('100%')
            .height('100%')
            .backgroundColor(FLOAT_MASK_COLOR)
            .onClick(() => {
              FloatWindowManager.requestClose(this.lxFloatKey, 'user');
            })
        }
        Column() {
          if (this.layout!.content === FLOAT_CONTENT_URL) {
            Web({ src: this.layout!.path, controller: this.urlController, incognitoMode: this.layout!.ephemeralWebData })
              .domStorageAccess(true)
              .javaScriptAccess(true)
              .fileAccess(!this.layout!.urlCallback)
              .mixedMode(MixedMode.None)
              .onErrorReceive((event) => {
                if (!event || !event.request || !event.error || !event.request.isMainFrame()) {
                  return;
                }
                // Cancellation is control flow (superseded load, intercepted
                // handoff), never an application-visible load error. It is
                // reported as exactly ERR_ABORTED — substring matching would
                // also swallow real failures like ERR_CONNECTION_ABORTED.
                const info = (event.error.getErrorInfo() ?? '').toLowerCase();
                if (info === 'err_aborted' || info === 'net::err_aborted') {
                  return;
                }
                const failingUrl = event.request.getRequestUrl() ?? this.layout!.path;
                try {
                  const html: string = webviewLoadErrorDocument(failingUrl);
                  this.urlController.loadData(html, 'text/html', 'UTF-8', failingUrl, failingUrl);
                } catch (error) {
                  LxLog.e(TAG, `failed to render float load error: ${error}`);
                }
              })
              .onLoadIntercept((event) => {
                if (!event || !event.data) {
                  return false;
                }
                const targetUrl = event.data.getRequestUrl();
                if (!targetUrl) {
                  return false;
                }
                // A registered URL-callback sentinel (e.g. an auth handoff) is
                // consumed by the waiting Rust channel; cancel the load.
                // Cross-origin http(s) is allowed (multi-origin auth journey);
                // non-http(s) deep links are handed to the system.
                if (urlCallbackDispatch(targetUrl)) {
                  return true;
                }
                if (isHttpUrl(targetUrl) || (!this.layout!.urlCallback && isFileUrl(targetUrl))) {
                  return false;
                }
                if (isFileUrl(targetUrl)) {
                  return true;
                }
                Lingxia.launchWithUrl(targetUrl);
                return true;
              })
              .width('100%')
              .height('100%')
          } else if (this.pageWebTag.length > 0) {
            LingXiaWebView({
              webTag: this.pageWebTag,
              controller: this.pageController,
              renderVisibility: Visibility.Visible,
              renderWidth: '100%',
              renderHeight: '100%',
              renderZIndex: 0
            })
          }
        }
        .width(this.isFullscreen() ? '100%' : `${this.layout!.widthVp}vp`)
        .height(this.isFullscreen() ? '100%' : `${this.layout!.heightVp}vp`)
        .position(this.isFullscreen() ? { x: 0, y: 0 } : { x: this.layout!.leftVp, y: this.layout!.topVp })
        .backgroundColor(this.layout!.content === FLOAT_CONTENT_URL ? Color.White : Color.Transparent)
        .borderRadius(this.isFullscreen() ? 0 : FLOAT_CORNER_RADIUS)
        .clip(!this.isFullscreen())
      }
    }
    .width('100%')
    .height('100%')
    .backgroundColor(Color.Transparent)
    .expandSafeArea([SafeAreaType.SYSTEM], [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM])
  }
}
