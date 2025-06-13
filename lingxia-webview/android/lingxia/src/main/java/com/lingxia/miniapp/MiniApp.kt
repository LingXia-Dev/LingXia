package com.lingxia.miniapp

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.util.Log

// Import the top-level action string constants
import com.lingxia.miniapp.ACTION_SWITCH_PAGE
import com.lingxia.miniapp.ACTION_CLOSE_MINIAPP

class MiniApp private constructor(private val context: Context) {
    companion object {
        private const val TAG = "LingXia.MiniApp"
        private var instance: MiniApp? = null

        // Properties to store home app details from native
        var HomeMiniAppId: String? = null
        var HomeMiniAppInitialRoute: String? = null

        private val pageConfigCache = mutableMapOf<String, NavigationBarConfig>()

        // Clear cache when app is closed to prevent memory leaks
        fun clearPageConfigCache() {
            pageConfigCache.clear()
        }

        init {
            System.loadLibrary("lingxia")
        }

        @JvmStatic
        fun initialize(context: Context) {
            if (instance != null && HomeMiniAppId != null && HomeMiniAppInitialRoute != null) {
                Log.d(TAG, "MiniApp already successfully initialized, skipping")
                return
            }

            if (instance == null) {
                instance = MiniApp(context.applicationContext)
            }
            val appContext = context.applicationContext

            val initResultString = nativeOnMiniAppInited(
                appContext.filesDir.absolutePath,
                appContext.cacheDir.absolutePath,
                appContext.assets
            )

            if (initResultString != null) {
                // Use a robust way to split, ensuring the delimiter is not misinterpreted if path contains it.
                // For a simple case like "appId:path/to/page", limit = 2 is good.
                val parts = initResultString.split(":", limit = 2)
                if (parts.size == 2) {
                    HomeMiniAppId = parts[0]
                    HomeMiniAppInitialRoute = parts[1]
                    Log.i(TAG, "Native init success. Home App ID: $HomeMiniAppId, Initial Route: $HomeMiniAppInitialRoute")
                } else {
                    Log.e(TAG, "Failed to parse home MiniAapp details from native (expected 2 parts): '$initResultString'")
                }
            } else {
                Log.e(TAG, "Failed to get home MiniApp details from native init.")
            }
        }

        @JvmStatic
        private external fun nativeOnMiniAppInited(
            dataDir: String,
            cacheDir: String,
            assetManager: android.content.res.AssetManager
        ): String?

        @JvmStatic
        private external fun nativeOnMiniAppOpened(appId: String, path: String): Int

        /**
         * Get the TabBar configuration for a mini app from the native layer
         *
         * @param appId The ID of the mini app to get TabBar configuration for
         * @return The TabBar configuration as a JSON string, or null if not available
         */
        @JvmStatic
        external fun nativeGetTabBarConfig(appId: String): String?

        @JvmStatic
        fun getInstance(): MiniApp {
            return instance ?: throw IllegalStateException("MiniApp not initialized")
        }

        /**
         * Opens a mini app in a new activity
         *
         * This method creates a new MiniAppActivity to host the specified mini app.
         * It notifies the native layer about the mini app being opened for state tracking.
         * The app configuration (including TabBar) will be fetched from the native layer.
         *
         * @param appId The unique identifier of the mini app to open
         * @param path The initial path to navigate to within the mini app
         */
        @JvmStatic
        fun openMiniApp(appId: String, path: String) {
            val instance = getInstance()
            instance.openInNewActivity(appId, path)
        }

        /**
         * Opens the home MiniApp
         * Its appId and initial path are provided by the native layer during initialization.
         *
         * If these details are not available, an error will be logged, and no app will be opened.
         */
        @JvmStatic
        fun openHomeMiniApp() {
            if (HomeMiniAppId != null && HomeMiniAppInitialRoute != null) {
                openMiniApp(HomeMiniAppId!!, HomeMiniAppInitialRoute!!)
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
        fun closeMiniApp(appId: String) {
            Log.d(TAG, "Closing MiniApp with appId: $appId")

            // Iterate through all activities, find and close the MiniAppActivity with matching appId
            // On actual devices, one mini app corresponds to one activity, so this implementation is sufficient
            // Future expansion can be made here if multiple activities are supported
            val activityManager = instance?.context?.getSystemService(Context.ACTIVITY_SERVICE) as? android.app.ActivityManager
            activityManager?.appTasks?.forEach { task ->
                task.taskInfo?.topActivity?.let { componentName ->
                    if (componentName.className == MiniAppActivity::class.java.name) {
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
         * Switches the current page within a running MiniAppActivity
         *
         * This method sends a broadcast intent to the specific MiniAppActivity instance
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
        fun getPageConfig(appId: String, path: String): NavigationBarConfig? {
            val key = "$appId|$path"
            return pageConfigCache[key] ?: run {
                val configJson = nativeGetPageConfig(appId, path)
                val config = configJson?.let { NavigationBarConfig.fromJson(it) }
                if (config != null) pageConfigCache[key] = config
                config
            }
        }

        @JvmStatic
        external fun nativeGetPageConfig(appId: String, path: String): String?
    }

    private fun openInNewActivity(appId: String, path: String) {
        val intent = Intent(context, MiniAppActivity::class.java).apply {
            putExtra(MiniAppActivity.EXTRA_APP_ID, appId)
            putExtra(MiniAppActivity.EXTRA_PATH, path)
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            // Add flags for faster activity launch
            addFlags(Intent.FLAG_ACTIVITY_NO_ANIMATION)
        }

        try {
            // Notify native layer BEFORE starting activity
            // This allows Rust layer to prepare resources while activity is starting
            val executor = java.util.concurrent.Executors.newSingleThreadExecutor()
            executor.submit {
                nativeOnMiniAppOpened(appId, path)
            }
            executor.shutdown()
            context.startActivity(intent)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to start MiniAppActivity: ${e.message}")
        }
    }

    fun getContext(): Context = context
}
