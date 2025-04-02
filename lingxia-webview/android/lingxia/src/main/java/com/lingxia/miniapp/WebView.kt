package com.lingxia.miniapp

import android.annotation.SuppressLint
import android.content.Context
import android.os.Handler
import android.os.Looper
import android.util.AttributeSet
import android.util.Log
import android.view.View
import android.view.ViewGroup
import android.view.ViewTreeObserver
import android.webkit.ConsoleMessage
import android.webkit.JavascriptInterface
import android.webkit.WebChromeClient
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.FrameLayout

private const val TAG = "LingXia.WebView"
private const val BRIDGE_NAME = "lingxia"

class MiniWebViewContainer @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
    defStyleAttr: Int = 0
) : FrameLayout(context, attrs, defStyleAttr), ViewTreeObserver.OnGlobalLayoutListener {
    private var webView: com.lingxia.miniapp.WebView? = null
    private val mainHandler = Handler(Looper.getMainLooper())

    init {
        // Set container layout parameters
        val params = LayoutParams(
            LayoutParams.MATCH_PARENT,
            LayoutParams.MATCH_PARENT
        )
        params.gravity = android.view.Gravity.CENTER
        layoutParams = params

        // Set visibility
        visibility = View.VISIBLE

        // Add view tree observer
        viewTreeObserver.addOnGlobalLayoutListener(this)
    }

    override fun onAttachedToWindow() {
        super.onAttachedToWindow()
        Log.d(TAG, "Container attached to window")
        viewTreeObserver.addOnGlobalLayoutListener(this)
        setWebViewVisible()
    }

    override fun onDetachedFromWindow() {
        super.onDetachedFromWindow()
        viewTreeObserver.removeOnGlobalLayoutListener(this)
    }

    override fun onLayout(changed: Boolean, left: Int, top: Int, right: Int, bottom: Int) {
        super.onLayout(changed, left, top, right, bottom)
        val width = right - left
        val height = bottom - top
        Log.d(TAG, "Container layout: ${width}x${height}")

        webView?.let { view ->
            Log.d(TAG, "WebView layout: ${view.width}x${view.height}")
            Log.d(TAG, "Container layout changed: ${width}x${height}")
            Log.d(TAG, "Found WebView child, updating layout")
            view.layout(0, 0, width, height)
            Log.d(TAG, "WebView dimensions after layout: ${view.width}x${view.height}")
            Log.d(TAG, "WebView visibility: ${view.visibility}")
            Log.d(TAG, "WebView parent: ${view.parent != null}")
            ensureWebViewVisible()
        }
    }

    override fun onGlobalLayout() {
        Log.d(TAG, "Global layout pass")
        Log.d(TAG, "Container dimensions in global layout: ${width}x${height}")
        webView?.let { view ->
            Log.d(TAG, "WebView dimensions in global layout: ${view.width}x${view.height}")
            Log.d(TAG, "WebView visibility in global layout: ${view.visibility}")
            Log.d(TAG, "WebView parent in global layout: ${view.parent != null}")
            ensureWebViewVisible()
        }
    }

    override fun addView(child: View, index: Int, params: ViewGroup.LayoutParams) {
        super.addView(child, index, params)
        Log.d(TAG, "Added view to container: $child")

        if (child is WebView) {
            // Ensure WebView is visible
            child.visibility = View.VISIBLE

            // Set WebView layout parameters
            child.layoutParams = LayoutParams(
                LayoutParams.MATCH_PARENT,
                LayoutParams.MATCH_PARENT
            ).apply {
                gravity = android.view.Gravity.CENTER
            }

            // Force layout update
            post {
                val width = width
                val height = height
                if (width > 0 && height > 0) {
                    child.measure(
                        MeasureSpec.makeMeasureSpec(width, MeasureSpec.EXACTLY),
                        MeasureSpec.makeMeasureSpec(height, MeasureSpec.EXACTLY)
                    )
                    child.layout(0, 0, width, height)
                }
                child.visibility = View.VISIBLE
                child.requestLayout()
                child.invalidate()
            }
        }
    }

    fun setWebView(webView: com.lingxia.miniapp.WebView?) {
        this.webView = webView
        removeAllViews()
        webView?.let {
            addView(it, LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            ))
            setWebViewVisible()
        }
    }

    fun setWebViewVisible() {
        Log.d(TAG, "Setting WebView visibility")
        visibility = View.VISIBLE
        webView?.visibility = View.VISIBLE
        requestLayout()
        invalidate()
    }

    private fun ensureWebViewVisible() {
        if (webView?.visibility != View.VISIBLE) {
            setWebViewVisible()
        }
    }
}

