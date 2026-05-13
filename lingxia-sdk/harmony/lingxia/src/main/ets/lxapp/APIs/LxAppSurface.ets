import { hilog } from '@kit.PerformanceAnalysisKit';
import { common } from '@kit.AbilityKit';
import { display, window } from '@kit.ArkUI';
import { webview } from '@kit.ArkWeb';
import { LingXiaWebView } from '../LingXiaWebView';
import { addWebViewUiCallback, removeWebViewUiCallback, WebViewInfo, WebViewManager, toWebTag } from '../WebViewBridge';
import { disposePageInstance, notifyPageInstanceHidden, notifyPageInstanceMounted, notifyPageInstanceVisible, onSurfaceClosed } from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.Surface';
const CONTENT_PAGE = 0;
const CONTENT_URL = 1;
const KIND_POPUP = 1;
const SURFACE_NOTIFY_RETRY_DELAY_MS = 32;
const SURFACE_NOTIFY_RETRY_LIMIT = 20;
const SURFACE_CLOSE_FINALIZE_DELAY_MS = 32;
const SURFACE_MASK_COLOR = 'rgba(0,0,0,0.45)';
const SURFACE_CORNER_RADIUS = 16;

export enum SurfacePosition {
  Center = 0,
  Bottom = 1,
  Left = 2,
  Right = 3,
  Top = 4
}

enum CornerMaskPosition {
  TopLeft = 0,
  TopRight = 1,
  BottomLeft = 2,
  BottomRight = 3
}

interface SurfaceDimensions {
  screenWidthVp: number;
  screenHeightVp: number;
  widthVp: number;
  heightVp: number;
  leftVp: number;
  topVp: number;
  bottomInset: number;
  fullWidth: boolean;
  fullHeight: boolean;
}

export interface SurfaceLayoutConfig {
  id: string;
  appId: string;
  path: string;
  pageInstanceId: string;
  content: number;
  screenWidthVp: number;
  screenHeightVp: number;
  widthVp: number;
  heightVp: number;
  leftVp: number;
  topVp: number;
  position: SurfacePosition;
  bottomInset: number;
  fullWidth: boolean;
  fullHeight: boolean;
}

export class LxAppSurface {
  private static managerRef: SurfaceManager | null = null;
  private static activeLayout: SurfaceLayoutConfig | null = null;

  public static setManagerRef(manager: SurfaceManager): void {
    LxAppSurface.managerRef = manager;
  }

  public static clearManagerRef(manager: SurfaceManager): void {
    if (LxAppSurface.managerRef === manager) {
      LxAppSurface.managerRef = null;
    }
  }

  public static present(
    id: string,
    appId: string,
    path: string,
    sessionId: string,
    pageInstanceId: string,
    contentValue: string,
    kindValue: string,
    widthValue: string,
    heightValue: string,
    widthRatioValue: string,
    heightRatioValue: string,
    positionValue: string
  ): boolean {
    const content = parseInt(contentValue, 10);
    const kind = parseInt(kindValue, 10);
    if (!id || !appId || !path || parseInt(sessionId, 10) <= 0) {
      return false;
    }
    if (kind !== KIND_POPUP) {
      return false;
    }
    if (content === CONTENT_PAGE && !pageInstanceId) {
      return false;
    }
    if (content === CONTENT_URL && !isHttpUrl(path)) {
      return false;
    }
    if (content !== CONTENT_PAGE && content !== CONTENT_URL) {
      return false;
    }
    if (!LxAppSurface.managerRef) {
      hilog.error(DOMAIN, TAG, 'present failed: SurfaceManager not initialized');
      return false;
    }

    const position = normalizePosition(positionValue);
    const dimensions = computeDimensions(
      parseSize(widthValue),
      parseSize(heightValue),
      parseRatio(widthRatioValue),
      parseRatio(heightRatioValue),
      position
    );

    const layout: SurfaceLayoutConfig = {
      id,
      appId,
      path,
      pageInstanceId,
      content,
      screenWidthVp: dimensions.screenWidthVp,
      screenHeightVp: dimensions.screenHeightVp,
      widthVp: dimensions.widthVp,
      heightVp: dimensions.heightVp,
      leftVp: dimensions.leftVp,
      topVp: dimensions.topVp,
      position,
      bottomInset: dimensions.bottomInset,
      fullWidth: dimensions.fullWidth,
      fullHeight: dimensions.fullHeight
    };

    const previous = LxAppSurface.activeLayout;
    if (previous) {
      if (previous.content === CONTENT_PAGE) {
        notifyPageInstanceHidden(previous.pageInstanceId, 'programmatic');
      }
      onSurfaceClosed(previous.appId, previous.id, 'programmatic');
    }
    LxAppSurface.activeLayout = layout;

    const success = LxAppSurface.managerRef.present(layout);
    if (success) {
      if (content === CONTENT_PAGE) {
        LxAppSurface.showPageSurfaceWhenReady(layout);
      }
    } else {
      LxAppSurface.activeLayout = null;
      if (content === CONTENT_PAGE) {
        disposePageInstance(pageInstanceId, 'failed');
      }
      onSurfaceClosed(appId, id, 'failed');
    }
    return success;
  }

