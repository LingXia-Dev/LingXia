package com.lingxia.lxapp.SameLevel

import android.graphics.Outline
import android.graphics.RectF
import android.view.View
import android.view.ViewGroup
import android.view.ViewOutlineProvider
import com.lingxia.webview.LingXiaWebView
import java.lang.ref.WeakReference
import kotlin.math.abs
import kotlin.math.roundToInt

/**
 * Manages the lifecycle of native components rendered in SameLevel overlay.
 * 
 * For Android's overlay-based same-level rendering, this manager tracks component
 * positions in WebView content coordinates and updates screen positions when
 * the WebView scrolls, eliminating JS polling latency.
 */
class SameLevelComponentManager(
    hostView: ViewGroup,
    private val defaultPageId: String,
    private val eventSink: (Map<String, Any>) -> Unit,
    webView: LingXiaWebView? = null
) {
    private val hostViewRef = WeakReference(hostView)
    private val webViewRef = webView?.let { WeakReference(it) }
    private val density = hostView.context.resources.displayMetrics.density

    private val components = mutableMapOf<String, LxNativeComponent>()
    private val componentPage = mutableMapOf<String, String>()
    // Content position (document coordinates = viewport + scroll at mount time)
    // This is the stable reference position in the WebView content
    private val componentContentRects = mutableMapOf<String, RectF>()
    // Pre-allocated screen rects to avoid GC during scroll
    private val componentScreenRects = mutableMapOf<String, RectF>()
    private val pageComponents = mutableMapOf<String, MutableSet<String>>()
    private val factories = mutableMapOf<String, LxNativeComponentFactory>()

    fun register(type: String, factory: LxNativeComponentFactory) {
        factories[type] = factory
    }

    fun handle(message: Map<String, Any?>) {
        when (message["action"] as? String) {
            "component.mount" -> handleMount(message)
            "component.update" -> handleUpdate(message)
            "component.unmount" -> handleUnmount(message)
            "component.focus" -> handleFocus(message)
            "component.blur" -> handleBlur(message)
            "component.command" -> handleCommand(message)
            "page.lifecycle" -> handlePageLifecycle(message)
        }
    }

    private fun handleMount(params: Map<String, Any?>) {
        val id = params["id"] as? String ?: return
        val type = params["type"] as? String ?: return
        val rectDict = params["rect"] as? Map<*, *> ?: return
        if (components.containsKey(id)) return

        val factory = factories[type] ?: return
        val host = hostViewRef.get() ?: return

        val pageId = resolvePageId(params)
        val props = params["props"].asStringMap()
        val zIndex = (params["zIndex"] as? Number)?.toFloat() ?: 0f
        val cornerRadius = (params["cornerRadius"] as? Number)?.toFloat() ?: 0f
        // JS sends document coordinates (content position = viewport + scroll)
        // Store directly as content rect for scroll tracking
        val contentRect = pixelAligned(rectFrom(rectDict))
        
        // Calculate initial screen position
        val screenRect = contentRectToScreenRect(contentRect)

        val component = factory.make(id, props) { sendEventToWeb(id, it) }

        components[id] = component
        componentPage[id] = pageId
        componentContentRects[id] = contentRect
        componentScreenRects[id] = screenRect
        pageComponents.getOrPut(pageId) { mutableSetOf() }.add(id)
        VideoPlayerRegistry.registerComponent(id, this)

        component.mount(host)
        component.setFrame(screenRect)
        component.update(props)
        applyCornerRadius(component.view, cornerRadius)
        component.view.translationZ = zIndex
    }

    private fun handleUpdate(params: Map<String, Any?>) {
        val id = params["id"] as? String ?: return
        val component = components[id] ?: return

        (params["rect"] as? Map<*, *>)?.let { rectDict ->
            // JS sends document coordinates (content position)
            val newContentRect = pixelAligned(rectFrom(rectDict))
            
            if (shouldUpdateFrame(componentContentRects[id], newContentRect)) {
                componentContentRects[id] = newContentRect
                val screenRect = componentScreenRects.getOrPut(id) { RectF() }
                updateScreenRect(screenRect, newContentRect)
                component.setFrame(screenRect)
            }
        }

        params["props"].asStringMap().takeIf { it.isNotEmpty() }?.let { component.update(it) }
        (params["zIndex"] as? Number)?.let { component.view.translationZ = it.toFloat() }
        (params["cornerRadius"] as? Number)?.toFloat()?.let { radius ->
            applyCornerRadius(component.view, radius)
            component.update(mapOf("cornerRadius" to radius))
        }
    }

    private fun handleUnmount(params: Map<String, Any?>) {
        val id = params["id"] as? String ?: return
        val pageId = resolvePageId(params)
        unmountComponent(id, pageId)
    }

    private fun handleFocus(params: Map<String, Any?>) {
        val id = params["id"] as? String ?: return
        components[id]?.focus()
    }

    private fun handleBlur(params: Map<String, Any?>) {
        val id = params["id"] as? String ?: return
        components[id]?.blur()
    }

    private fun handleCommand(params: Map<String, Any?>) {
        val id = params["id"] as? String ?: return
        val name = params["name"] as? String ?: return
        components[id]?.handleCommand(name, params["params"].asStringMap().ifEmpty { null })
    }

    private fun handlePageLifecycle(params: Map<String, Any?>) {
        when (params["state"] as? String) {
            "inactive" -> pausePage(resolvePageId(params))
            "active" -> resumePage(resolvePageId(params))
            "destroyed" -> unmountPage(resolvePageId(params))
        }
    }

    fun teardownAll() {
        val allIds = components.keys.toList()
        allIds.forEach { id ->
            unmountComponent(id, componentPage[id])
        }
        pageComponents.clear()
    }
    
    /**
     * Called before each frame draw to sync component positions with WebView scroll.
     * Uses pre-allocated RectF objects to avoid GC pressure during scroll.
     * 
     * @param scrollX Current horizontal scroll position in pixels
     * @param scrollY Current vertical scroll position in pixels
     */
    fun onWebViewScroll(scrollX: Int, scrollY: Int) {
        val scrollXPx = scrollX.toFloat()
        val scrollYPx = scrollY.toFloat()
        
        components.forEach { (id, component) ->
            val contentRect = componentContentRects[id] ?: return@forEach
            val screenRect = componentScreenRects.getOrPut(id) { RectF() }
            screenRect.set(
                contentRect.left - scrollXPx,
                contentRect.top - scrollYPx,
                contentRect.right - scrollXPx,
                contentRect.bottom - scrollYPx
            )
            component.setFrame(screenRect)
        }
    }

    private fun sendEventToWeb(componentId: String, event: Map<String, Any>) {
        val payload = event.toMutableMap()
        payload["action"] = "component.event"
        payload["id"] = componentId
        componentPage[componentId]?.let { payload["pageId"] = it }
        eventSink(payload)
    }

    private fun resolvePageId(dict: Map<String, Any?>): String {
        val pageId = dict["pageId"] as? String
        return if (!pageId.isNullOrEmpty()) pageId else defaultPageId
    }

    private fun Any?.asStringMap(): Map<String, Any?> {
        val raw = this as? Map<*, *> ?: return emptyMap()
        val filtered = mutableMapOf<String, Any?>()
        raw.forEach { (k, v) ->
            if (k is String) {
                filtered[k] = v
            }
        }
        return filtered
    }

    private fun rectFrom(dict: Map<*, *>): RectF {
        val x = ((dict["x"] as? Number)?.toFloat() ?: 0f) * density
        val y = ((dict["y"] as? Number)?.toFloat() ?: 0f) * density
        val w = ((dict["width"] as? Number)?.toFloat() ?: 0f) * density
        val h = ((dict["height"] as? Number)?.toFloat() ?: 0f) * density
        return RectF(x, y, x + w, y + h)
    }

    private fun pixelAligned(rect: RectF): RectF = RectF(
        rect.left.roundToInt().toFloat(),
        rect.top.roundToInt().toFloat(),
        rect.right.roundToInt().toFloat(),
        rect.bottom.roundToInt().toFloat()
    )

    private fun shouldUpdateFrame(old: RectF?, new: RectF): Boolean {
        if (old == null) return true
        return abs(old.left - new.left) > 0.5f || abs(old.top - new.top) > 0.5f ||
               abs(old.right - new.right) > 0.5f || abs(old.bottom - new.bottom) > 0.5f
    }

    private fun contentRectToScreenRect(contentRect: RectF): RectF {
        val webView = webViewRef?.get()
        val scrollX = (webView?.scrollX ?: 0).toFloat()
        val scrollY = (webView?.scrollY ?: 0).toFloat()
        return RectF(
            contentRect.left - scrollX,
            contentRect.top - scrollY,
            contentRect.right - scrollX,
            contentRect.bottom - scrollY
        )
    }

    private fun updateScreenRect(screenRect: RectF, contentRect: RectF) {
        val webView = webViewRef?.get()
        val scrollX = (webView?.scrollX ?: 0).toFloat()
        val scrollY = (webView?.scrollY ?: 0).toFloat()
        screenRect.set(
            contentRect.left - scrollX,
            contentRect.top - scrollY,
            contentRect.right - scrollX,
            contentRect.bottom - scrollY
        )
    }

    private fun applyCornerRadius(view: View, radius: Float) {
        if (radius > 0) {
            view.clipToOutline = true
            view.outlineProvider = object : ViewOutlineProvider() {
                override fun getOutline(v: View, outline: Outline) {
                    outline.setRoundRect(0, 0, v.width, v.height, radius * density)
                }
            }
        } else {
            view.clipToOutline = false
            view.outlineProvider = null
        }
    }

    private fun unmountPage(pageId: String) {
        pageComponents.remove(pageId)?.forEach { unmountComponent(it, pageId) }
    }

    private fun pausePage(pageId: String) {
        pageComponents[pageId]?.forEach { id ->
            components[id]?.apply {
                blur()
                view.visibility = View.GONE
                handleCommand("pause", null)
            }
        }
    }

    private fun resumePage(pageId: String) {
        val host = hostViewRef.get() ?: return
        pageComponents[pageId]?.forEach { id ->
            components[id]?.apply {
                if (view.parent != host) {
                    (view.parent as? ViewGroup)?.removeView(view)
                    host.addView(view)
                }
                view.visibility = View.VISIBLE
                focus()
            }
        }
    }

    private fun unmountComponent(id: String, pageId: String?) {
        components.remove(id)?.unmount()
        componentContentRects.remove(id)
        componentScreenRects.remove(id)
        componentPage.remove(id)
        pageId?.let { pageComponents[it]?.remove(id) }
    }
}
