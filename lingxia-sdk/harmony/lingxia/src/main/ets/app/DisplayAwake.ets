import { window } from '@kit.ArkUI';
import { BusinessError } from '@kit.BasicServicesKit';
import { Lingxia } from './Lingxia';
import { LxLog } from './LxLog';

const TAG = 'LingXia.DisplayAwake';

/** Keep the app window's screen on while an automation run needs it. */
export function setKeepScreenOn(on: boolean): void {
  const ctx = Lingxia.context;
  if (!ctx) {
    LxLog.w(TAG, 'LxApp UIAbility context is not available');
    return;
  }
  window.getLastWindow(ctx)
    .then((win: window.Window) => win.setWindowKeepScreenOn(on))
    .catch((error: BusinessError) => {
      LxLog.w(TAG, `setWindowKeepScreenOn(${on}) failed: code=${error.code} msg=${error.message}`);
    });
}
