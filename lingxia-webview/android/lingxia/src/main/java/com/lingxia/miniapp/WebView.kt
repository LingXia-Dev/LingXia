package com.lingxia.miniapp

import android.annotation.SuppressLint
import android.content.Context
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.util.AttributeSet
import android.util.Log
import android.view.View
import android.view.ViewGroup
import android.view.ViewTreeObserver
import android.webkit.ConsoleMessage
import android.webkit.WebChromeClient
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import android.webkit.WebResourceResponse
import android.webkit.WebMessage
import android.webkit.WebMessagePort
import android.widget.FrameLayout
import org.json.JSONObject
import java.io.ByteArrayInputStream

private const val TAG = "LingXia.WebView"

data class WebResourceResponseData(
    val mimeType: String,
    val encoding: String,
    val statusCode: Int,
    val reasonPhrase: String,
    val responseHeaders: Map<String, String>,
    val data: ByteArray?
) {
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (javaClass != other?.javaClass) return false

        other as WebResourceResponseData

        if (mimeType != other.mimeType) return false
        if (encoding != other.encoding) return false
        if (statusCode != other.statusCode) return false
        if (reasonPhrase != other.reasonPhrase) return false
        if (responseHeaders != other.responseHeaders) return false
        if (data != null) {
            if (other.data == null) return false
            if (!data.contentEquals(other.data)) return false
        } else if (other.data != null) return false

        return true
    }

    override fun hashCode(): Int {
        var result = mimeType.hashCode()
        result = 31 * result + encoding.hashCode()
        result = 31 * result + statusCode
        result = 31 * result + reasonPhrase.hashCode()
        result = 31 * result + responseHeaders.hashCode()
        result = 31 * result + (data?.contentHashCode() ?: 0)
        return result
    }
}

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

    /**
     * Returns the WebView contained within this container.
     * @return The WebView instance, or null if none is set
     */
    fun getWebView(): com.lingxia.miniapp.WebView? {
        return webView
    }
}

