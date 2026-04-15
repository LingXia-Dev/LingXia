package com.lingxia.lxapp

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.os.Looper
import android.util.Log
import android.view.View
import android.view.ViewGroup
import androidx.appcompat.app.AppCompatActivity
import java.net.URISyntaxException
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

/**
 * Data class representing LxApp information from the native layer
 */
internal data class LxAppInfo(
    val appName: String,
    val version: String,
    val releaseType: String,
    val cacheDir: String,
)

/**
 * Data class representing the current active LxApp from Rust stack
 */
internal data class CurrentLxApp(
    val appId: String,
    val path: String,
    val sessionId: Long,
) {
    /**
     * Check if this represents a valid LxApp
     */
    fun isValid(): Boolean {
        return appId.isNotEmpty() && sessionId > 0L
    }

    /**
     * Check if this is empty (no active LxApp)
     */
    fun isEmpty(): Boolean {
        return appId.isEmpty()
    }
}

class LxApp private constructor(private val context: Context) {
    companion object {
        private const val TAG = "LingXia.LxApp"
        private var instance: LxApp? = null
        // Properties to store home app details from native
        var HomeLxAppId: String? = null
        @JvmField var capabilities: Int = 0

        // Reference to the current LxAppActivity instance
        private var currentActivity: LxAppActivity? = null
        @Volatile
        private var lifecycleCallbacksRegistered: Boolean = false

        @JvmStatic
        internal fun initialize(context: Context) {
            synchronized(this) {
                if (instance != null && HomeLxAppId != null) {
                    Log.d(TAG, "LxApp already successfully initialized, skipping")
                    return
                }

                if (instance == null) {
                    instance = LxApp(context.applicationContext)
                }
                val appContext = context.applicationContext

                // Handle DeepLink for the current activity if it's being initialized from an Activity
                if (context is android.app.Activity) {
                    handleAppLink(context.intent)
                }

                // Register global activity lifecycle callbacks exactly once
                if (!lifecycleCallbacksRegistered) {
                    lifecycleCallbacksRegistered = registerActivityLifecycleCallbacks(appContext)
                }

                // Set application context for WebView creation
                com.lingxia.webview.LingXiaWebView.setApplicationContext(appContext)

                val initResultString = NativeApi.lingxiaInit(
                    appContext.filesDir.absolutePath,
                    appContext.cacheDir.absolutePath,
                    appContext.assets,
                    LxApp.getLocale()
                )

                if (initResultString != null) {
                    HomeLxAppId = initResultString
                    capabilities = NativeApi.getAppCapabilities()
                } else {
                    Log.e(TAG, "Failed to get home LxApp details from native init.")
                }

                // Configure transparent system bars if we're in an Activity context
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

        @JvmStatic
        fun getInstance(): LxApp {
            return instance ?: throw IllegalStateException("LxApp not initialized")
        }

        // Expose application context for internal helpers (e.g., content resolver)
        @JvmStatic
        fun getApplicationContext(): Context {
            return getInstance().context
        }

        @JvmStatic
        fun exitApp(): Boolean {
            val activity = currentActivity ?: (instance?.context as? Activity)
            if (activity == null) {
                Log.w(TAG, "exitApp failed: no active Activity")
                return false
            }

            if (Looper.myLooper() == Looper.getMainLooper()) {
                activity.finishAffinity()
            } else {
                activity.runOnUiThread {
                    activity.finishAffinity()
                }
            }
            return true
        }

        /**
         * Launch external application with URI
         * @param uri Complete URI to open the target app (e.g., "weixin://dl/scan")
         */
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
                val activity = currentActivity
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
            val activity = currentActivity
            if (activity != null) {
                activity.runOnUiThread {
                    val opened = launchExternalUrl(activity, uri, 0)
                    if (!opened) {
                        Log.w(TAG, "No external handler for URL: $uri")
                    }
                }
                return
            }

            val opened = launchExternalUrl(getInstance().context, uri, 0)
            if (!opened) {
                Log.w(TAG, "No external handler for URL: $uri")
            }
        }

        private fun launchExternalUrl(context: Context, uri: String, depth: Int): Boolean {
            if (depth > 2) {
                return false
            }
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
                        return false
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

        /**
         * Opens a mini app in the current activity
         *
         * This method updates the content of the current LxAppActivity to host the specified lxapp.
         * It notifies the native layer about the mini app being opened for state tracking.
         * The app configuration (including TabBar) will be fetched from the native layer.
         *
         * @param appId The unique identifier of the mini app to open
         * @param path The initial path to navigate to within the mini app
         */
        @JvmStatic
        fun openLxApp(appId: String, path: String) {
            val sessionId = NativeApi.getLxAppSessionId(appId)
            if (sessionId <= 0L) {
                Log.e(TAG, "Missing valid session for appId=$appId")
                return
            }
            openLxAppWithSession(appId, path, sessionId)
        }

        /**
         * Runtime bridge entry (called from Rust/JNI) with explicit session.
         * Keep this overload for FFI signature stability.
         */
        @JvmStatic
        fun openLxApp(appId: String, path: String, sessionId: Long) {
            openLxAppWithSession(appId, path, sessionId)
        }

        /**
         * Internal/runtime open entry with explicit session.
         */
        @JvmStatic
        internal fun openLxAppWithSession(appId: String, path: String, sessionId: Long) {
            val instance = getInstance()
            instance.openInCurrentActivity(appId, path, sessionId)
        }

        /**
         * Opens the home LxApp
         * Its appId is provided by the native layer during initialization.
         * The initial route will be resolved by on_lxapp_opened.
         *
         * If the appId is not available, an error will be logged, and no app will be opened.
         */
        @JvmStatic
        internal fun openHomeLxApp() {
            if (HomeLxAppId != null) {
                val current = NativeApi.getCurrentLxApp()
                val sessionId = current?.takeIf { it.appId == HomeLxAppId }?.sessionId
                    ?: NativeApi.getLxAppSessionId(HomeLxAppId!!)
                if (sessionId <= 0L) {
                    Log.e(TAG, "Missing valid session for home LxApp: ${HomeLxAppId}")
                    return
                }
                openLxAppWithSession(HomeLxAppId!!, "", sessionId)
            } else {
                Log.e(TAG, "Native home app details not available. Cannot open home mini app.")
            }
        }

        /**
         * Notifies the system to close a lxapp with the specified appId
         * This method is typically called by the native layer when a lxapp needs to be closed
         *
         * @param appId The ID of the mini app to close
         */
        @JvmStatic
        internal fun closeLxAppWithSession(appId: String, sessionId: Long) {
            if (sessionId <= 0L) {
                Log.e(TAG, "Refusing to close LxApp without valid sessionId: appId=$appId")
                return
            }
            Log.d(TAG, "Closing LxApp with appId: $appId")

            // Notify the current activity to close the LxApp
            val activity = currentActivity?.takeIf { it.getAppId() == appId }
            if (activity != null) {
                activity.runOnUiThread {
                    activity.closeLxApp(sessionId)
                }
            } else {
                Log.w(TAG, "No matching activity for appId: $appId")
            }
        }

        /**
         * Public close entry without exposing session id.
         */
        @JvmStatic
        fun closeLxApp(appId: String) {
            val activity = currentActivity?.takeIf { it.getAppId() == appId }
            val sessionId = activity?.getSessionId() ?: NativeApi.getLxAppSessionId(appId)
            if (sessionId <= 0L) {
                Log.e(TAG, "Missing valid session for close appId=$appId")
                return
            }
            closeLxAppWithSession(appId, sessionId)
        }

        /**
         * Runtime bridge entry (called from Rust/JNI) with explicit session.
         * Keep this overload for FFI signature stability.
         */
        @JvmStatic
        fun closeLxApp(appId: String, sessionId: Long) {
            closeLxAppWithSession(appId, sessionId)
        }

        /**
         * Navigate to a specific path within the lxapp with animation type
         * This method is called from Rust FFI
         *
         * @param appId The unique identifier of the lxapp
         * @param path The target path to navigate to within the lxapp
         * @param animationTypeInt The type of animation to perform as integer
         * @return true if navigation was successful, false otherwise
         */
        @JvmStatic
        fun navigate(appId: String, path: String, animationTypeInt: Int): Boolean {
            val animationType = AnimationType.fromInt(animationTypeInt)
            Log.d(TAG, "navigate called for appId: $appId, path: $path, type: $animationType (from int: $animationTypeInt)")
            val activity = currentActivity?.takeIf { it.getAppId() == appId }
            return if (activity != null) {
                activity.runOnUiThread {
                    activity.navigate(path, animationType)
                }
                true
            } else {
                Log.w(TAG, "No matching activity for appId: $appId")
                false
            }
        }

        /**
         * Update TabBar UI for a specific LxApp
         * This triggers a refresh of TabBar data from the native layer
         *
         * @param appId The unique identifier of the mini app whose TabBar needs updating
         * @return true if successful, false otherwise
         */
        @JvmStatic
        fun updateTabBarUI(appId: String): Boolean {
            Log.d(TAG, "updateTabBarUI called for appId: $appId")
            val activity = currentActivity?.takeIf { it.getAppId() == appId }
            return if (activity != null) {
                activity.runOnUiThread {
                    LxAppActivity.updateTabBarUI(appId)
                }
                true
            } else {
                Log.w(TAG, "No matching activity for appId: $appId in updateTabBarUI")
                false
            }
        }

        /**
         * Register activity lifecycle callbacks to automatically handle DeepLinks
         */
        private fun registerActivityLifecycleCallbacks(context: Context): Boolean {
            val application = context.applicationContext as? android.app.Application
            application?.registerActivityLifecycleCallbacks(object : android.app.Application.ActivityLifecycleCallbacks {
                override fun onActivityCreated(activity: android.app.Activity, savedInstanceState: Bundle?) {
                    handleAppLink(activity.intent)
                    if (activity is LxAppActivity) {
                        setCurrentActivity(activity)
                    }
                }

                override fun onActivityResumed(activity: android.app.Activity) {
                    if (activity is LxAppActivity) {
                        setCurrentActivity(activity)
                    }
                }

                override fun onActivityDestroyed(activity: android.app.Activity) {
                    if (activity is LxAppActivity && currentActivity == activity) {
                        setCurrentActivity(null)
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

        /**
         * Handle DeepLink from an Activity's intent (internal use)
         */
        @JvmStatic
        internal fun handleAppLink(intent: Intent) {
            val data = intent.data
            if (intent.action == Intent.ACTION_VIEW && data?.scheme == "https") {
                val url = data.toString()
                NativeApi.onAppLinkReceived(url)
            }
        }

        /**
         * Set the current LxAppActivity instance
         */
        @JvmStatic
        internal fun setCurrentActivity(activity: LxAppActivity?) {
            currentActivity = activity
            UpdateManager.init(activity)
        }

        /**
         * Get the current LxAppActivity instance
         */
        @JvmStatic
        fun getCurrentActivity(): LxAppActivity? = currentActivity

        @JvmStatic
        fun applicationContext(): Context? = instance?.context

        /**
         * Show toast
         * @param title Toast message
         * @param icon Toast icon type (default: None for simple text toast)
         * @param image Custom image path (absolute path only)
         * @param duration Duration in seconds (default: 1.5, use 0.0 for no auto-hide)
         * @param mask Whether to show mask to prevent touch through
         * @param position Toast position
         */
        /**
         * Hide toast
         */
        /**
         * Show popup WebView overlay.
         */
        /**
         * Hide popup WebView overlay.
         */
        /**
         * Show modal dialog
         * @param title Modal title
         * @param content Modal content/message
         * @param showCancel Whether to show cancel button (default: true)
         * @param cancelText Cancel button text (default: "Cancel")
         * @param confirmText Confirm button text (default: "OK")
         * @param confirmColor Custom color for confirm button
         * @param callbackId for async result
         */
        /**
         * Show action sheet with options
         * @param options Action sheet options including items, cancel text, and callback ID
         */
        /**
         * Show picker with options
         * @param options Picker options including columns, buttons, and callback ID
         */
        /**
         * Update NavigationBar UI for a specific LxApp
         * This is called by the native layer to trigger navbar UI refresh
         * The NavigationBar will read fresh state from Rust and update itself
         *
         * @param appId The unique identifier of the mini app whose NavigationBar needs updating
         * @return true if successful, false otherwise
         */
        @JvmStatic
        fun updateNavBarUI(appId: String): Boolean {
            Log.d(TAG, "updateNavBarUI called for appId: $appId")
            val activity = currentActivity?.takeIf { it.getAppId() == appId }
            return if (activity != null) {
                activity.runOnUiThread {
                    LxAppActivity.updateNavBarUI(appId)
                }
                true
            } else {
                Log.w(TAG, "No matching activity for appId: $appId in updateNavBarUI")
                false
            }
        }

        /**
         * Update orientation for a specific LxApp
         * This triggers re-application of the current page orientation.
         *
         * @param appId The unique identifier of the mini app whose orientation needs updating
         * @return true if successful, false otherwise
         */
        @JvmStatic
        fun updateOrientationUI(appId: String): Boolean {
            Log.d(TAG, "updateOrientationUI called for appId: $appId")
            val activity = currentActivity?.takeIf { it.getAppId() == appId }
            return if (activity != null) {
                activity.runOnUiThread {
                    LxAppActivity.updateOrientationUI(appId)
                }
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
                return activity.getCapsuleRectJSON()
            }

            val result = AtomicReference("{}")
            val latch = CountDownLatch(1)
            activity.runOnUiThread {
                try {
                    result.set(activity.getCapsuleRectJSON())
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
    }

    private fun openInCurrentActivity(appId: String, path: String, sessionId: Long) {
        if (sessionId <= 0L) {
            Log.e(TAG, "Refusing to open LxApp without valid sessionId: appId=$appId")
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
                    Log.d(TAG, "Opening app in current activity")
                    activity.openLxApp(appId, resolvedPath, sessionId)
                } else {
                    Log.d(TAG, "Creating new activity")
                    val intent = Intent(context, LxAppActivity::class.java).apply {
                        putExtra(LxAppActivity.EXTRA_APP_ID, appId)
                        putExtra(LxAppActivity.EXTRA_PATH, resolvedPath)
                        putExtra(LxAppActivity.EXTRA_SESSION_ID, sessionId)
                        addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                    }
                    context.startActivity(intent)
                }
            } catch (e: Exception) {
                Log.e(TAG, "Failed to open LxApp: ${e.message}")
            }
        }

        if (Looper.myLooper() == Looper.getMainLooper()) {
            openTask.run()
        } else {
            currentActivity?.runOnUiThread(openTask)
                ?: android.os.Handler(Looper.getMainLooper()).post(openTask)
        }
    }

    fun getContext(): Context = context
}
