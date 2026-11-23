import { display } from '@kit.ArkUI';

export type HingeOrientation = 'vertical' | 'horizontal';
export type HingePosture = 'flat' | 'half-opened';

export interface GpuCanvasOrigin {
  x: number;
  y: number;
}

export interface GpuCanvasSize {
  width: number;
  height: number;
}

export interface HingeRect {
  x: number;
  y: number;
  width: number;
  height: number;
  orientation: HingeOrientation;
  posture: HingePosture;
}

/**
 * Read the current physical fold crease intersecting a page canvas (in local vp),
 * returning `null` on non-foldable devices or when the crease does not intersect.
 */
export function readLocalHinge(
  canvasOriginPx: GpuCanvasOrigin,
  canvasSizePx: GpuCanvasSize,
  densityPixels: number,
): HingeRect | null {
  try {
    if (!display.isFoldable()) {
      return null;
    }
    const region = display.getCurrentFoldCreaseRegion();
    const rects = region?.creaseRects ?? [];
    if (rects.length === 0) {
      return null;
    }
    const crease = rects[0];
    const left = Math.max(canvasOriginPx.x, crease.left);
    const top = Math.max(canvasOriginPx.y, crease.top);
    const right = Math.min(canvasOriginPx.x + canvasSizePx.width, crease.left + crease.width);
    const bottom = Math.min(canvasOriginPx.y + canvasSizePx.height, crease.top + crease.height);
    // A seamless fold has zero thickness; across the crease it must lie strictly inside the
    // canvas, or the page would split at its own edge.
    const horizontal = crease.width >= crease.height;
    const crosses = horizontal
      ? right > left && crease.top > canvasOriginPx.y
        && crease.top + crease.height < canvasOriginPx.y + canvasSizePx.height
      : bottom > top && crease.left > canvasOriginPx.x
        && crease.left + crease.width < canvasOriginPx.x + canvasSizePx.width;
    if (!crosses) {
      return null;
    }
    const scale = densityPixels > 0 ? densityPixels : 1;
    const widthVp = Math.max(0, right - left) / scale;
    const heightVp = Math.max(0, bottom - top) / scale;
    const orientation: HingeOrientation = horizontal ? 'horizontal' : 'vertical';
    const status = display.getFoldStatus();
    const posture: HingePosture =
      status === display.FoldStatus.FOLD_STATUS_HALF_FOLDED ? 'half-opened' : 'flat';
    const hinge: HingeRect = {
      x: (left - canvasOriginPx.x) / scale,
      y: (top - canvasOriginPx.y) / scale,
      width: widthVp,
      height: heightVp,
      orientation,
      posture,
    };
    return hinge;
  } catch {
    return null;
  }
}

/**
 * Subscribe to HarmonyOS `foldStatusChange` events and return an unsubscribe callback.
 */
export function observeFoldStatus(onChange: (posture: HingePosture) => void): () => void {
  const listener = (status: display.FoldStatus): void => {
    onChange(status === display.FoldStatus.FOLD_STATUS_HALF_FOLDED ? 'half-opened' : 'flat');
  };
  try {
    display.on('foldStatusChange', listener);
    return (): void => {
      try {
        display.off('foldStatusChange', listener);
      } catch {
        // Ignore teardown errors on non-foldable targets.
      }
    };
  } catch {
    return (): void => {};
  }
}
