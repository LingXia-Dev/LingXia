import { hilog } from '@kit.PerformanceAnalysisKit';
import { LxLog } from '../../app/LxLog';
import { Lingxia } from '../../app/Lingxia';
import { common } from '@kit.AbilityKit';
import { display, window } from '@kit.ArkUI';
import { webview } from '@kit.ArkWeb';
import { LingXiaWebView } from '../LingXiaWebView';
import { addWebViewUiCallback, removeWebViewUiCallback, WebViewInfo, WebViewManager, toWebTag } from '../WebViewBridge';
import { disposePageInstance, notifyPageInstanceHidden, notifyPageInstanceMounted, notifyPageInstanceVisible, onSurfaceClosed, urlCallbackDispatch } from 'liblingxia.so';
import { FloatWindowManager } from './LxAppFloatWindow';

const DOMAIN = 0x0000;
// Auth handoff surfaces (lingxia-cloud-client open_url_callback_surface) use
// this id prefix; their Web runs incognito so each interactive login starts
// with a clean IdP session (real logout; lx.auth.add() can switch accounts).
const AUTH_CALLBACK_SURFACE_ID_PREFIX = 'cloud-auth-';
const TAG = 'LingXia.Surface';
const CONTENT_PAGE = 0;
const CONTENT_URL = 1;
const KIND_WINDOW = 0;
const KIND_POPUP = 1;
const ROLE_MAIN = 0;
const ROLE_ASIDE = 1;
const ROLE_FLOAT = 2;
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
  kind: number;
  role: number;
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

interface SurfacePlanAside {
  id: string;
}

interface SurfacePlanFloat {
  id: string;
}

interface SurfaceLayoutPlan {
  mains?: string[];
  asides?: SurfacePlanAside[];
  floats?: SurfacePlanFloat[];
}