  public static close(id: string, appId: string, reason: string): boolean {
    hilog.info(DOMAIN, TAG, `close requested id=${id} appId=${appId} reason=${reason}`);
    if (!id || !appId) {
      hilog.warn(DOMAIN, TAG, 'close rejected: empty id or appId');
      return false;
    }
    const normalizedReason = normalizeReason(reason);
    const layout = LxAppSurface.activeLayout;
    const manager = LxAppSurface.managerRef;
    if (!layout || layout.id !== id || layout.appId !== appId) {
      hilog.warn(DOMAIN, TAG, `close without matching active layout id=${id} appId=${appId}`);
      if (manager && manager.hasSurface(id, appId)) {
        manager.close(appId);
        LxAppSurface.activeLayout = null;
        setTimeout(() => {
          manager.finishClose(id, appId);
          onSurfaceClosed(appId, id, normalizedReason);
        }, SURFACE_CLOSE_FINALIZE_DELAY_MS);
        return true;
      }
      onSurfaceClosed(appId, id, normalizedReason);
      return true;
    }
    const result = manager ? manager.close(appId) : true;
    if (result) {
      if (layout.content === CONTENT_PAGE) {
        notifyPageInstanceHidden(layout.pageInstanceId, normalizedReason);
      }
      LxAppSurface.activeLayout = null;
      setTimeout(() => {
        if (manager) {
          manager.finishClose(layout.id, layout.appId);
        }
        onSurfaceClosed(layout.appId, layout.id, normalizedReason);
      }, SURFACE_CLOSE_FINALIZE_DELAY_MS);
    }
    return result;
  }

  /**
   * Toggle the active surface to visible without tearing it down. Layout state
   * stays cached in the SurfaceManager so re-show restores the same frame,
   * URL, and page instance. Fires the page-side onShow lifecycle so JS
   * observes the visibility transition.
   */
  public static show(id: string, appId: string): boolean {
    hilog.info(DOMAIN, TAG, `show requested id=${id} appId=${appId}`);
    if (!id || !appId) {
      return false;
    }
    const layout = LxAppSurface.activeLayout;
    if (!layout || layout.id !== id || layout.appId !== appId) {
      hilog.warn(DOMAIN, TAG, `show: surface not found id=${id} appId=${appId}`);
      return false;
    }
    if (layout.content === CONTENT_URL) {
      if (!LxAppSurface.managerRef) {
        return false;
      }
      return LxAppSurface.managerRef.setVisible(appId, true);
    } else {
      if (!LxAppSurface.managerRef || !LxAppSurface.managerRef.setVisible(appId, true)) {
        return false;
      }
      LxAppSurface.showPageSurfaceWhenReady(layout);
      if (layout.pageInstanceId) {
        notifyPageInstanceVisible(layout.pageInstanceId);
      }
    }
    return true;
  }

  /**
   * Toggle the active surface to hidden without tearing it down. The page
   * instance and WebView stay alive; a subsequent show() restores the same
   * state instead of remounting. Fires the page-side onHide lifecycle so JS
   * can pause timers / save state.
   */
  public static hide(id: string, appId: string): boolean {
    hilog.info(DOMAIN, TAG, `hide requested id=${id} appId=${appId}`);
    if (!id || !appId) {
      return false;
    }
    const layout = LxAppSurface.activeLayout;
    const manager = LxAppSurface.managerRef;
    if (!layout || layout.id !== id || layout.appId !== appId) {
      hilog.warn(DOMAIN, TAG, `hide: surface not found id=${id} appId=${appId}`);
      return manager ? manager.setVisible(appId, false, id) : false;
    }
    if (layout.content === CONTENT_URL) {
      if (!manager) {
        return false;
      }
      return manager.setVisible(appId, false, id);
    } else {
      if (!manager || !manager.setVisible(appId, false, id)) {
        return false;
      }
      if (layout.pageInstanceId) {
        notifyPageInstanceHidden(layout.pageInstanceId, 'hidden');
      }
    }
    return true;
  }

