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
import android.net.Uri
import android.os.Build
import android.os.Bundle
import androidx.core.content.ContextCompat
import com.lingxia.app.Lingxia
import com.lingxia.app.PermissionManager
import com.lingxia.lxapp.LxApp
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * Local notifications for `lx.app.notification`. Permission is requested on
 * `requestPermission` / the first `show` that reaches the OS, never at start.
 *
 * Nothing here is keyed by a hash of the id: a posted notification uses the id
 * as its tag, and an alarm carries it in the intent data, so two ids can never
 * replace each other. What is scheduled is persisted, because `cancelAll` must
 * still find it after the process was killed.
 */
internal object LxAppNotification {
    private const val CHANNEL_ID = "lingxia.local"
    private const val SILENT_CHANNEL_ID = "lingxia.local.silent"
    private const val NOTIFY_ID = 1
    private const val PREFS = "lingxia.local.notifications"
    private const val PREF_SCHEDULED = "scheduled"
    private const val PREF_ASKED = "asked"
    /** Carries an alarm's public notification id. Not the tap envelope below. */
    private const val ALARM_SCHEME = "lxalarm"
    /**
     * The activation envelope, as `ACTIVATION_ENVELOPE` in
     * `crates/lingxia-platform/src/traits/app_runtime.rs` spells it. A tap
     * Intent's data is the only slot that survives PendingIntent identity.
     */
    private const val ACTIVATION_SCHEME = "lxnotify"
    private const val ACTIVATION_VERSION = "v1"
    const val EXTRA_LOCAL = "lingxia.local"
    const val EXTRA_TITLE = "lingxia.local.title"
    const val EXTRA_BODY = "lingxia.local.body"
    const val EXTRA_TOKEN = "lingxia.local.token"
    const val EXTRA_SILENT = "lingxia.local.silent"
    private const val ACTION_FIRE = "com.lingxia.lxapp.LOCAL_NOTIFICATION_FIRE"
    private const val PROMPT_TIMEOUT_SECONDS = 60L

    @Volatile
    private var lastError = ""

    @JvmStatic
    fun takeLastError(): String {
        val value = lastError
        lastError = ""
        return value
    }

    /** `granted` / `denied` / `default`, never prompting. */
    @JvmStatic
    fun permission(): String {
        val context = appContext() ?: return "denied"
        if (notificationsEnabled(context)) return "granted"
        val promptable = Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            !prefs(context).getBoolean(PREF_ASKED, false)
        return if (promptable) "default" else "denied"
    }

    /** `granted` / `denied`; empty when the prompt was left unanswered. */
    @JvmStatic
    fun requestPermission(): String {
        val context = appContext() ?: return "denied"
        if (notificationsEnabled(context)) return "granted"
        // Below Android 13 there is no prompt: the setting is the answer.
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return "denied"
        val activity = LxApp.getCurrentActivity() ?: Lingxia.getLastResumedActivity()
            ?: return "denied"
        val latch = CountDownLatch(1)
        var granted = false
        PermissionManager.ensurePermissions(
            activity,
            arrayOf(android.Manifest.permission.POST_NOTIFICATIONS)
        ) { ok ->
            granted = ok
            latch.countDown()
        }
        if (!latch.await(PROMPT_TIMEOUT_SECONDS, TimeUnit.SECONDS)) {
            lastError = "notification permission prompt was not answered"
            return ""
        }
        prefs(context).edit().putBoolean(PREF_ASKED, true).apply()
        return if (granted || notificationsEnabled(context)) "granted" else "denied"
    }

    /** `posted` / `scheduled` / `suppressed`; empty on failure. */
    @JvmStatic
    fun show(
        id: String,
        title: String,
        body: String,
        activationToken: String,
        deliverAtMs: Long,
        silent: Boolean
    ): String {
        val context = appContext()
        if (context == null) {
            lastError = "no application context"
            return ""
        }
        val scheduled = deliverAtMs > System.currentTimeMillis() + 500L
        if (!scheduled && isFrontmost()) {
            // Still an upsert: whatever the id held must not outlive this call.
            cancel(id)
            return "suppressed"
        }
        val permission = requestPermission()
        if (permission != "granted") {
            if (permission.isNotEmpty()) lastError = "notification permission is $permission"
            return ""
        }
        cancel(id)
        if (scheduled) {
            schedule(context, id, title, body, activationToken, deliverAtMs, silent)
            return "scheduled"
        }
        publishNow(context, id, title, body, activationToken, silent)
        return "posted"
    }

