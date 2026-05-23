import { hilog } from '@kit.PerformanceAnalysisKit';
import { notificationManager } from '@kit.NotificationKit';
import { pushService } from '@kit.PushKit';
import { BusinessError } from '@kit.BasicServicesKit';
import { LxApp } from '../../app/LxApp';
import { onPushTokenReceived, onPushlinkReceived } from 'liblingxia.so';
import { UIAbility } from '@kit.AbilityKit';

const DOMAIN = 0x0000;
const TAG = 'LingXia.Push';

export class LxAppPush {
  private static receiverAttached: boolean = false;
  /**
   * Forward push-related link with trigger to native.
   * @param url Full https URL
   * @param trigger 0=Background, 1=Tap, 2=Launch
   */
  public static handlePushLink(url: string, trigger: number): void {
    try {
      const validTriggers: number[] = [0, 1, 2];
      const t: number = validTriggers.indexOf(trigger) >= 0 ? trigger : 1;
      const rc: number = onPushlinkReceived(url, t);
      hilog.info(DOMAIN, TAG, `PushLink forwarded, trigger=${t}, rc=${rc}`);
    } catch (e) {
      const err: Error = e as Error;
      hilog.error(DOMAIN, TAG, `handlePushLink error for ${url}: ${JSON.stringify(err)}`);
    }
  }
  /**
   * Request user permission to enable notifications.
   */
  public static requestEnableNotification(): boolean {
    try {
      const ctx = LxApp.context;
      if (!ctx) {
        hilog.error(DOMAIN, TAG, 'requestEnableNotification: no context');
        return false;
      }
      (async () => {
        try {
          await notificationManager.requestEnableNotification(ctx);
        } catch (e) {
          const err: BusinessError = e as BusinessError;
          hilog.error(DOMAIN, TAG, `requestEnableNotification failed: ${err.code} ${err.message}`);
        }
      })();
      return true;
    } catch (e) {
      hilog.error(DOMAIN, TAG, `requestEnableNotification error: ${e}`);
      return false;
    }
  }

  /**
   * Clear app badge count to zero.
   */
  public static clearBadge(): boolean {
    (async () => {
      try {
        await notificationManager.setBadgeNumber(0);
      } catch (e) {
        const err: BusinessError = e as BusinessError;
        hilog.error(DOMAIN, TAG, `clearBadge failed: ${err.code} ${err.message}`);
      }
    })();
    return true;
  }

  /**
   * Get Push Token via Push Kit and forward to native immediately.
   */
  public static initToken(): void {
    (async () => {
      try {
        const token: string = await pushService.getToken();
        onPushTokenReceived(token);
      } catch (e) {
        const err: BusinessError = e as BusinessError;
        hilog.error(DOMAIN, TAG, `initToken failed: ${err.code} ${err.message}`);
      }
    })();
  }

  /**
   * Optional: Attach a push receiver to the given UIAbility to forward messages to native.
   * Note: scenario messages (e.g., IM) are not supported here and the receiver is not attached.
   */
  public static attachReceiver(ability: UIAbility): void {
    try {
      if (LxAppPush.receiverAttached) {
        return;
      }
      // Scenario messages are not used in this SDK build; receiver not attached.
      LxAppPush.receiverAttached = true;
      hilog.info(DOMAIN, TAG, '%{public}s', 'Scenario messages not used. Receiver not attached.');
    } catch (e) {
      const err: BusinessError = e as BusinessError;
      hilog.error(DOMAIN, TAG, `attachReceiver failed: ${err.code} ${err.message}`);
    }
  }
}
