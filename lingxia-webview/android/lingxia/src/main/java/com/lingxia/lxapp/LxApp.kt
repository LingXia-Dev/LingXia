package com.lingxia.miniapp

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.util.Log

// Import the top-level action string constants
import com.lingxia.miniapp.ACTION_SWITCH_PAGE
import com.lingxia.miniapp.ACTION_CLOSE_MINIAPP

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
                val appInfo = nativeGetLxAppInfo(appId)
                if (appInfo != null) {
                    initialRouteCache[appId] = appInfo.initialRoute
                    Log.d(TAG, "Cached initial route for $appId: ${appInfo.initialRoute}")
                }
            }
        }

        init {
            System.loadLibrary("lingxia")
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

            val initResultString = nativeOnLxAppInited(
                appContext.filesDir.absolutePath,
                appContext.cacheDir.absolutePath,
                appContext.assets
            )

            if (initResultString != null) {
                HomeLxAppId = initResultString
                // Get initial route and other app info using new API
                val appInfo = nativeGetLxAppInfo(initResultString)
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
        }

        @JvmStatic
        private external fun nativeOnLxAppInited(
            dataDir: String,
            cacheDir: String,
            assetManager: android.content.res.AssetManager
        ): String?

        @JvmStatic
        private external fun nativeOnLxAppOpened(appId: String, path: String): Int

        /**
         * Get LxApp information using new typed API
         */
        @JvmStatic
        external fun nativeGetLxAppInfo(appId: String): LxAppInfo?

        /**
         * Get the TabBar configuration for a mini app using new typed API
         */
        @JvmStatic
        external fun nativeGetTabBarConfig(appId: String): TabBarConfig?

        /**
         * Get a specific TabBar item by index using new typed API
         */
        @JvmStatic
        external fun nativeGetTabBarItem(appId: String, index: Int): TabBarItem?

        @JvmStatic
        fun getInstance(): LxApp {
            return instance ?: throw IllegalStateException("LxApp not initialized")
        }

        /**
         * Opens a mini app in a new activity
         *
         * This method creates a new LxAppActivity to host the specified mini app.
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
            instance.openInNewActivity(appId, path)
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

            // Iterate through all activities, find and close the LxAppActivity with matching appId
            // On actual devices, one mini app corresponds to one activity, so this implementation is sufficient
            // Future expansion can be made here if multiple activities are supported
            val activityManager = instance?.context?.getSystemService(Context.ACTIVITY_SERVICE) as? android.app.ActivityManager
            activityManager?.appTasks?.forEach { task ->
                task.taskInfo?.topActivity?.let { componentName ->
                    if (componentName.className == LxAppActivity::class.java.name) {
                        // Send broadcast to notify activity to close
                        val intent = Intent(ACTION_CLOSE_MINIAPP)
                        intent.putExtra("appId", appId)
                        intent.setPackage(instance?.context?.packageName)
                        instance?.context?.sendBroadcast(intent)
                    }
                }
            }
        }

        /**
         * Switches the current page within a running LxAppActivity
         *
         * This method sends a broadcast intent to the specific LxAppActivity instance
         * identified by appId, instructing it to navigate to the targetPath.
         * Unlike switching tabs, this navigation typically implies showing the back button.
         *
         * @param appId The unique identifier of the mini app whose page needs switching
         * @param path The target path to navigate to within the mini app
         */
        @JvmStatic
        fun switchPage(appId: String, path: String) {
            Log.d(TAG, "Requesting page switch for appId: $appId to path: $path")
            val instance = getInstance()
            val intent = Intent(ACTION_SWITCH_PAGE).apply {
                // Ensure the intent is targeted only to our app's components
                `package` = instance.context.packageName
                putExtra("appId", appId)
                putExtra("path", path)
            }
            instance.context.sendBroadcast(intent)
        }

        /**
         * Creates a WebView for the specified appId and path.
         * This method is called from the Rust layer to create WebViews.
         * The WebView crate handles thread switching, so no need for Looper checks here.
         *
         * @param appId The miniapp ID
         * @param path The page path
         */
        @JvmStatic
        fun createWebView(appId: String, path: String): com.lingxia.miniapp.WebView? {

            return try {
                val context = getInstance().context

                val webView = com.lingxia.miniapp.WebView.createWebView(
                    context = context,
                    appId = appId,
                    path = path
                )
                Log.d(TAG, "Successfully created WebView for appId=$appId, path=$path")
                webView
            } catch (e: Exception) {
                Log.e(TAG, "Failed to create WebView for appId=$appId, path=$path: ${e.message}", e)
                null
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
                val config = nativeGetNavigationBarConfig(appId, path)
                if (config != null) pageConfigCache[key] = config
                config
            }
        }

        /**
         * Get the navigation bar configuration for a specific page/path
         */
        @JvmStatic
        external fun nativeGetNavigationBarConfig(appId: String, path: String): NavigationBarConfig?
    }

    private fun openInNewActivity(appId: String, path: String) {
        val intent = Intent(context, LxAppActivity::class.java).apply {
            putExtra(LxAppActivity.EXTRA_APP_ID, appId)
            putExtra(LxAppActivity.EXTRA_PATH, path)
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            // Add flags for faster activity launch
            addFlags(Intent.FLAG_ACTIVITY_NO_ANIMATION)
        }

        try {
            // Notify native layer BEFORE starting activity
            // This allows Rust layer to prepare resources while activity is starting
            val executor = java.util.concurrent.Executors.newSingleThreadExecutor()
            executor.submit {
                nativeOnLxAppOpened(appId, path)
            }
            executor.shutdown()
            context.startActivity(intent)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to start LxAppActivity: ${e.message}")
        }
    }

    fun getContext(): Context = context
}
