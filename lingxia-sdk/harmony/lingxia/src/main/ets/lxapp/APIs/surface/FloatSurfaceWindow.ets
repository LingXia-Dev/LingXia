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
import { LxLog } from '../../../app/LxLog';
import { window } from '@kit.ArkUI';
import { display } from '@kit.ArkUI';
import { webview } from '@kit.ArkWeb';
import { Lingxia } from '../../../app/Lingxia';
import { LingXiaWebView } from '../../LingXiaWebView';
import { addWebViewUiCallback, extractWebTag, removeWebViewUiCallback, WebViewInfo, WebViewManager, toWebTag } from '../../WebViewBridge';
import { browserTabPathForId, getBuiltinBrowserAppId, notifyPageInstanceMounted, notifyPageInstanceVisible, urlCallbackDispatch, webviewLoadErrorDocument } from 'liblingxia.so';
import type { SurfaceLayoutConfig } from './SurfaceModel';
import { isFileUrl, isHttpUrl } from './SurfaceUrlPolicy';
import { SurfaceCloseButton } from './SurfaceChrome';

const DOMAIN = 0x0000;
const TAG = 'LingXia.FloatSurfaceWindow';
const FLOAT_ROUTE = 'LingXiaFloatSurface';
const FLOAT_CONTENT_PAGE = 0;
const FLOAT_CONTENT_URL = 1;
const FLOAT_MASK_COLOR = 'rgba(0,0,0,0.45)';
const FLOAT_CORNER_RADIUS = 16;

interface FloatSurfaceWindowEntry {
  win: window.Window;
  layout: SurfaceLayoutConfig;
}

/** Close request raised from float chrome (mask tap / back). */
type FloatSurfaceCloseHandler = (id: string, appId: string, reason: string) => void;
type FloatSurfaceAppearanceHandler = (appId: string, dark: boolean) => void;

export class FloatSurfaceWindowManager {
  private static entries: Map<string, FloatSurfaceWindowEntry> = new Map();
  private static closeHandler: FloatSurfaceCloseHandler | null = null;
  private static appearanceByApp: Map<string, boolean> = new Map();
  private static appearanceHandlers: Set<FloatSurfaceAppearanceHandler> = new Set();
  /**
   * Key handoff to the content page: `@Entry({ routeName })` does not receive
   * the LocalStorage passed to loadContentByName, so the page reads the key
   * here in aboutToAppear. Opens are serialized (loadContentByName is awaited
   * before the next open starts), so one slot suffices.
   */
  private static pendingKey: string = '';

  public static takePendingKey(): string {
    const key = FloatSurfaceWindowManager.pendingKey;
    FloatSurfaceWindowManager.pendingKey = '';
    return key;
  }

  public static setCloseHandler(handler: FloatSurfaceCloseHandler): void {
    FloatSurfaceWindowManager.closeHandler = handler;
  }

  public static applyAppearance(appId: string, dark: boolean): void {
    FloatSurfaceWindowManager.appearanceByApp.set(appId, dark);
    FloatSurfaceWindowManager.appearanceHandlers.forEach((handler: FloatSurfaceAppearanceHandler) => {
      handler(appId, dark);
    });
  }

  public static appearanceDarkForWebTag(webTag: string): boolean {
    const appId = extractWebTag(webTag)?.appId ?? '';
    return FloatSurfaceWindowManager.appearanceByApp.get(appId) ?? false;
  }

  public static addAppearanceHandler(handler: FloatSurfaceAppearanceHandler): void {
    FloatSurfaceWindowManager.appearanceHandlers.add(handler);
  }

  public static removeAppearanceHandler(handler: FloatSurfaceAppearanceHandler): void {
    FloatSurfaceWindowManager.appearanceHandlers.delete(handler);
  }

  public static requestClose(key: string, reason: string): void {
    const entry = FloatSurfaceWindowManager.entries.get(key);
    if (!entry || !FloatSurfaceWindowManager.closeHandler) {
      return;
    }
    FloatSurfaceWindowManager.closeHandler(entry.layout.id, entry.layout.appId, reason);
  }

  public static layoutFor(key: string): SurfaceLayoutConfig | null {
    return FloatSurfaceWindowManager.entries.get(key)?.layout ?? null;
  }

  public static has(key: string): boolean {
    return FloatSurfaceWindowManager.entries.has(key);
  }

  /** Create (or re-present) the float's sub-window and show it. */
  public static async open(key: string, layout: SurfaceLayoutConfig): Promise<boolean> {
    const existing = FloatSurfaceWindowManager.entries.get(key);
    if (existing) {
      existing.layout = layout;
      return FloatSurfaceWindowManager.show(key);
    }
    const stage = Lingxia.windowStage;
    if (!stage) {
      LxLog.e(TAG, 'open failed: windowStage not initialized');
      return false;
    }
    try {
      const win = await stage.createSubWindow(FloatSurfaceWindowManager.windowName(key));
      FloatSurfaceWindowManager.entries.set(key, { win, layout });
      FloatSurfaceWindowManager.pendingKey = key;
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
      await FloatSurfaceWindowManager.close(key);
      return false;
    }
  }

