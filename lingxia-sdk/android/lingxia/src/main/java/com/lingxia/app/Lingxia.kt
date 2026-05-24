package com.lingxia.app

import android.app.Activity
import android.app.ActivityManager
import android.app.Application
import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.Process
import android.util.Log
import androidx.appcompat.app.AppCompatActivity
import com.lingxia.lxapp.LxAppActivity
import com.lingxia.lxapp.LxApp
import com.lingxia.lxapp.LxAppBrowserOverlay
import java.net.URISyntaxException
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Top-level entry point for the LingXia host SDK. Operations scoped to the
 * host application (process, window, external URL handoff, lifecycle) live
 * here. LxApp-scope bridge APIs live in [com.lingxia.lxapp.LxApp].
 */
object Lingxia {
    private const val TAG = "LingXia.Lingxia"
    // Delay killProcess on exit until ATM has processed the task removal — killing
    // in the same tick races with ATM and the system relaunches the LAUNCHER.
    private const val EXIT_KILL_DELAY_MS = 150L

    const val CAP_SHELL: Int = 0x1
    const val CAP_NOTIFICATIONS: Int = 0x2

    @JvmField var capabilities: Int = 0

    private val hostAddonInstalled = AtomicBoolean(false)
    private var appContext: Context? = null
    private var lastResumedActivity: Activity? = null
    @Volatile
    private var lifecycleCallbacksRegistered: Boolean = false

    /** Product-app entry point. Initializes the runtime and opens the home LxApp. */
    @JvmStatic
    fun quickStart(activity: AppCompatActivity) {
        quickStart(activity, null)
    }

    /**
     * Product-app entry point with an app-owned native addon registrar.
     *
     * The SDK loads liblingxia before invoking [registerHostAddon], so host apps do not need to
     * call System.loadLibrary themselves.
     */
    @JvmStatic
    fun quickStart(activity: AppCompatActivity, registerHostAddon: (() -> Unit)?) {
        if (!NativeApi.ensureLoaded()) {
            throw IllegalStateException("Failed to load native library 'lingxia'")
        }
        if (registerHostAddon != null && hostAddonInstalled.compareAndSet(false, true)) {
            try {
                registerHostAddon()
            } catch (error: Throwable) {
                hostAddonInstalled.set(false)
                throw error
            }
        }
        initializeRuntime(activity)
        LxApp.openHome()
    }

    @JvmStatic
    internal fun initializeRuntime(context: Context) {
        synchronized(this) {
            if (appContext != null && LxApp.homeAppId != null) {
                Log.d(TAG, "Lingxia already successfully initialized, skipping")
                return
            }
            val ctx = context.applicationContext
            if (appContext == null) {
                appContext = ctx
            }

            if (context is Activity) {
                handleAppLink(context.intent)
            }

            if (!lifecycleCallbacksRegistered) {
                lifecycleCallbacksRegistered = registerActivityLifecycleCallbacks(ctx)
            }

            com.lingxia.webview.LingXiaWebView.setApplicationContext(ctx)

            val initResultString = NativeApi.lingxiaInit(
                ctx.filesDir.absolutePath,
                ctx.cacheDir.absolutePath,
                ctx.assets,
                ctx,
                getLocale()
            )

            if (initResultString != null) {
                LxApp.homeAppId = initResultString
                capabilities = NativeApi.getAppCapabilities()
            } else {
                Log.e(TAG, "Failed to get home app details from native init.")
            }

            if (context is AppCompatActivity) {
                LxAppActivity.configureTransparentSystemBars(context)
            }
        }
    }

    @JvmStatic
    fun enableWebViewDebugging() {
        com.lingxia.lxapp.WebView.enableDebugging()
    }

    @JvmStatic
    fun getLocale(): String {
        return try {
            val locale = java.util.Locale.getDefault()
            "${locale.language}-${locale.country}"
        } catch (e: Exception) {
            Log.w(TAG, "Failed to get system locale, using default", e)
            "en-US"
        }
    }

    /** Application context registered during [initializeRuntime]. */
    @JvmStatic
    fun applicationContext(): Context? = appContext

    /** Like [applicationContext] but throws if the SDK is not initialized. */
    @JvmStatic
    fun getApplicationContext(): Context =
        appContext ?: throw IllegalStateException("Lingxia not initialized")

    /**
     * The most recent foreground Activity of any type — surfaces non-LxAppActivity
     * hosts (e.g. an app embedding LingXia inside a custom AppCompatActivity).
     */
    @JvmStatic
    fun getLastResumedActivity(): Activity? = lastResumedActivity

    /** JNI entry: terminate the host process. Called from Rust via lx.app.exit. */
    @JvmStatic
    fun exitApp(): Boolean {
        val activity = LxApp.getCurrentActivity() ?: (appContext as? Activity)
        if (activity == null) {
            Log.w(TAG, "exitApp failed: no active Activity")
            return false
        }
        val doExit = {
            activity.finishAffinity()
            // Drop the task from recents — finishAffinity alone leaves it lingering.
            (activity.getSystemService(Context.ACTIVITY_SERVICE) as? ActivityManager)
                ?.appTasks?.forEach { it.finishAndRemoveTask() }
            // Kill the process so the native runtime is reset; otherwise the next
            // launch restores the lxapp's previous page instead of opening home.
            Handler(Looper.getMainLooper()).postDelayed(
                { Process.killProcess(Process.myPid()) },
                EXIT_KILL_DELAY_MS,
            )
        }
        if (Looper.myLooper() == Looper.getMainLooper()) {
            doExit()
        } else {
            activity.runOnUiThread { doExit() }
        }
        return true
    }

