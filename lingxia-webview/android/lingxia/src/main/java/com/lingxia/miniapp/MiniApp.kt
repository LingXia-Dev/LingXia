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
        }

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

        @JvmStatic
        fun openMiniAppInNewActivity(appId: String, path: String) {
            val instance = getInstance()
            instance.openInNewActivity(appId, path)
        }
    }

    private fun openInNewActivity(appId: String, path: String) {
        Log.d(TAG, "Opening MiniApp in new activity: $appId, path: $path")
        val intent = Intent(context, MiniAppActivity::class.java).apply {
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            putExtra(MiniAppActivity.EXTRA_APP_ID, appId)
            putExtra(MiniAppActivity.EXTRA_PATH, path)
        }
        context.startActivity(intent)
    }

    private fun createMiniAppWebView(appId: String, path: String): com.lingxia.miniapp.WebView {
        Log.d(TAG, "Creating WebView for appId: $appId, path: $path")
        val webView = com.lingxia.miniapp.WebView(context).apply {
            layoutParams = ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            registerWebViewToNative(appId, path)
        }
        return webView
    }

    private external fun nativeOnMiniAppDestroy()

    fun getContext(): Context = context
}
