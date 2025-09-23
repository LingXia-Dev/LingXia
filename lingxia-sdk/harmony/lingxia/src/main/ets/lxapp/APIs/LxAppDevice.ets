import { hilog } from '@kit.PerformanceAnalysisKit';
import { display } from '@kit.ArkUI';
import { vibrator } from '@kit.SensorServiceKit';
import { call } from '@kit.TelephonyKit';
import liblingxia from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.Device';

export interface ScreenInfo {
  width: number;
  height: number;
  scale: number;
}

const DEFAULT_SCREEN_INFO: ScreenInfo = {
  width: 360,
  height: 640,
  scale: 2.0
};

export class LxAppDevice {
  public static getScreenInfo(callbackId: string): void {
    try {
      const defaultDisplay = display.getDefaultDisplaySync();

      const screenInfo: ScreenInfo = {
        width: Math.round(defaultDisplay.width / defaultDisplay.densityPixels),
        height: Math.round(defaultDisplay.height / defaultDisplay.densityPixels),
        scale: defaultDisplay.densityPixels
      };

      hilog.info(DOMAIN, TAG, `Screen info: ${screenInfo.width}x${screenInfo.height}, scale: ${screenInfo.scale}`);

      const success: boolean = liblingxia.onCallback(callbackId, true, JSON.stringify(screenInfo));
      if (!success) {
        hilog.error(DOMAIN, TAG, `Failed to send screen info callback for ID: ${callbackId}`);
      }
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to get screen info: ${error}`);
      liblingxia.onCallback(callbackId, false, JSON.stringify(DEFAULT_SCREEN_INFO));
    }
  }

  public static vibrate(isLong: boolean): void {
    try {
      const duration = isLong ? 400 : 15;
      hilog.info(DOMAIN, TAG, `Vibrate requested: ${isLong} (${duration}ms)`);

      vibrator.startVibration({ type: 'time', duration }, { id: 0, usage: 'alarm' }, (error) => {
        if (error) {
          hilog.error(DOMAIN, TAG, `Failed to vibrate: ${error.message}`);
        } else {
          hilog.info(DOMAIN, TAG, `Vibration completed: ${duration}ms`);
        }
      });
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to vibrate: ${error}`);
    }
  }

  public static makePhoneCall(phoneNumber: string): void {
    try {
      hilog.info(DOMAIN, TAG, `Phone call requested: ${phoneNumber}`);

      call.makeCall(phoneNumber, (err) => {
        if (err) {
          hilog.error(DOMAIN, TAG, `Failed to make call: ${err.code} - ${err.message}`);
        } else {
          hilog.info(DOMAIN, TAG, `Successfully opened dialer for: ${phoneNumber}`);
        }
      });
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to make phone call: ${error}`);
    }
  }
}
