import { common, ConfigurationConstant } from '@kit.AbilityKit';
import { image } from '@kit.ImageKit';
import { fileIo } from '@kit.CoreFileKit';

/**
 * Runtime half of the launch screen: resource loading and process-wide state
 * for the full-screen splash layer LxAppContainer renders over the home app
 * until its page first renders (or a timeout).
 *
 * Resources are looked up by the names the CLI generates
 * (`lingxia_splash_background` color, `lingxia_splash` media — the resource
 * manager picks dark variants itself); when the color is absent the splash is
 * disabled entirely. An online-updated image dropped under
 * `<filesDir>/lingxia/splash/{light,dark}.png` takes precedence over the
 * bundled media on the next launch.
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
    const dark = context.config?.colorMode === ConfigurationConstant.ColorMode.COLOR_MODE_DARK;
    const base = context.filesDir + '/lingxia/splash/';
    const candidates = dark ? [base + 'dark.png', base + 'light.png'] : [base + 'light.png'];
    for (const path of candidates) {
      try {
        if (fileIo.accessSync(path)) {
          const source = image.createImageSource(path);
          if (source) {
            return await source.createPixelMap();
          }
        }
      } catch (_error) {
        // fall through to the next candidate
      }
    }
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
}
