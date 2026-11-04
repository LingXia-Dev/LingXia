import { NodeContent } from '@kit.ArkUI';
import { LxLog } from '../app/LxLog';

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

class GpuPageIdentity {
  appId: string = '';
  path: string = '';
  webTag: string = '';
}

class GpuRecovery {
  error: string = '';
  retained: boolean = false;
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
  textMenu: string = '';
  topLayer: boolean = false;
  recovery: GpuRecovery | null = null;
  holdSurface: boolean = false;

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
      case 'textMenu':
        page.textMenu = payload;
        break;
      case 'topLayer':
        page.topLayer = payload === 'true';
        break;
      case 'recovery':
        page.recovery = payload.length > 0 ? JSON.parse(payload) as GpuRecovery : null;
        break;
      case 'revealed':
        page.holdSurface = payload === 'true';
        break;
      case 'bootstrapOverdue':
      case 'retainSurface':
        break;
      default:
        LxLog.w(TAG, `unknown GPU page event ${kind}`);
        return false;
    }
    GpuPages.changed.forEach((listener: GpuPageListener) => listener(page));
    return true;
  }

  private static copy(text: string): void {
    // The clipboard write goes through LxAppClipboard once the container integration lands.
    LxLog.d(TAG, `copy ${text.length} chars`);
  }
}