export class LxAppSurface {
  private static managerRef: SurfaceManager | null = null;
  private static layouts: Map<string, SurfaceLayoutConfig> = new Map();
  private static visibleSurfaceKeys: Set<string> = new Set();

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
	    positionValue: string,
	    roleValue: string
	  ): boolean {
	    const content = parseInt(contentValue, 10);
	    const kind = parseInt(kindValue, 10);
    const role = normalizeRole(roleValue);
	    if (!id || !appId || !path || parseInt(sessionId, 10) <= 0) {
	      return false;
	    }
	    if (kind !== KIND_POPUP && kind !== KIND_WINDOW) {
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
      LxLog.e(TAG, 'present failed: SurfaceManager not initialized');
      return false;
    }

    const position = normalizePosition(positionValue);
    const dimensions = computeDimensions(
      parseSize(widthValue),
      parseSize(heightValue),
      parseRatio(widthRatioValue),
      parseRatio(heightRatioValue),
      position,
      kind,
      role
    );

    const layout: SurfaceLayoutConfig = {
      id,
      appId,
	      path,
	      pageInstanceId,
	      content,
      kind,
      role,
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

    const key = LxAppSurface.surfaceKey(id, appId);
    LxAppSurface.layouts.delete(key);
    LxAppSurface.layouts.set(key, layout);
    LxAppSurface.visibleSurfaceKeys.add(key);

    // Floats live in their own sub-window (never in the main tree — a hidden
    // in-tree ArkWeb would keep intercepting touches); the window's content
    // page attaches the page webview and fires mounted/visible itself.
    if (role === ROLE_FLOAT) {
      FloatWindowManager.setCloseHandler((closeId: string, closeAppId: string, reason: string) => {
        LxAppSurface.close(closeId, closeAppId, reason);
      });
      FloatWindowManager.open(key, layout).then((opened: boolean) => {
        if (!opened) {
          LxAppSurface.failPageSurface(layout);
        }
      });
      return true;
    }

	    const success = LxAppSurface.managerRef.present(layout);
	    if (success) {
      if (content === CONTENT_PAGE) {
	        LxAppSurface.showPageSurfaceWhenReady(layout);
	      }
	    } else {
      LxAppSurface.layouts.delete(key);
      LxAppSurface.visibleSurfaceKeys.delete(key);
	      if (content === CONTENT_PAGE) {
	        disposePageInstance(pageInstanceId, 'failed');
	      }
      onSurfaceClosed(appId, id, 'failed');
    }
	    return success;
	  }

  public static presentLayout(windowId: string, layoutJson: string): boolean {
    if (!windowId || !layoutJson) {
      return false;
    }
    let plan: SurfaceLayoutPlan;
    try {
      plan = JSON.parse(layoutJson) as SurfaceLayoutPlan;
    } catch (error) {
      LxLog.e(TAG, `presentLayout failed to parse windowId=${windowId}: ${error}`);
      return false;
    }

    const desiredIds: Set<string> = new Set();
    const mains = plan.mains ?? [];
    mains.forEach((id: string) => desiredIds.add(id));
    const asides = plan.asides ?? [];
    asides.forEach((aside: SurfacePlanAside) => desiredIds.add(aside.id));
    const floats = plan.floats ?? [];
    floats.forEach((surface: SurfacePlanFloat) => desiredIds.add(surface.id));

    const toDismiss: SurfaceLayoutConfig[] = [];
    LxAppSurface.layouts.forEach((layout: SurfaceLayoutConfig) => {
      const key = LxAppSurface.surfaceKey(layout.id, layout.appId);
      if (
        layout.fullWidth &&
        layout.fullHeight &&
        LxAppSurface.visibleSurfaceKeys.has(key) &&
        !desiredIds.has(layout.id)
      ) {
        toDismiss.push(layout);
      }
    });
    toDismiss.forEach((layout: SurfaceLayoutConfig) => {
      LxAppSurface.close(layout.id, layout.appId, 'programmatic');
    });
    return true;
  }

	  public static close(id: string, appId: string, reason: string): boolean {
	    hilog.info(DOMAIN, TAG, `close requested id=${id} appId=${appId} reason=${reason}`);
	    if (!id || !appId) {
      hilog.warn(DOMAIN, TAG, 'close rejected: empty id or appId');
	      return false;
	    }
	    const normalizedReason = normalizeReason(reason);
    const key = LxAppSurface.surfaceKey(id, appId);
	    const layout = LxAppSurface.layouts.get(key) ?? null;
	    const manager = LxAppSurface.managerRef;
    if (layout && layout.role === ROLE_FLOAT) {
      if (layout.content === CONTENT_PAGE) {
        notifyPageInstanceHidden(layout.pageInstanceId, normalizedReason);
      }
      LxAppSurface.layouts.delete(key);
      LxAppSurface.visibleSurfaceKeys.delete(key);
      FloatWindowManager.close(key).then(() => {
        onSurfaceClosed(layout.appId, layout.id, normalizedReason);
      });
      return true;
    }
	    if (!layout) {
	      hilog.warn(DOMAIN, TAG, `close without matching active layout id=${id} appId=${appId}`);
	      if (manager && manager.hasSurface(id, appId)) {
	        manager.close(id, appId);
        LxAppSurface.visibleSurfaceKeys.delete(key);
	        setTimeout(() => {
	          manager.finishClose(id, appId);
	          onSurfaceClosed(appId, id, normalizedReason);
        }, SURFACE_CLOSE_FINALIZE_DELAY_MS);
        return true;
      }
	      onSurfaceClosed(appId, id, normalizedReason);
	      return true;
	    }
	    const result = manager ? manager.close(id, appId) : true;
	    if (result) {
	      if (layout.content === CONTENT_PAGE) {
	        notifyPageInstanceHidden(layout.pageInstanceId, normalizedReason);
	      }
      LxAppSurface.layouts.delete(key);
      LxAppSurface.visibleSurfaceKeys.delete(key);
	      setTimeout(() => {
	        if (manager) {
	          manager.finishClose(layout.id, layout.appId);
        }
        onSurfaceClosed(layout.appId, layout.id, normalizedReason);
      }, SURFACE_CLOSE_FINALIZE_DELAY_MS);
	    }
	    return result;
	  }

  public static closeTopmostVisible(reason: string): boolean {
    const layout = LxAppSurface.getActiveLayout();
    if (!layout) {
      return false;
    }
    return LxAppSurface.close(layout.id, layout.appId, reason);
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
    const key = LxAppSurface.surfaceKey(id, appId);
	    const layout = LxAppSurface.layouts.get(key) ?? null;
	    if (!layout) {
	      hilog.warn(DOMAIN, TAG, `show: surface not found id=${id} appId=${appId}`);
	      return false;
	    }
    const manager = LxAppSurface.managerRef;
    if (layout.role === ROLE_FLOAT) {
      LxAppSurface.visibleSurfaceKeys.add(key);
      FloatWindowManager.show(key);
      if (layout.content === CONTENT_PAGE && layout.pageInstanceId) {
        notifyPageInstanceVisible(layout.pageInstanceId);
      }
      return true;
    }
	    if (layout.content === CONTENT_URL) {
	      if (!manager) {
	        return false;
	      }
      const success = manager.setVisible(appId, true, id);
      if (success) {
        LxAppSurface.visibleSurfaceKeys.add(key);
      }
	      return success;
	    } else {
	      if (!manager || !manager.setVisible(appId, true, id)) {
	        return false;
	      }
      LxAppSurface.visibleSurfaceKeys.add(key);
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
    const key = LxAppSurface.surfaceKey(id, appId);
	    const layout = LxAppSurface.layouts.get(key) ?? null;
	    const manager = LxAppSurface.managerRef;
	    if (!layout) {
	      hilog.warn(DOMAIN, TAG, `hide: surface not found id=${id} appId=${appId}`);
	      return manager ? manager.setVisible(appId, false, id) : false;
	    }
    if (layout.role === ROLE_FLOAT) {
      LxAppSurface.visibleSurfaceKeys.delete(key);
      FloatWindowManager.hide(key);
      if (layout.content === CONTENT_PAGE && layout.pageInstanceId) {
        notifyPageInstanceHidden(layout.pageInstanceId, 'hidden');
      }
      return true;
    }
	    if (layout.content === CONTENT_URL) {
	      if (!manager) {
	        return false;
	      }
      const success = manager.setVisible(appId, false, id);
      if (success) {
        LxAppSurface.visibleSurfaceKeys.delete(key);
      }
	      return success;
	    } else {
	      if (!manager || !manager.setVisible(appId, false, id)) {
	        return false;
	      }
      LxAppSurface.visibleSurfaceKeys.delete(key);
	      if (layout.pageInstanceId) {
	        notifyPageInstanceHidden(layout.pageInstanceId, 'hidden');
	      }
    }
    return true;
	  }

	  public static getActiveLayout(): SurfaceLayoutConfig | null {
    let active: SurfaceLayoutConfig | null = null;
    LxAppSurface.layouts.forEach((layout: SurfaceLayoutConfig) => {
      if (LxAppSurface.visibleSurfaceKeys.has(LxAppSurface.surfaceKey(layout.id, layout.appId))) {
        active = layout;
      }
    });
	    return active;
	  }

  public static isSurfacePageWebView(appId: string, webTag: string): boolean {
    let matched = false;
    LxAppSurface.layouts.forEach((layout: SurfaceLayoutConfig) => {
      if (!matched && layout.appId === appId && layout.pageInstanceId && LxAppSurface.matchesPageSurface(layout, webTag)) {
        matched = true;
      }
    });
    return matched;
  }

  private static surfaceKey(id: string, appId: string): string {
    return `${appId}::${id}`;
  }

  private static matchesPageSurface(layout: SurfaceLayoutConfig, webTag: string): boolean {
    const exactTag = `${toWebTag(layout.appId, layout.path)}#${layout.pageInstanceId}`;
    return webTag === exactTag || webTag.startsWith(`${exactTag}#`);
  }

	  private static showPageSurfaceWhenReady(layout: SurfaceLayoutConfig, attempt: number = 0): boolean {
	    if (layout.content !== CONTENT_PAGE) {
	      return true;
	    }
    const current = LxAppSurface.layouts.get(LxAppSurface.surfaceKey(layout.id, layout.appId)) ?? null;
	    if (!current) {
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
    LxLog.e(TAG, `showPageSurfaceWhenReady failed id=${layout.id} appId=${layout.appId} path=${layout.path} pageInstanceId=${layout.pageInstanceId}`);
    LxAppSurface.failPageSurface(layout);
    return false;
	  }

	  private static failPageSurface(layout: SurfaceLayoutConfig): void {
    const key = LxAppSurface.surfaceKey(layout.id, layout.appId);
	    const current = LxAppSurface.layouts.get(key) ?? null;
	    if (!current) {
	      return;
	    }
    if (layout.role === ROLE_FLOAT) {
      FloatWindowManager.close(key);
    }
	    const manager = LxAppSurface.managerRef;
	    if (manager) {
	      manager.close(layout.id, layout.appId);
	    }
	    if (layout.content === CONTENT_PAGE && layout.pageInstanceId) {
	      notifyPageInstanceHidden(layout.pageInstanceId, 'failed');
	      disposePageInstance(layout.pageInstanceId, 'failed');
	    }
    LxAppSurface.layouts.delete(key);
    LxAppSurface.visibleSurfaceKeys.delete(key);
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

function normalizeRole(value: string): number {
  const parsed = parseInt(value, 10);
  if (parsed === ROLE_MAIN || parsed === ROLE_ASIDE || parsed === ROLE_FLOAT) {
    return parsed;
  }
  return ROLE_FLOAT;
}

function computeDimensions(
  widthValue: number,
  heightValue: number,
  widthRatio: number,
  heightRatio: number,
  position: SurfacePosition,
  kind: number,
  role: number
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
  if (kind === KIND_WINDOW || role === ROLE_ASIDE) {
    return {
      screenWidthVp: screenWidth,
      screenHeightVp: screenHeight,
      widthVp: screenWidth,
      heightVp: screenHeight,
      leftVp: 0,
      topVp: 0,
      bottomInset: 0,
      fullWidth: true,
      fullHeight: true
    };
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
  if (trimmed === 'user' || trimmed === 'programmatic' || trimmed === 'owner_closed' || trimmed === 'app_closed' || trimmed === 'reclaimed' || trimmed === 'failed') {
    return trimmed;
  }
  return 'unknown';
}

export function isHttpUrl(value: string): boolean {
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

export function isSameOrigin(baseUrl: string, targetUrl: string): boolean {
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

interface SurfaceRenderState extends SurfaceLayoutConfig {
  isVisible: boolean;
  pageWebTag: string;
}

function surfaceRenderState(layout: SurfaceLayoutConfig, isVisible: boolean, pageWebTag: string): SurfaceRenderState {
  return {
    id: layout.id,
    appId: layout.appId,
    path: layout.path,
    pageInstanceId: layout.pageInstanceId,
    content: layout.content,
    kind: layout.kind,
    role: layout.role,
    screenWidthVp: layout.screenWidthVp,
    screenHeightVp: layout.screenHeightVp,
    widthVp: layout.widthVp,
    heightVp: layout.heightVp,
    leftVp: layout.leftVp,
    topVp: layout.topVp,
    position: layout.position,
    bottomInset: layout.bottomInset,
    fullWidth: layout.fullWidth,
    fullHeight: layout.fullHeight,
    isVisible,
    pageWebTag
  };
}

@Component
export struct SurfaceManager {
  @State private surfaces: SurfaceRenderState[] = [];
  private pageControllers: Map<string, webview.WebviewController> = new Map();
  private webControllers: Map<string, webview.WebviewController> = new Map();
  private windowOpVersion: number = 0;
  private fullscreenWindowActive: boolean = false;
  private webViewUiCallback = (action: string, info: WebViewInfo): void => {
    if (action !== 'create') {
      return;
    }
    for (let i = 0; i < this.surfaces.length; i++) {
      const surface = this.surfaces[i];
      if (surface.content === CONTENT_PAGE && this.matchesPageSurface(surface, info.webtag)) {
        this.attachPageSurface(surface);
        return;
      }
    }
  }

  aboutToAppear() {
    LxAppSurface.setManagerRef(this);
    addWebViewUiCallback(this.webViewUiCallback);
  }

  aboutToDisappear() {
    LxAppSurface.clearManagerRef(this);
    removeWebViewUiCallback(this.webViewUiCallback);
    this.surfaces = [];
    this.pageControllers.clear();
    this.webControllers.clear();
    this.exitFullscreenMode();
  }

  public present(state: SurfaceLayoutConfig): boolean {
    const key = this.surfaceKey(state.id, state.appId);
    if (state.content === CONTENT_URL && !this.webControllers.has(key)) {
      this.webControllers.set(key, new webview.WebviewController());
    }

    const existing = this.findSurface(state.id, state.appId);
    const pageWebTag = existing ? existing.pageWebTag : '';
    const nextSurface = surfaceRenderState(state, true, pageWebTag);
    const next: SurfaceRenderState[] = [];
    for (let i = 0; i < this.surfaces.length; i++) {
      const surface = this.surfaces[i];
      if (this.surfaceKey(surface.id, surface.appId) !== key) {
        next.push(surface);
      }
    }
    next.push(nextSurface);
    this.surfaces = next;
    this.syncFullscreenMode();
    hilog.info(DOMAIN, TAG, `present surface id=${state.id} role=${state.role} content=${state.content} pos=${state.position} url=${state.path} frame=${state.leftVp},${state.topVp},${state.widthVp},${state.heightVp}`);
    return true;
  }

  public attachPageSurface(layout: SurfaceLayoutConfig): boolean {
    const key = this.surfaceKey(layout.id, layout.appId);
    const current = this.findSurface(layout.id, layout.appId);
    if (!current) {
      return false;
    }
    const infos = WebViewManager.getAllWebViewInfos();
    for (let i = 0; i < infos.length; i++) {
      const info = infos[i];
      if (this.matchesPageSurface(layout, info.webtag)) {
        const alreadyAttached = current.pageWebTag === info.webtag;
        this.pageControllers.set(key, info.controller);
        this.replaceSurface(surfaceRenderState(current, true, info.webtag));
        this.syncFullscreenMode();
        if (!alreadyAttached) {
          notifyPageInstanceMounted(layout.pageInstanceId);
          notifyPageInstanceVisible(layout.pageInstanceId);
          hilog.info(DOMAIN, TAG, `attached page surface id=${layout.id} webtag=${info.webtag}`);
        }
        return true;
      }
    }
    hilog.warn(DOMAIN, TAG, `attachPageSurface: WebView not found for ${layout.appId}:${layout.path}#${layout.pageInstanceId}`);
    return false;
  }

  public hasSurface(id: string, appId: string): boolean {
    return this.findSurface(id, appId) !== null;
  }

  public close(id: string, appId: string): boolean {
    const surface = this.findSurface(id, appId);
    if (!surface) {
      hilog.warn(DOMAIN, TAG, `close: surface not found id=${id} appId=${appId}`);
      return false;
    }
    this.replaceSurface(surfaceRenderState(surface, false, surface.pageWebTag));
    this.syncFullscreenMode();
    return true;
  }

  public finishClose(id: string, appId: string): void {
    const key = this.surfaceKey(id, appId);
    const next: SurfaceRenderState[] = [];
    for (let i = 0; i < this.surfaces.length; i++) {
      const surface = this.surfaces[i];
      if (this.surfaceKey(surface.id, surface.appId) !== key) {
        next.push(surface);
      }
    }
    this.surfaces = next;
    this.pageControllers.delete(key);
    this.webControllers.delete(key);
    this.syncFullscreenMode();
  }

  public setVisible(appId: string, visible: boolean, surfaceId?: string): boolean {
    let found = false;
    const next: SurfaceRenderState[] = [];
    for (let i = 0; i < this.surfaces.length; i++) {
      const surface = this.surfaces[i];
      if (surface.appId === appId && (!surfaceId || surface.id === surfaceId)) {
        next.push(surfaceRenderState(surface, visible, surface.pageWebTag));
        found = true;
      } else {
        next.push(surface);
      }
    }
    if (!found) {
      hilog.warn(DOMAIN, TAG, `setVisible: surface not found appId=${appId} id=${surfaceId ?? ''}`);
      return false;
    }
    this.surfaces = next;
    this.syncFullscreenMode();
    return true;
  }

  build() {
    if (this.surfaces.length > 0) {
      Stack() {
        ForEach(this.surfaces, (surface: SurfaceRenderState) => {
          this.SurfaceLayer(surface)
        }, (surface: SurfaceRenderState) => this.surfaceKey(surface.id, surface.appId))
      }
      .width('100%')
      .height('100%')
      .position({ x: 0, y: 0 })
      .hitTestBehavior(this.hasVisibleSurface() ? HitTestMode.Default : HitTestMode.Transparent)
    } else {
      Column()
        .width(0)
        .height(0)
        .hitTestBehavior(HitTestMode.Transparent)
    }
  }

  @Builder
  private SurfaceLayer(surface: SurfaceRenderState): void {
    if ((surface.content === CONTENT_URL && surface.isVisible) || (surface.content === CONTENT_PAGE && surface.pageWebTag.length > 0)) {
      Stack() {
        if (surface.isVisible) {
          this.buildMaskLayer(surface)
        }
        Column() {
          if (surface.content === CONTENT_URL) {
            Web({ src: '', controller: this.getWebController(surface), incognitoMode: surface.id.startsWith(AUTH_CALLBACK_SURFACE_ID_PREFIX) })
              .domStorageAccess(true)
              .javaScriptAccess(true)
              .fileAccess(false)
              .mixedMode(MixedMode.None)
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
                // URL surfaces host multi-origin journeys (an auth page may
                // hop through an external IdP and back), so cross-origin
                // http(s) navigation is allowed. Non-http(s) schemes are app
                // deep links (dingtalk://, feishu://, ...) that IdP pages use
                // to launch their native app; hand those to the system.
                if (urlCallbackDispatch(targetUrl)) {
                  return true;
                }
                if (isHttpUrl(targetUrl)) {
                  return false;
                }
                Lingxia.launchWithUrl(targetUrl);
                return true;
              })
              .width('100%')
              .height('100%')
              .onAppear(() => {
                const url = surface.path;
                const controller = this.getWebController(surface);
                const isAuth = surface.id.startsWith(AUTH_CALLBACK_SURFACE_ID_PREFIX);
                setTimeout(() => {
                  try {
                    if (isAuth) {
                      // `incognitoMode` only stops on-disk persistence; the
                      // incognito cookie/storage lives in a shared in-memory
                      // store, so a prior IdP SSO session would carry into the
                      // next lx.auth.add() and re-recognize the last account.
                      // Wipe that store (synchronously, before loadUrl issues
                      // the first request) so every login starts clean.
                      try {
                        webview.WebCookieManager.clearAllCookiesSync(true);
                        webview.WebStorage.deleteAllData(true);
                      } catch (clearErr) {
                        LxLog.e(TAG, `failed to clear incognito web data: ${clearErr}`);
                      }
                    }
                    hilog.info(DOMAIN, TAG, `load surface url=${url}`);
                    controller.loadUrl(url);
                  } catch (error) {
                    LxLog.e(TAG, `failed to load surface URL: ${error}`);
                  }
                }, 0);
              })
              .onPageBegin((event) => {
                hilog.info(DOMAIN, TAG, `surface page begin: ${event?.url ?? ''}`);
              })
              .onPageEnd((event) => {
                hilog.info(DOMAIN, TAG, `surface page end: ${event?.url ?? ''}`);
              })
              .hitTestBehavior(surface.isVisible ? HitTestMode.Block : HitTestMode.None)
          } else if (surface.pageWebTag.length > 0) {
            LingXiaWebView({
              webTag: surface.pageWebTag,
              controller: this.getPageController(surface),
              renderVisibility: Visibility.Visible,
              renderWidth: '100%',
              renderHeight: '100%',
              renderBorderRadius: this.getBorderRadius(surface),
              renderClip: true,
              renderZIndex: 0,
              renderHitTest: surface.isVisible ? HitTestMode.Block : HitTestMode.None
            })
          }
        }
        .width(surface.isVisible ? (surface.widthVp > 0 ? `${surface.widthVp}vp` : '90%') : 1)
        .height(surface.isVisible ? (surface.heightVp > 0 ? `${surface.heightVp}vp` : '60%') : 1)
        .backgroundColor(surface.content === CONTENT_URL ? Color.White : Color.Transparent)
        .borderRadius(this.getBorderRadius(surface))
        .clip(!this.shouldUseFullscreenWindow(surface))
        .visibility(Visibility.Visible)
        .hitTestBehavior(surface.isVisible ? HitTestMode.Default : HitTestMode.None)
        .position({ x: surface.isVisible ? surface.leftVp : -10000, y: surface.isVisible ? surface.topVp : -10000 })
        .zIndex(1)
        if (surface.isVisible && !this.shouldUseFullscreenWindow(surface)) {
          this.buildCornerMaskLayer(surface)
        }
        // Drill-in asides/windows get a page-like Back affordance. A full-screen
        // float draws no SDK chrome — the lxapp owns its own close UI and the
        // system back gesture (closeTopmostVisible) is the escape hatch.
        if (surface.isVisible && this.shouldUseFullscreenWindow(surface) && surface.role !== ROLE_FLOAT) {
          this.buildFullscreenBackButton(surface)
        }
      }
      .width(surface.isVisible ? '100%' : 1)
      .height(surface.isVisible ? '100%' : 1)
      .position({ x: 0, y: 0 })
      .hitTestBehavior(surface.isVisible ? HitTestMode.Default : HitTestMode.Transparent)
      .expandSafeArea(
        this.shouldUseFullscreenWindow(surface) ? [SafeAreaType.SYSTEM] : [],
        this.shouldUseFullscreenWindow(surface) ? [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM] : []
      )
    }
  }

  @Builder
  private buildMaskLayer(surface: SurfaceRenderState): void {
    Stack() {
      this.buildMaskBlock(surface, 0, 0, surface.screenWidthVp, surface.topVp)
      this.buildMaskBlock(surface, 0, surface.topVp + surface.heightVp, surface.screenWidthVp, this.getBottomMaskHeight(surface))
      this.buildMaskBlock(surface, 0, surface.topVp, surface.leftVp, surface.heightVp)
      this.buildMaskBlock(surface, surface.leftVp + surface.widthVp, surface.topVp, this.getRightMaskWidth(surface), surface.heightVp)
    }
    .width('100%')
    .height('100%')
    .hitTestBehavior(HitTestMode.Transparent)
    .zIndex(0)
  }

  @Builder
  private buildMaskBlock(surface: SurfaceRenderState, x: number, y: number, width: number, height: number): void {
    if (width > 0 && height > 0) {
      Column()
        .width(`${width}vp`)
        .height(`${height}vp`)
        .backgroundColor(SURFACE_MASK_COLOR)
        .position({ x, y })
        .hitTestBehavior(HitTestMode.Default)
        .onClick(() => {
          LxAppSurface.close(surface.id, surface.appId, 'user');
        })
    }
  }

  @Builder
  private buildCornerMaskLayer(surface: SurfaceRenderState): void {
    if (this.shouldMaskTopLeftCorner(surface)) {
      this.buildCornerMaskBlock(surface.leftVp, surface.topVp, CornerMaskPosition.TopLeft)
    }
    if (this.shouldMaskTopRightCorner(surface)) {
      this.buildCornerMaskBlock(this.getCornerMaskRight(surface), surface.topVp, CornerMaskPosition.TopRight)
    }
    if (this.shouldMaskBottomLeftCorner(surface)) {
      this.buildCornerMaskBlock(surface.leftVp, this.getCornerMaskBottom(surface), CornerMaskPosition.BottomLeft)
    }
    if (this.shouldMaskBottomRightCorner(surface)) {
      this.buildCornerMaskBlock(this.getCornerMaskRight(surface), this.getCornerMaskBottom(surface), CornerMaskPosition.BottomRight)
    }
  }

  @Builder
  private buildCornerMaskBlock(x: number, y: number, cornerPosition: CornerMaskPosition): void {
    SurfaceCornerMask({ radius: SURFACE_CORNER_RADIUS, cornerPosition })
    .position({ x, y })
    .hitTestBehavior(HitTestMode.Transparent)
    .zIndex(2)
  }

  @Builder
  private buildFullscreenBackButton(surface: SurfaceRenderState): void {
    Image($rawfile('icons/icon_back.svg'))
      .width(18)
      .height(18)
      .fillColor('#1A1A1A')
      .padding(9)
      .width(36)
      .height(36)
      .backgroundColor('rgba(255,255,255,0.88)')
      .borderRadius(18)
      .position({ x: 12, y: 10 })
      .zIndex(3)
      .hitTestBehavior(HitTestMode.Default)
      .onClick(() => {
        LxAppSurface.close(surface.id, surface.appId, 'user');
      })
  }

  private findSurface(id: string, appId: string): SurfaceRenderState | null {
    for (let i = 0; i < this.surfaces.length; i++) {
      const surface = this.surfaces[i];
      if (surface.id === id && surface.appId === appId) {
        return surface;
      }
    }
    return null;
  }

  private replaceSurface(nextSurface: SurfaceRenderState): void {
    const key = this.surfaceKey(nextSurface.id, nextSurface.appId);
    const next: SurfaceRenderState[] = [];
    for (let i = 0; i < this.surfaces.length; i++) {
      const surface = this.surfaces[i];
      if (this.surfaceKey(surface.id, surface.appId) === key) {
        next.push(nextSurface);
      } else {
        next.push(surface);
      }
    }
    this.surfaces = next;
  }

  private getWebController(surface: SurfaceRenderState): webview.WebviewController {
    const key = this.surfaceKey(surface.id, surface.appId);
    let controller = this.webControllers.get(key) ?? null;
    if (!controller) {
      controller = new webview.WebviewController();
      this.webControllers.set(key, controller);
    }
    return controller;
  }

  private getPageController(surface: SurfaceRenderState): webview.WebviewController {
    return this.pageControllers.get(this.surfaceKey(surface.id, surface.appId)) ?? new webview.WebviewController();
  }

  private matchesPageSurface(layout: SurfaceLayoutConfig, webTag: string): boolean {
    const exactTag = `${toWebTag(layout.appId, layout.path)}#${layout.pageInstanceId}`;
    return webTag === exactTag || webTag.startsWith(`${exactTag}#`);
  }

  private surfaceKey(id: string, appId: string): string {
    return `${appId}::${id}`;
  }

  private hasVisibleSurface(): boolean {
    for (let i = 0; i < this.surfaces.length; i++) {
      if (this.surfaces[i].isVisible) {
        return true;
      }
    }
    return false;
  }

  private getCornerMaskRight(surface: SurfaceRenderState): number {
    return surface.leftVp + surface.widthVp - SURFACE_CORNER_RADIUS;
  }

  private getCornerMaskBottom(surface: SurfaceRenderState): number {
    return surface.topVp + surface.heightVp - SURFACE_CORNER_RADIUS;
  }

  private shouldMaskTopLeftCorner(surface: SurfaceRenderState): boolean {
    return surface.position === SurfacePosition.Center || surface.position === SurfacePosition.Bottom || surface.position === SurfacePosition.Right;
  }

  private shouldMaskTopRightCorner(surface: SurfaceRenderState): boolean {
    return surface.position === SurfacePosition.Center || surface.position === SurfacePosition.Bottom || surface.position === SurfacePosition.Left;
  }

  private shouldMaskBottomLeftCorner(surface: SurfaceRenderState): boolean {
    return surface.position === SurfacePosition.Center || surface.position === SurfacePosition.Top || surface.position === SurfacePosition.Right;
  }

  private shouldMaskBottomRightCorner(surface: SurfaceRenderState): boolean {
    return surface.position === SurfacePosition.Center || surface.position === SurfacePosition.Top || surface.position === SurfacePosition.Left;
  }

  private getRightMaskWidth(surface: SurfaceRenderState): number {
    return Math.max(0, surface.screenWidthVp - surface.leftVp - surface.widthVp);
  }

  private getBottomMaskHeight(surface: SurfaceRenderState): number {
    return Math.max(0, surface.screenHeightVp - surface.topVp - surface.heightVp);
  }

  private getBorderRadius(surface: SurfaceRenderState): BorderRadiuses {
    if (this.shouldUseFullscreenWindow(surface)) {
      return { topLeft: 0, topRight: 0, bottomLeft: 0, bottomRight: 0 };
    }
    if (surface.widthVp > 0 && surface.heightVp > 0) {
      switch (surface.position) {
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

  private shouldUseFullscreenWindow(surface: SurfaceRenderState): boolean {
    return surface.fullWidth && surface.fullHeight;
  }

  private hasVisibleFullscreenSurface(): boolean {
    for (let i = 0; i < this.surfaces.length; i++) {
      const surface = this.surfaces[i];
      if (surface.isVisible && this.shouldUseFullscreenWindow(surface)) {
        return true;
      }
    }
    return false;
  }

  private syncFullscreenMode(): void {
    if (this.hasVisibleFullscreenSurface()) {
      this.enterFullscreenMode();
    } else {
      this.exitFullscreenMode();
    }
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
            this.fullscreenWindowActive = false;
            LxLog.e(TAG, 'Failed to apply fullscreen surface bars');
          });
      }).catch(() => {
        if (opVersion !== this.windowOpVersion) {
          return;
        }
        this.fullscreenWindowActive = false;
        LxLog.e(TAG, 'Failed to get main window for fullscreen surface');
      });
    } catch {
      this.fullscreenWindowActive = false;
      LxLog.e(TAG, 'Failed to enter fullscreen surface mode');
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
            LxLog.e(TAG, 'Failed to restore fullscreen surface bars');
          });
      }).catch(() => {
        if (opVersion !== this.windowOpVersion) {
          return;
        }
        LxLog.e(TAG, 'Failed to get main window for fullscreen surface restore');
      });
    } catch {
      LxLog.e(TAG, 'Failed to exit fullscreen surface mode');
    }
  }
}
