import { HostOverlays } from '../lxapp/HostOverlays';
import { componentSnapshot, NodeContent } from '@kit.ArkUI';
import { image } from '@kit.ImageKit';
import { util } from '@kit.ArkTS';
import { BusinessError, pasteboard } from '@kit.BasicServicesKit';
import { LxLog } from '../app/LxLog';
import { GpuFrameTextMenu } from './generated/GpuFrame';
import { gpuPageFocusByKey, gpuPageReveal, gpuPageScreenshotReply, gpuPageWebViewPicture } from 'liblingxia.so';
import { webview } from '@kit.ArkWeb';
import { WebViewManager } from '../lxapp/WebViewBridge';
import { GpuRect } from './GpuEnvironment';

const TAG = 'LingXia.GpuPages';

/** Colours the host paints its chrome with while a GPU page is shown (ARGB). */
export class GpuChrome {
  canvas: number = 0;
  bar: number = 0;
  barLine: number = 0;
  content: number = 0;
  contentMuted: number = 0;
  accent: number = 0;
  dark: boolean = false;
}

/** Press or focus-ring feedback: the node's rounded rect, then its clip masks (vp). */
export class GpuFeedback {
  ring: boolean = false;
  shape: number[] = [];
  color: number = 0;
  width: number = 0;
}

/** Handles over a static-text selection, page vp: each end is [center x, line bottom, line
 *  middle], or null while that end is scrolled out or clipped. */
export class GpuSelectionHandles {
  identity: string = '';
  color: number = 0;
  ends: (number[] | null)[] = [];
}

class GpuPageIdentity {
  appId: string = '';
  path: string = '';
  webTag: string = '';
}

export class GpuRecovery {
  error: string = '';
  retained: boolean = false;
  /** Index into the panel's messages: update required, session failed, update failed, display failed. */
  message: number = 3;
  /** Index into its actions, retry or reload; -1 offers none. */
  action: number = -1;
  /** Offer Back: the user's back path leaves this page. */
  back: boolean = false;
  /** Set on arrival: each panel the core shows gets its own nodes and automation source. */
  serial: number = 0;
}

let recoverySerial = 0;

export interface GpuClipMount {
  bounds: number[];
  radius: number;
  radii: number[][] | null;
}

interface GpuPictureRequest {
  ids: string[];
  scale: number;
}

/** Embedded `<webview>` mount geometry and state for a GPU page, in page virtual pixels. */
export class GpuWebViewMount {
  webId: string = '';
  webTag: string = '';
  bounds: number[] = [0, 0, 0, 0];
  visibleBounds: number[] = [0, 0, 0, 0];
  radius: number = 0;
  radii: number[][] | null = null;
  clips: GpuClipMount[] = [];
  degraded: boolean = false;
  accessibilityHidden: boolean = false;
  /** The web page handles its own drags: none passes to the View page. */
  handlesDrags: boolean = false;
  /** The page's appearance, followed live. */
  dark?: boolean;
}

/** One GPU page as ArkTS sees it: the slot Rust mounts its XComponent node into, and chrome state. */
@Observed
export class GpuPageModel {
  readonly token: number;
  readonly appId: string;
  readonly path: string;
  readonly webTag: string;
  readonly content: NodeContent = new NodeContent();
  mounted: boolean = false;
  loaded: boolean = false;
  chrome: GpuChrome | null = null;
  press: GpuFeedback | null = null;
  focusRing: GpuFeedback | null = null;
  textMenu: GpuFrameTextMenu | null = null;
  handles: GpuSelectionHandles | null = null;
  webViews: GpuWebViewMount[] = [];
  /** Web views being pictured, and those asked before their `Web` attached. */
  picturing: Set<string> = new Set();
  unattachedPictures: Set<string> = new Set();
  topLayer: boolean = false;
  /** The bar-snapshot generation the frame on screen draws; 0 when none. */
  chromeSnapshot: number = 0;
  recovery: GpuRecovery | null = null;
  holdSurface: boolean = false;
  /** Its surface has shown a frame and still holds it. */
  revealed: boolean = false;
  /** Mounts the XComponent; the core lets a hidden page give its surface up to save memory. */
  surfaceKept: boolean = true;
  /** Runs once the core reveals the page; a later reveal replaces it. */
  revealWaiter: (() => void) | null = null;
  /** Whether the window's focus is in its keyboard state; set while the page's component lives. */
  keyboardFocus: (() => boolean) | null = null;
  /** Where the page shows in its window, device pixels; null while it is hidden. A window
   *  snapshot leaves the XComponent surface out, so screenshots draw this page in beneath. */
  onScreen: GpuRect | null = null;

