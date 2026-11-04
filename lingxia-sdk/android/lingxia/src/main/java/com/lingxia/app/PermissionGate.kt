package com.lingxia.app

import android.app.Activity
import android.app.Application
import android.content.Context
import android.content.pm.PackageManager
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import java.util.ArrayDeque

/** The lx feature that needs a runtime permission. */
enum class PermissionFeature { CAMERA, SCAN_CODE, MEDIA_LIBRARY, LOCATION, WIFI, NOTIFICATION }

enum class PermissionOutcome {
    GRANTED,
    DENIED,

    /** Denied and the system will not prompt again; only Settings can grant it. */
    DENIED_PERMANENTLY,
}

/** A system permission prompt LingXia is about to show. */
class PermissionPrompt internal constructor(
    val activity: Activity,
    val feature: PermissionFeature,
    /** Missing `android.permission.*` names the prompt asks for. */
    val permissions: List<String>,
    /** The lxapp that triggered it, when one is foreground. */
    val appId: String?,
    /**
     * The user denied it for good: [proceed] shows no system prompt and only
     * Settings can grant it, so guide the user there instead of explaining.
     */
    val blocked: Boolean,
    private val decide: (Boolean) -> Unit,
) {
    /** Show the system prompt. */
    fun proceed() = decide(true)

    /** Skip the system prompt; the lx API fails as permission denied. */
    fun deny() = decide(false)

    override fun toString(): String =
        "PermissionPrompt(feature=$feature, permissions=$permissions, appId=$appId, blocked=$blocked)"
}

/**
 * Host hook around every system permission prompt LingXia shows. Prompts are
 * gated one at a time, on the main thread.
 */
fun interface PermissionGate {
    /**
     * Call [PermissionPrompt.proceed] or [PermissionPrompt.deny], now or later;
     * only the first call counts. Undecided when the activity is destroyed
     * means denied.
     */
    fun beforePrompt(prompt: PermissionPrompt)

    /** The system prompt closed. Not called after [PermissionPrompt.deny]. */
    fun afterPrompt(prompt: PermissionPrompt, outcome: PermissionOutcome) {}
}

/** SDK side of [PermissionGate]; public only for the media module AARs. */
object PermissionGates {
    @Volatile
    internal var gate: PermissionGate? = null

    private val mainHandler = Handler(Looper.getMainLooper())
    private val queue = ArrayDeque<() -> Unit>()
    private var busy = false

    /**
     * Runs [prompt] unless the host gate denies, in which case [declined] runs
     * instead. [prompt] receives a completion to call with the result. Without
     * a gate, [prompt] runs synchronously on the caller's thread.
     */
    fun run(
        activity: Activity,
        feature: PermissionFeature,
        permissions: Array<String>,
        declined: () -> Unit,
        prompt: (finish: (Boolean) -> Unit) -> Unit,
    ) {
        val gate = gate ?: run {
            prompt {}
            return
        }
        onMain {
            queue.add { gateOne(gate, activity, feature, permissions.toList(), declined, prompt) }
            if (!busy) next()
        }
    }

    private fun next() {
        val job = queue.poll()
        busy = job != null
        job?.invoke()
    }

    private fun gateOne(
        gate: PermissionGate,
        activity: Activity,
        feature: PermissionFeature,
        permissions: List<String>,
        declined: () -> Unit,
        prompt: (finish: (Boolean) -> Unit) -> Unit,
    ) {
        if (activity.isFinishing || activity.isDestroyed) {
            declined()
            next()
            return
        }
        // A permission without rationale is blocked only if it was asked
        // before; otherwise it has simply never been requested.
        val askedBefore = permissions.associateWith { asked(activity, it) }
        val rationaleBefore = permissions.associateWith { rationale(activity, it) }
        val blocked = permissions.any { askedBefore.getValue(it) && !rationaleBefore.getValue(it) }
        // 0 undecided, 1 prompting, 2 done.
        var stage = 0
        lateinit var request: PermissionPrompt
        lateinit var watcher: Application.ActivityLifecycleCallbacks

        fun done(outcome: PermissionOutcome?) {
            if (stage == 2) return
            val prompted = stage == 1
            stage = 2
            activity.application.unregisterActivityLifecycleCallbacks(watcher)
            if (prompted && outcome != null) gate.afterPrompt(request, outcome) else declined()
            next()
        }

        watcher = object : Application.ActivityLifecycleCallbacks {
            override fun onActivityDestroyed(a: Activity) {
                if (a === activity) done(if (stage == 1) PermissionOutcome.DENIED else null)
            }
            override fun onActivityCreated(a: Activity, s: Bundle?) {}
            override fun onActivityStarted(a: Activity) {}
            override fun onActivityResumed(a: Activity) {}
            override fun onActivityPaused(a: Activity) {}
            override fun onActivityStopped(a: Activity) {}
            override fun onActivitySaveInstanceState(a: Activity, s: Bundle) {}
        }
        activity.application.registerActivityLifecycleCallbacks(watcher)

        request = PermissionPrompt(activity, feature, permissions, currentAppId(), blocked) { allowed ->
            onMain {
                if (stage != 0) return@onMain
                if (!allowed) {
                    done(null)
                    return@onMain
                }
                stage = 1
                markAsked(activity, permissions)
                prompt { granted ->
                    onMain {
                        // Dismissing the prompt also leaves no rationale; it is
                        // permanent only if the user could have been asked again.
                        val permanent = !granted && permissions.any {
                            !isGranted(activity, it) && !rationale(activity, it) &&
                                (rationaleBefore.getValue(it) || askedBefore.getValue(it))
                        }
                        done(
                            when {
                                granted -> PermissionOutcome.GRANTED
                                permanent -> PermissionOutcome.DENIED_PERMANENTLY
                                else -> PermissionOutcome.DENIED
                            }
                        )
                    }
                }
            }
        }
        gate.beforePrompt(request)
    }

    private fun isGranted(activity: Activity, permission: String) =
        ContextCompat.checkSelfPermission(activity, permission) == PackageManager.PERMISSION_GRANTED

    private fun rationale(activity: Activity, permission: String) =
        ActivityCompat.shouldShowRequestPermissionRationale(activity, permission)

    private fun prefs(activity: Activity) =
        activity.applicationContext.getSharedPreferences("lingxia_permission_prompts", Context.MODE_PRIVATE)

    private fun asked(activity: Activity, permission: String) = prefs(activity).getBoolean(permission, false)

    private fun markAsked(activity: Activity, permissions: List<String>) {
        prefs(activity).edit().apply { permissions.forEach { putBoolean(it, true) } }.apply()
    }

    private fun currentAppId(): String? = try {
        NativeApi.getCurrentLxApp()?.appId?.takeIf { it.isNotEmpty() }
    } catch (_: LinkageError) {
        null
    }

    private fun onMain(block: () -> Unit) {
        if (Looper.myLooper() == Looper.getMainLooper()) block() else mainHandler.post(block)
    }
}
