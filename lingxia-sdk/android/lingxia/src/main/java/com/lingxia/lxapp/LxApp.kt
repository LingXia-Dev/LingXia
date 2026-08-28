package com.lingxia.lxapp

import com.lingxia.app.CurrentLxApp

import android.content.Intent
import android.os.Handler
import android.os.Looper
import android.util.Log
import androidx.appcompat.app.AppCompatDelegate
import com.lingxia.app.Lingxia
import com.lingxia.app.LxLog
import com.lingxia.app.NativeApi
import com.lingxia.app.UpdateManager
import java.util.concurrent.CountDownLatch
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

/**
 * JNI bridge / static facade for LxApp-scope operations. Host-scope SDK APIs
 * live in [com.lingxia.app.Lingxia].
 */
object LxApp {
    private const val TAG = "LingXia.LxApp"

    /** App id of the home LxApp, populated by the native layer during [Lingxia.initializeRuntime]. */
    @JvmStatic
    var homeAppId: String? = null
        internal set

    private var currentActivity: LxAppActivity? = null
    private val appearanceByApp = ConcurrentHashMap<String, Boolean>()

    @JvmStatic
    internal fun setCurrentActivity(activity: LxAppActivity?) {
        currentActivity = activity
        UpdateManager.init(activity)
    }

    /**
     * Replay the stored scheme for a (re)created activity. Must run after the
     * activity's chrome views exist: applying runs synchronously on the main
     * thread and drives navbar/tabbar updates.
     */
    @JvmStatic
    internal fun replayStoredAppearance(activity: LxAppActivity) {
        appearanceByApp[activity.appId]?.let { dark -> applyAppearance(activity.appId, dark) }
    }

    @JvmStatic
    fun getCurrentActivity(): LxAppActivity? = currentActivity

    /** The lxapp's applied scheme, or null before one resolves. */
    @JvmStatic
    fun appearanceDarkFor(appId: String): Boolean? = appearanceByApp[appId]

    @JvmStatic
    fun hostAppearanceDark(): Boolean {
        // Activity resources may already carry an lxapp-local night override.
        val resources = Lingxia.applicationContext()?.resources ?: return false
        return (resources.configuration.uiMode and android.content.res.Configuration.UI_MODE_NIGHT_MASK) ==
            android.content.res.Configuration.UI_MODE_NIGHT_YES
    }

    @JvmStatic
    fun applyAppearance(appId: String, dark: Boolean): Boolean {
        appearanceByApp[appId] = dark
        val activity = currentActivity?.takeIf { it.getAppId() == appId } ?: return true
        activity.runOnUiThread {
            activity.delegate.localNightMode = if (dark) {
                AppCompatDelegate.MODE_NIGHT_YES
            } else {
                AppCompatDelegate.MODE_NIGHT_NO
            }
            // uiMode is a declared configChange, so nothing re-dispatches the
            // updated configuration to attached views; WebViews re-evaluate
            // prefers-color-scheme only when it reaches them.
            activity.window.decorView.dispatchConfigurationChanged(
                activity.resources.configuration
            )
            activity.applyCanvasBackground()
            LxAppActivity.updateNavBarUI(appId)
            LxAppActivity.updateTabBarUI(appId)
        }
        return true
    }

    @JvmStatic
    fun open(appId: String, path: String) {
        val sessionId = NativeApi.getLxAppSessionId(appId)
        if (sessionId <= 0L) {
            LxLog.e(TAG, "Missing valid session for appId=$appId")
            return
        }
        openWithSession(appId, path, sessionId)
    }

    /** Runtime bridge entry (called from Rust/JNI) with explicit session. */
    @JvmStatic
    fun open(appId: String, path: String, sessionId: Long) {
        openWithSession(appId, path, sessionId)
    }

    @JvmStatic
    internal fun openWithSession(appId: String, path: String, sessionId: Long) {
        openInCurrentActivity(appId, path, sessionId)
    }