  constructor(token: number, identity: GpuPageIdentity) {
    this.token = token;
    this.appId = identity.appId;
    this.path = identity.path;
    this.webTag = identity.webTag;
  }
}

export type GpuPageListener = (page: GpuPageModel) => void;

/**
 * Registry of live GPU pages by token, fed by Rust through `gpuPageEvent`. The container
 * hosts a `GpuPage` for a page's webtag in place of its WebView.
 */
export class GpuPages {
  /** ArkWeb errors: the controller has no `Web` yet; the snapshot cannot be taken yet. */
  private static readonly WEB_NOT_ATTACHED = 17100001;
  private static readonly WEB_NOT_ENABLED = 17100004;
  private static pages: Map<number, GpuPageModel> = new Map();
  private static created: GpuPageListener[] = [];
  private static disposed: GpuPageListener[] = [];
  private static changed: GpuPageListener[] = [];

  static byToken(token: number): GpuPageModel | undefined {
    return GpuPages.pages.get(token);
  }

  static byWebTag(webTag: string): GpuPageModel | undefined {
    for (const page of GpuPages.pages.values()) {
      if (page.webTag === webTag) {
        return page;
      }
    }
    return undefined;
  }

  static all(): GpuPageModel[] {
    return Array.from(GpuPages.pages.values());
  }

  static onCreated(listener: GpuPageListener): void {
    GpuPages.created.push(listener);
  }

  static onDisposed(listener: GpuPageListener): void {
    GpuPages.disposed.push(listener);
  }

  /** Drop `listener` from every list it was added to. */
  static off(listener: GpuPageListener): void {
    GpuPages.created = GpuPages.created.filter((l: GpuPageListener) => l !== listener);
    GpuPages.disposed = GpuPages.disposed.filter((l: GpuPageListener) => l !== listener);
    GpuPages.changed = GpuPages.changed.filter((l: GpuPageListener) => l !== listener);
  }

  /**
   * Ask the core to reveal `page` in place of the one on screen; `then` runs when it has a frame
   * to show (or the core gives up waiting), never with the surface still empty.
   */
  static reveal(page: GpuPageModel, then: () => void): void {
    page.revealWaiter = then;
    page.surfaceKept = true;
    gpuPageReveal(page.token, page.revealed);
  }

  /** Chrome, top layer or recovery changed; whoever shows the page re-reads it. */
  static onChanged(listener: GpuPageListener): void {
    GpuPages.changed.push(listener);
  }

