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
        private const val TAG = "LingXia.WebView"
        private var instance: MiniApp? = null

        /**
         * The ID of the home/main app
         */
        const val HOME_APP_ID = "home"

        init {
            System.loadLibrary("lingxia")
        }

        @JvmStatic
        fun initialize(context: Context) {
            if (instance == null) {
                instance = MiniApp(context.applicationContext)
            }
            val appContext = context.applicationContext
            nativeOnMiniAppInited(
                appContext.filesDir.absolutePath,
                appContext.cacheDir.absolutePath,
                appContext.assets
            )
        }

        @JvmStatic
        private external fun nativeOnMiniAppInited(
            dataDir: String,
            cacheDir: String,
            assetManager: android.content.res.AssetManager
        ): Int

        @JvmStatic
        private external fun nativeOnMiniAppOpened(appId: String): Int

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
    }

    private fun openInNewActivity(appId: String, path: String) {
        val intent = Intent(context, MiniAppActivity::class.java).apply {
            putExtra(MiniAppActivity.EXTRA_APP_ID, appId)
            putExtra(MiniAppActivity.EXTRA_PATH, path)
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        }

        try {
            context.startActivity(intent)
            // Notify native in background thread to avoid UI blocking
            Thread { nativeOnMiniAppOpened(appId) }.start()
        } catch (e: Exception) {
            Log.e(TAG, "Failed to start MiniAppActivity: ${e.message}")
        }
    }

    fun getContext(): Context = context
}
