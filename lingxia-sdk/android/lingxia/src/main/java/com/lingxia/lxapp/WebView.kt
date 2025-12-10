package com.lingxia.lxapp

import android.content.Context
import android.util.Log
import android.view.MotionEvent
import android.webkit.WebView as AndroidWebView
import com.lingxia.lxapp.SameLevel.SameLevelBridge
import com.lingxia.webview.LingXiaWebView

class WebView(context: Context) : LingXiaWebView(context) {

    companion object {
        private const val TAG = "LingXia.WebView"

        fun findWebView(appId: String, path: String): WebView? {
            Log.d(TAG, "Finding WebView for appId: $appId, path: $path")
            return NativeApi.findWebView(appId, path)
        }

        /**
         * This affects all WebView instances created after this call
         */
        fun enableDebugging() {
            AndroidWebView.setWebContentsDebuggingEnabled(true)
            Log.d(TAG, "WebView debugging enabled globally")
        }
    }

    var pullToRefreshCallback: ((MotionEvent) -> Boolean)? = null

    override fun initializeWebView(appId: String, path: String) {
        super.initializeWebView(appId, path)
        // Register SameLevel JavaScriptInterface right after WebView init, before content loads
        SameLevelBridge.registerJsInterface(this)
        // Disable overscroll glow effect - native components stay fixed at boundaries
        overScrollMode = OVER_SCROLL_NEVER
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        // Let pull-to-refresh handler intercept first
        pullToRefreshCallback?.let { callback ->
            if (callback(event)) {
                return true // Event consumed by pull-to-refresh
            }
        }
        // Otherwise, let WebView handle it normally
        return super.onTouchEvent(event)
    }

    fun pause() {
        Log.d(TAG, "Pausing WebView operations")
        onPause()
    }

    fun resume() {
        Log.d(TAG, "Resuming WebView operations")
        onResume()
    }

    override fun destroy() {
        SameLevelBridge.notifyPageDestroyed(this)
        super.destroy()
    }
}
