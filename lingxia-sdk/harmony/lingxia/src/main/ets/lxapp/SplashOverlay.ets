import { common } from '@kit.AbilityKit';
import { image } from '@kit.ImageKit';

/**
 * Runtime half of the launch screen: resource loading and process-wide state
 * for the full-screen splash layer LxAppContainer renders over the home app
 * until its page first renders (or a timeout).
 *
 * Resources are looked up by the names the CLI generates
 * (`lingxia_splash_background` color, `lingxia_splash` media); when the color
 * is absent the splash is disabled entirely.
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

  public static async loadImage(context: common.UIAbilityContext): Promise<image.PixelMap | null> {
    try {
      const data = context.resourceManager.getMediaByNameSync('lingxia_splash');
      const buffer = data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength);
      const source = image.createImageSource(buffer as ArrayBuffer);
      if (source) {
        return await source.createPixelMap();
      }
    } catch (_error) {
      // no bundled splash media
    }
    return null;
  }

  /** A cover the host's splash hook picked at runtime — a plain file path. */
  public static async loadImageFile(path: string): Promise<image.PixelMap | null> {
    try {
      const source = image.createImageSource(path);
      if (source) {
        return await source.createPixelMap();
      }
    } catch (_error) {
      // unreadable cover file; keep the bundled one
    }
    return null;
  }
}