    @JvmStatic
    fun cancel(id: String): Boolean {
        val context = appContext() ?: return true
        cancelAlarm(context, id)
        forgetScheduled(context, id)
        notificationManager(context)?.cancel(id, NOTIFY_ID)
        return true
    }

    @JvmStatic
    fun cancelAll(): Boolean {
        val context = appContext() ?: return true
        for (id in scheduledIds(context)) {
            cancelAlarm(context, id)
        }
        prefs(context).edit().remove(PREF_SCHEDULED).apply()
        val manager = notificationManager(context) ?: return true
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            for (posted in manager.activeNotifications) {
                if (posted.notification.extras?.getBoolean(EXTRA_LOCAL, false) == true) {
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
        activationToken: String,
        silent: Boolean
    ) {
        forgetScheduled(context, id)
        ensureChannels(context)
        // The token lives in the data URI, not an extra: PendingIntent identity
        // ignores extras, so a replacement would otherwise rewrite the banner
        // that is already on screen and send an old tap to the new target.
        val tap = Intent(context, LxLocalNotificationTapActivity::class.java).apply {
            action = Intent.ACTION_VIEW
            data = tokenUri(activationToken)
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP)
        }
        val content = PendingIntent.getActivity(context, 0, tap, pendingFlags())
        val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            // From Android 8 the channel decides sound, so silence is a channel.
            Notification.Builder(context, if (silent) SILENT_CHANNEL_ID else CHANNEL_ID)
        } else {
            @Suppress("DEPRECATION")
            Notification.Builder(context).setPriority(Notification.PRIORITY_HIGH).apply {
                if (!silent) setDefaults(Notification.DEFAULT_SOUND)
            }
        }
        val icon = context.applicationInfo.icon.takeIf { it != 0 }
            ?: android.R.drawable.stat_notify_chat
        val notification = builder
            .setSmallIcon(icon)
            .setContentTitle(title)
            .setContentText(body)
            .setContentIntent(content)
            .setAutoCancel(true)
            .addExtras(Bundle().apply { putBoolean(EXTRA_LOCAL, true) })
            .build()
        notificationManager(context)?.notify(id, NOTIFY_ID, notification)
    }

    fun idFromFireIntent(intent: Intent): String? {
        val data = intent.data ?: return null
        if (data.scheme != ALARM_SCHEME) return null
        return data.schemeSpecificPart?.takeIf { it.isNotEmpty() }
    }

    private fun idUri(id: String): Uri = Uri.fromParts(ALARM_SCHEME, id, null)

    /** `lxnotify:v1:<token>` — the same envelope every other platform uses. */
    private fun tokenUri(token: String): Uri =
        Uri.fromParts(ACTIVATION_SCHEME, "$ACTIVATION_VERSION:$token", null)

    fun tokenFromTapIntent(intent: Intent): String {
        val data = intent.data ?: return ""
        if (data.scheme != ACTIVATION_SCHEME) return ""
        return data.schemeSpecificPart
            ?.removePrefix("$ACTIVATION_VERSION:")
            ?.takeIf { it.isNotEmpty() }
            .orEmpty()
    }

    /** Intent equality ignores extras, so the id has to live in the data. */
    private fun fireIntent(context: Context, id: String): Intent {
        return Intent(context, LxLocalNotificationReceiver::class.java).apply {
            action = ACTION_FIRE
            data = idUri(id)
        }
    }

