import { isIOS } from "../platform.js";
import { effectiveOpacity } from "./style.js";
import { isFallbackElement } from "./structure.js";

export interface IOSCompositionAnchor {
  viewportRect: { x: number; y: number; width: number; height: number };
  contentRect: { x: number; y: number; width: number; height: number };
  scrollExtent: { x: number; y: number };
}

let nextAnchor = 0;

/** Distinguish coincident roots without guessing from their screen position. */
export function iosAnchorExtent(index: number): { x: number; y: number } {
  if (!Number.isSafeInteger(index) || index < 0 || index >= 4096) {
    throw new RangeError("iOS composition anchor capacity exceeded");
  }
  return { x: 16 + (index % 64) * 4, y: 16 + Math.floor(index / 64) * 4 };
}

/** An isolated scrolling layer; author children stay in their original slots. */
export class IOSRootComposition {
  private anchor?: HTMLDivElement;
  private extent?: { x: number; y: number };
  private presented = false;
  private ready = false;

  setup(shadow: ShadowRoot): void {
    if (!isIOS() || this.anchor || nextAnchor >= 4096) return;
    this.extent = iosAnchorExtent(nextAnchor++);
    const anchor = document.createElement("div");
    anchor.setAttribute("aria-hidden", "true");
    anchor.style.cssText = "position:absolute;inset:0;overflow:scroll;-webkit-overflow-scrolling:touch;overscroll-behavior:none;scrollbar-width:none;";
    const content = document.createElement("div");
    content.style.cssText = `width:calc(100% + ${this.extent.x}px);height:calc(100% + ${this.extent.y}px);pointer-events:none;`;
    anchor.append(content);
    shadow.append(anchor);
    this.anchor = anchor;
    this.updatePointerEvents();
  }

  measure(root: HTMLElement): IOSCompositionAnchor | undefined {
    if (!this.anchor || !this.extent) return;
    // The initial contract uses untransformed CSS pixels. Never attach a native
    // view to a guessed layer when WebKit and DOM coordinates cannot agree.
    if ((window.visualViewport?.scale ?? 1) !== 1 || effectiveOpacity(root) !== 1) return;
    for (let node: Element | null = root; node; node = node.parentElement) {
      const css = getComputedStyle(node);
      if ([css.transform, css.translate, css.rotate, css.scale, css.perspective]
        .some(value => value && value !== "none")) return;
      if (css.zoom && css.zoom !== "1" && css.zoom !== "normal") return;
    }
    const box = this.anchor.getBoundingClientRect();
    if (box.width <= 0 || box.height <= 0) return;
    // A scrolling layer clips to its bounds. Preserve overflowing roots via
    // the overlay path until their larger composition bounds are supported.
    for (const child of Array.from(root.querySelectorAll("lx-native-view,lx-native-text,lx-native-button,lx-native-cover,lx-video"))) {
      if (isFallbackElement(child) || child.closest("lx-native-root") !== root) continue;
      const rect = child.getBoundingClientRect();
      if (rect.width && rect.height && (rect.left < box.left - .5 || rect.top < box.top - .5 ||
          rect.right > box.right + .5 || rect.bottom > box.bottom + .5)) return;
    }
    return {
      viewportRect: { x: box.x, y: box.y, width: box.width, height: box.height },
      contentRect: { x: box.x + window.scrollX, y: box.y + window.scrollY, width: box.width, height: box.height },
      scrollExtent: this.extent,
    };
  }

  setPresentation(sameLayer: boolean): void {
    this.presented = sameLayer;
    this.updatePointerEvents();
  }

  setReady(ready: boolean): void {
    this.ready = ready;
    this.updatePointerEvents();
  }

  private updatePointerEvents(): void {
    // Layer discovery can finish before the lease. Never cover DOM fallback
    // controls until the native presentation is ready to own input.
    if (this.anchor) this.anchor.style.pointerEvents = this.presented && this.ready ? "auto" : "none";
  }

  destroy(): void {
    this.anchor?.remove();
    this.anchor = undefined;
    this.extent = undefined;
    this.presented = false;
    this.ready = false;
  }
}
