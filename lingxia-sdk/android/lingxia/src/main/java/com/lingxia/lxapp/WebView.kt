package com.lingxia.lxapp

import android.content.Context
import android.util.Log
import android.webkit.WebView as AndroidWebView
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

    fun pause() {
        Log.d(TAG, "Pausing WebView operations")
        onPause()
    }

    fun resume() {
        Log.d(TAG, "Resuming WebView operations")
        onResume()
    }
}