  /** `gpuPageEvent(token, kind, payload)` from Rust; tokens are decimal strings. */
  static event(tokenText: string, kind: string, payload: string): boolean {
    const token = Number.parseInt(tokenText);
    if (kind === 'copy') {
      GpuPages.copy(payload);
      return true;
    }
    if (kind === 'create') {
      const page = new GpuPageModel(token, JSON.parse(payload) as GpuPageIdentity);
      GpuPages.pages.set(token, page);
      GpuPages.created.forEach((listener: GpuPageListener) => listener(page));
      return true;
    }
    const page = GpuPages.pages.get(token);
    if (page === undefined) {
      LxLog.w(TAG, `${kind} for unknown GPU page ${tokenText}`);
      return false;
    }
    switch (kind) {
      case 'dispose':
        GpuPages.pages.delete(token);
        GpuPages.dropRecovery(page);
        GpuPages.disposed.forEach((listener: GpuPageListener) => listener(page));
        return true;
      case 'loaded':
        page.loaded = true;
        break;
      case 'hostChrome':
        page.chrome = JSON.parse(payload) as GpuChrome;
        break;
      case 'feedback': {
        const feedback = JSON.parse(payload) as GpuFeedback;
        const shown: GpuFeedback | null = feedback.shape.length > 0 ? feedback : null;
        if (feedback.ring) {
          page.focusRing = shown;
        } else {
          page.press = shown;
        }
        break;
      }
      case 'focusGained':
        // A touch, a tab switch and a returning window focus the node too; only a key shows a ring.
        if (page.keyboardFocus?.() === true) {
          gpuPageFocusByKey(token);
        }
        break;
      case 'textMenu':
        page.textMenu = payload.length > 0 ? JSON.parse(payload) as GpuFrameTextMenu : null;
        break;
      case 'selectionHandles':
        page.handles = payload.length > 0 ? JSON.parse(payload) as GpuSelectionHandles : null;
        break;
      case 'topLayer':
        page.topLayer = payload === 'true';
        break;
      case 'chromeSnapshot':
        page.chromeSnapshot = Number(payload) || 0;
        break;
      case 'recovery':
        if (payload.length > 0) {
          const recovery = JSON.parse(payload) as GpuRecovery;
          const shown = page.recovery;
          // The same panel again keeps its nodes: a new one would leave the chrome empty
          // until it lays out.
          if (shown !== null && shown.retained === recovery.retained && shown.message === recovery.message
            && shown.action === recovery.action && shown.back === recovery.back) {
            break;
          }
          // A panel replaced or cleared before it laid out never drops itself; readers must not
          // wait for it.
          GpuPages.dropRecovery(page);
          recovery.serial = ++recoverySerial;
          page.recovery = recovery;
          const parts: string[] = ['message'];
          if (recovery.back) {
            parts.push('back');
          }
          if (recovery.action >= 0) {
            parts.push('action');
          }
          // Readers wait for these until the panel has laid them out.
          HostOverlays.expect(`gpu-recovery-${page.token}-${recovery.serial}`, parts);
        } else {
          GpuPages.dropRecovery(page);
          page.recovery = null;
        }
        break;
      case 'revealed': {
        LxLog.d(TAG, `revealed ${page.webTag} hold=${payload}`);
        page.holdSurface = payload === 'true';
        page.revealed = true;
        const waiter = page.revealWaiter;
        page.revealWaiter = null;
        waiter?.();
        break;
      }
      case 'retainSurface':
        if (payload === 'true') {
          page.surfaceKept = true;
        } else if (page.onScreen === null && page.revealWaiter === null) {
          // A decision that raced the page back onto the screen is stale.
          page.surfaceKept = false;
          page.revealed = false;
        }
        break;
      case 'webviews':
        page.webViews = payload.length > 0 ? JSON.parse(payload) as GpuWebViewMount[] : [];
        break;
      case 'screenshot':
        GpuPages.captureScreenshot(token, payload);
        return true;
      case 'pictureWebviews':
        GpuPages.pictureWebViews(page, payload);
        return true;
      default:
        LxLog.w(TAG, `unknown GPU page event ${kind}`);
        return false;
    }
    GpuPages.changed.forEach((listener: GpuPageListener) => listener(page));
    return true;
  }

  private static dropRecovery(page: GpuPageModel): void {
    if (page.recovery !== null) {
      HostOverlays.drop(`gpu-recovery-${page.token}-${page.recovery.serial}`);
    }
  }

  /** Picture each asked web view as its page shows it, for the page to draw while it moves. */
  private static pictureWebViews(page: GpuPageModel, payload: string): void {
    let ids: string[] = [];
    try {
      ids = (JSON.parse(payload) as GpuPictureRequest).ids;
    } catch (_) {
      return;
    }
    for (const id of ids) {
      GpuPages.pictureWebView(page, id, 0);
    }
  }

