package com.lingxia.lxapp


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

internal class WebView(context: Context) : LingXiaWebView(context) {

    companion object {
        private const val TAG = "LingXia.WebView"

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
        // force-inverted into a fake dark palette. Opt out explicitly on both
        // levers: algorithmic darkening (apps targeting API 33+) and the
        // view's force-dark flag, which older WebView providers and OEM
        // force-dark modes follow when the system is dark.
        if (WebViewFeature.isFeatureSupported(WebViewFeature.ALGORITHMIC_DARKENING)) {
            WebSettingsCompat.setAlgorithmicDarkeningAllowed(settings, false)
        }
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.Q) {
            isForceDarkAllowed = false
        }
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
    private var pullClaimedGesture = false

    override fun initializeWebView(appId: String, path: String, sessionId: Long) {
        super.initializeWebView(appId, path, sessionId)
        if (allowsNativeComponentBridge()) {
            // Register before app_page content loads; arbitrary browser or embedded
            // pages must never receive the native-component JavaScript interface.
            NativeBridge.registerJsInterface(this)
            // LxApp pages are native surfaces; overlay bars would look like a
            // webpage. Browser-tab WebViews keep the system bars.
            isVerticalScrollBarEnabled = false
            isHorizontalScrollBarEnabled = false
        }
        // Disable overscroll glow effect - native components stay fixed at boundaries
        overScrollMode = OVER_SCROLL_NEVER
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        if (event.actionMasked == MotionEvent.ACTION_DOWN) pullClaimedGesture = false
        pullToRefreshCallback?.let { callback ->
            if (callback(event)) {
                // Chromium saw this gesture's DOWN; without a CANCEL it waits for an
                // end that never comes and ignores the next pull.
                if (!pullClaimedGesture) {
                    pullClaimedGesture = true
                    val cancel = MotionEvent.obtain(event)
                    cancel.action = MotionEvent.ACTION_CANCEL
                    super.onTouchEvent(cancel)
                    cancel.recycle()
                }
                return true
            }
        }
        if (pullClaimedGesture) return true
        return super.onTouchEvent(event)
    }

    fun pause() {
        Log.d(TAG, "Pausing WebView operations")
        NativeBridge.notifyPageInactive(this)
        onPause()
    }

    fun resume() {
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
