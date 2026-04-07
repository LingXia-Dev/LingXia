import { window } from '@kit.ArkUI';
import common from '@ohos.app.ability.common';
import { LxApp } from './LxApp';

/**
 * Top-level entry point for the LingXia SDK.
 */
export class Lingxia {

  /**
   * Initialize the LingXia SDK and automatically open the Home LxApp.
   *
   * Call this once from `onWindowStageCreate`, before any LxApp UI is presented.
   *
   * @param context UIAbility context
   * @param windowStage WindowStage from onWindowStageCreate
   */
  public static initialize(context?: common.UIAbilityContext, windowStage?: window.WindowStage): void {
    LxApp.initialize(context, windowStage);
  }
}
