import { window } from '@kit.ArkUI';
import common from '@ohos.app.ability.common';
import { LxApp } from './LxApp';

/**
 * Top-level entry point for the LingXia SDK.
 */
export class Lingxia {

  /**
   * Product-app entry point. Initializes the runtime for use with `LxAppNavigation`.
   */
  public static quickStart(context: common.UIAbilityContext, windowStage: window.WindowStage): void {
    LxApp.initializeRuntime(context, windowStage);
  }
}
