package com.lingxia.lxapp.APIs

import android.app.Activity
import android.app.AlarmManager
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.core.content.ContextCompat
import com.lingxia.app.Lingxia
import com.lingxia.app.LxLog
import com.lingxia.app.NativeApi
import com.lingxia.app.PermissionManager
import com.lingxia.lxapp.LxApp
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * Local notifications for `lx.app.notification`. Permission is requested on
 * `requestPermission` / first `show`, never at process start.
 */
internal object LxAppNotification {
    private const val TAG = "LingXia.Notification"
    private const val CHANNEL_ID = "lingxia.local"
    const val EXTRA_ID = "lingxia.local.id"
    const val EXTRA_TITLE = "lingxia.local.title"
    const val EXTRA_BODY = "lingxia.local.body"
    const val EXTRA_APPLINK = "lingxia.local.applink"
    const val EXTRA_SILENT = "lingxia.local.silent"
    private const val ACTION_FIRE = "com.lingxia.lxapp.LOCAL_NOTIFICATION_FIRE"

    @Volatile
    private var lastError = ""
    private val scheduledIds = ConcurrentHashMap.newKeySet<String>()
    private val liveIds = ConcurrentHashMap.newKeySet<String>()

    @JvmStatic
    fun takeLastError(): String {
        val value = lastError
        lastError = ""
        return value
    }

    @JvmStatic
    fun requestPermission(): String {
        val context = appContext() ?: return "denied"
        if (notificationsEnabled(context)) {
            return "granted"
        }
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) {
            return if (notificationsEnabled(context)) "granted" else "denied"
        }
        val activity = LxApp.getCurrentActivity() ?: Lingxia.getLastResumedActivity()
        if (activity == null) {
            return "denied"
        }
        val latch = CountDownLatch(1)
        var granted = false
        PermissionManager.ensurePermissions(
            activity,
            arrayOf(android.Manifest.permission.POST_NOTIFICATIONS)
        ) { ok ->
            granted = ok
            latch.countDown()
        }
        if (!latch.await(30, TimeUnit.SECONDS)) {
            lastError = "notification permission timed out"
            return "denied"
        }
        return if (granted || notificationsEnabled(context)) "granted" else "denied"
    }

    @JvmStatic
    fun show(
        id: String,
        title: String,
        body: String,
        applink: String,
        deliverAtMs: Long,
        silent: Boolean
    ): String {
        val context = appContext()
        if (context == null) {
            lastError = "no application context"
            return ""
        }
        if (requestPermission() != "granted") {
            lastError = "notification permission is denied"
            return ""
        }
        val now = System.currentTimeMillis()
        if (deliverAtMs > now + 500L) {
            schedule(context, id, title, body, applink, deliverAtMs, silent)
            scheduledIds.add(id)
            return id
        }
        if (isFrontmost()) {
            return id
        }
        scheduledIds.remove(id)
        publishNow(context, id, title, body, applink, silent)
        return id
    }

    @JvmStatic
    fun cancel(id: String): Boolean {
        val context = appContext() ?: return true
        scheduledIds.remove(id)
        liveIds.remove(id)
        cancelAlarm(context, id)
        notificationManager(context)?.cancel(CHANNEL_ID, notifyId(id))
        return true
    }

    @JvmStatic
    fun cancelAll(): Boolean {
        val context = appContext() ?: return true
        for (id in scheduledIds.toList()) {
            cancelAlarm(context, id)
        }
        scheduledIds.clear()
        val manager = notificationManager(context) ?: return true
        for (id in liveIds.toList()) {
            manager.cancel(CHANNEL_ID, notifyId(id))
        }
        liveIds.clear()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            for (posted in manager.activeNotifications) {
                if (posted.tag == CHANNEL_ID) {
                    manager.cancel(posted.tag, posted.id)
                }
            }
        }
        return true
    }

    fun publishNow(
        context: Context,
        id: String,
        title: String,
        body: String,
        applink: String,
        silent: Boolean
    ) {
        scheduledIds.remove(id)
        liveIds.add(id)
        ensureChannel(context)
        val tap = Intent(context, LxLocalNotificationTapActivity::class.java).apply {
            putExtra(EXTRA_APPLINK, applink)
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP)
        }
        val flags = pendingFlags()
        val content = PendingIntent.getActivity(context, notifyId(id), tap, flags)
        val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            Notification.Builder(context, CHANNEL_ID)
        } else {
            @Suppress("DEPRECATION")
            Notification.Builder(context).setPriority(Notification.PRIORITY_HIGH)
        }
        val icon = context.applicationInfo.icon.takeIf { it != 0 }
            ?: android.R.drawable.stat_notify_chat
        val notification = builder
            .setSmallIcon(icon)
            .setContentTitle(title)
            .setContentText(body)
            .setContentIntent(content)
            .setAutoCancel(true)
            .setOnlyAlertOnce(silent)
            .apply {
                if (silent && Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                    setSilent(true)
                }
            }
            .build()
        notificationManager(context)?.notify(CHANNEL_ID, notifyId(id), notification)
    }

    fun fireIntent(context: Context, id: String): Intent {
        return Intent(context, LxLocalNotificationReceiver::class.java).apply {
            action = ACTION_FIRE
            putExtra(EXTRA_ID, id)
        }
    }

    private fun schedule(
        context: Context,
        id: String,
        title: String,
        body: String,
        applink: String,
        deliverAtMs: Long,
        silent: Boolean
    ) {
        cancelAlarm(context, id)
        val intent = fireIntent(context, id).apply {
            putExtra(EXTRA_TITLE, title)
            putExtra(EXTRA_BODY, body)
            putExtra(EXTRA_APPLINK, applink)
            putExtra(EXTRA_SILENT, silent)
        }
        val pending = PendingIntent.getBroadcast(context, notifyId(id), intent, pendingFlags())
        val alarms = context.getSystemService(Context.ALARM_SERVICE) as AlarmManager
        alarms.set(AlarmManager.RTC_WAKEUP, deliverAtMs, pending)
    }

    private fun cancelAlarm(context: Context, id: String) {
        val pending = PendingIntent.getBroadcast(
            context,
            notifyId(id),
            fireIntent(context, id),
            pendingFlags()
        )
        val alarms = context.getSystemService(Context.ALARM_SERVICE) as AlarmManager
        alarms.cancel(pending)
    }

    private fun ensureChannel(context: Context) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val manager = notificationManager(context) ?: return
        if (manager.getNotificationChannel(CHANNEL_ID) != null) return
        manager.createNotificationChannel(
            NotificationChannel(
                CHANNEL_ID,
                "LingXia",
                NotificationManager.IMPORTANCE_HIGH
            )
        )
    }

    private fun notificationsEnabled(context: Context): Boolean {
        val manager = notificationManager(context) ?: return false
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N && !manager.areNotificationsEnabled()) {
            return false
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            return ContextCompat.checkSelfPermission(
                context,
                android.Manifest.permission.POST_NOTIFICATIONS
            ) == PackageManager.PERMISSION_GRANTED
        }
        return true
    }

    private fun isFrontmost(): Boolean {
        val activity = Lingxia.getLastResumedActivity() ?: return false
        return !activity.isFinishing && activity.hasWindowFocus()
    }

    private fun notificationManager(context: Context): NotificationManager? {
        return context.getSystemService(Context.NOTIFICATION_SERVICE) as? NotificationManager
    }

    private fun appContext(): Context? {
        return Lingxia.applicationContext() ?: LxApp.getCurrentActivity()
    }

    private fun pendingFlags(): Int {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        } else {
            PendingIntent.FLAG_UPDATE_CURRENT
        }
    }

    fun notifyId(id: String): Int {
        val hashed = id.hashCode()
        return if (hashed == 0) 1 else hashed
    }
}

