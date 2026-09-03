import hilog from '@ohos.hilog';
import { LxLog } from '../../app/LxLog';
import { LxApp } from '../LxApp';
import liblingxia, { getCurrentLxapp, type CurrentLxApp } from 'liblingxia.so';
import { measuredCapsuleRectJson } from '../capsuleGeometry';

const DOMAIN = 0xFF00;
const TAG = 'LingXia.Capsule';

export class LxAppCapsule {
  public static getCapsuleRect(callbackId: string, appId: string): void {
    (async () => {
      try {
        const current: CurrentLxApp = getCurrentLxapp();
        const homeAppId: string | null = LxApp.getInstance().getHomeAppId();
        if (appId === homeAppId || current.appid !== appId) {
          liblingxia.onCallback(callbackId, true, 'null');
          return;
        }

        let json = measuredCapsuleRectJson();
        for (let attempt = 0; attempt < 8 && json === 'null'; attempt++) {
          await new Promise<void>((resolve) => setTimeout(resolve, 16));
          json = measuredCapsuleRectJson();
        }

        hilog.info(DOMAIN, TAG, `[getCapsuleRect] ${json}`);

        const success: boolean = liblingxia.onCallback(callbackId, true, json);
        if (!success) {
          LxLog.e(TAG, `Failed to send capsule rect callback for ID: ${callbackId}`);
        }
      } catch (error) {
        LxLog.e(TAG, `Failed to get capsule rect: ${error}`);
        const errorData = JSON.stringify({
          error: String(error),
          message: 'Failed to get capsule rect'
        });
        liblingxia.onCallback(callbackId, false, errorData);
      }
    })();
  }
}
