import display from '@ohos.display';
import { window } from '@kit.ArkUI';
import hilog from '@ohos.hilog';
import liblingxia from 'liblingxia.so';

const DOMAIN = 0xFF00;
const TAG = 'LingXia.Capsule';

const CAPSULE_BUTTON_TOTAL_WIDTH = 84.5;
const CAPSULE_BUTTON_HEIGHT = 32;
const CAPSULE_TOP_MARGIN = 8;
const CAPSULE_RIGHT_MARGIN = 12;
const DEFAULT_STATUS_BAR_HEIGHT = 44;

export interface CapsuleRect {
  width: number;
  height: number;
  top: number;
  right: number;
  bottom: number;
  left: number;
}

export class LxAppCapsule {
  public static getCapsuleRect(callbackId: string): void {
    (async () => {
      try {
        const defaultDisplay = display.getDefaultDisplaySync();
        const screenWidthVp = Math.round(defaultDisplay.width / defaultDisplay.densityPixels);

        let statusBarHeight = DEFAULT_STATUS_BAR_HEIGHT;
        try {
          const mainWindow = await window.getLastWindow(getContext());
          if (mainWindow) {
            const safeArea = await mainWindow.getWindowAvoidArea(window.AvoidAreaType.TYPE_SYSTEM);
            if (safeArea.topRect.height > 0) {
              statusBarHeight = Math.round(safeArea.topRect.height / defaultDisplay.densityPixels);
            }
          }
        } catch (windowError) {
          hilog.warn(DOMAIN, TAG, `Failed to get window safe area, using default: ${windowError}`);
        }

        const width = CAPSULE_BUTTON_TOTAL_WIDTH;
        const height = CAPSULE_BUTTON_HEIGHT;
        const top = statusBarHeight + CAPSULE_TOP_MARGIN;
        const right = screenWidthVp - CAPSULE_RIGHT_MARGIN;
        const left = right - width;
        const bottom = top + height;

        const capsuleRect: CapsuleRect = {
          width,
          height,
          top,
          right,
          bottom,
          left
        };

        hilog.info(
          DOMAIN,
          TAG,
          `[getCapsuleRect] statusBarHeight=${statusBarHeight}vp, top=${top}vp, width=${width}vp, ` +
          `height=${height}vp, left=${left.toFixed(1)}vp, right=${right.toFixed(1)}vp, bottom=${bottom}vp`
        );

        const success: boolean = liblingxia.onCallback(callbackId, true, JSON.stringify(capsuleRect));
        if (!success) {
          hilog.error(DOMAIN, TAG, `Failed to send capsule rect callback for ID: ${callbackId}`);
        }
      } catch (error) {
        hilog.error(DOMAIN, TAG, `Failed to get capsule rect: ${error}`);
        const errorData = JSON.stringify({
          error: String(error),
          message: 'Failed to get capsule rect'
        });
        liblingxia.onCallback(callbackId, false, errorData);
      }
    })();
  }
}
