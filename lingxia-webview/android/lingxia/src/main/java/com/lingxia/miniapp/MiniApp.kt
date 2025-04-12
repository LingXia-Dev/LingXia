package com.lingxia.miniapp

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.graphics.Color
import android.util.Log
import android.view.ViewGroup
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat

class MiniApp private constructor(private val context: Context) {
    companion object {
        private const val TAG = "LingXia.WebView"
        private var instance: MiniApp? = null

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

        @JvmStatic
        fun getInstance(): MiniApp {
            return instance ?: throw IllegalStateException("MiniApp not initialized")
        }

        @JvmStatic
        fun destroy() {
            getInstance().nativeOnMiniAppDestroy()
            instance = null
        }

        @JvmStatic
        fun configureTransparentSystemBars(
            activity: Activity,
            lightStatusBars: Boolean = true,
            lightNavigationBars: Boolean = false,
            showStatusBars: Boolean = true,
            showNavigationBars: Boolean = false
        ) {
            // Configure system windows
            WindowCompat.setDecorFitsSystemWindows(activity.window, false)
            activity.window.statusBarColor = Color.TRANSPARENT
            activity.window.navigationBarColor = Color.TRANSPARENT

            // Configure WindowInsetsControllerCompat
            WindowInsetsControllerCompat(activity.window, activity.window.decorView).apply {
                isAppearanceLightStatusBars = lightStatusBars
                isAppearanceLightNavigationBars = lightNavigationBars
                if (showStatusBars) {
                    show(WindowInsetsCompat.Type.statusBars())
                } else {
                    hide(WindowInsetsCompat.Type.statusBars())
                }
                if (showNavigationBars) {
                    show(WindowInsetsCompat.Type.navigationBars())
                } else {
                    hide(WindowInsetsCompat.Type.navigationBars())
                }
            }
        }

        @JvmStatic
        fun attachMiniApp(appId: String, path: String): com.lingxia.miniapp.WebView {
            val instance = getInstance()
            return instance.createMiniAppWebView(appId, path)
        }

        /**
         * Opens a mini app in a new activity
         *
         * This method creates a new MiniAppActivity to host the specified mini app.
         * It notifies the native layer about the mini app being opened for state tracking.
         *
         * @param appId The unique identifier of the mini app to open
         * @param path The initial path to navigate to within the mini app
         * @param tabBarConfig Optional JSON configuration for the TabBar (if null, no TabBar will be shown)
         */
        @JvmStatic
        fun openMiniApp(appId: String, path: String, tabBarConfig: String? = null) {
            val instance = getInstance()
            instance.openInNewActivity(appId, path, tabBarConfig)
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
                        val intent = Intent("com.lingxia.CLOSE_MINIAPP_ACTION")
                        intent.putExtra("appId", appId)
                        instance?.context?.sendBroadcast(intent)
                    }
                }
            }
        }
    }

    private fun openInNewActivity(appId: String, path: String, tabBarConfig: String? = null) {
        val intent = Intent(context, MiniAppActivity::class.java).apply {
            putExtra(MiniAppActivity.EXTRA_APP_ID, appId)
            putExtra(MiniAppActivity.EXTRA_PATH, path)
            tabBarConfig?.let {
                putExtra(MiniAppActivity.EXTRA_TAB_BAR_CONFIG, it)
            }
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

    private fun createMiniAppWebView(appId: String, path: String): com.lingxia.miniapp.WebView {
        Log.d(TAG, "Creating WebView for appId: $appId, path: $path")
        val webView = com.lingxia.miniapp.WebView(context).apply {
            layoutParams = ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            handleWebViewCreated(appId, path)
        }
        return webView
    }

    private external fun nativeOnMiniAppDestroy()

    fun getContext(): Context = context
}
