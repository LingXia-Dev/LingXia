package com.lingxia.lxapp.SameLevel

import android.graphics.Color
import android.os.Handler
import android.os.Looper
import android.view.View
import android.view.ViewGroup
import android.view.ViewTreeObserver
import android.webkit.JavascriptInterface
import android.widget.FrameLayout
import com.lingxia.lxapp.SameLevel.Components.VideoComponentFactory
import com.lingxia.webview.LingXiaWebView
import org.json.JSONArray
import org.json.JSONObject
import java.lang.ref.WeakReference

/**
 * Bridge between JS component.* messages and native SameLevel components.
 * Uses JavaScriptInterface for View→Native and evaluateJavascript for Native→View.
 */
class SameLevelBridge private constructor(
    webView: LingXiaWebView
) {
    private val webViewRef = WeakReference(webView)
    private var overlayHost: SameLevelOverlayHost? = null
    private var componentManager: SameLevelComponentManager? = null
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

        val manager = SameLevelComponentManager(
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
                val scrollX = wv.scrollX
                val scrollY = wv.scrollY
                // Only update if scroll position changed to avoid redundant work
                if (scrollX != lastSyncedScrollX || scrollY != lastSyncedScrollY) {
                    lastSyncedScrollX = scrollX
                    lastSyncedScrollY = scrollY
                    manager.onWebViewScroll(scrollX, scrollY)
                }
            }
            true // Proceed with drawing
        }
        webView.viewTreeObserver.addOnPreDrawListener(preDrawListener)
    }

    private class JsInterface(webView: LingXiaWebView) {
        private val webViewId = System.identityHashCode(webView)

        @JavascriptInterface
        fun postMessage(messageJson: String) {
            Handler(Looper.getMainLooper()).post {
                bridgeMap[webViewId]?.handleMessage(messageJson)
            }
        }
    }

    private fun makeOrFindOverlayHost(webView: LingXiaWebView): SameLevelOverlayHost {
        val parent = webView.parent as? ViewGroup

        overlayHost?.let { existing ->
            if (existing.parent != parent && parent != null) {
                (existing.parent as? ViewGroup)?.removeView(existing)
                addHostToParent(parent, webView, existing)
            }
            return existing
        }

        parent?.let { p ->
            for (i in 0 until p.childCount) {
                (p.getChildAt(i) as? SameLevelOverlayHost)?.takeIf { it.tag == OVERLAY_TAG }?.let { return it }
            }
        }

        val host = SameLevelOverlayHost(webView.context).apply {
            tag = OVERLAY_TAG
            setBackgroundColor(Color.TRANSPARENT)
            isClickable = false
            isFocusable = false
            // Enable hardware layer for smoother rendering during scroll
            setLayerType(View.LAYER_TYPE_HARDWARE, null)
        }
        parent?.let { addHostToParent(it, webView, host) }
        return host
    }

    private fun addHostToParent(parent: ViewGroup, webView: LingXiaWebView, host: SameLevelOverlayHost) {
        // Match WebView's exact position and size in parent
        val params = FrameLayout.LayoutParams(webView.width, webView.height).apply {
            leftMargin = webView.left
            topMargin = webView.top
        }
        parent.addView(host, parent.indexOfChild(webView) + 1, params)
        
        // Update overlay position when WebView layout changes
        webView.addOnLayoutChangeListener { _, left, top, right, bottom, _, _, _, _ ->
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
            val json = JSONObject(mapOf("type" to "event", "name" to "samelevel", "payload" to payload)).toString()
            val escaped = JSONArray().put(json).toString().let { it.substring(1, it.length - 1) }
            val script = "(function(){if(typeof window.__LingXiaRecvMessage==='function'){try{window.__LingXiaRecvMessage($escaped);}catch(e){}}})();"
            mainHandler.post { webView.evaluateJavascript(script, null) }
        } catch (_: Exception) {}
    }

    fun ensureOverlayHostAttached() {
        val webView = webViewRef.get() ?: return
        val parent = webView.parent as? ViewGroup ?: return
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
        componentManager?.handle(mapOf("action" to "page.lifecycle", "state" to "destroyed", "pageId" to pageKey))
        componentManager?.teardownAll()
        
        // Clean up pre-draw listener
        preDrawListener?.let { listener ->
            webViewRef.get()?.viewTreeObserver?.let { observer ->
                if (observer.isAlive) {
                    observer.removeOnPreDrawListener(listener)
                }
            }
        }
        preDrawListener = null
        lastSyncedScrollX = Int.MIN_VALUE
        lastSyncedScrollY = Int.MIN_VALUE
    }

    private fun refreshPageKeyIfNeeded() {
        webViewRef.get()?.let { pageKey = makePageKey(it) }
    }

    private fun makePageKey(webView: LingXiaWebView) = "${webView.appId ?: "app"}:${webView.currentPath ?: "page"}"

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
        private const val OVERLAY_TAG = "SameLevelOverlay"
        private val registeredFactories = mutableMapOf<String, LxNativeComponentFactory>()
        private var defaultsRegistered = false
        private val bridgeMap = mutableMapOf<Int, SameLevelBridge>()
        private val jsInterfaceRegistered = mutableSetOf<Int>()

        @JvmStatic
        fun registerJsInterface(webView: LingXiaWebView) {
            val id = System.identityHashCode(webView)
            if (jsInterfaceRegistered.add(id)) {
                webView.addJavascriptInterface(JsInterface(webView), "SameLevelNative")
            }
        }

        @JvmStatic
        fun attachIfNeeded(webView: LingXiaWebView) {
            val id = System.identityHashCode(webView)
            bridgeMap[id]?.ensureOverlayHostAttached() ?: run {
                registerDefaultComponents()
                bridgeMap[id] = SameLevelBridge(webView).also { it.install() }
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
        }

        @JvmStatic fun notifyPageInactive(webView: LingXiaWebView?) { webView?.let { bridgeMap[System.identityHashCode(it)]?.markPageInactive() } }
        @JvmStatic fun notifyPageActive(webView: LingXiaWebView?) { webView?.let { bridgeMap[System.identityHashCode(it)]?.markPageActive() } }

        @JvmStatic
        fun notifyPageDestroyed(webView: LingXiaWebView?) {
            webView?.let {
                val id = System.identityHashCode(it)
                bridgeMap.remove(id)?.markPageDestroyed()
                jsInterfaceRegistered.remove(id)
            }
        }
    }
}

/** Overlay host that passes through touches to children or WebView. */
class SameLevelOverlayHost(context: android.content.Context) : FrameLayout(context)
