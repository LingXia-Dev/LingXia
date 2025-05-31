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

data class WebViewConfig(
    val enableJavaScript: Boolean = true,
    val enableDomStorage: Boolean = false
)

class WebView @JvmOverloads constructor(
    context: Context,
    private val config: WebViewConfig = WebViewConfig()
) : WebView(context) {
    internal var appId: String? = null
    internal var currentPath: String? = null
    private var isRegistered = false  // Track if WebView has been registered with native layer
    private var isFirstLoad = true
    private var pageLoaded = false
    private var savedScrollX: Int = 0
    private var savedScrollY: Int = 0
    private var savedScale: Float = 1.0f
    private var savedUrl: String? = null
    private var showEventSent = false  // Track if we've sent a show event in this session
    private var messageChannel: WebMessagePort? = null
    private var channelInitialized = false  // Track if the channel has been initialized

    companion object {
        private const val TAG = "WebView"
        private const val ANDROID_PORT_INIT_MESSAGE_DATA = "LingXia-port-init"

        @JvmStatic
        external fun nativeFindWebView(appId: String, path: String): com.lingxia.miniapp.WebView?

        /**
         * Helper function to apply proper layout to a view with screen dimensions
         */
        @JvmStatic
        fun applyScreenLayout(view: View, container: ViewGroup? = null) {
            val context = view.context
            val displayMetrics = context.resources.displayMetrics
            applyLayout(view, displayMetrics.widthPixels, displayMetrics.heightPixels, container)
        }

        /**
         * Helper function to apply layout with custom dimensions
         */
        @JvmStatic
        fun applyLayout(view: View, width: Int, height: Int, container: ViewGroup? = null) {
            val widthSpec = View.MeasureSpec.makeMeasureSpec(width, View.MeasureSpec.EXACTLY)
            val heightSpec = View.MeasureSpec.makeMeasureSpec(height, View.MeasureSpec.EXACTLY)

            container?.let {
                it.measure(widthSpec, heightSpec)
                it.layout(0, 0, width, height)
            }

            view.measure(widthSpec, heightSpec)
            view.layout(0, 0, width, height)

            Log.d(TAG, "Applied layout: ${width}x${height}")
        }

        /**
         * Creates a new WebView instance with the specified parameters.
         * This is the primary API for creating WebView instances from both Kotlin and Rust.
         *
         * @param context The Android context
         * @param appId The mini app ID
         * @param path The page path
         * @param enableJavaScript Whether to enable JavaScript
         * @param enableDomStorage Whether to enable DOM storage
         * @return A configured WebView instance
         */
        @JvmStatic
        @JvmOverloads
        fun createWebView(
            context: Context,
            appId: String,
            path: String,
            enableJavaScript: Boolean = true,
            enableDomStorage: Boolean = false
        ): com.lingxia.miniapp.WebView {
            // Ensure we're on the main thread
            if (android.os.Looper.myLooper() != android.os.Looper.getMainLooper()) {
                // We're not on the main thread, use Handler to post to main thread
                var result: com.lingxia.miniapp.WebView? = null
                var exception: Exception? = null
                val latch = java.util.concurrent.CountDownLatch(1)

                android.os.Handler(android.os.Looper.getMainLooper()).post {
                    try {
                        val config = WebViewConfig(enableJavaScript, enableDomStorage)
                        result = com.lingxia.miniapp.WebView(context, config)

                        // Set appId and path directly
                        result!!.appId = appId
                        result!!.currentPath = path

                        // All WebViews are created as invisible by default
                        // Visibility will be controlled by Rust layer
                        result!!.visibility = android.view.View.GONE

                        Log.d(TAG, "WebView created: appId=$appId, path=$path, visible=false")
                    } catch (e: Exception) {
                        exception = e
                    } finally {
                        latch.countDown()
                    }
                }

                try {
                    latch.await()
                } catch (e: InterruptedException) {
                    throw RuntimeException("Interrupted while waiting for WebView creation", e)
                }

                exception?.let { throw it }
                return result ?: throw RuntimeException("Failed to create WebView")
            }

            val config = WebViewConfig(enableJavaScript, enableDomStorage)
            val webView = com.lingxia.miniapp.WebView(context, config)

            // Set appId and path directly
            webView.appId = appId
            webView.currentPath = path

            // All WebViews are created as invisible by default
            // Visibility will be controlled by Rust layer
            webView.visibility = android.view.View.GONE

            Log.d(TAG, "WebView created: appId=$appId, path=$path, visible=false")
            return webView
        }
    }

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

                nativeOnConsoleMessage(appId ?: return true, currentPath ?: return true, level, message.message())
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

                // Setup message channel and handle page finished in sequence
                postDelayed({
                    setupMessageChannel()

                    // Call nativeOnPageFinished after channel setup
                    postDelayed({
                        handlePageFinishedAfterChannelSetup(url)
                    }, 50)
                }, 50)
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
        messageChannel = null
        channelInitialized = false

        // Create new message channel
        val ports = createWebMessageChannel()
        messageChannel = ports[0]

        // Set up native side message handler
        messageChannel?.setWebMessageCallback(object : WebMessagePort.WebMessageCallback() {
            override fun onMessage(port: WebMessagePort, message: WebMessage) {
                nativeHandlePostMessage(appId ?: return, currentPath ?: return, message.data)
            }
        }, Handler(Looper.getMainLooper()))

        // Transfer port2 to WebView
        post {
            if (isAttachedToWindow && windowToken != null) {
                val origin = url?.let { Uri.parse(it) } ?: Uri.EMPTY
                try {
                    postWebMessage(WebMessage(ANDROID_PORT_INIT_MESSAGE_DATA, arrayOf(ports[1])), origin)
                    channelInitialized = true
                    Log.d(TAG, "WebMessage channel initialized and port transferred to WebView")
                } catch (e: Exception) {
                    Log.e(TAG, "Failed to post WebMessage: ${e.message}", e)
                    channelInitialized = false
                    // Clean up on failure
                    messageChannel?.close()
                    messageChannel = null
                }
            } else {
                Log.w(TAG, "WebView not fully attached (isAttachedToWindow: $isAttachedToWindow, windowToken: $windowToken), skipping postWebMessage for '$ANDROID_PORT_INIT_MESSAGE_DATA'.")
                channelInitialized = false
                // Clean up on failure
                messageChannel?.close()
                messageChannel = null
            }
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
        // Log.d(TAG, "Resume called from: $callStackTrace")

        onResume()

        // Set to visible
        visibility = View.VISIBLE

        // Ensure message channel is working when resuming
        if (isAttachedToWindow) {
            // If channel was lost during pause/resume cycle, re-establish it
            if (!channelInitialized || messageChannel == null) {
                Log.d(TAG, "Message channel lost during pause/resume, re-establishing")
                post {
                    setupMessageChannel()
                }
            }
        }

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
    }

    override fun onAttachedToWindow() {
        super.onAttachedToWindow()
        Log.d(TAG, "WebView attached to window")

        // Register with native layer if not already registered and we have appId/path
        if (!isRegistered && appId != null && currentPath != null) {
            Log.d(TAG, "WebView attached to window: appId=$appId, path=$currentPath")
            val result = nativeOnWebViewAttached(appId!!, currentPath!! )
            if (result == 0) {
                isRegistered = true
                Log.d(TAG, "WebView registered successfully: appId=$appId, path=$currentPath")
            } else {
                Log.e(TAG, "Failed to register WebView: appId=$appId, path=$currentPath")
            }
        }

        // If no working channel, set it up
        if (!channelInitialized || messageChannel == null) {
            Log.d(TAG, "WebView attached but no message channel, setting up")
            post {
                setupMessageChannel()
            }
        }
    }

    override fun onDetachedFromWindow() {
        Log.d(TAG, "WebView detached from window")
        messageChannel?.close()
        messageChannel = null
        channelInitialized = false  // Reset the flag when detached
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
    private external fun nativeOnWebViewAttached(appId: String, path: String): Int
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
    private external fun nativeOnConsoleMessage(appId: String, path:String, level: Int, message: String):Int
    private external fun nativeGetPageConfig(appId: String, path: String): String?  // Returns JSON string of page config

    private fun handlePageFinishedAfterChannelSetup(url: String?) {
        if (channelInitialized) {
            nativeOnPageFinished(appId ?: return, currentPath ?: return)

            // If page is loaded and attached to window, and we haven't sent PageShow yet
            if (isAttachedToWindow && url != null && !showEventSent) {
                Log.d(TAG, "Page loaded and attached to window, triggering PageShow")
                nativeOnPageShow(appId ?: return, currentPath ?: return)
                showEventSent = true
            } else if (showEventSent) {
                Log.d(TAG, "Skipping PageShow in onPageFinished - already sent in this session")
            }
        } else {
            Log.w(TAG, "Message channel not initialized, retrying once")
            // Single retry after a short delay
            postDelayed({
                if (channelInitialized) {
                    nativeOnPageFinished(appId ?: return@postDelayed, currentPath ?: return@postDelayed)

                    if (isAttachedToWindow && url != null && !showEventSent) {
                        Log.d(TAG, "Page loaded and attached to window, triggering PageShow (retry)")
                        nativeOnPageShow(appId ?: return@postDelayed, currentPath ?: return@postDelayed)
                        showEventSent = true
                    }
                } else {
                    Log.e(TAG, "Message channel still not initialized after retry")
                }
            }, 100)
        }
    }
}
