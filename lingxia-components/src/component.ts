import { isIOS } from "./platform.js";

export type Rect = { x: number; y: number; width: number; height: number };

let autoId = 0;

export function ensureComponentId(el: HTMLElement, prefix: string, currentId?: string | null): string {
  const attrId = el.getAttribute("id");
  if (attrId && attrId.length > 0) {
    return attrId;
  }
  if (currentId && currentId.length > 0) {
    return currentId;
  }
  autoId += 1;
  const id = `${prefix}-${Date.now().toString(36)}-${autoId.toString(36)}`;
  el.setAttribute("id", id);
  return id;
}

/**
 * iOS Native Component Rendering Helper.
 * Creates a scroll container that triggers WKChildScrollView creation.
 * Reusable by all native components (video, map, camera, etc.)
 */
export class iOSNativeComponentHelper {
  private scrollContainer: HTMLDivElement | null = null;
  private host: HTMLElement;
  private componentId: string;

  constructor(host: HTMLElement, componentId: string) {
    this.host = host;
    this.componentId = componentId;
  }

  /**
   * Setup scroll container for iOS. No-op on other platforms.
   */
  setup(): void {
    if (!isIOS() || this.scrollContainer) return;

    const container = document.createElement("div");
    container.id = `${this.componentId}-scroll-container`;
    container.style.cssText = `
      position: absolute;
      top: 0;
      left: 0;
      width: 100%;
      height: 100%;
      overflow: scroll;
      -webkit-overflow-scrolling: touch;
      pointer-events: none;
    `;

    // Add inner content larger than container to trigger WKChildScrollView creation
    const inner = document.createElement("div");
    inner.style.cssText = "width: 200%; height: 200%;";
    container.appendChild(inner);

    // Disable scrolling but keep overflow:scroll to trigger WKChildScrollView
    container.addEventListener('touchmove', (e) => e.preventDefault(), { passive: false });
    container.addEventListener('scroll', () => {
      container.scrollTop = 0;
      container.scrollLeft = 0;
    });

    this.scrollContainer = container;
    this.host.appendChild(container);
  }

  /**
   * Cleanup scroll container.
   */
  cleanup(): void {
    if (this.scrollContainer && this.host.contains(this.scrollContainer)) {
      this.host.removeChild(this.scrollContainer);
    }
    this.scrollContainer = null;
  }

  /**
   * Get the rect to send to native for WKChildScrollView lookup.
   * Returns null on non-iOS platforms.
   */
  getScrollContainerRect(): Rect | null {
    if (!this.scrollContainer) return null;
    const rect = this.scrollContainer.getBoundingClientRect();
    return {
      x: rect.left,
      y: rect.top,
      width: rect.width,
      height: rect.height
    };
  }

  /**
   * Enhance payload with iOS-specific data.
   * Call this before sending mount/update messages.
   */
  enhancePayload(payload: Record<string, unknown>): void {
    const rect = this.getScrollContainerRect();
    if (rect) {
      payload.scrollContainerRect = rect;
    }
  }
}

export function rectEquals(a: Rect | null, b: Rect, epsilon = 0): boolean {
  if (!a) return false;
  return (
    Math.abs(a.x - b.x) <= epsilon &&
    Math.abs(a.y - b.y) <= epsilon &&
    Math.abs(a.width - b.width) <= epsilon &&
    Math.abs(a.height - b.height) <= epsilon
  );
}

export class NativeComponentUpdateState {
  private lastPropsJson: string | null = null;
  private lastRect: Rect | null = null;
  private lastZIndex: number | null = null;

  reset() {
    this.lastPropsJson = null;
    this.lastRect = null;
    this.lastZIndex = null;
  }

  decide(
    rect: Rect,
    props: Record<string, unknown> | null,
    zIndex: number,
    force = false
  ): { shouldSend: boolean; propsChanged: boolean; rectChanged: boolean; zChanged: boolean } {
    const propsJson = props === null ? this.lastPropsJson : JSON.stringify(props);
    const rectChanged = !rectEquals(this.lastRect, rect);
    const propsChanged = props === null ? false : propsJson !== this.lastPropsJson || force;
    const zChanged = this.lastZIndex !== zIndex;

    const shouldSend = force || rectChanged || propsChanged || zChanged;
    if (!shouldSend) {
      return { shouldSend: false, propsChanged: false, rectChanged: false, zChanged: false };
    }

    if (props !== null) {
      this.lastPropsJson = propsJson;
    }
    this.lastRect = rect;
    this.lastZIndex = zIndex;
    return { shouldSend: true, propsChanged, rectChanged, zChanged };
  }

  shouldSend(rect: Rect, props: Record<string, unknown> | null, zIndex: number, force = false): boolean {
    return this.decide(rect, props, zIndex, force).shouldSend;
  }
}
