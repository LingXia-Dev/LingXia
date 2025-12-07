package com.lingxia.lxapp.SameLevel

import android.graphics.Outline
import android.graphics.RectF
import android.view.View
import android.view.ViewGroup
import android.view.ViewOutlineProvider
import java.lang.ref.WeakReference
import kotlin.math.abs
import kotlin.math.roundToInt

/**
 * Manages the lifecycle of native components rendered in SameLevel overlay.
 */
class SameLevelComponentManager(
    hostView: ViewGroup,
    private val defaultPageId: String,
    private val eventSink: (Map<String, Any>) -> Unit
) {
    private val hostViewRef = WeakReference(hostView)
    private val density = hostView.context.resources.displayMetrics.density

    private val components = mutableMapOf<String, LxNativeComponent>()
    private val componentPage = mutableMapOf<String, String>()
    private val componentRects = mutableMapOf<String, RectF>()
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
        val rect = pixelAligned(rectFrom(rectDict))

        val component = factory.make(id, props) { sendEventToWeb(id, it) }

        components[id] = component
        componentPage[id] = pageId
        componentRects[id] = rect
        pageComponents.getOrPut(pageId) { mutableSetOf() }.add(id)
        VideoPlayerRegistry.registerComponent(id, this)

        component.mount(host)
        component.setFrame(rect)
        component.update(props)
        applyCornerRadius(component.view, cornerRadius)
        component.view.translationZ = zIndex
    }

    private fun handleUpdate(params: Map<String, Any?>) {
        val id = params["id"] as? String ?: return
        val component = components[id] ?: return

        (params["rect"] as? Map<*, *>)?.let { rectDict ->
            val newRect = pixelAligned(rectFrom(rectDict))
            if (shouldUpdateFrame(componentRects[id], newRect)) {
                componentRects[id] = newRect
                component.setFrame(newRect)
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
        componentRects.remove(id)
        componentPage.remove(id)
        pageId?.let { pageComponents[it]?.remove(id) }
    }
}
