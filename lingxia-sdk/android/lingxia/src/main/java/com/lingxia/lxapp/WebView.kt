package com.lingxia.lxapp

import com.lingxia.app.NativeApi

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.MotionEvent
import android.webkit.WebView as AndroidWebView
import androidx.webkit.WebSettingsCompat
import androidx.webkit.WebViewFeature
import com.lingxia.lxapp.NativeComponents.NativeBridge
import com.lingxia.webview.LingXiaWebView
import com.lingxia.webview.LingXiaWebViewHost

internal class WebView(context: Context) : LingXiaWebView(context) {

    companion object {
        private const val TAG = "LingXia.WebView"

        fun findWebView(appId: String, path: String, sessionId: Long): LingXiaWebViewHost? {
            Log.d(TAG, "Finding WebView for appId: $appId, path: $path")
            return NativeApi.findWebView(appId, path, sessionId)
        }

        /**
         * This affects all WebView instances created after this call
         */
        fun enableDebugging() {
            AndroidWebView.setWebContentsDebuggingEnabled(true)
            Log.d(TAG, "WebView debugging enabled globally")
        }
    }

    init {
        // No algorithmic darkening: the runtime owns page theming through the
        // data-theme/colorScheme stamp, and Chromium's inversion fights it —
        // a page explicitly rendering light under a dark-created webview gets
        // force-inverted into a fake dark palette.
        // Pre-first-paint canvas follows the resolved DayNight theme instead
        // of stock white, so dark lxapps don't flash on load — except during a
        // cold start under the launch cover, where the launch background is
        // what the user is already looking at. A home page that redirects on
        // boot builds a second WebView while the cover is still up, and this
        // canvas is what fills the cover's frame until that page paints.
        val launch = SplashOverlay.backgroundColor(context)
        val background = android.util.TypedValue()
        if (launch != null && SplashOverlay.coverOnScreen()) {
            setBackgroundColor(launch)
        } else if (context.theme.resolveAttribute(android.R.attr.colorBackground, background, true)) {
            setBackgroundColor(background.data)
        }
    }

    var pullToRefreshCallback: ((MotionEvent) -> Boolean)? = null

    override fun initializeWebView(appId: String, path: String, sessionId: Long) {
        super.initializeWebView(appId, path, sessionId)
        if (usesStrictSecurityProfile()) {
            // Register before strict lxapp content loads; arbitrary browser pages
            // must never receive the native-component JavaScript interface.
            NativeBridge.registerJsInterface(this)
        }
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

    override fun pause() {
        Log.d(TAG, "Pausing WebView operations")
        NativeBridge.notifyPageInactive(this)
        onPause()
    }

    override fun resume() {
        Log.d(TAG, "Resuming WebView operations")
        onResume()
    }

    override fun destroy() {
        if (Looper.myLooper() == Looper.getMainLooper()) {
            NativeBridge.notifyPageDestroyed(this)
        } else {
            Handler(Looper.getMainLooper()).post {
                NativeBridge.notifyPageDestroyed(this)
            }
        }
        super.destroy()
    }
}
