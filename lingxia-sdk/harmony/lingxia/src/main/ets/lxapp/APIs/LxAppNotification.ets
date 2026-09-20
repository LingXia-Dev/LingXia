import { notificationManager } from '@kit.NotificationKit';
import { reminderAgentManager } from '@kit.BackgroundTasksKit';
import { preferences } from '@kit.ArkData';
import { wantAgent, Want } from '@kit.AbilityKit';
import { BusinessError } from '@kit.BasicServicesKit';
import { onCallback } from 'liblingxia.so';
import { Lingxia } from '../../app/Lingxia';
import { LxLog } from '../../app/LxLog';

const TAG = 'LingXia.Notification';
const STORE = 'lingxia.local.notifications';
const ASKED_KEY = 'asked';
const REMINDER_PREFIX = 'reminder:';
// The numeric id is only a hash; the label carries the real id, and the pair
// is what the system keys on, so two ids never replace each other.
const LABEL_PREFIX = 'lx:';

function numericId(id: string): number {
  let hash = 0;
  for (let index = 0; index < id.length; index++) {
    hash = ((hash << 5) - hash) + id.charCodeAt(index);
    hash |= 0;
  }
  return hash === 0 ? 1 : hash;
}

function describe(error: Object): string {
  const err = error as BusinessError;
  return `${err.code ?? ''} ${err.message ?? String(error)}`.trim();
}

/**
 * Every call completes through its callback id, after the system has answered:
 * a word on success, `error:<why>` otherwise.
 */
export class LxAppNotification {
  private static store(): preferences.Preferences | null {
    const ctx = Lingxia.context;
    if (!ctx) {
      return null;
    }
    return preferences.getPreferencesSync(ctx, { name: STORE });
  }

  private static done(callbackId: string, word: string): void {
    onCallback(callbackId, true, word);
  }

  private static fail(callbackId: string, why: string): void {
    LxLog.e(TAG, why);
    onCallback(callbackId, true, `error:${why}`);
  }

  private static async currentPermission(): Promise<string> {
    if (await notificationManager.isNotificationEnabled()) {
      return 'granted';
    }
    const asked = LxAppNotification.store()?.getSync(ASKED_KEY, false) as boolean;
    return asked ? 'denied' : 'default';
  }

  private static async ensurePermission(): Promise<string> {
    const current = await LxAppNotification.currentPermission();
    if (current !== 'default') {
      return current;
    }
    const ctx = Lingxia.context;
    if (!ctx) {
      return 'denied';
    }
    try {
      await notificationManager.requestEnableNotification(ctx);
    } catch (error) {
      LxLog.w(TAG, `requestEnableNotification: ${describe(error as Object)}`);
    }
    const store = LxAppNotification.store();
    store?.putSync(ASKED_KEY, true);
    await store?.flush();
    return (await notificationManager.isNotificationEnabled()) ? 'granted' : 'denied';
  }

  public static permission(callbackId: string): void {
    LxAppNotification.currentPermission()
      .then((word: string) => LxAppNotification.done(callbackId, word))
      .catch((error: Object) => LxAppNotification.fail(callbackId, `permission: ${describe(error)}`));
  }

  public static requestPermission(callbackId: string): void {
    LxAppNotification.ensurePermission()
      .then((word: string) => LxAppNotification.done(callbackId, word))
      .catch((error: Object) =>
        LxAppNotification.fail(callbackId, `requestPermission: ${describe(error)}`));
  }

  private static async remove(id: string): Promise<void> {
    const store = LxAppNotification.store();
    const key = REMINDER_PREFIX + id;
    const reminderId = store?.getSync(key, -1) as number;
    if (reminderId !== undefined && reminderId >= 0) {
      try {
        await reminderAgentManager.cancelReminder(reminderId);
      } catch (error) {
        // Already fired or gone: nothing left to cancel.
      }
      store?.deleteSync(key);
      await store?.flush();
    }
    try {
      await notificationManager.cancel(numericId(id), LABEL_PREFIX + id);
    } catch (error) {
      // Not currently displayed.
    }
  }