class WebView @JvmOverloads constructor(
    context: Context,
    private val config: WebViewConfig = WebViewConfig()
) : WebView(context) {
    private var appId: String? = null
    private var currentPath: String? = null
    private var isFirstLoad = true
    private var pageLoaded = false
    private var savedScrollX: Int = 0
    private var savedScrollY: Int = 0
    private var savedScale: Float = 1.0f
    private var savedUrl: String? = null

    companion object {
        private const val TAG = "WebView"

        @JvmStatic
        external fun nativeGetExistingWebView(appId: String, path: String): com.lingxia.miniapp.WebView?
    }

    data class WebViewConfig(
        val enableDevTools: Boolean = true,
        val enableJavaScript: Boolean = true,
        val enableDomStorage: Boolean = true,
        val allowMixedContent: Boolean = true
    )

    init {
        initializeWebView()
    }

    private fun initializeWebView() {
        applyWebViewSettings()
        setDevToolsEnabled(config.enableDevTools)
        setupWebViewClients()
        setupJavaScriptBridge()
    }

    @SuppressLint("SetJavaScriptEnabled")
    private fun applyWebViewSettings() {
        settings.apply {
            javaScriptEnabled = config.enableJavaScript
            domStorageEnabled = config.enableDomStorage
            mixedContentMode = if (config.allowMixedContent) {
                WebSettings.MIXED_CONTENT_ALWAYS_ALLOW
            } else {
                WebSettings.MIXED_CONTENT_NEVER_ALLOW
            }
            useWideViewPort = true
            loadWithOverviewMode = true
            setSupportZoom(true)
            builtInZoomControls = true
            displayZoomControls = false
            cacheMode = WebSettings.LOAD_NO_CACHE
            allowFileAccess = false
            allowContentAccess = false
        }
    }

    private fun setupWebViewClients() {
        // Set WebChromeClient
        webChromeClient = object : WebChromeClient() {
            override fun onConsoleMessage(message: ConsoleMessage): Boolean {
                Log.d(TAG, "${message.message()} -- From line ${message.lineNumber()} of ${message.sourceId()}")
                return true
            }
        }

        // Set WebViewClient
        webViewClient = object : WebViewClient() {
            override fun onPageStarted(view: WebView?, url: String?, favicon: android.graphics.Bitmap?) {
                super.onPageStarted(view, url, favicon)
                Log.d(TAG, "Page started loading: $url")
                pageLoaded = false
                nativeOnPageStarted(appId ?: return, currentPath ?: return)
            }

            override fun onPageFinished(view: WebView?, url: String?) {
                super.onPageFinished(view, url)
                Log.d(TAG, "Page finished loading: $url")
                pageLoaded = true
                resetViewport()  // Reset viewport after page load
                nativeOnPageFinished(appId ?: return, currentPath ?: return)
            }

            override fun shouldOverrideUrlLoading(view: WebView?, request: WebResourceRequest?): Boolean {
                request?.url?.let { url ->
                    Log.d(TAG, "Should override URL loading: $url")
                    return nativeShouldOverrideUrlLoading(appId ?: return false, url.toString()) == 1
                }
                return false
            }

            override fun onReceivedError(view: WebView?, request: WebResourceRequest?, error: WebResourceError?) {
                super.onReceivedError(view, request, error)
                Log.e(TAG, "Error loading page: ${error?.description}")
            }
        }
    }

    private fun setupJavaScriptBridge() {
        addJavascriptInterface(object {
            @JavascriptInterface
            fun postMessage(message: String) {
                Log.d(TAG, "Message from WebView: $message")
                nativeHandlePostMessage(appId ?: return, currentPath ?: return, message)
            }
        }, "MiniApp")
    }

    fun registerWebViewToNative(appId: String, path: String) {
        this.appId = appId
        this.currentPath = path
        nativeOnWebViewRegistered(appId, path, this)
        Log.d(TAG, "WebView registered to native layer: appId=$appId, path=$path")
    }

    fun clearBrowsingData() {
        Log.d(TAG, "Clearing browsing data")
        clearHistory()
        clearCache(true)
        clearFormData()
    }

    fun setDevToolsEnabled(enabled: Boolean) {
        WebView.setWebContentsDebuggingEnabled(enabled)
    }

    fun resetViewport() {
        settings.apply {
            useWideViewPort = true
            loadWithOverviewMode = true
            setSupportZoom(true)
            builtInZoomControls = true
            displayZoomControls = false
        }
    }

    fun pause() {
        Log.d(TAG, "Pausing WebView operations")
        if (pageLoaded) {
            savedScrollX = scrollX
            savedScrollY = scrollY
            savedScale = scaleX
            savedUrl = url
            onPause()
        }
    }

    fun resume() {
        Log.d(TAG, "Resuming WebView operations")
        onResume()
        if (!isFirstLoad && pageLoaded) {
            post {
                visibility = View.VISIBLE
                scrollTo(savedScrollX, savedScrollY)
                setInitialScale((savedScale * 100).toInt())
                if (url != savedUrl && savedUrl != null) {
                    Log.d(TAG, "Restoring URL: $savedUrl")
                    loadUrl(savedUrl!!)
                } else {
                    invalidate()
                }
            }
        } else if (isFirstLoad) {
            isFirstLoad = false
            visibility = View.VISIBLE
        }
        requestLayout()
        invalidate()
    }

    override fun onAttachedToWindow() {
        super.onAttachedToWindow()
        Log.d(TAG, "WebView attached to window")
        resume()
    }

    override fun onDetachedFromWindow() {
        Log.d(TAG, "WebView detached from window")
        pause()
        super.onDetachedFromWindow()
    }

    override fun onWindowVisibilityChanged(visibility: Int) {
        super.onWindowVisibilityChanged(visibility)
        Log.d(TAG, "Window visibility changed: $visibility")
        if (visibility == View.VISIBLE) {
            post {
                resume()
            }
        } else {
            pause()
        }
    }

    fun setUserAgent(userAgent: String) {
        settings.userAgentString = userAgent
    }

    override fun loadUrl(url: String) {
        Log.d(TAG, "Loading URL: $url")
        savedUrl = url
        resetViewport()
        visibility = View.VISIBLE
        super.loadUrl(url)
    }

    // Native instance methods
    private external fun nativeOnWebViewRegistered(appId: String, path: String, webview: WebView): Int
    private external fun nativeHandlePostMessage(appId: String, path: String, message: String): Int
    private external fun nativeOnPageStarted(appId: String, path: String): Int
    private external fun nativeOnPageFinished(appId: String, path: String): Int
    private external fun nativeShouldOverrideUrlLoading(appId: String, url: String): Int
    private external fun nativeDestroyAllWebViews(): Int
    external fun nativeOnMiniAppHidden(appId: String, path: String): Int
}
