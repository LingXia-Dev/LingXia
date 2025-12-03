package com.lingxia.lxapp.SameLevel

import android.graphics.Color
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.webkit.JavascriptInterface
import android.widget.FrameLayout
import com.lingxia.lxapp.SameLevel.Components.VideoComponentFactory
import com.lingxia.webview.LingXiaWebView
import org.json.JSONArray
import org.json.JSONObject
import java.lang.ref.WeakReference

private const val TAG = "SameLevelBridge"

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
            eventSink = { payload -> sendEventToView(payload) }
        )

        registeredFactories.forEach { (type, factory) ->
            manager.register(type, factory)
        }

        componentManager = manager
        Log.i(TAG, "SameLevelBridge installed for WebView")
    }

    /** JavaScriptInterface exposed to WebView - routes to bridge via webViewId lookup */
    private class JsInterface(webView: LingXiaWebView) {
        private val webViewRef = WeakReference(webView)
        private val webViewId = System.identityHashCode(webView)

        @JavascriptInterface
        fun postMessage(messageJson: String) {
            Handler(Looper.getMainLooper()).post {
                val bridge = bridgeMap[webViewId]
                if (bridge != null) {
                    bridge.handleMessage(messageJson)
                } else {
                    Log.w(TAG, "JsInterface.postMessage: no bridge for webViewId=$webViewId, queuing not supported yet")
                }
            }
        }
    }

    private fun makeOrFindOverlayHost(webView: LingXiaWebView): SameLevelOverlayHost {
        val parent = webView.parent as? ViewGroup

        // First check if we already have an overlayHost - reuse it if possible
        overlayHost?.let { existingHost ->
            // If existing host is in a different parent, move it to current parent
            if (existingHost.parent != parent && parent != null) {
                (existingHost.parent as? ViewGroup)?.removeView(existingHost)
                val webViewIndex = parent.indexOfChild(webView)
                val params = FrameLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.MATCH_PARENT
                )
                parent.addView(existingHost, webViewIndex + 1, params)
                Log.d(TAG, "Moved existing overlay host to new parent")
            }
            return existingHost
        }

        // Find existing host in current parent by tag
        parent?.let { p ->
            for (i in 0 until p.childCount) {
                val child = p.getChildAt(i)
                if (child is SameLevelOverlayHost && child.tag == OVERLAY_TAG) {
                    return child
                }
            }
        }

        // Create new host
        val host = SameLevelOverlayHost(webView.context).apply {
            tag = OVERLAY_TAG
            setBackgroundColor(Color.TRANSPARENT)
            isClickable = false
            isFocusable = false
        }

        // Add host as sibling to WebView, on top
        parent?.let { p ->
            val webViewIndex = p.indexOfChild(webView)
            val params = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            p.addView(host, webViewIndex + 1, params)
        }

        return host
    }

    fun handleMessage(messageJson: String) {
        try {
            val json = JSONObject(messageJson)
            val message = jsonToMap(json)

            val action = message["action"] as? String
            val id = message["id"] as? String
            Log.d(TAG, "handleMessage action=$action id=$id")

            var messageWithPage = message.toMutableMap()
            if (messageWithPage["pageId"] == null) {
                messageWithPage["pageId"] = pageKey
            }

            componentManager?.handle(messageWithPage)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to parse message: $messageJson", e)
        }
    }

    private fun sendEventToView(payload: Map<String, Any>) {
        val webView = webViewRef.get() ?: return

        try {
            val fullMessage = mapOf(
                "type" to "event",
                "name" to "samelevel",
                "payload" to payload
            )
            val jsonString = JSONObject(fullMessage).toString()
            // Escape for safe JS embedding
            val escaped = JSONArray().put(jsonString).toString()
            val safeJsLiteral = escaped.substring(1, escaped.length - 1)

            val script = """
                (function(){
                  if (typeof window.__LingXiaRecvMessage === 'function') {
                    try { window.__LingXiaRecvMessage($safeJsLiteral); } catch (e) {}
                  }
                })();
            """.trimIndent()

            mainHandler.post {
                webView.evaluateJavascript(script, null)
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to send event to JavaScript", e)
        }
    }

    /**
     * Ensure overlay host is attached to the correct parent (WebView's container).
     */
    fun ensureOverlayHostAttached() {
        val webView = webViewRef.get() ?: return
        val parent = webView.parent as? ViewGroup ?: return
        val host = overlayHost ?: return

        // Check if host needs to be moved to current parent
        if (host.parent != parent) {
            (host.parent as? ViewGroup)?.removeView(host)
            val webViewIndex = parent.indexOfChild(webView)
            val params = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            parent.addView(host, webViewIndex + 1, params)
            host.visibility = View.VISIBLE
            Log.d(TAG, "ensureOverlayHostAttached: moved overlay host to new parent")
        }
    }

    fun markPageInactive() {
        componentManager?.handle(mapOf(
            "action" to "page.lifecycle",
            "state" to "inactive",
            "pageId" to pageKey
        ))
    }

    fun markPageActive() {
        refreshPageKeyIfNeeded()
        // Ensure overlay host is in correct parent before resuming components
        ensureOverlayHostAttached()
        componentManager?.handle(mapOf(
            "action" to "page.lifecycle",
            "state" to "active",
            "pageId" to pageKey
        ))
    }

    fun markPageDestroyed() {
        refreshPageKeyIfNeeded()
        componentManager?.handle(mapOf(
            "action" to "page.lifecycle",
            "state" to "destroyed",
            "pageId" to pageKey
        ))
        componentManager?.teardownAll()
    }

    private fun refreshPageKeyIfNeeded() {
        val webView = webViewRef.get() ?: return
        val newKey = makePageKey(webView)
        if (newKey != pageKey) {
            pageKey = newKey
        }
    }

    private fun makePageKey(webView: LingXiaWebView): String {
        val app = webView.appId ?: "app"
        val path = webView.currentPath ?: "page"
        return "$app:$path"
    }

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

        /** Register JavaScriptInterface early (called from WebView.onAttachedToWindow) */
        @JvmStatic
        fun registerJsInterface(webView: LingXiaWebView) {
            val webViewId = System.identityHashCode(webView)
            if (jsInterfaceRegistered.contains(webViewId)) return
            jsInterfaceRegistered.add(webViewId)
            webView.addJavascriptInterface(JsInterface(webView), "SameLevelNative")
            Log.i(TAG, "SameLevelNative JavaScriptInterface registered for webViewId=$webViewId")
        }

        @JvmStatic
        fun attachIfNeeded(webView: LingXiaWebView) {
            val webViewId = System.identityHashCode(webView)

            val existingBridge = bridgeMap[webViewId]
            if (existingBridge != null) {
                existingBridge.ensureOverlayHostAttached()
                return
            }

            registerDefaultComponents()

            val bridge = SameLevelBridge(webView)
            bridge.install()
            bridgeMap[webViewId] = bridge
        }

        @JvmStatic
        fun register(type: String, factory: LxNativeComponentFactory) {
            registeredFactories[type] = factory
            Log.i(TAG, "Registered component type: $type")
        }

        private fun registerDefaultComponents() {
            if (defaultsRegistered) return
            defaultsRegistered = true

            if (!registeredFactories.containsKey("video.native")) {
                registeredFactories["video.native"] = VideoComponentFactory()
            }
        }

        @JvmStatic
        fun notifyPageInactive(webView: LingXiaWebView?) {
            webView?.let { bridgeMap[System.identityHashCode(it)]?.markPageInactive() }
        }

        @JvmStatic
        fun notifyPageActive(webView: LingXiaWebView?) {
            webView?.let { bridgeMap[System.identityHashCode(it)]?.markPageActive() }
        }

        @JvmStatic
        fun notifyPageDestroyed(webView: LingXiaWebView?) {
            webView?.let {
                val id = System.identityHashCode(it)
                bridgeMap[id]?.markPageDestroyed()
                bridgeMap.remove(id)
            }
        }
    }
}

/**
 * Overlay host view that passes through touches to components or WebView.
 */
class SameLevelOverlayHost(context: android.content.Context) : FrameLayout(context) {
    override fun onInterceptTouchEvent(ev: MotionEvent?): Boolean {
        // Don't intercept - let children handle touches
        return false
    }

    override fun onTouchEvent(event: MotionEvent?): Boolean {
        // Don't consume touches - let them pass through to WebView
        return false
    }
}
