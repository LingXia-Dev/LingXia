package com.lingxia.miniapp

import android.app.Activity
import android.os.Bundle
import android.util.Log
import android.widget.FrameLayout
import android.view.ViewGroup
import java.lang.ref.WeakReference

class MiniAppActivity : Activity() {
    companion object {
        private const val TAG = "LingXia.WebView"
        const val EXTRA_APP_ID = "appId"
        const val EXTRA_PATH = "path"

        private var lastWebView: WeakReference<com.lingxia.miniapp.WebView>? = null
    }

    private var webView: com.lingxia.miniapp.WebView? = null
    private lateinit var container: FrameLayout
    private var isDestroyed = false
    private var pendingWebViewSetup = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        try {
            // Configure transparent status bar and navigation bar
            MiniApp.configureTransparentSystemBars(
                activity = this,
                lightStatusBars = true,
                lightNavigationBars = false,
                showStatusBars = true,
                showNavigationBars = false
            )

            // Get required parameters, finish if missing
            val appId = intent.getStringExtra(EXTRA_APP_ID)
            val path = intent.getStringExtra(EXTRA_PATH) ?: ""

            if (appId.isNullOrEmpty()) {
                Log.e(TAG, "Missing required parameter: appId")
                finish()
                return
            }

            Log.d(TAG, "Creating WebView for appId: $appId, path: $path")

            // Create and setup container
            container = FrameLayout(this).apply {
                layoutParams = ViewGroup.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.MATCH_PARENT
                )
            }
            setContentView(container)

            // Try to get existing WebView, create new one if not available
            webView = com.lingxia.miniapp.WebView.nativeGetExistingWebView(appId, path)?.also { existingWebView ->
                Log.d(TAG, "Reusing existing WebView for appId: $appId")
                // Remove from previous parent view
                (existingWebView.parent as? ViewGroup)?.removeView(existingWebView)

                // If this is the last used WebView, wait a moment before setting up
                if (lastWebView?.get() == existingWebView) {
                    pendingWebViewSetup = true
                    container.postDelayed({
                        if (!isDestroyed) {
                            setupWebView(existingWebView, path)
                            pendingWebViewSetup = false
                        }
                    }, 100)
                } else {
                    setupWebView(existingWebView, path)
                }
            } ?: com.lingxia.miniapp.WebView(this).apply {
                Log.d(TAG, "Creating new WebView for appId: $appId")
                registerWebViewToNative(appId, path)
                setupWebView(this, null)
            }

            // Update last used WebView
            webView?.let { view ->
                lastWebView = WeakReference(view)
            }
        } catch (e: Exception) {
            Log.e(TAG, "Error in onCreate: ${e.message}")
            e.printStackTrace()
            finish()
        }
    }

    private fun setupWebView(view: com.lingxia.miniapp.WebView, path: String?) {
        if (!isDestroyed) {
            // Reset WebView state
            view.visibility = android.view.View.VISIBLE

            // Set new path
            intent.getStringExtra(EXTRA_APP_ID)?.let { appId ->
                if (!path.isNullOrEmpty()) {
                    view.registerWebViewToNative(appId, path)
                }
            }

            // Add to container
            if (view.parent != container) {
                container.addView(view)
            }

            // Resume WebView
            view.resume()
        }
    }

    override fun onResume() {
        super.onResume()
        if (!pendingWebViewSetup) {
            webView?.visibility = android.view.View.VISIBLE
            container.visibility = android.view.View.VISIBLE
            webView?.resume()
        }
    }

    override fun onPause() {
        super.onPause()
        webView?.pause()
    }

    @Deprecated("Deprecated in Java")
    override fun onBackPressed() {
        webView?.pause()
        finish()
    }

    override fun onDestroy() {
        isDestroyed = true
        webView?.let { view ->
            view.pause()
            container.removeView(view)
            view.visibility = android.view.View.GONE
        }
        webView = null
        super.onDestroy()
    }
}