internal class LxLocalNotificationReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val id = intent.getStringExtra(LxAppNotification.EXTRA_ID) ?: return
        LxAppNotification.publishNow(
            context.applicationContext,
            id,
            intent.getStringExtra(LxAppNotification.EXTRA_TITLE).orEmpty(),
            intent.getStringExtra(LxAppNotification.EXTRA_BODY).orEmpty(),
            intent.getStringExtra(LxAppNotification.EXTRA_APPLINK).orEmpty(),
            intent.getBooleanExtra(LxAppNotification.EXTRA_SILENT, false)
        )
    }
}

internal class LxLocalNotificationTapActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val applink = intent?.getStringExtra(LxAppNotification.EXTRA_APPLINK).orEmpty()
        if (applink.isNotEmpty()) {
            try {
                NativeApi.onAppLinkReceived(applink)
            } catch (error: Throwable) {
                LxLog.w("LingXia.Notification", "tap applink failed", error)
            }
        }
        val launch = packageManager.getLaunchIntentForPackage(packageName)
        if (launch != null) {
            launch.addFlags(
                Intent.FLAG_ACTIVITY_NEW_TASK or
                    Intent.FLAG_ACTIVITY_REORDER_TO_FRONT or
                    Intent.FLAG_ACTIVITY_SINGLE_TOP
            )
            startActivity(launch)
        }
        finish()
    }
}
