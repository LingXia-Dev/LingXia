import { common } from '@kit.AbilityKit';
import { image } from '@kit.ImageKit';

/**
 * Runtime half of the launch screen: resource loading and process-wide state
 * for the splash layer LxAppContainer renders over the home app until its
 * page first renders (or a timeout). Everything decodes synchronously — the
 * layer must be complete in the container's very first frame, because that
 * frame is what the OS start window's exit reveals.
 *
 * Resources are looked up by the names the CLI generates
 * (`lingxia_splash_background` color, `lingxia_splash` cover media,
 * `lingxia_splash_mark` mark media); when the color is absent the splash is
 * disabled entirely.
 */
export class SplashResources {
  public static shownThisProcess: boolean = false;
  public static homeReadySeen: boolean = false;

  public static resolveBackground(context: common.UIAbilityContext): number | null {
    try {
      return context.resourceManager.getColorByNameSync('lingxia_splash_background');
    } catch (_error) {
      return null;
    }
  }

  /** The bundled cover. */
  public static loadImage(context: common.UIAbilityContext): image.PixelMap | null {
    try {
      const data = context.resourceManager.getMediaByNameSync('lingxia_splash');
      const buffer = data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength);
      const source = image.createImageSource(buffer as ArrayBuffer);
      if (source) {
        return source.createPixelMapSync();
      }
    } catch (_error) {
      // no cover configured
    }
    return null;
  }

  /** The start window's mark, for the pixel-identical no-cover overlay. */
  public static loadMark(context: common.UIAbilityContext): image.PixelMap | null {
    try {
      const data = context.resourceManager.getMediaByNameSync('lingxia_splash_mark');
      const buffer = data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength);
      const source = image.createImageSource(buffer as ArrayBuffer);
      if (source) {
        return source.createPixelMapSync();
      }
    } catch (_error) {
      // no mark configured
    }
    return null;
  }

  /** A cover the host's splash hook picked at runtime — a plain file path. */
  public static loadImageFile(path: string): image.PixelMap | null {
    try {
      const source = image.createImageSource(path);
      if (source) {
        return source.createPixelMapSync();
      }
    } catch (_error) {
      // unreadable cover file; fall back to the default launch
    }
    return null;
  }
}
