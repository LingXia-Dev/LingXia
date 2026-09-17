import { notificationManager } from '@kit.NotificationKit';
import { wantAgent, Want } from '@kit.AbilityKit';
import { BusinessError } from '@kit.BasicServicesKit';
import { onCallback } from 'liblingxia.so';
import { Lingxia } from '../../app/Lingxia';
import { LxLog } from '../../app/LxLog';

const TAG = 'LingXia.Notification';
const publishedIds = new Set<number>();

function numericId(id: string): number {
  let hash = 0;
  for (let index = 0; index < id.length; index++) {
    hash = ((hash << 5) - hash) + id.charCodeAt(index);
    hash |= 0;
  }
  return hash === 0 ? 1 : hash;
}

export class LxAppNotification {
  public static requestPermission(callbackId: string): void {
    const id = String(callbackId ?? '').trim();
    if (!id) {
      return;
    }
    (async () => {
      try {
        const enabled = await notificationManager.isNotificationEnabled();
        if (enabled) {
          onCallback(id, true, 'granted');
          return;
        }
        const ctx = Lingxia.context;
        if (!ctx) {
          onCallback(id, true, 'denied');
          return;
        }
        await notificationManager.requestEnableNotification(ctx);
        const after = await notificationManager.isNotificationEnabled();
        onCallback(id, true, after ? 'granted' : 'denied');
      } catch (error) {
        const err = error as BusinessError;
        LxLog.e(TAG, `requestPermission failed: ${err.code} ${err.message}`);
        onCallback(id, true, 'denied');
      }
    })();
  }

  public static show(
    id: string,
    title: string,
    body: string,
    applink: string,
    deliverAtMs: string,
    _silent: string
  ): boolean {
    const key = String(id ?? '').trim();
    if (!key) {
      return false;
    }
    const at = Number(deliverAtMs);
    const now = Date.now();
    if (!(at > now + 500) && Lingxia.inForeground) {
      return true;
    }
    (async () => {
      try {
        const enabled = await notificationManager.isNotificationEnabled();
        if (!enabled) {
          const ctx = Lingxia.context;
          if (ctx) {
            await notificationManager.requestEnableNotification(ctx);
          }
        }
        const request: notificationManager.NotificationRequest = {
          id: numericId(key),
          content: {
            notificationContentType: notificationManager.ContentType.NOTIFICATION_CONTENT_BASIC_TEXT,
            normal: {
              title: title,
              text: body
            }
          }
        };
        if (at > now + 500) {
          request.deliveryTime = at;
        }
        const link = String(applink ?? '').trim();
        const ctx = Lingxia.context;
        if (ctx && link.startsWith('https://')) {
          const want: Want = {
            bundleName: ctx.abilityInfo.bundleName,
            abilityName: ctx.abilityInfo.name,
            uri: link
          };
          request.wantAgent = await wantAgent.getWantAgent({
            wants: [want],
            operationType: wantAgent.OperationType.START_ABILITY,
            requestCode: numericId(key)
          });
        }
        await notificationManager.publish(request);
        publishedIds.add(numericId(key));
      } catch (error) {
        const err = error as BusinessError;
        LxLog.e(TAG, `show failed: ${err.code} ${err.message}`);
      }
    })();
    return true;
  }

  public static cancel(id: string): boolean {
    const key = String(id ?? '').trim();
    if (!key) {
      return false;
    }
    const nid = numericId(key);
    publishedIds.delete(nid);
    notificationManager.cancel(nid).catch((error: BusinessError) => {
      LxLog.e(TAG, `cancel failed: ${error.code} ${error.message}`);
    });
    return true;
  }

  public static cancelAll(): boolean {
    const ids = Array.from(publishedIds);
    publishedIds.clear();
    for (const nid of ids) {
      notificationManager.cancel(nid).catch((error: BusinessError) => {
        LxLog.e(TAG, `cancelAll failed: ${error.code} ${error.message}`);
      });
    }
    return true;
  }
}