class WebView @JvmOverloads constructor(
    context: Context,
    private val config: WebViewConfig = WebViewConfig()
) : WebView(context) {
    private var appId: String? = null
    internal var currentPath: String? = null
    private var isFirstLoad = true
    private var pageLoaded = false
    private var savedScrollX: Int = 0
    private var savedScrollY: Int = 0
    private var savedScale: Float = 1.0f
    private var savedUrl: String? = null
    private var showEventSent = false  // Track if we've sent a show event in this session
    private var messageChannel: WebMessagePort? = null

    companion object {
        private const val TAG = "WebView"

        @JvmStatic
        external fun nativeGetExistingWebView(appId: String, path: String): com.lingxia.miniapp.WebView?
    }

    data class WebViewConfig(
        val enableJavaScript: Boolean = true,
        val enableDomStorage: Boolean = true
    )

    init {
        initializeWebView()
    }

    private fun initializeWebView() {
        applyWebViewSettings()
        WebView.setWebContentsDebuggingEnabled(false)
        setupWebViewClients()
    }

    @SuppressLint("SetJavaScriptEnabled")
    private fun applyWebViewSettings() {
        settings.apply {
            javaScriptEnabled = config.enableJavaScript
            domStorageEnabled = config.enableDomStorage

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
                val level = when (message.messageLevel()) {
                    ConsoleMessage.MessageLevel.TIP -> 2      // VERBOSE
                    ConsoleMessage.MessageLevel.DEBUG -> 3    // DEBUG
                    ConsoleMessage.MessageLevel.LOG -> 4      // INFO
                    ConsoleMessage.MessageLevel.WARNING -> 5  // WARN
                    ConsoleMessage.MessageLevel.ERROR -> 6    // ERROR
                    else -> 4  // Default to INFO
                }

                nativeOnConsoleMessage(appId ?: return true, level, message.message())
                return true
            }

            override fun onProgressChanged(view: WebView?, newProgress: Int) {
                super.onProgressChanged(view, newProgress)
                Log.d(TAG, "Loading progress: $newProgress%")
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

                // Record that the page has finished loading
                pageLoaded = true

                // Update isFirstLoad flag after the first load completes
                if (isFirstLoad) {
                    isFirstLoad = false
                }

                resetViewport()  // Reset viewport after page load
                setupMessageChannel()  // Setup message channel after page load
                nativeOnPageFinished(appId ?: return, currentPath ?: return)

                // If page is loaded and attached to window, and we haven't sent PageShow yet
                if (isAttachedToWindow && url != null && !showEventSent) {
                    Log.d(TAG, "Page loaded and attached to window, triggering PageShow")
                    nativeOnPageShow(appId ?: return, currentPath ?: return)
                    showEventSent = true
                } else if (showEventSent) {
                    Log.d(TAG, "Skipping PageShow in onPageFinished - already sent in this session")
                }
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
                Log.e(TAG, "Error loading page: ${error?.description}, code: ${error?.errorCode}, failing URL: ${request?.url}")
            }

            override fun shouldInterceptRequest(
                view: WebView?,
                request: WebResourceRequest
            ): WebResourceResponse? {
                val url = request.url.toString()
                val method = request.method
                val headers = request.requestHeaders

                // Log.d(TAG, "Intercepting request: $method $url")
                // Log.d(TAG, "Request headers: $headers")

                // Convert headers to JSON string
                val headersJson = JSONObject().apply {
                    headers.forEach { (key, value) ->
                        put(key, value)
                    }
                }.toString()

                // Call native to handle request
                val response = nativeHandleRequest(
                    appId ?: return null,
                    url,
                    method,
                    headersJson
                )

                if (response == null) {
                    return null
                }

                // Log.d(TAG, "Got response from native layer: ${response.statusCode} ${response.reasonPhrase}")
                // Log.d(TAG, "Response headers: ${response.responseHeaders}")

                return WebResourceResponse(
                    response.mimeType,
                    response.encoding,
                    response.statusCode,
                    response.reasonPhrase,
                    response.responseHeaders,
                    response.data?.let { ByteArrayInputStream(it) }
                )
            }
        }
    }

    private fun setupMessageChannel() {
        // Clean up existing channel if any
        messageChannel?.close()

        // Create new message channel
        val ports = createWebMessageChannel()
        messageChannel = ports[0]

        // Set up native side message handler
        messageChannel?.setWebMessageCallback(object : WebMessagePort.WebMessageCallback() {
            override fun onMessage(port: WebMessagePort, message: WebMessage) {
                nativeHandlePostMessage(appId ?: return, currentPath ?: return, message.data)
            }
        })

        // Transfer port2 to WebView after page is loaded
        post {
            val origin = url?.let { Uri.parse(it) } ?: Uri.EMPTY
            postWebMessage(WebMessage(null, arrayOf(ports[1])), origin)
        }
    }

    /**
     * Posts a message to the WebView's JavaScript context using the WebMessagePort channel.
     * The message will be received by the JavaScript side through the message channel established during WebView initialization.
     * The message should be a valid JSON string that can be parsed by the JavaScript side.
     *
     * @param message The message to be sent to the JavaScript context
     * @see com.lingxia.miniapp.WebView.setupMessageChannel
     */
    fun postMessageToWebView(message: String) {
        messageChannel?.postMessage(WebMessage(message))
    }

    fun handleWebViewCreated(appId: String, path: String) {
        // If appId and path are the same as current values, no need to re-register
        if (appId == this.appId && path == this.currentPath) {
            return
        }

        this.appId = appId
        this.currentPath = path
        nativeOnWebViewCreated(appId, path, this)
        Log.d(TAG, "WebView registered to native layer: appId=$appId, path=$path")
    }

    fun clearBrowsingData() {
        Log.d(TAG, "Clearing browsing data")
        clearHistory()
        clearCache(true)
        clearFormData()
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
            showEventSent = false  // Reset the flag when paused
            onPause()
        }
    }

    fun resume() {
        val callStackTrace = Exception("Resume call stack trace").stackTraceToString()
        Log.d(TAG, "Resuming WebView operations, appId=$appId, path=$currentPath, isFirstLoad=$isFirstLoad, pageLoaded=$pageLoaded, showEventSent=$showEventSent")
        Log.d(TAG, "Resume called from: $callStackTrace")

        onResume()

        // Set to visible
        visibility = View.VISIBLE

        // Only trigger PageShow if we haven't already in this session
        // Only consider triggering PageShow when window is visible and appId/path are valid
        if (isAttachedToWindow && appId != null && currentPath != null && !showEventSent) {
            if (!isFirstLoad && pageLoaded) {
                // Page already loaded, restore scroll position and scale
                post {
                    scrollTo(savedScrollX, savedScrollY)
                    setInitialScale((savedScale * 100).toInt())

                    // Only reload URL if needed
                    // PageShow will be triggered in onPageFinished
                    if (url != savedUrl && savedUrl != null) {
                        Log.d(TAG, "Restoring URL: $savedUrl (current URL: $url)")
                        loadUrl(savedUrl!!)
                    } else {
                        // If we're resuming an already loaded page, trigger PageShow
                        // Avoid duplicate triggers with onPageFinished
                        Log.d(TAG, "Page already loaded, triggering PageShow on resume")
                        nativeOnPageShow(appId!!, currentPath!!)
                        showEventSent = true  // Mark that we've sent the event
                        invalidate()
                    }
                }
            } else if (isFirstLoad) {
                // First load, PageShow will be triggered in onPageFinished
                Log.d(TAG, "First load of WebView, visibility set to VISIBLE")
                // Note: isFirstLoad will be set to false in onPageFinished
            }
        } else if (showEventSent) {
            Log.d(TAG, "Skipping PageShow event - already sent in this session")
        } else {
            Log.d(TAG, "WebView not ready for PageShow: attached=$isAttachedToWindow, appId=$appId, path=$currentPath")
        }

        requestLayout()
        invalidate()
    }

    override fun onAttachedToWindow() {
        super.onAttachedToWindow()
        Log.d(TAG, "WebView attached to window")
        setupMessageChannel()
    }

    override fun onDetachedFromWindow() {
        Log.d(TAG, "WebView detached from window")
        messageChannel?.close()
        messageChannel = null
        pause()
        super.onDetachedFromWindow()
    }

    override fun onWindowVisibilityChanged(visibility: Int) {
        super.onWindowVisibilityChanged(visibility)
        Log.d(TAG, "Window visibility changed: $visibility")

        // Only handle visibility changes to GONE/INVISIBLE
        // VISIBLE state is managed by MiniAppActivity's lifecycle methods
        if (visibility != View.VISIBLE) {
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

    fun getPageConfig(): NavigationBarConfig? {
        return nativeGetPageConfig(appId ?: "", currentPath ?: "")?.let {
            NavigationBarConfig.fromJson(it)
        }
    }

    /**
     * Destroy this WebView instance and release all resources.
     * This method is called from the Rust layer when the WebView instance is being dropped.
     */
    override fun destroy() {
        Log.d(TAG, "Destroying WebView for appId=$appId, path=$currentPath")

        try {
            // First, make the view invisible to prevent any visual artifacts
            visibility = View.GONE

            // Stop all active operations
            stopLoading()

            // Clear all clients to prevent callbacks during destruction
            webViewClient = WebViewClient()
            webChromeClient = WebChromeClient()

            // Clean up message channel
            messageChannel?.close()
            messageChannel = null

            // Clear all data
            try {
                clearHistory()
                clearCache(true)
                clearFormData()
            } catch (e: Exception) {
                Log.w(TAG, "Error clearing WebView data: ${e.message}")
                // Continue with destruction even if clearing data fails
            }

            // Remove from parent view if attached
            try {
                (parent as? ViewGroup)?.removeView(this)
            } catch (e: Exception) {
                Log.w(TAG, "Error removing WebView from parent: ${e.message}")
                // Continue with destruction even if removal fails
            }

            // Finally destroy the WebView
            try {
                super.destroy()
            } catch (e: Exception) {
                Log.e(TAG, "Error destroying WebView: ${e.message}")
                throw e  // Rethrow as this is critical
            }

            Log.d(TAG, "WebView destroyed successfully")
        } catch (e: Exception) {
            Log.e(TAG, "Critical error during WebView destruction", e)
            throw e  // Rethrow to inform Rust layer of failure
        }
    }

    // Native instance methods
    private external fun nativeOnWebViewCreated(appId: String, path: String, webview: WebView): Int
    private external fun nativeHandlePostMessage(appId: String, path: String, message: String): Int
    private external fun nativeOnPageStarted(appId: String, path: String): Int
    private external fun nativeOnPageFinished(appId: String, path: String): Int
    private external fun nativeOnPageShow(appId: String, path: String)
    private external fun nativeShouldOverrideUrlLoading(appId: String, url: String): Int
    private external fun nativeHandleRequest(
        appId: String,
        url: String,
        method: String,
        headers: String
    ): WebResourceResponseData?
    private external fun nativeOnConsoleMessage(appId: String, level: Int, message: String):Int
    private external fun nativeGetPageConfig(appId: String, path: String): String?  // Returns JSON string of page config
}
