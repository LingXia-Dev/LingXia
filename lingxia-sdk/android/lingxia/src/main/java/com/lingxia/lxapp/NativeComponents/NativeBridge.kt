package com.lingxia.lxapp.NativeComponents

import android.graphics.Color
import android.os.Handler
import android.os.Looper
import android.view.View
import android.view.ViewGroup
import android.view.ViewTreeObserver
import android.webkit.JavascriptInterface
import android.widget.FrameLayout
import com.lingxia.lxapp.NativeComponents.Components.MediaSwiperComponentFactory
import com.lingxia.lxapp.NativeComponents.Components.VideoComponentFactory
import com.lingxia.lxapp.NativeComponents.Components.PickerComponentFactory
import com.lingxia.webview.LingXiaServoView
import com.lingxia.webview.LingXiaWebView
import com.lingxia.webview.LingXiaWebViewHost
import org.json.JSONArray
import org.json.JSONObject
import java.lang.ref.WeakReference

/**
 * Bridge between JS component.* messages and native components.
 * Uses JavaScriptInterface for View→Native component lifecycle/control.
 */
internal class NativeBridge private constructor(
    webView: LingXiaWebViewHost
) {
    private val webViewRef = WeakReference(webView)
    private var overlayHost: ComponentOverlayHost? = null
    private var componentManager: NativeComponentManager? = null
    private var pageKey: String
    private val mainHandler = Handler(Looper.getMainLooper())

    // Pre-draw sync for frame-perfect scroll tracking
    private var preDrawListener: ViewTreeObserver.OnPreDrawListener? = null
    private var lastSyncedScrollX = Int.MIN_VALUE
    private var lastSyncedScrollY = Int.MIN_VALUE

    init {
        pageKey = makePageKey(webView)
    }

    private fun install() {
        val webView = webViewRef.get() ?: return
        val host = makeOrFindOverlayHost(webView)
        overlayHost = host

        val manager = NativeComponentManager(
            hostView = host,
            defaultPageId = pageKey,
            eventSink = { sendEventToView(it) },
            webView = webView
        )
        registeredFactories.forEach { (type, factory) -> manager.register(type, factory) }
        componentManager = manager

        // Use OnPreDrawListener for frame-perfect scroll sync
        // This ensures native components update BEFORE the frame is drawn,
        // eliminating the 1-frame lag from setOnScrollChangeListener
        preDrawListener = ViewTreeObserver.OnPreDrawListener {
            val wv = webViewRef.get()
            if (wv != null) {
                val scrollX = wv.hostView.scrollX
                val scrollY = wv.hostView.scrollY
                // Only update if scroll position changed to avoid redundant work
                if (scrollX != lastSyncedScrollX || scrollY != lastSyncedScrollY) {
                    lastSyncedScrollX = scrollX
                    lastSyncedScrollY = scrollY
                    manager.onWebViewScroll(scrollX, scrollY)
                }
            }
            true // Proceed with drawing
        }
        webView.hostView.viewTreeObserver.addOnPreDrawListener(preDrawListener)
    }

    private class JsInterface(webView: LingXiaWebView) {
        private val webViewRef = WeakReference(webView)

        @JavascriptInterface
        fun postMessage(messageJson: String) {
            Handler(Looper.getMainLooper()).post {
                webViewRef.get()?.takeIf { it.usesStrictSecurityProfile() }?.let { webView ->
                    val webViewId = System.identityHashCode(webView)
                    bridgeMap[webViewId]?.handleMessage(messageJson)
                }
            }
        }
    }

    private fun makeOrFindOverlayHost(webView: LingXiaWebViewHost): ComponentOverlayHost {
        val hostView = webView.hostView
        val parent = hostView.parent as? ViewGroup

        overlayHost?.let { existing ->
            if (existing.parent != parent && parent != null) {
                (existing.parent as? ViewGroup)?.removeView(existing)
                addHostToParent(parent, webView, existing)
            }
            return existing
        }

        parent?.let { p ->
            for (i in 0 until p.childCount) {
                (p.getChildAt(i) as? ComponentOverlayHost)?.takeIf { it.tag == OVERLAY_TAG }?.let { return it }
            }
        }

        val host = ComponentOverlayHost(hostView.context).apply {
            tag = OVERLAY_TAG
            setBackgroundColor(Color.TRANSPARENT)
            isClickable = false
            isFocusable = false
        }
        parent?.let { addHostToParent(it, webView, host) }
        return host
    }

    private fun addHostToParent(parent: ViewGroup, webView: LingXiaWebViewHost, host: ComponentOverlayHost) {
        val hostView = webView.hostView
        // Match WebView's exact position and size in parent
        val params = FrameLayout.LayoutParams(hostView.width, hostView.height).apply {
            leftMargin = hostView.left
            topMargin = hostView.top
        }
        parent.addView(host, parent.indexOfChild(hostView) + 1, params)
        
        // Update overlay position when WebView layout changes
        hostView.addOnLayoutChangeListener { _, left, top, right, bottom, _, _, _, _ ->
            host.layoutParams = (host.layoutParams as? FrameLayout.LayoutParams)?.apply {
                width = right - left
                height = bottom - top
                leftMargin = left
                topMargin = top
            } ?: FrameLayout.LayoutParams(right - left, bottom - top).apply {
                leftMargin = left
                topMargin = top
            }
        }
    }

    fun handleMessage(messageJson: String) {
        try {
            val message = jsonToMap(JSONObject(messageJson)).toMutableMap()
            if (message["pageId"] == null) message["pageId"] = pageKey
            componentManager?.handle(message)
        } catch (_: Exception) {}
    }

    private fun sendEventToView(payload: Map<String, Any>) {
        val webView = webViewRef.get() ?: return
        try {
            val json = JSONObject(mapOf("type" to "event", "name" to "nativecomponent", "payload" to payload)).toString()
            val escaped = JSONArray().put(json).toString().let { it.substring(1, it.length - 1) }
            val script = "(function(){try{window.__LingXiaRecvMessage($escaped);}catch(e){}})();"
            mainHandler.post { webView.evaluateJavascript(script, null) }
        } catch (_: Exception) {}
    }

    fun ensureOverlayHostAttached() {
        val webView = webViewRef.get() ?: return
        val parent = webView.hostView.parent as? ViewGroup ?: return
        val host = overlayHost ?: return
        if (host.parent != parent) {
            (host.parent as? ViewGroup)?.removeView(host)
            addHostToParent(parent, webView, host)
            host.visibility = View.VISIBLE
        }
    }

    fun markPageInactive() {
        componentManager?.handle(mapOf("action" to "page.lifecycle", "state" to "inactive", "pageId" to pageKey))
    }

    fun markPageActive() {
        refreshPageKeyIfNeeded()
        ensureOverlayHostAttached()
        componentManager?.handle(mapOf("action" to "page.lifecycle", "state" to "active", "pageId" to pageKey))
    }

    fun markPageDestroyed() {
        refreshPageKeyIfNeeded()
        // WebView is being torn down; release everything once to avoid duplicate destroy paths.
        componentManager?.teardownAll()
        
        // Clean up pre-draw listener
        preDrawListener?.let { listener ->
            webViewRef.get()?.hostView?.viewTreeObserver?.let { observer ->
                if (observer.isAlive) {
                    observer.removeOnPreDrawListener(listener)
                }
            }
        }
        preDrawListener = null
        lastSyncedScrollX = Int.MIN_VALUE
        lastSyncedScrollY = Int.MIN_VALUE
        (overlayHost?.parent as? ViewGroup)?.removeView(overlayHost)
        overlayHost = null
    }

    private fun refreshPageKeyIfNeeded() {
        webViewRef.get()?.let { pageKey = makePageKey(it) }
    }

    private fun makePageKey(webView: LingXiaWebViewHost) = "${webView.appId ?: "app"}:${webView.currentPath ?: "page"}"

    private fun jsonToMap(json: JSONObject): Map<String, Any?> {
        val map = mutableMapOf<String, Any?>()
        json.keys().forEach { key ->
            val value = json.opt(key)
            map[key] = when (value) {
                is JSONObject -> jsonToMap(value)
                is org.json.JSONArray -> jsonArrayToList(value)
                JSONObject.NULL -> null
                else -> value
            }
        }
        return map
    }

    private fun jsonArrayToList(array: org.json.JSONArray): List<Any?> {
        return (0 until array.length()).map { i ->
            when (val value = array.opt(i)) {
                is JSONObject -> jsonToMap(value)
                is org.json.JSONArray -> jsonArrayToList(value)
                JSONObject.NULL -> null
                else -> value
            }
        }
    }

    companion object {
        private const val OVERLAY_TAG = "ComponentOverlay"
        private val registeredFactories = mutableMapOf<String, LxNativeComponentFactory>()
        private var defaultsRegistered = false
        private val bridgeMap = mutableMapOf<Int, NativeBridge>()
        private val jsInterfaceRegistered = mutableSetOf<Int>()

        @JvmStatic
        fun registerJsInterface(host: LingXiaWebViewHost) {
            val webView = host as? LingXiaWebView ?: return
            if (!webView.usesStrictSecurityProfile()) return
            val id = System.identityHashCode(webView)
            if (jsInterfaceRegistered.add(id)) {
                val jsInterface = JsInterface(webView)
                webView.addJavascriptInterface(jsInterface, "NativeComponentBridge")
            }
        }

        @JvmStatic
        fun attachIfNeeded(webView: LingXiaWebViewHost) {
            if (!webView.usesStrictSecurityProfile()) return
            val id = System.identityHashCode(webView)
            val bridge = bridgeMap[id]?.also { it.ensureOverlayHostAttached() } ?: run {
                registerDefaultComponents()
                NativeBridge(webView).also {
                    it.install()
                    bridgeMap[id] = it
                }
            }
            if (webView is LingXiaServoView) {
                val webViewRef = WeakReference(webView)
                webView.setNativeComponentMessageHandler(
                    object : LingXiaServoView.NativeComponentMessageHandler {
                        override fun onMessage(message: String) {
                            bridge.handleMessage(message)
                        }

                        override fun onDestroyed() {
                            webViewRef.get()?.let(::notifyPageDestroyed)
                        }
                    }
                )
            }
        }

        @JvmStatic
        fun register(type: String, factory: LxNativeComponentFactory) {
            registeredFactories[type] = factory
        }

        private fun registerDefaultComponents() {
            if (defaultsRegistered) return
            defaultsRegistered = true
            registeredFactories.getOrPut("video.native") { VideoComponentFactory() }
            registeredFactories.getOrPut("media-swiper.native") { MediaSwiperComponentFactory() }
            registeredFactories.getOrPut("picker.native") { PickerComponentFactory() }
        }

        @JvmStatic fun notifyPageInactive(webView: LingXiaWebViewHost?) { webView?.let { bridgeMap[System.identityHashCode(it)]?.markPageInactive() } }
        @JvmStatic fun notifyPageActive(webView: LingXiaWebViewHost?) { webView?.let { bridgeMap[System.identityHashCode(it)]?.markPageActive() } }

        @JvmStatic
        fun notifyPageDestroyed(webView: LingXiaWebViewHost?) {
            webView?.let {
                val id = System.identityHashCode(it)
                bridgeMap.remove(id)?.markPageDestroyed()
                jsInterfaceRegistered.remove(id)
            }
        }
    }
}

/** Overlay host that passes through touches to children or WebView. */
internal class ComponentOverlayHost(context: android.content.Context) : FrameLayout(context)
