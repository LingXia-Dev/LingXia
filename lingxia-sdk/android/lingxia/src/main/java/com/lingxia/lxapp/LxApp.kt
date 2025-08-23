package com.lingxia.lxapp

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.util.Log
import android.view.View
import android.view.ViewGroup
import androidx.appcompat.app.AppCompatActivity

/**
 * Data class representing LxApp information from the native layer
 */
data class LxAppInfo(
    val initialRoute: String,
    val appName: String,
    val debug: Boolean
)

class LxApp private constructor(private val context: Context) {
    companion object {
        private const val TAG = "LingXia.LxApp"
        private var instance: LxApp? = null

        // Properties to store home app details from native
        var HomeLxAppId: String? = null
        var HomeLxAppInitialRoute: String? = null

        private val pageConfigCache = mutableMapOf<String, NavigationBarConfig>()

        // Cache for initial routes of each app
        private val initialRouteCache = mutableMapOf<String, String>()

        // Reference to the current LxAppActivity instance
        private var currentActivity: LxAppActivity? = null

        // Clear cache when app is closed to prevent memory leaks
        fun clearPageConfigCache() {
            pageConfigCache.clear()
        }

        // Clear initial route cache
        fun clearInitialRouteCache() {
            initialRouteCache.clear()
        }

        // Cache initial route for an app
        fun cacheInitialRoute(appId: String) {
            if (!initialRouteCache.containsKey(appId)) {
                val appInfo = NativeApi.getLxAppInfo(appId)
                if (appInfo != null) {
                    initialRouteCache[appId] = appInfo.initialRoute
                    Log.d(TAG, "Cached initial route for $appId: ${appInfo.initialRoute}")
                }
            }
        }

        @JvmStatic
        fun initialize(context: Context) {
            if (instance != null && HomeLxAppId != null && HomeLxAppInitialRoute != null) {
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

            // Register global activity lifecycle callbacks to automatically handle DeepLinks
            registerActivityLifecycleCallbacks(appContext)

            // Set application context for WebView creation
            com.lingxia.webview.LingXiaWebView.setApplicationContext(appContext)

            val initResultString = NativeApi.onLxAppInited(
                appContext.filesDir.absolutePath,
                appContext.cacheDir.absolutePath,
                appContext.assets
            )

            if (initResultString != null) {
                HomeLxAppId = initResultString
                // Get initial route and other app info using new API
                val appInfo = NativeApi.getLxAppInfo(initResultString)
                if (appInfo != null) {
                    HomeLxAppInitialRoute = appInfo.initialRoute
                    Log.i(TAG, "Native init success. Home App ID: $HomeLxAppId, Initial Route: $HomeLxAppInitialRoute")
                } else {
                    Log.e(TAG, "Failed to get LxApp info from new API")
                    HomeLxAppInitialRoute = "/"
                }
            } else {
                Log.e(TAG, "Failed to get home LxApp details from native init.")
            }

            // Configure transparent system bars if we're in an Activity context
            if (context is AppCompatActivity) {
                LxAppActivity.configureTransparentSystemBars(context)
            }
        }

        @JvmStatic
        fun enableWebViewDebugging() {
            com.lingxia.lxapp.WebView.enableDebugging()
        }

        @JvmStatic
        fun getInstance(): LxApp {
            return instance ?: throw IllegalStateException("LxApp not initialized")
        }

        /**
         * Launch external application with URI
         * @param uri Complete URI to open the target app (e.g., "weixin://dl/scan")
         */
        @JvmStatic
        fun launchWithUrl(uri: String) {
            Log.d(TAG, "launchWithUrl called with URI: $uri")
            try {
                val intent = Intent(Intent.ACTION_VIEW, android.net.Uri.parse(uri)).apply {
                    addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                }
                getInstance().context.startActivity(intent)
            } catch (e: Exception) {
                Log.e(TAG, "Failed to launch with URL: $uri", e)
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
            // Cache the initial route for this app when opening
            cacheInitialRoute(appId)

            val instance = getInstance()
            instance.openInCurrentActivity(appId, path)
        }

        /**
         * Opens the home LxApp
         * Its appId and initial path are provided by the native layer during initialization.
         *
         * If these details are not available, an error will be logged, and no app will be opened.
         */
        @JvmStatic
        fun openHomeLxApp() {
            if (HomeLxAppId != null && HomeLxAppInitialRoute != null) {
                openLxApp(HomeLxAppId!!, HomeLxAppInitialRoute!!)
            } else {
                Log.e(TAG, "Native home app details not available. Cannot open home mini app.")
            }
        }

        /**
         * Notifies the system to close a mini app with the specified appId
         *
         * This method is typically called by the native layer when a mini app needs to be closed
         *
         * @param appId The ID of the mini app to close
         */
        @JvmStatic
        fun closeLxApp(appId: String) {
            Log.d(TAG, "Closing LxApp with appId: $appId")

            // Notify the current activity to close if it matches the appId
            if (currentActivity?.getAppId() == appId) {
                currentActivity?.closeApp()
            }
        }

        /**
         * Switches the current page within a running LxAppActivity
         *
         * This method calls the switchPage method directly on the current activity
         *
         * @param appId The unique identifier of the mini app whose page needs switching
         * @param path The target path to navigate to within the mini app
         */
        @JvmStatic
        fun switchPage(appId: String, path: String) {
            Log.d(TAG, "Requesting page switch for appId: $appId to path: $path")

            // Switch page in the current activity if it matches the appId
            if (currentActivity?.getAppId() == appId) {
                currentActivity?.switchPage(path)
            }
        }

        @JvmStatic
        fun getNavigationBarConfig(appId: String, path: String): NavigationBarConfig? {
            // Check if this is the initial route of ANY app using cached data
            // Initial route should never show navbar
            val cachedInitialRoute = initialRouteCache[appId]
            if (cachedInitialRoute != null && path == cachedInitialRoute) {
                Log.d(TAG, "Page is initial route ($appId, $path), navbar should be hidden")
                return null
            }

            val key = "$appId|$path"
            return pageConfigCache[key] ?: run {
                val config = NativeApi.getNavigationBarConfig(appId, path)
                if (config != null) pageConfigCache[key] = config
                config
            }
        }

        /**
         * Register activity lifecycle callbacks to automatically handle DeepLinks
         */
        private fun registerActivityLifecycleCallbacks(context: Context) {
            val application = context.applicationContext as? android.app.Application
            application?.registerActivityLifecycleCallbacks(object : android.app.Application.ActivityLifecycleCallbacks {
                override fun onActivityCreated(activity: android.app.Activity, savedInstanceState: Bundle?) {
                    handleAppLink(activity.intent)
                    if (activity is LxAppActivity) {
                        currentActivity = activity
                    }
                }

                override fun onActivityResumed(activity: android.app.Activity) {
                    if (activity is LxAppActivity) {
                        currentActivity = activity
                    }
                }

                override fun onActivityDestroyed(activity: android.app.Activity) {
                    if (activity is LxAppActivity && currentActivity == activity) {
                        currentActivity = null
                    }
                }

                override fun onActivityStarted(activity: Activity) {}
                override fun onActivityPaused(activity: Activity) {}
                override fun onActivityStopped(activity: Activity) {}
                override fun onActivitySaveInstanceState(activity: Activity, outState: Bundle) {}
            }) ?: Log.w(TAG, "Failed to register ActivityLifecycleCallbacks: Application not found")
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
        }

        /**
         * Show toast
         * @param title Toast message
         * @param icon Toast icon type (default: None for simple text toast)
         * @param image Custom image path (absolute path only)
         * @param duration Duration in seconds (default: 1.5, use 0.0 for no auto-hide)
         * @param mask Whether to show mask to prevent touch through
         * @param position Toast position
         */
        @JvmStatic
        fun show(
            title: String,
            icon: ToastIcon = ToastIcon.None,
            image: String? = null,
            duration: Double = 1.5,
            mask: Boolean = false,
            position: ToastPosition = ToastPosition.Center
        ) {
            currentActivity?.let { activity ->
                LxAppToast.showToast(
                    context = activity,
                    title = title,
                    icon = icon,
                    image = image,
                    duration = duration,
                    mask = mask,
                    position = position
                )
            }
        }

        /**
         * Hide toast
         */
        @JvmStatic
        fun hide() {
            LxAppToast.hideToast()
        }
    }

    private fun openInCurrentActivity(appId: String, path: String) {
        try {
            NativeApi.onLxAppOpened(appId, path)

            if (currentActivity != null) {
                Log.d(TAG, "Opening app in current activity")
                currentActivity?.openApp(appId, path)
            } else {
                Log.d(TAG, "Creating new activity")
                val intent = Intent(context, LxAppActivity::class.java).apply {
                    putExtra(LxAppActivity.EXTRA_APP_ID, appId)
                    putExtra(LxAppActivity.EXTRA_PATH, path)
                    addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                }
                context.startActivity(intent)
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to open LxApp: ${e.message}")
        }
    }

    fun getContext(): Context = context
}
