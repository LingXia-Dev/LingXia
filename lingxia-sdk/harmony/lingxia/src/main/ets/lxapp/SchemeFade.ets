import { componentSnapshot, componentUtils } from '@kit.ArkUI';
import { image } from '@kit.ImageKit';
import { accessibility } from '@kit.AccessibilityKit';
import { gpuPageFrame } from 'liblingxia.so';
import { GpuPageModel } from '../view/GpuPages';

/** Native bars repaint the moment the scheme changes while the GPU page needs a frame, so the
 *  page column holds a picture of itself until the page shows the new scheme. */
export const SCHEME_FADE_TIMEOUT_MS = 300;
export const SCHEME_FADE_DURATION_MS = 200;

/** One piece of the picture, placed in vp from the container's top-left. */
export interface SchemeFadePicture {
  pixels: image.PixelMap;
  x: number;
  y: number;
  width: number;
  height: number;
}

/** The page's last frame beneath the bars around it; empty when the page holds no frame. A
 *  snapshot leaves the XComponent surface out, so the frame comes from the page itself. */
export function takeSchemePictures(rootId: string, page: GpuPageModel, barIds: string[]): SchemeFadePicture[] {
  const area = page.onScreen;
  if (area === null) {
    return [];
  }
  const frame = framePixels(page.token);
  if (frame === null) {
    return [];
  }
  const root = componentUtils.getRectangleById(rootId);
  const originX = root.windowOffset.x;
  const originY = root.windowOffset.y;
  const pictures: SchemeFadePicture[] = [{
    pixels: frame,
    x: px2vp(area.x - originX), y: px2vp(area.y - originY),
    width: px2vp(area.width), height: px2vp(area.height)
  }];
  for (const id of barIds) {
    try {
      const rect = componentUtils.getRectangleById(id);
      if (rect.size.width <= 0 || rect.size.height <= 0) {
        continue;
      }
      pictures.push({
        pixels: componentSnapshot.getSync(id),
        x: px2vp(rect.windowOffset.x - originX), y: px2vp(rect.windowOffset.y - originY),
        width: px2vp(rect.size.width), height: px2vp(rect.size.height)
      });
    } catch (_) {
      // A bar not on screen has no node to picture.
    }
  }
  return pictures;
}

export function animationsReduced(): boolean {
  try {
    return accessibility.isAnimationReduceEnabledSync();
  } catch (_) {
    return false;
  }
}

function framePixels(token: number): image.PixelMap | null {
  const bytes = gpuPageFrame(token);
  if (!bytes || bytes.byteLength <= 8) {
    return null;
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset, 8);
  const width = view.getUint32(0, true);
  const height = view.getUint32(4, true);
  if (width === 0 || height === 0 || bytes.byteLength !== 8 + width * height * 4) {
    return null;
  }
  const rows = bytes.buffer.slice(bytes.byteOffset + 8, bytes.byteOffset + bytes.byteLength);
  const options: image.InitializationOptions = {
    size: { width: width, height: height },
    srcPixelFormat: image.PixelMapFormat.RGBA_8888,
    pixelFormat: image.PixelMapFormat.RGBA_8888,
    alphaType: image.AlphaType.UNPREMUL,
    editable: false
  };
  try {
    return image.createPixelMapSync(rows, options);
  } catch (_) {
    return null;
  }
}