  public static getActiveLayout(): SurfaceLayoutConfig | null {
    return LxAppSurface.activeLayout;
  }

  private static showPageSurfaceWhenReady(layout: SurfaceLayoutConfig, attempt: number = 0): boolean {
    if (layout.content !== CONTENT_PAGE) {
      return true;
    }
    const current = LxAppSurface.activeLayout;
    if (!current || current.id !== layout.id || current.appId !== layout.appId) {
      return false;
    }
    if (LxAppSurface.managerRef && LxAppSurface.managerRef.attachPageSurface(layout)) {
      return true;
    }
    if (attempt < SURFACE_NOTIFY_RETRY_LIMIT) {
      setTimeout(() => {
        LxAppSurface.showPageSurfaceWhenReady(layout, attempt + 1);
      }, SURFACE_NOTIFY_RETRY_DELAY_MS);
      return true;
    }
    hilog.error(DOMAIN, TAG, `showPageSurfaceWhenReady failed id=${layout.id} appId=${layout.appId} path=${layout.path} pageInstanceId=${layout.pageInstanceId}`);
    LxAppSurface.failPageSurface(layout);
    return false;
  }

  private static failPageSurface(layout: SurfaceLayoutConfig): void {
    const current = LxAppSurface.activeLayout;
    if (!current || current.id !== layout.id || current.appId !== layout.appId) {
      return;
    }
    const manager = LxAppSurface.managerRef;
    if (manager) {
      manager.close(layout.appId);
    }
    if (layout.content === CONTENT_PAGE && layout.pageInstanceId) {
      notifyPageInstanceHidden(layout.pageInstanceId, 'failed');
      disposePageInstance(layout.pageInstanceId, 'failed');
    }
    LxAppSurface.activeLayout = null;
    if (manager) {
      manager.finishClose(layout.id, layout.appId);
    }
    onSurfaceClosed(layout.appId, layout.id, 'failed');
  }
}

function parseRatio(value: string): number {
  const parsed = parseFloat(value);
  if (isNaN(parsed) || !isFinite(parsed) || parsed <= 0) {
    return NaN;
  }
  return Math.min(parsed, 1);
}

function parseSize(value: string): number {
  const parsed = parseFloat(value);
  if (isNaN(parsed) || !isFinite(parsed) || parsed <= 0) {
    return NaN;
  }
  return parsed;
}

function finite(value: number): boolean {
  return !isNaN(value) && isFinite(value) && value > 0;
}

function normalizePosition(value: string): SurfacePosition {
  const parsed = parseInt(value, 10);
  if (parsed === SurfacePosition.Bottom) return SurfacePosition.Bottom;
  if (parsed === SurfacePosition.Left) return SurfacePosition.Left;
  if (parsed === SurfacePosition.Right) return SurfacePosition.Right;
  if (parsed === SurfacePosition.Top) return SurfacePosition.Top;
  return SurfacePosition.Center;
}

function computeDimensions(
  widthValue: number,
  heightValue: number,
  widthRatio: number,
  heightRatio: number,
  position: SurfacePosition
): SurfaceDimensions {
  let screenWidth = 360;
  let screenHeight = 640;
  try {
    const displayInfo = display.getDefaultDisplaySync();
    const density = displayInfo.densityPixels || 1;
    screenWidth = Math.max(1, displayInfo.width / density);
    screenHeight = Math.max(1, displayInfo.height / density);
  } catch (error) {
    hilog.warn(DOMAIN, TAG, `failed to read display metrics: ${error}`);
  }
  const defaultWidthRatio = 0.90;
  const defaultHeightRatio = 0.55;
  const widthVp = Math.min(screenWidth, Math.max(160, finite(widthValue) ? widthValue : screenWidth * (finite(widthRatio) ? widthRatio : defaultWidthRatio)));
  const heightVp = Math.min(screenHeight, Math.max(160, finite(heightValue) ? heightValue : screenHeight * (finite(heightRatio) ? heightRatio : defaultHeightRatio)));
  let leftVp = (screenWidth - widthVp) / 2;
  let topVp = (screenHeight - heightVp) / 2;
  switch (position) {
    case SurfacePosition.Bottom:
      topVp = screenHeight - heightVp;
      break;
    case SurfacePosition.Left:
      leftVp = 0;
      break;
    case SurfacePosition.Right:
      leftVp = screenWidth - widthVp;
      break;
    case SurfacePosition.Top:
      topVp = 0;
      break;
    default:
      break;
  }

  return {
    screenWidthVp: screenWidth,
    screenHeightVp: screenHeight,
    widthVp,
    heightVp,
    leftVp,
    topVp,
    bottomInset: 0,
    fullWidth: widthVp >= screenWidth,
    fullHeight: heightVp >= screenHeight
  };
}

