import { hilog } from '@kit.PerformanceAnalysisKit';
import { display } from '@kit.ArkUI';
import { webview } from '@kit.ArkWeb';
import { WebViewManager } from '../WebViewBridge';
import { disposePageInstance, notifyPageInstanceHidden, onSurfaceClosed } from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.Surface';
const CONTENT_PAGE = 0;
const CONTENT_URL = 1;
const KIND_POPUP = 1;

export enum SurfacePosition {
  Center = 0,
  Bottom = 1,
  Left = 2,
  Right = 3,
  Top = 4
}

interface SurfaceDimensions {
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
        WebViewManager.notifySurfaceState(previous.appId, previous.path, false, previous.pageInstanceId);
        notifyPageInstanceHidden(previous.pageInstanceId, 'programmatic');
      }
      onSurfaceClosed(previous.appId, previous.id, 'programmatic');
    }
    LxAppSurface.activeLayout = layout;

    const success = LxAppSurface.managerRef.present(layout);
    if (success) {
      if (content === CONTENT_PAGE) {
        WebViewManager.notifySurfaceState(appId, path, true, pageInstanceId);
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
    if (!id || !appId) {
      return false;
    }
    const normalizedReason = normalizeReason(reason);
    const layout = LxAppSurface.activeLayout;
    if (!layout || layout.id !== id || layout.appId !== appId) {
      onSurfaceClosed(appId, id, normalizedReason);
      return true;
    }
    const result = LxAppSurface.managerRef ? LxAppSurface.managerRef.close(appId) : true;
    if (result) {
      if (layout.content === CONTENT_PAGE) {
        WebViewManager.notifySurfaceState(layout.appId, layout.path, false, layout.pageInstanceId);
        notifyPageInstanceHidden(layout.pageInstanceId, normalizedReason);
      }
      onSurfaceClosed(layout.appId, layout.id, normalizedReason);
      LxAppSurface.activeLayout = null;
    }
    return result;
  }

  public static getActiveLayout(): SurfaceLayoutConfig | null {
    return LxAppSurface.activeLayout;
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
export struct SurfaceManager {
  @State private isVisible: boolean = false;
  @State private appId: string = '';
  @State private path: string = '';
  @State private content: number = CONTENT_PAGE;
  @State private widthVp: number = 0;
  @State private heightVp: number = 0;
  @State private leftVp: number = 0;
  @State private topVp: number = 0;
  @State private surfacePosition: SurfacePosition = SurfacePosition.Center;
  private webController: webview.WebviewController = new webview.WebviewController();

  aboutToAppear() {
    LxAppSurface.setManagerRef(this);
  }

  aboutToDisappear() {
    LxAppSurface.clearManagerRef(this);
  }

  public present(state: SurfaceLayoutConfig): boolean {
    this.appId = state.appId;
    this.path = state.path;
    this.content = state.content;
    this.widthVp = state.widthVp;
    this.heightVp = state.heightVp;
    this.leftVp = state.leftVp;
    this.topVp = state.topVp;
    this.surfacePosition = state.position;
    this.isVisible = true;
    hilog.info(DOMAIN, TAG, `present surface id=${state.id} content=${state.content} pos=${state.position} url=${state.path} frame=${state.leftVp},${state.topVp},${state.widthVp},${state.heightVp}`);
    return true;
  }

  public close(appId: string): boolean {
    if (!this.isVisible) {
      return true;
    }
    if (this.appId && this.appId !== appId) {
      hilog.warn(DOMAIN, TAG, `close mismatch: expected ${this.appId}, got ${appId}`);
    }
    this.isVisible = false;
    this.appId = '';
    this.path = '';
    this.content = CONTENT_PAGE;
    this.leftVp = 0;
    this.topVp = 0;
    return true;
  }

  build() {
    if (this.isVisible && this.content === CONTENT_URL) {
      Stack() {
        Column()
          .width('100%')
          .height('100%')
          .backgroundColor('rgba(0,0,0,0.45)')
          .hitTestBehavior(HitTestMode.Default)
          .onClick(() => {
            const layout = LxAppSurface.getActiveLayout();
            if (layout) {
              LxAppSurface.close(layout.id, layout.appId, 'user');
            }
          })
        Column() {
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
        }
        .width(this.widthVp > 0 ? `${this.widthVp}vp` : '90%')
        .height(this.heightVp > 0 ? `${this.heightVp}vp` : '60%')
        .backgroundColor(Color.White)
        .borderRadius(12)
        .clip(true)
        .position({ x: this.leftVp, y: this.topVp })
      }
      .width('100%')
      .height('100%')
    } else {
      Column()
        .width(0)
        .height(0)
        .hitTestBehavior(HitTestMode.Transparent)
    }
  }

}