    @JvmStatic
    internal fun openHome() {
        val homeId = homeAppId
        if (homeId == null) {
            LxLog.e(TAG, "Native home app details not available. Cannot open home LxApp.")
            return
        }
        val current = NativeApi.getCurrentLxApp()
        val sessionId = current?.takeIf { it.appId == homeId }?.sessionId
            ?: NativeApi.getLxAppSessionId(homeId)
        if (sessionId <= 0L) {
            LxLog.e(TAG, "Missing valid session for home app: $homeId")
            return
        }
        openWithSession(homeId, "", sessionId)
    }

    @JvmStatic
    internal fun closeWithSession(appId: String, sessionId: Long) {
        if (sessionId <= 0L) {
            LxLog.e(TAG, "Refusing to close LxApp without valid sessionId: appId=$appId")
            return
        }
        Log.d(TAG, "Closing LxApp with appId: $appId")
        val activity = currentActivity?.takeIf { it.getAppId() == appId }
        if (activity != null) {
            activity.runOnUiThread { activity.closeLxApp(sessionId) }
        } else {
            Log.w(TAG, "No matching activity for appId: $appId")
        }
    }

    @JvmStatic
    fun close(appId: String) {
        val activity = currentActivity?.takeIf { it.getAppId() == appId }
        val sessionId = activity?.getSessionId() ?: NativeApi.getLxAppSessionId(appId)
        if (sessionId <= 0L) {
            LxLog.e(TAG, "Missing valid session for close appId=$appId")
            return
        }
        closeWithSession(appId, sessionId)
    }

    /** Runtime bridge entry (called from Rust/JNI) with explicit session. */
    @JvmStatic
    fun close(appId: String, sessionId: Long) {
        closeWithSession(appId, sessionId)
    }

    @JvmStatic
    fun navigate(appId: String, path: String, animationTypeInt: Int): Boolean {
        val animationType = AnimationType.fromInt(animationTypeInt)
        Log.d(TAG, "navigate called for appId: $appId, path: $path, type: $animationType")
        val activity = currentActivity?.takeIf { it.getAppId() == appId }
        return if (activity != null) {
            activity.runOnUiThread { activity.navigate(path, animationType) }
            true
        } else {
            Log.w(TAG, "No matching activity for appId: $appId")
            false
        }
    }

    /** Runtime signal (JNI): the home page finished its first render. */
    @JvmStatic
    fun onHomeFirstReady() {
        SplashOverlay.notifyHomeReady()
    }

    /**
     * Runtime signal (JNI): the home page is ready and the host has a campaign
     * to show before the launch layer lifts.
     */
    @JvmStatic
    fun showSplashCampaign(imagePath: String, durationMs: Int) {
        SplashOverlay.showCampaign(imagePath, durationMs)
    }

    /**
     * Runtime signal (JNI): the app's own appearance, pinned or following the
     * system (-1). Persisted with the system so the *next* launch — the splash
     * window the OS composes before any code runs included — resolves the same
     * night mode. Below API 31 there is no such lever, and a pinned app keeps
     * one launch frame in the system's appearance.
     */
    @JvmStatic
    fun setHostColorMode(mode: Int) {
        val activity = currentActivity ?: return
        if (android.os.Build.VERSION.SDK_INT < android.os.Build.VERSION_CODES.S) return
        val manager = activity.getSystemService(android.app.UiModeManager::class.java) ?: return
        val nightMode = when (mode) {
            0 -> android.app.UiModeManager.MODE_NIGHT_NO
            1 -> android.app.UiModeManager.MODE_NIGHT_YES
            else -> android.app.UiModeManager.MODE_NIGHT_AUTO
        }
        runCatching { manager.setApplicationNightMode(nightMode) }
    }

    @JvmStatic
    fun updateTabBarUI(appId: String): Boolean {
        val activity = currentActivity?.takeIf { it.getAppId() == appId }
        return if (activity != null) {
            LxAppActivity.updateTabBarUI(appId)
        } else {
            Log.w(TAG, "No matching activity for appId: $appId in updateTabBarUI")
            false
        }
    }