function normalizeReason(reason: string): string {
  const trimmed = reason.trim();
  if (trimmed === 'user' || trimmed === 'programmatic' || trimmed === 'owner_closed' || trimmed === 'app_closed' || trimmed === 'failed') {
    return trimmed;
  }
  return 'unknown';
}

function isHttpUrl(value: string): boolean {
  const normalized = value.trim().toLowerCase();
  return normalized.startsWith('https://') || normalized.startsWith('http://');
}

function originOf(value: string): string {
  const normalized = value.trim().toLowerCase();
  const schemeEnd = normalized.indexOf('://');
  if (schemeEnd <= 0) {
    return '';
  }
  const scheme = normalized.substring(0, schemeEnd);
  let authority = normalized.substring(schemeEnd + 3);
  const pathStart = authority.search(/[/?#]/);
  if (pathStart >= 0) {
    authority = authority.substring(0, pathStart);
  }
  if (!authority) {
    return '';
  }
  const at = authority.lastIndexOf('@');
  if (at >= 0) {
    authority = authority.substring(at + 1);
  }
  if (authority.startsWith('[')) {
    const close = authority.indexOf(']');
    if (close > 0) {
      const host = authority.substring(0, close + 1);
      const portPart = authority.substring(close + 1);
      const port = portPart.startsWith(':') ? portPart.substring(1) : '';
      return `${scheme}://${host}:${normalizePort(scheme, port)}`;
    }
  }
  const colon = authority.lastIndexOf(':');
  const host = colon > 0 ? authority.substring(0, colon) : authority;
  const port = colon > 0 ? authority.substring(colon + 1) : '';
  return `${scheme}://${host}:${normalizePort(scheme, port)}`;
}

function normalizePort(scheme: string, port: string): string {
  if (port) {
    return port;
  }
  if (scheme === 'http') {
    return '80';
  }
  if (scheme === 'https') {
    return '443';
  }
  return '';
}

function isSameOrigin(baseUrl: string, targetUrl: string): boolean {
  const baseOrigin = originOf(baseUrl);
  const targetOrigin = originOf(targetUrl);
  return !!baseOrigin && baseOrigin === targetOrigin;
}

@Component
struct SurfaceCornerMask {
  @Prop radius: number = SURFACE_CORNER_RADIUS
  @Prop cornerPosition: CornerMaskPosition = CornerMaskPosition.TopLeft
  private settings: RenderingContextSettings = new RenderingContextSettings(true);
  private ctx: CanvasRenderingContext2D = new CanvasRenderingContext2D(this.settings);
  private ready: boolean = false;

  aboutToRender() {
    if (this.ready) {
      this.draw();
    }
  }

  private draw(): void {
    const r = Math.max(0, this.radius);
    const ctx = this.ctx;
    ctx.clearRect(0, 0, r, r);
    if (r <= 0) {
      return;
    }
    ctx.beginPath();
    ctx.fillStyle = SURFACE_MASK_COLOR;
    switch (this.cornerPosition) {
      case CornerMaskPosition.TopLeft:
        ctx.moveTo(0, 0);
        ctx.lineTo(r, 0);
        ctx.arc(r, r, r, -Math.PI / 2, Math.PI, true);
        ctx.lineTo(0, 0);
        break;
      case CornerMaskPosition.TopRight:
        ctx.moveTo(r, 0);
        ctx.lineTo(r, r);
        ctx.arc(0, r, r, 0, -Math.PI / 2, true);
        ctx.lineTo(r, 0);
        break;
      case CornerMaskPosition.BottomLeft:
        ctx.moveTo(0, r);
        ctx.lineTo(0, 0);
        ctx.arc(r, 0, r, Math.PI, Math.PI / 2, true);
        ctx.lineTo(0, r);
        break;
      case CornerMaskPosition.BottomRight:
        ctx.moveTo(r, r);
        ctx.lineTo(0, r);
        ctx.arc(0, 0, r, Math.PI / 2, 0, true);
        ctx.lineTo(r, r);
        break;
      default:
        break;
    }
    ctx.closePath();
    ctx.fill();
  }

  build() {
    Canvas(this.ctx)
      .width(`${this.radius}vp`)
      .height(`${this.radius}vp`)
      .backgroundColor(Color.Transparent)
      .hitTestBehavior(HitTestMode.Transparent)
      .onReady(() => {
        this.ready = true;
        this.draw();
      })
  }
}

@Component
export struct SurfaceManager {
  @State private isVisible: boolean = false;
  @State private surfaceId: string = '';
  @State private appId: string = '';
  @State private path: string = '';
  @State private content: number = CONTENT_PAGE;
  @State private screenWidthVp: number = 0;
  @State private screenHeightVp: number = 0;
  @State private widthVp: number = 0;
  @State private heightVp: number = 0;
  @State private leftVp: number = 0;
  @State private topVp: number = 0;
  @State private fullWidth: boolean = false;
  @State private fullHeight: boolean = false;
  @State private surfacePosition: SurfacePosition = SurfacePosition.Center;
  @State private pageWebTag: string = '';
  @State private pageController: webview.WebviewController = new webview.WebviewController();
  private webController: webview.WebviewController = new webview.WebviewController();
  private windowOpVersion: number = 0;
  private fullscreenWindowActive: boolean = false;
  private webViewUiCallback = (action: string, info: WebViewInfo): void => {
    if (action !== 'create') {
      return;
    }
    const layout = LxAppSurface.getActiveLayout();
    if (layout && layout.content === CONTENT_PAGE && this.matchesPageSurface(layout, info.webtag)) {
      this.attachPageSurface(layout);
    }
  }

  aboutToAppear() {
    LxAppSurface.setManagerRef(this);
    addWebViewUiCallback(this.webViewUiCallback);
  }

  aboutToDisappear() {
    LxAppSurface.clearManagerRef(this);
    removeWebViewUiCallback(this.webViewUiCallback);
    this.exitFullscreenMode();
  }

  public present(state: SurfaceLayoutConfig): boolean {
    this.surfaceId = state.id;
    this.appId = state.appId;
    this.path = state.path;
    this.content = state.content;
    this.screenWidthVp = state.screenWidthVp;
    this.screenHeightVp = state.screenHeightVp;
    this.widthVp = state.widthVp;
    this.heightVp = state.heightVp;
    this.leftVp = state.leftVp;
    this.topVp = state.topVp;
    this.fullWidth = state.fullWidth;
    this.fullHeight = state.fullHeight;
    this.surfacePosition = state.position;
    this.pageWebTag = '';
    this.isVisible = true;
    if (this.shouldUseFullscreenWindow()) {
      this.enterFullscreenMode();
    } else {
      this.exitFullscreenMode();
    }
    hilog.info(DOMAIN, TAG, `present surface id=${state.id} content=${state.content} pos=${state.position} url=${state.path} frame=${state.leftVp},${state.topVp},${state.widthVp},${state.heightVp}`);
    return true;
  }

  public attachPageSurface(layout: SurfaceLayoutConfig): boolean {
    const infos = WebViewManager.getAllWebViewInfos();
    for (let i = 0; i < infos.length; i++) {
      const info = infos[i];
      if (this.matchesPageSurface(layout, info.webtag)) {
        const alreadyAttached = this.pageWebTag === info.webtag;
        this.pageWebTag = info.webtag;
        this.pageController = info.controller;
        this.isVisible = true;
        if (!alreadyAttached) {
          notifyPageInstanceMounted(layout.pageInstanceId);
          notifyPageInstanceVisible(layout.pageInstanceId);
          hilog.info(DOMAIN, TAG, `attached page surface webtag=${info.webtag}`);
        }
        return true;
      }
    }
    hilog.warn(DOMAIN, TAG, `attachPageSurface: WebView not found for ${layout.appId}:${layout.path}#${layout.pageInstanceId}`);
    return false;
  }

  private matchesPageSurface(layout: SurfaceLayoutConfig, webTag: string): boolean {
    const exactTag = `${toWebTag(layout.appId, layout.path)}#${layout.pageInstanceId}`;
    return webTag === exactTag || webTag.startsWith(`${exactTag}#`);
  }

  public hasSurface(id: string, appId: string): boolean {
    if (!this.surfaceId || !this.appId) {
      return false;
    }
    return this.surfaceId === id && this.appId === appId;
  }

  public close(appId: string): boolean {
    if (this.appId && this.appId !== appId) {
      hilog.warn(DOMAIN, TAG, `close mismatch: expected ${this.appId}, got ${appId}`);
    }
    this.isVisible = false;
    if (this.shouldUseFullscreenWindow()) {
      this.exitFullscreenMode();
    }
    return true;
  }

  public finishClose(id: string, appId: string): void {
    if (this.surfaceId && this.surfaceId !== id) {
      return;
    }
    if (this.appId && this.appId !== appId) {
      return;
    }
    this.surfaceId = '';
    this.appId = '';
    this.path = '';
    this.content = CONTENT_PAGE;
    this.screenWidthVp = 0;
    this.screenHeightVp = 0;
    this.widthVp = 0;
    this.heightVp = 0;
    this.leftVp = 0;
    this.topVp = 0;
    this.fullWidth = false;
    this.fullHeight = false;
    this.surfacePosition = SurfacePosition.Center;
    this.pageWebTag = '';
    this.pageController = new webview.WebviewController();
    this.exitFullscreenMode();
  }

  /**
   * Toggle visibility without resetting cached layout state. show() restores
   * the same frame the previous present() configured.
   */
  public setVisible(appId: string, visible: boolean, surfaceId?: string): boolean {
    if (surfaceId && this.surfaceId && this.surfaceId !== surfaceId) {
      hilog.warn(DOMAIN, TAG, `setVisible mismatch: expected id ${this.surfaceId}, got ${surfaceId}`);
      return false;
    }
    if (this.appId && this.appId !== appId) {
      hilog.warn(DOMAIN, TAG, `setVisible mismatch: expected ${this.appId}, got ${appId}`);
      return false;
    }
    if (!this.appId) {
      hilog.warn(DOMAIN, TAG, `setVisible: no active surface for ${appId}`);
      return false;
    }
    this.isVisible = visible;
    if (visible && this.shouldUseFullscreenWindow()) {
      this.enterFullscreenMode();
    } else if (!visible) {
      this.exitFullscreenMode();
    }
    return true;
  }

  build() {
    if ((this.content === CONTENT_URL && this.isVisible) || (this.content === CONTENT_PAGE && this.pageWebTag.length > 0)) {
      Stack() {
        if (this.isVisible) {
          this.buildMaskLayer()
        }
        Column() {
          if (this.content === CONTENT_URL) {
            Web({ src: '', controller: this.webController })
              .domStorageAccess(true)
              .javaScriptAccess(true)
              .fileAccess(false)
              .mixedMode(MixedMode.None)
              .onLoadIntercept((event) => {
                if (!event || !event.data) {
                  return false;
                }
                const targetUrl = event.data.getRequestUrl();
                if (!targetUrl || isSameOrigin(this.path, targetUrl)) {
                  return false;
                }
                hilog.warn(DOMAIN, TAG, `blocked surface navigation: ${targetUrl}`);
                return true;
              })
              .width('100%')
              .height('100%')
              .onAppear(() => {
                const url = this.path;
                setTimeout(() => {
                  try {
                    hilog.info(DOMAIN, TAG, `load surface url=${url}`);
                    this.webController.loadUrl(url);
                  } catch (error) {
                    hilog.error(DOMAIN, TAG, `failed to load surface URL: ${error}`);
                  }
                }, 0);
              })
              .onPageBegin((event) => {
                hilog.info(DOMAIN, TAG, `surface page begin: ${event?.url ?? ''}`);
              })
              .onPageEnd((event) => {
                hilog.info(DOMAIN, TAG, `surface page end: ${event?.url ?? ''}`);
              })
              .hitTestBehavior(this.isVisible ? HitTestMode.Block : HitTestMode.None)
          } else if (this.pageWebTag.length > 0) {
            LingXiaWebView({
              webTag: this.pageWebTag,
              controller: this.pageController,
              renderVisibility: Visibility.Visible,
              renderWidth: '100%',
              renderHeight: '100%',
              renderBorderRadius: this.getBorderRadius(),
              renderClip: true,
              renderZIndex: 0,
              renderHitTest: this.isVisible ? HitTestMode.Block : HitTestMode.None
            })
          }
        }
        .width(this.isVisible ? (this.widthVp > 0 ? `${this.widthVp}vp` : '90%') : 1)
        .height(this.isVisible ? (this.heightVp > 0 ? `${this.heightVp}vp` : '60%') : 1)
        .backgroundColor(this.content === CONTENT_URL ? Color.White : Color.Transparent)
        .borderRadius(this.getBorderRadius())
        .clip(!this.shouldUseFullscreenWindow())
        .visibility(Visibility.Visible)
        .hitTestBehavior(this.isVisible ? HitTestMode.Default : HitTestMode.None)
        .position({ x: this.isVisible ? this.leftVp : -10000, y: this.isVisible ? this.topVp : -10000 })
        .zIndex(1)
        if (this.isVisible && !this.shouldUseFullscreenWindow()) {
          this.buildCornerMaskLayer()
        }
      }
      .width(this.isVisible ? '100%' : 1)
      .height(this.isVisible ? '100%' : 1)
      .position({ x: 0, y: 0 })
      .hitTestBehavior(this.isVisible ? HitTestMode.Default : HitTestMode.Transparent)
      .expandSafeArea(
        this.shouldUseFullscreenWindow() ? [SafeAreaType.SYSTEM] : [],
        this.shouldUseFullscreenWindow() ? [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM] : []
      )
    } else {
      Column()
        .width(0)
        .height(0)
        .hitTestBehavior(HitTestMode.Transparent)
    }
  }

  @Builder
  private buildMaskLayer(): void {
    Stack() {
      this.buildMaskBlock(0, 0, this.screenWidthVp, this.topVp)
      this.buildMaskBlock(0, this.topVp + this.heightVp, this.screenWidthVp, this.getBottomMaskHeight())
      this.buildMaskBlock(0, this.topVp, this.leftVp, this.heightVp)
      this.buildMaskBlock(this.leftVp + this.widthVp, this.topVp, this.getRightMaskWidth(), this.heightVp)
    }
    .width('100%')
    .height('100%')
    .hitTestBehavior(HitTestMode.Transparent)
    .zIndex(0)
  }

  @Builder
  private buildMaskBlock(x: number, y: number, width: number, height: number): void {
    if (width > 0 && height > 0) {
      Column()
        .width(`${width}vp`)
        .height(`${height}vp`)
        .backgroundColor(SURFACE_MASK_COLOR)
        .position({ x, y })
        .hitTestBehavior(HitTestMode.Default)
        .onClick(() => {
          const layout = LxAppSurface.getActiveLayout();
          if (layout) {
            LxAppSurface.close(layout.id, layout.appId, 'user');
          }
        })
    }
  }

  @Builder
  private buildCornerMaskLayer(): void {
    if (this.shouldMaskTopLeftCorner()) {
      this.buildCornerMaskBlock(this.leftVp, this.topVp, CornerMaskPosition.TopLeft)
    }
    if (this.shouldMaskTopRightCorner()) {
      this.buildCornerMaskBlock(this.getCornerMaskRight(), this.topVp, CornerMaskPosition.TopRight)
    }
    if (this.shouldMaskBottomLeftCorner()) {
      this.buildCornerMaskBlock(this.leftVp, this.getCornerMaskBottom(), CornerMaskPosition.BottomLeft)
    }
    if (this.shouldMaskBottomRightCorner()) {
      this.buildCornerMaskBlock(this.getCornerMaskRight(), this.getCornerMaskBottom(), CornerMaskPosition.BottomRight)
    }
  }

  @Builder
  private buildCornerMaskBlock(x: number, y: number, cornerPosition: CornerMaskPosition): void {
    SurfaceCornerMask({ radius: SURFACE_CORNER_RADIUS, cornerPosition })
    .position({ x, y })
    .hitTestBehavior(HitTestMode.Transparent)
    .zIndex(2)
  }

  private getCornerMaskRight(): number {
    return this.leftVp + this.widthVp - SURFACE_CORNER_RADIUS;
  }

  private getCornerMaskBottom(): number {
    return this.topVp + this.heightVp - SURFACE_CORNER_RADIUS;
  }

  private shouldMaskTopLeftCorner(): boolean {
    return this.surfacePosition === SurfacePosition.Center || this.surfacePosition === SurfacePosition.Bottom || this.surfacePosition === SurfacePosition.Right;
  }

  private shouldMaskTopRightCorner(): boolean {
    return this.surfacePosition === SurfacePosition.Center || this.surfacePosition === SurfacePosition.Bottom || this.surfacePosition === SurfacePosition.Left;
  }

  private shouldMaskBottomLeftCorner(): boolean {
    return this.surfacePosition === SurfacePosition.Center || this.surfacePosition === SurfacePosition.Top || this.surfacePosition === SurfacePosition.Right;
  }

  private shouldMaskBottomRightCorner(): boolean {
    return this.surfacePosition === SurfacePosition.Center || this.surfacePosition === SurfacePosition.Top || this.surfacePosition === SurfacePosition.Left;
  }

  private getRightMaskWidth(): number {
    return Math.max(0, this.screenWidthVp - this.leftVp - this.widthVp);
  }

  private getBottomMaskHeight(): number {
    return Math.max(0, this.screenHeightVp - this.topVp - this.heightVp);
  }

  private getBorderRadius(): BorderRadiuses {
    if (this.shouldUseFullscreenWindow()) {
      return { topLeft: 0, topRight: 0, bottomLeft: 0, bottomRight: 0 };
    }
    if (this.widthVp > 0 && this.heightVp > 0) {
      switch (this.surfacePosition) {
        case SurfacePosition.Bottom:
          return { topLeft: SURFACE_CORNER_RADIUS, topRight: SURFACE_CORNER_RADIUS, bottomLeft: 0, bottomRight: 0 };
        case SurfacePosition.Left:
          return { topLeft: 0, topRight: SURFACE_CORNER_RADIUS, bottomLeft: 0, bottomRight: SURFACE_CORNER_RADIUS };
        case SurfacePosition.Right:
          return { topLeft: SURFACE_CORNER_RADIUS, topRight: 0, bottomLeft: SURFACE_CORNER_RADIUS, bottomRight: 0 };
        case SurfacePosition.Top:
          return { topLeft: 0, topRight: 0, bottomLeft: SURFACE_CORNER_RADIUS, bottomRight: SURFACE_CORNER_RADIUS };
        default:
          break;
      }
    }
    return {
      topLeft: SURFACE_CORNER_RADIUS,
      topRight: SURFACE_CORNER_RADIUS,
      bottomLeft: SURFACE_CORNER_RADIUS,
      bottomRight: SURFACE_CORNER_RADIUS
    };
  }

  private shouldUseFullscreenWindow(): boolean {
    return this.fullWidth && this.fullHeight;
  }

  private enterFullscreenMode(): void {
    if (this.fullscreenWindowActive) {
      return;
    }
    this.fullscreenWindowActive = true;
    const opVersion = ++this.windowOpVersion;
    try {
      const context = getContext(this) as common.UIAbilityContext;
      context.windowStage.getMainWindow().then((mainWindow) => {
        if (opVersion !== this.windowOpVersion) {
          return;
        }
        mainWindow.setWindowSystemBarEnable([])
          .then(() => {
            if (opVersion !== this.windowOpVersion) {
              return;
            }
            hilog.info(DOMAIN, TAG, 'Entered fullscreen surface mode');
          })
          .catch(() => {
            if (opVersion !== this.windowOpVersion) {
              return;
            }
            hilog.error(DOMAIN, TAG, 'Failed to apply fullscreen surface bars');
          });
      }).catch(() => {
        if (opVersion !== this.windowOpVersion) {
          return;
        }
        hilog.error(DOMAIN, TAG, 'Failed to get main window for fullscreen surface');
      });
    } catch {
      hilog.error(DOMAIN, TAG, 'Failed to enter fullscreen surface mode');
    }
  }

  private exitFullscreenMode(): void {
    if (!this.fullscreenWindowActive) {
      return;
    }
    this.fullscreenWindowActive = false;
    const opVersion = ++this.windowOpVersion;
    try {
      const context = getContext(this) as common.UIAbilityContext;
      context.windowStage.getMainWindow().then((mainWindow) => {
        if (opVersion !== this.windowOpVersion) {
          return;
        }
        mainWindow.setWindowSystemBarEnable(['status', 'navigation'])
          .then(() => {
            if (opVersion !== this.windowOpVersion) {
              return;
            }
            return mainWindow.setPreferredOrientation(window.Orientation.UNSPECIFIED);
          })
          .then(() => {
            if (opVersion !== this.windowOpVersion) {
              return;
            }
            hilog.info(DOMAIN, TAG, 'Exited fullscreen surface mode');
          })
          .catch(() => {
            if (opVersion !== this.windowOpVersion) {
              return;
            }
            hilog.error(DOMAIN, TAG, 'Failed to restore fullscreen surface bars');
          });
      }).catch(() => {
        if (opVersion !== this.windowOpVersion) {
          return;
        }
        hilog.error(DOMAIN, TAG, 'Failed to get main window for fullscreen surface restore');
      });
    } catch {
      hilog.error(DOMAIN, TAG, 'Failed to exit fullscreen surface mode');
    }
  }

}
