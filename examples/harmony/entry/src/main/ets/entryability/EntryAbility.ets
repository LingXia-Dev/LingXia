import { hilog } from '@kit.PerformanceAnalysisKit';
import { window } from '@kit.ArkUI';
import { LxApp, LingxiaBaseAbility } from 'lingxia';
import { lingxiaRegisterExtensions } from 'liblingxia.so';

const DOMAIN = 0x0000;

export default class EntryAbility extends LingxiaBaseAbility {
  /**
   * Register custom native extensions.
   */
  protected registerExtensions(): void {
    lingxiaRegisterExtensions();
  }

  onWindowStageCreate(windowStage: window.WindowStage): void {
    super.onWindowStageCreate(windowStage);
    hilog.info(DOMAIN, 'testTag', '%{public}s', 'Ability onWindowStageCreate');

    // Initialize LxApp with context and windowStage
    LxApp.initialize(this.context, windowStage);

    // Enable WebView debugging
    LxApp.enableWebViewDebugging();

    windowStage.loadContent('pages/Index', (err) => {
      if (err.code) {
        hilog.error(DOMAIN, 'testTag', 'Failed to load the content. Cause: %{public}s', JSON.stringify(err));
        return;
      }
      hilog.info(DOMAIN, 'testTag', 'Succeeded in loading the content.');
    });
  }

  onWindowStageDestroy(): void {
    hilog.info(DOMAIN, 'testTag', '%{public}s', 'Ability onWindowStageDestroy');
  }
}