    /** JNI entry: open an arbitrary URI. */
    @JvmStatic
    fun launchWithUrl(
        uri: String,
        target: String = "external",
        ownerAppId: String = "",
        ownerSessionId: Long = 0L
    ) {
        Log.d(TAG, "launchWithUrl called with URI: $uri, target: $target")
        // Mobile currently uses a single in-app browser experience:
        // treat "new_browser_tab" the same as "self" for now.
        val inAppBrowserTarget = target == "self" || target == "new_browser_tab"
        if (inAppBrowserTarget) {
            val activity = LxApp.getCurrentActivity()
            if (activity != null) {
                activity.runOnUiThread {
                    val current = NativeApi.getCurrentLxApp()
                    val fallbackOwnerAppId = current?.appId ?: activity.getAppId()
                    val resolvedOwnerAppId = ownerAppId.takeIf { it.isNotBlank() } ?: fallbackOwnerAppId
                    val resolvedOwnerSessionId = when {
                        ownerSessionId > 0L -> ownerSessionId
                        current != null &&
                            current.appId == resolvedOwnerAppId &&
                            current.sessionId > 0L -> current.sessionId
                        resolvedOwnerAppId.isNotBlank() -> NativeApi.getLxAppSessionId(resolvedOwnerAppId)
                        else -> 0L
                    }
                    if (resolvedOwnerAppId.isBlank() || resolvedOwnerSessionId <= 0L) {
                        Log.w(
                            TAG,
                            "launchWithUrl target=in_app: invalid owner appId=$resolvedOwnerAppId session=$resolvedOwnerSessionId"
                        )
                        return@runOnUiThread
                    }
                    val tabId = NativeApi.openBrowserTab(
                        resolvedOwnerAppId,
                        resolvedOwnerSessionId,
                        uri
                    )
                    if (tabId.isNullOrBlank()) {
                        Log.w(
                            TAG,
                            "launchWithUrl target=in_app: openBrowserTab failed appId=$resolvedOwnerAppId session=$resolvedOwnerSessionId"
                        )
                        return@runOnUiThread
                    }
                    LxAppBrowserOverlay.show(activity, tabId, uri)
                }
                return
            }
            Log.w(TAG, "launchWithUrl target=in_app: no current activity")
            return
        }
        val activity = LxApp.getCurrentActivity()
        if (activity != null) {
            activity.runOnUiThread {
                if (!launchExternalUrl(activity, uri, 0)) {
                    Log.w(TAG, "No external handler for URL: $uri")
                }
            }
            return
        }
        val ctx = appContext ?: run {
            Log.w(TAG, "launchWithUrl: Lingxia not initialized")
            return
        }
        if (!launchExternalUrl(ctx, uri, 0)) {
            Log.w(TAG, "No external handler for URL: $uri")
        }
    }

    private fun launchExternalUrl(context: Context, uri: String, depth: Int): Boolean {
        if (depth > 2) return false
        return try {
            if (uri.startsWith("intent://", ignoreCase = true)) {
                val parsedIntent = Intent.parseUri(uri, Intent.URI_INTENT_SCHEME).apply {
                    addCategory(Intent.CATEGORY_BROWSABLE)
                    addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                    component = null
                }
                try {
                    context.startActivity(parsedIntent)
                    true
                } catch (_: ActivityNotFoundException) {
                    val fallbackUrl = parsedIntent.getStringExtra("browser_fallback_url")
                    if (!fallbackUrl.isNullOrBlank()) {
                        return launchExternalUrl(context, fallbackUrl, depth + 1)
                    }
                    false
                }
            } else {
                val intent = Intent(Intent.ACTION_VIEW, android.net.Uri.parse(uri)).apply {
                    addCategory(Intent.CATEGORY_BROWSABLE)
                    addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                }
                try {
                    context.startActivity(intent)
                    true
                } catch (_: ActivityNotFoundException) {
                    false
                }
            }
        } catch (e: URISyntaxException) {
            Log.e(TAG, "Invalid intent URI: $uri", e)
            false
        } catch (e: Exception) {
            Log.e(TAG, "Failed to launch with URL: $uri", e)
            false
        }
    }

    @JvmStatic
    internal fun handleAppLink(intent: Intent) {
        val data = intent.data
        if (intent.action == Intent.ACTION_VIEW && data?.scheme == "https") {
            NativeApi.onAppLinkReceived(data.toString())
        }
    }

    private fun registerActivityLifecycleCallbacks(context: Context): Boolean {
        val application = context.applicationContext as? Application
        application?.registerActivityLifecycleCallbacks(object : Application.ActivityLifecycleCallbacks {
            override fun onActivityCreated(activity: Activity, savedInstanceState: Bundle?) {
                handleAppLink(activity.intent)
                if (activity is LxAppActivity) {
                    LxApp.setCurrentActivity(activity)
                }
            }
            override fun onActivityResumed(activity: Activity) {
                lastResumedActivity = activity
                if (activity is LxAppActivity) {
                    LxApp.setCurrentActivity(activity)
                }
            }
            override fun onActivityDestroyed(activity: Activity) {
                if (lastResumedActivity === activity) lastResumedActivity = null
                if (activity is LxAppActivity && LxApp.getCurrentActivity() == activity) {
                    LxApp.setCurrentActivity(null)
                }
            }
            override fun onActivityStarted(activity: Activity) {}
            override fun onActivityPaused(activity: Activity) {}
            override fun onActivityStopped(activity: Activity) {}
            override fun onActivitySaveInstanceState(activity: Activity, outState: Bundle) {}
        }) ?: run {
            Log.w(TAG, "Failed to register ActivityLifecycleCallbacks: Application not found")
            return false
        }
        return true
    }
}