    private fun schedule(
        context: Context,
        id: String,
        title: String,
        body: String,
        activationToken: String,
        deliverAtMs: Long,
        silent: Boolean
    ) {
        // The fire Intent keys on the id, so FLAG_UPDATE_CURRENT replaces a
        // pending alarm's extras with this generation's token — which is what
        // replacing an id means.
        val intent = fireIntent(context, id).apply {
            putExtra(EXTRA_TITLE, title)
            putExtra(EXTRA_BODY, body)
            putExtra(EXTRA_TOKEN, activationToken)
            putExtra(EXTRA_SILENT, silent)
        }
        val pending = PendingIntent.getBroadcast(context, 0, intent, pendingFlags())
        val alarms = context.getSystemService(Context.ALARM_SERVICE) as AlarmManager
        // Battery saver defers a plain `set` indefinitely once the app is backgrounded.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            alarms.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, deliverAtMs, pending)
        } else {
            alarms.set(AlarmManager.RTC_WAKEUP, deliverAtMs, pending)
        }
        rememberScheduled(context, id)
    }

    private fun cancelAlarm(context: Context, id: String) {
        val pending = PendingIntent.getBroadcast(context, 0, fireIntent(context, id), pendingFlags())
        val alarms = context.getSystemService(Context.ALARM_SERVICE) as AlarmManager
        alarms.cancel(pending)
    }

    private fun prefs(context: Context) =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    private fun scheduledIds(context: Context): Set<String> =
        prefs(context).getStringSet(PREF_SCHEDULED, emptySet())?.toSet() ?: emptySet()

    @Synchronized
    private fun rememberScheduled(context: Context, id: String) {
        prefs(context).edit().putStringSet(PREF_SCHEDULED, scheduledIds(context) + id).apply()
    }

    @Synchronized
    private fun forgetScheduled(context: Context, id: String) {
        val ids = scheduledIds(context)
        if (id in ids) {
            prefs(context).edit().putStringSet(PREF_SCHEDULED, ids - id).apply()
        }
    }

    private fun ensureChannels(context: Context) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val manager = notificationManager(context) ?: return
        if (manager.getNotificationChannel(CHANNEL_ID) == null) {
            manager.createNotificationChannel(
                NotificationChannel(CHANNEL_ID, "LingXia", NotificationManager.IMPORTANCE_HIGH)
            )
        }
        if (manager.getNotificationChannel(SILENT_CHANNEL_ID) == null) {
            manager.createNotificationChannel(
                NotificationChannel(
                    SILENT_CHANNEL_ID,
                    "LingXia (silent)",
                    NotificationManager.IMPORTANCE_HIGH
                ).apply {
                    setSound(null, null)
                    enableVibration(false)
                }
            )
        }
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
}

/** A scheduled notification is presented even if the product is frontmost. */
internal class LxLocalNotificationReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val id = LxAppNotification.idFromFireIntent(intent) ?: return
        LxAppNotification.publishNow(
            context.applicationContext,
            id,
            intent.getStringExtra(LxAppNotification.EXTRA_TITLE).orEmpty(),
            intent.getStringExtra(LxAppNotification.EXTRA_BODY).orEmpty(),
            intent.getStringExtra(LxAppNotification.EXTRA_TOKEN).orEmpty(),
            intent.getBooleanExtra(LxAppNotification.EXTRA_SILENT, false)
        )
    }
}

/**
 * Tap target. It brings the product forward first, then hands the activation
 * token to the runtime — so a token that no longer resolves still lands the
 * user on a visible product that can say why.
 */
internal class LxLocalNotificationTapActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val token = intent?.let { LxAppNotification.tokenFromTapIntent(it) }.orEmpty()
        val launch = packageManager.getLaunchIntentForPackage(packageName)
        val cold = Lingxia.applicationContext() == null
        if (launch != null) {
            if (cold) {
                // Nothing is listening yet, so the entry Intent carries the
                // token and the SDK delivers it once the runtime is up.
                launch.flags = Intent.FLAG_ACTIVITY_NEW_TASK
                launch.putExtra(Lingxia.NOTIFICATION_TOKEN_EXTRA, token)
                startActivity(launch)
            } else {
                // Reorder the live lxapp activity to the front rather than
                // starting the launcher Intent. That Intent does not
                // `filterEquals` the root this task was actually started with,
                // so it builds a second entry activity whose `quickStart`
                // reopens the home lxapp — burying the page this tap asked for.
                val live = LxApp.getCurrentActivity()
                if (live != null) {
                    startActivity(Intent(this, live::class.java).apply {
                        flags = Intent.FLAG_ACTIVITY_REORDER_TO_FRONT or
                            Intent.FLAG_ACTIVITY_NEW_TASK
                    })
                } else {
                    launch.flags = Intent.FLAG_ACTIVITY_NEW_TASK
                    startActivity(launch)
                }
            }
        }
        if (!cold) {
            Lingxia.deliverNotificationActivation(token)
        }
        finish()
    }
}