    /**
     * Async TabBar update for callers that await the real result
     * (lx.tabBar.update). Runs on the UI thread and reports
     * completion through NativeApi.onCallback — never blocks the caller.
     */
    @JvmStatic
    fun updateTabBarUIAsync(callbackId: Long, appId: String) {
        val activity = currentActivity?.takeIf { it.getAppId() == appId }
        if (activity == null) {
            Log.w(TAG, "No matching activity for appId: $appId in updateTabBarUIAsync")
            NativeApi.onCallback(callbackId, false, "1000")
            return
        }
        activity.runOnUiThread {
            val updated = LxAppActivity.updateTabBarUI(appId)
            if (updated) {
                NativeApi.onCallback(callbackId, true, "{}")
            } else {
                NativeApi.onCallback(callbackId, false, "1000")
            }
        }
    }

    @JvmStatic
    fun updateNavBarUI(appId: String): Boolean {
        val activity = currentActivity?.takeIf { it.getAppId() == appId }
        return if (activity != null) {
            activity.runOnUiThread { LxAppActivity.updateNavBarUI(appId) }
            true
        } else {
            Log.w(TAG, "No matching activity for appId: $appId in updateNavBarUI")
            false
        }
    }

    @JvmStatic
    fun updateOrientationUI(appId: String): Boolean {
        val activity = currentActivity?.takeIf { it.getAppId() == appId }
        return if (activity != null) {
            activity.runOnUiThread { LxAppActivity.updateOrientationUI(appId) }
            true
        } else {
            Log.w(TAG, "No matching activity for appId: $appId in updateOrientationUI")
            false
        }
    }

    @JvmStatic
    fun getCapsuleRect(): String {
        val activity = currentActivity ?: return "{}"
        if (Looper.myLooper() == Looper.getMainLooper()) {
            return activity.getCapsuleRectJSON(activity.getAppId())
        }
        val result = AtomicReference("{}")
        val latch = CountDownLatch(1)
        activity.runOnUiThread {
            try {
                result.set(activity.getCapsuleRectJSON(activity.getAppId()))
            } finally {
                latch.countDown()
            }
        }
        try {
            latch.await(300, TimeUnit.MILLISECONDS)
        } catch (e: InterruptedException) {
            Thread.currentThread().interrupt()
        }
        return result.get()
    }

    private fun openInCurrentActivity(appId: String, path: String, sessionId: Long) {
        if (sessionId <= 0L) {
            LxLog.e(TAG, "Refusing to open LxApp without valid sessionId: appId=$appId")
            return
        }
        val openTask = Runnable {
            try {
                val resolvedPath = NativeApi.onLxAppOpened(appId, path, sessionId)
                if (resolvedPath.isBlank()) {
                    Log.w(TAG, "onLxAppOpened rejected open request (stale session?) appId=$appId sessionId=$sessionId")
                    return@Runnable
                }
                val activity = currentActivity
                if (activity != null) {
                    activity.openLxApp(appId, resolvedPath, sessionId)
                } else {
                    val ctx = Lingxia.applicationContext()
                    if (ctx == null) {
                        LxLog.e(TAG, "Lingxia not initialized; cannot start LxAppActivity")
                        return@Runnable
                    }
                    val intent = Intent(ctx, LxAppActivity::class.java).apply {
                        putExtra(LxAppActivity.EXTRA_APP_ID, appId)
                        putExtra(LxAppActivity.EXTRA_PATH, resolvedPath)
                        putExtra(LxAppActivity.EXTRA_SESSION_ID, sessionId)
                        addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                    }
                    ctx.startActivity(intent)
                }
            } catch (e: Exception) {
                LxLog.e(TAG, "Failed to open LxApp: ${e.message}")
            }
        }
        if (Looper.myLooper() == Looper.getMainLooper()) {
            openTask.run()
        } else {
            currentActivity?.runOnUiThread(openTask)
                ?: Handler(Looper.getMainLooper()).post(openTask)
        }
    }
}

/** Snapshot of an LxApp's identity, returned by the native layer. */
data class LxAppInfo(
    val appName: String,
    val version: String,
    val releaseType: String,
    val cacheDir: String,
)