  /** A web view's `Web` attached: take the pictures asked before it could. */
  static webViewAttached(page: GpuPageModel, webTag: string): void {
    const mount = page.webViews.find((m: GpuWebViewMount) => m.webTag === webTag);
    if (mount && page.unattachedPictures.delete(mount.webId)) {
      GpuPages.pictureWebView(page, mount.webId, 0);
    }
  }

  private static pictureWebView(page: GpuPageModel, id: string, attempt: number): void {
    if (page.picturing.has(id)) {
      return;
    }
    const mount = page.webViews.find((m: GpuWebViewMount) => m.webId === id);
    const controller = mount ? WebViewManager.getController(mount.webTag) : undefined;
    if (!mount || !controller) {
      return;
    }
    page.picturing.add(id);
    const failed = (code: number, why: string) => {
      page.picturing.delete(id);
      if (code === GpuPages.WEB_NOT_ATTACHED) {
        // The native web view loads before ArkUI builds its `Web`.
        page.unattachedPictures.add(id);
      } else if (code === GpuPages.WEB_NOT_ENABLED && attempt < 4) {
        setTimeout(() => GpuPages.pictureWebView(page, id, attempt + 1), 250 * (attempt + 1));
      } else {
        LxLog.w(TAG, `web view picture ${id}: ${why}`);
      }
    };
    try {
      controller.webPageSnapshot({ id: id, size: { width: mount.bounds[2], height: mount.bounds[3] } },
        (err: BusinessError, result: webview.SnapshotResult) => {
          const pixelMap = result?.imagePixelMap;
          if ((err && err.code !== 0) || !pixelMap) {
            failed(err?.code ?? 0, `${err?.code} ${err?.message}`);
            return;
          }
          const packer = image.createImagePacker();
          const opts: image.PackingOption = { format: 'image/png', quality: 100 };
          packer.packing(pixelMap, opts, (packErr: BusinessError | null, buffer: ArrayBuffer) => {
            page.picturing.delete(id);
            try {
              if (!packErr || packErr.code === 0) {
                LxLog.d(TAG, `web view picture ${id}: ${buffer.byteLength} bytes`);
                gpuPageWebViewPicture(page.token, id, buffer);
              }
            } finally {
              try { packer.release(); } catch (_) {}
              try { pixelMap.release(); } catch (_) {}
            }
          });
        });
    } catch (e) {
      failed((e as BusinessError).code ?? 0, `${e}`);
    }
  }

  private static captureScreenshot(token: number, requestId: string): void {
    try {
      componentSnapshot.get(`lx-gpu-page-${token}`, (err: BusinessError, pixelMap: image.PixelMap) => {
        if (err && err.code !== 0 || !pixelMap) {
          gpuPageScreenshotReply(requestId, '');
          return;
        }
        try {
          const packer = image.createImagePacker();
          const opts: image.PackingOption = { format: 'image/png', quality: 100 };
          packer.packing(pixelMap, opts, (packErr: BusinessError | null, buffer: ArrayBuffer) => {
            try {
              if (packErr && packErr.code !== 0) {
                gpuPageScreenshotReply(requestId, '');
                return;
              }
              const b64 = new util.Base64Helper().encodeToStringSync(new Uint8Array(buffer));
              gpuPageScreenshotReply(requestId, b64);
            } finally {
              try { packer.release(); } catch (_) {}
              try { pixelMap.release(); } catch (_) {}
            }
          });
        } catch (_) {
          try { pixelMap.release(); } catch (_) {}
          gpuPageScreenshotReply(requestId, '');
        }
      });
    } catch (_) {
      gpuPageScreenshotReply(requestId, '');
    }
  }

  private static copy(text: string): void {
    pasteboard.getSystemPasteboard().setData(pasteboard.createData(pasteboard.MIMETYPE_TEXT_PLAIN, text))
      .catch((error: Object) => LxLog.e(TAG, `copy failed: ${error}`));
  }
}