  private static async post(
    id: string,
    title: string,
    body: string,
    applink: string,
    deliverAtMs: number
  ): Promise<string> {
    const ctx = Lingxia.context;
    if (!ctx) {
      throw new Error('no ability context');
    }
    const scheduled = deliverAtMs > Date.now() + 500;
    if (!scheduled && Lingxia.inForeground) {
      // Still an upsert: whatever the id held must not outlive this call.
      await LxAppNotification.remove(id);
      return 'suppressed';
    }
    const permission = await LxAppNotification.ensurePermission();
    if (permission !== 'granted') {
      throw new Error(`notification permission is ${permission}`);
    }
    await LxAppNotification.remove(id);

    if (scheduled) {
      // `deliveryTime` on a request is only the timestamp it displays; the
      // system fires nothing later. An agent reminder is what survives us.
      const seconds = Math.max(1, Math.round((deliverAtMs - Date.now()) / 1000));
      const timer: reminderAgentManager.ReminderRequestTimer = {
        reminderType: reminderAgentManager.ReminderType.REMINDER_TYPE_TIMER,
        triggerTimeInSeconds: seconds,
        title: title,
        content: body,
        notificationId: numericId(id),
        slotType: notificationManager.SlotType.SOCIAL_COMMUNICATION,
        wantAgent: {
          pkgName: ctx.abilityInfo.bundleName,
          abilityName: ctx.abilityInfo.name,
          uri: applink.startsWith('https://') ? applink : undefined
        }
      };
      let reminderId = -1;
      try {
        reminderId = await reminderAgentManager.publishReminder(timer);
      } catch (error) {
        // 1700002 with a quota of zero: the system grants agent reminders per
        // app, and this one has none. Nothing else fires after we are suspended.
        if ((error as BusinessError).code === 1700002) {
          throw new Error('scheduling needs the HarmonyOS agent-reminder privilege for this app');
        }
        throw error as Error;
      }
      const store = LxAppNotification.store();
      store?.putSync(REMINDER_PREFIX + id, reminderId);
      await store?.flush();
      return 'scheduled';
    }

    const want: Want = {
      bundleName: ctx.abilityInfo.bundleName,
      abilityName: ctx.abilityInfo.name
    };
    if (applink.startsWith('https://')) {
      want.uri = applink;
    }
    const request: notificationManager.NotificationRequest = {
      id: numericId(id),
      label: LABEL_PREFIX + id,
      // The default OTHER_TYPES slot never surfaces to the user.
      notificationSlotType: notificationManager.SlotType.SOCIAL_COMMUNICATION,
      tapDismissed: true,
      content: {
        notificationContentType: notificationManager.ContentType.NOTIFICATION_CONTENT_BASIC_TEXT,
        normal: { title: title, text: body }
      },
      // Without an agent a tap does not bring the product forward.
      wantAgent: await wantAgent.getWantAgent({
        wants: [want],
        actionType: wantAgent.OperationType.START_ABILITY,
        requestCode: numericId(id),
        actionFlags: [wantAgent.WantAgentFlags.UPDATE_PRESENT_FLAG]
      })
    };
    await notificationManager.publish(request);
    return 'shown';
  }

  public static show(
    callbackId: string,
    id: string,
    title: string,
    body: string,
    applink: string,
    deliverAtMs: string
  ): void {
    LxAppNotification.post(id, title, body, String(applink ?? '').trim(), Number(deliverAtMs))
      .then((word: string) => LxAppNotification.done(callbackId, word))
      .catch((error: Object) => LxAppNotification.fail(callbackId, `show: ${describe(error)}`));
  }

  /**
   * `lx.app.setBadge` on the home-screen icon. An empty value clears it.
   * Independent of any posted notification: nothing here publishes one.
   */
  public static setAppBadge(callbackId: string, text: string): void {
    const trimmed = String(text ?? '').trim();
    const count = trimmed.length === 0 ? 0 : Number(trimmed);
    if (!Number.isFinite(count) || count < 0) {
      LxAppNotification.fail(callbackId, `setAppBadge: ${trimmed} is not a count`);
      return;
    }
    notificationManager.setBadgeNumber(Math.floor(count))
      .then(() => LxAppNotification.done(callbackId, 'ok'))
      .catch((error: Object) =>
        LxAppNotification.fail(callbackId, `setAppBadge: ${describe(error)}`));
  }

  public static cancel(callbackId: string, id: string): void {
    LxAppNotification.remove(id)
      .then(() => LxAppNotification.done(callbackId, 'ok'))
      .catch((error: Object) => LxAppNotification.fail(callbackId, `cancel: ${describe(error)}`));
  }

  private static async removeAll(): Promise<void> {
    const store = LxAppNotification.store();
    const entries = store?.getAllSync() as Record<string, Object> | undefined;
    const keys: string[] = entries ? Object.keys(entries) : [];
    for (const key of keys) {
      if (key.startsWith(REMINDER_PREFIX)) {
        await LxAppNotification.remove(key.substring(REMINDER_PREFIX.length));
      }
    }
    const active = await notificationManager.getActiveNotifications();
    for (const request of active) {
      const label = request.label ?? '';
      if (label.startsWith(LABEL_PREFIX) && request.id !== undefined) {
        await notificationManager.cancel(request.id, label);
      }
    }
  }

  public static cancelAll(callbackId: string): void {
    LxAppNotification.removeAll()
      .then(() => LxAppNotification.done(callbackId, 'ok'))
      .catch((error: Object) => LxAppNotification.fail(callbackId, `cancelAll: ${describe(error)}`));
  }
}