  public static async show(key: string): Promise<boolean> {
    const entry = FloatSurfaceWindowManager.entries.get(key);
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
    const entry = FloatSurfaceWindowManager.entries.get(key);
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
    const entry = FloatSurfaceWindowManager.entries.get(key);
    if (!entry) {
      return;
    }
    FloatSurfaceWindowManager.entries.delete(key);
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
  @State private appearanceDark: boolean = false;
  private pageController: webview.WebviewController = new webview.WebviewController();
  private urlController: webview.WebviewController = new webview.WebviewController();
  private webViewUiCallback = (action: string, info: WebViewInfo): void => {
    if (action === 'create') {
      this.tryAttachPage();
      this.tryAttachUrl();
    }
  };
  private appearanceHandler = (appId: string, dark: boolean): void => {
    const webTagAppId = extractWebTag(this.pageWebTag)?.appId ?? '';
    if (webTagAppId === appId) this.appearanceDark = dark;
  };

  aboutToAppear() {
    this.lxFloatKey = FloatSurfaceWindowManager.takePendingKey();
    this.layout = FloatSurfaceWindowManager.layoutFor(this.lxFloatKey);
    if (!this.layout) {
      LxLog.e(TAG, `float page has no layout key=${this.lxFloatKey}`);
    }
    addWebViewUiCallback(this.webViewUiCallback);
    FloatSurfaceWindowManager.addAppearanceHandler(this.appearanceHandler);
    this.tryAttachPage();
    this.tryAttachUrl();
  }

  aboutToDisappear() {
    removeWebViewUiCallback(this.webViewUiCallback);
    FloatSurfaceWindowManager.removeAppearanceHandler(this.appearanceHandler);
  }

  onBackPress(): boolean {
    FloatSurfaceWindowManager.requestClose(this.lxFloatKey, 'user');
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
        this.appearanceDark = FloatSurfaceWindowManager.appearanceDarkForWebTag(info.webtag);
        notifyPageInstanceMounted(layout.pageInstanceId);
        notifyPageInstanceVisible(layout.pageInstanceId);
        hilog.info(DOMAIN, TAG, `attached float page webtag=${info.webtag}`);
        return;
      }
    }
  }

  private tryAttachUrl(): void {
    const layout = this.layout;
    if (!layout || layout.content !== FLOAT_CONTENT_URL || !layout.browserTabId || this.pageWebTag.length > 0) {
      return;
    }
    const appId = String(getBuiltinBrowserAppId() ?? '').trim();
    const path = String(browserTabPathForId(layout.browserTabId) ?? '').trim();
    if (!appId || !path) return;
    const controller = WebViewManager.findWebview(appId, path);
    if (!controller) return;
    const infos = WebViewManager.getAllWebViewInfos();
    for (let i = 0; i < infos.length; i++) {
      if (infos[i].controller === controller) {
        this.urlController = controller;
        this.pageWebTag = infos[i].webtag;
        this.appearanceDark = FloatSurfaceWindowManager.appearanceDarkForWebTag(infos[i].webtag);
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
        if (!this.isFullscreen() && (this.layout!.modal || this.layout!.dismissOnOutside)) {
          Column()
            .width('100%')
            .height('100%')
            .backgroundColor(this.layout!.modal ? FLOAT_MASK_COLOR : Color.Transparent)
            .onClick(() => {
              if (this.layout!.dismissOnOutside) {
                FloatSurfaceWindowManager.requestClose(this.lxFloatKey, 'user');
              }
            })
        }
        Column() {
          if (this.layout!.content === FLOAT_CONTENT_URL && this.layout!.browserTabId && this.pageWebTag.length > 0) {
            LingXiaWebView({
              webTag: this.pageWebTag,
              controller: this.urlController,
              appearanceDark: this.appearanceDark,
              renderVisibility: Visibility.Visible,
              renderWidth: '100%',
              renderHeight: '100%',
              renderZIndex: 0
            })
          } else if (this.layout!.content === FLOAT_CONTENT_URL && !this.layout!.browserTabId) {
            Web({
              src: this.layout!.path,
              controller: this.urlController,
              incognitoMode: this.layout!.ephemeralWebData
            })
              .domStorageAccess(true)
              .javaScriptAccess(true)
              .fileAccess(!this.layout!.urlCallback)
              .mixedMode(MixedMode.None)
              .onErrorReceive((event) => {
                if (!event || !event.request || !event.error || !event.request.isMainFrame()) return;
                const info = (event.error.getErrorInfo() ?? '').toLowerCase();
                if (info === 'err_aborted' || info === 'net::err_aborted') return;
                const failingUrl = event.request.getRequestUrl() ?? this.layout!.path;
                const html: string = webviewLoadErrorDocument(failingUrl);
                this.urlController.loadData(html, 'text/html', 'UTF-8', failingUrl, failingUrl);
              })
              .onLoadIntercept((event) => {
                const targetUrl = event?.data?.getRequestUrl() ?? '';
                if (!targetUrl) return false;
                if (urlCallbackDispatch(targetUrl)) return true;
                if (isHttpUrl(targetUrl) || (!this.layout!.urlCallback && isFileUrl(targetUrl))) return false;
                if (isFileUrl(targetUrl)) return true;
                Lingxia.launchWithUrl(targetUrl);
                return true;
              })
              .width('100%')
              .height('100%')
          } else if (this.pageWebTag.length > 0) {
            LingXiaWebView({
              webTag: this.pageWebTag,
              controller: this.pageController,
              appearanceDark: this.appearanceDark,
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
        if (this.layout!.closeButton) {
          SurfaceCloseButton({
            surfaceRightVp: this.isFullscreen() ? this.layout!.screenWidthVp : this.layout!.leftVp + this.layout!.widthVp,
            surfaceTopVp: this.isFullscreen() ? 0 : this.layout!.topVp,
            onPressed: () => {
              FloatSurfaceWindowManager.requestClose(this.lxFloatKey, 'user');
            }
          })
        }
      }
    }
    .width('100%')
    .height('100%')
    .backgroundColor(Color.Transparent)
    .expandSafeArea([SafeAreaType.SYSTEM], [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM])
  }
}
