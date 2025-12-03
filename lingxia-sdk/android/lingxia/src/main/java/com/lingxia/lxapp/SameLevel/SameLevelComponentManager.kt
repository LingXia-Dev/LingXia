package com.lingxia.lxapp.SameLevel

import android.graphics.RectF
import android.util.Log
import android.view.View
import android.view.ViewGroup
import java.lang.ref.WeakReference

private const val TAG = "SameLevelComponentMgr"

/**
 * Manages the lifecycle of native components rendered in SameLevel overlay.
 */
class SameLevelComponentManager(
    hostView: ViewGroup,
    private val defaultPageId: String,
    private val eventSink: (Map<String, Any>) -> Unit
) {
    private val hostViewRef = WeakReference(hostView)

    private val components = mutableMapOf<String, LxNativeComponent>()
    private val componentPage = mutableMapOf<String, String>()
    private val pageComponents = mutableMapOf<String, MutableSet<String>>()
    private val factories = mutableMapOf<String, LxNativeComponentFactory>()

    fun register(type: String, factory: LxNativeComponentFactory) {
        factories[type] = factory
        Log.d(TAG, "Registered component type: $type")
    }

    fun handle(message: Map<String, Any?>) {
        val action = message["action"] as? String ?: return
        val id = message["id"] as? String

        Log.d(TAG, "handle action=$action id=$id")

        when (action) {
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

        val pageId = resolvePageId(params)
        val props = params["props"].asStringMap()
        val zIndex = (params["zIndex"] as? Number)?.toFloat() ?: 0f
        val cornerRadius = (params["cornerRadius"] as? Number)?.toFloat() ?: 0f

        val rect = rectFrom(rectDict)

        if (components.containsKey(id)) {
            Log.w(TAG, "Component $id already mounted, skipping")
            return
        }

        val factory = factories[type]
        if (factory == null) {
            Log.e(TAG, "No factory registered for type: $type")
            return
        }

        val host = hostViewRef.get() ?: return

        val component = factory.make(id, props) { event ->
            sendEventToWeb(id, event)
        }

        components[id] = component
        componentPage[id] = pageId
        pageComponents.getOrPut(pageId) { mutableSetOf() }.add(id)

        component.mount(host)
        component.setFrame(rect)
        component.update(props)

        component.view.apply {
            translationZ = zIndex
            if (cornerRadius > 0) {
                clipToOutline = true
                outlineProvider = android.view.ViewOutlineProvider.BACKGROUND
            }
        }

        if (cornerRadius > 0) {
            component.update(mapOf("cornerRadius" to cornerRadius))
        }
    }

    private fun handleUpdate(params: Map<String, Any?>) {
        val id = params["id"] as? String ?: return
        val component = components[id] ?: return

        (params["rect"] as? Map<*, *>)?.let { rectDict ->
            component.setFrame(rectFrom(rectDict))
        }

        params["props"].asStringMap().takeIf { it.isNotEmpty() }?.let { component.update(it) }

        (params["zIndex"] as? Number)?.let { zIndex ->
            component.view.translationZ = zIndex.toFloat()
        }

        (params["cornerRadius"] as? Number)?.let { radius ->
            component.view.clipToOutline = true
            component.update(mapOf("cornerRadius" to radius.toFloat()))
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
        val cmdParams = params["params"].asStringMap().ifEmpty { null }
        Log.d(TAG, "handleCommand name=$name id=$id")
        components[id]?.handleCommand(name, cmdParams)
    }

    private fun handlePageLifecycle(params: Map<String, Any?>) {
        val pageId = resolvePageId(params)
        val state = params["state"] as? String
        Log.d(TAG, "handlePageLifecycle pageId=$pageId state=$state pageComponents=${pageComponents.keys}")
        when (state) {
            "inactive" -> pausePage(pageId)
            "active" -> resumePage(pageId)
            "destroyed" -> unmountPage(pageId)
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
        // Convert CSS pixels to physical pixels using display density
        val density = hostViewRef.get()?.context?.resources?.displayMetrics?.density ?: 1f
        val x = ((dict["x"] as? Number)?.toFloat() ?: 0f) * density
        val y = ((dict["y"] as? Number)?.toFloat() ?: 0f) * density
        val w = ((dict["width"] as? Number)?.toFloat() ?: 0f) * density
        val h = ((dict["height"] as? Number)?.toFloat() ?: 0f) * density
        return RectF(x, y, x + w, y + h)
    }

    private fun unmountPage(pageId: String) {
        val ids = pageComponents.remove(pageId) ?: return
        ids.forEach { id -> unmountComponent(id, pageId) }
    }

    private fun pausePage(pageId: String) {
        Log.d(TAG, "pausePage pageId=$pageId, ids=${pageComponents[pageId]}")
        val ids = pageComponents[pageId] ?: return
        ids.forEach { id ->
            val component = components[id] ?: return@forEach
            Log.d(TAG, "pausePage: hiding component $id")
            component.blur()
            component.view.visibility = View.GONE
            component.handleCommand("pause", null)
        }
    }

    private fun resumePage(pageId: String) {
        Log.d(TAG, "resumePage pageId=$pageId, ids=${pageComponents[pageId]}")
        val ids = pageComponents[pageId] ?: return
        val host = hostViewRef.get()
        ids.forEach { id ->
            val component = components[id] ?: return@forEach
            Log.d(TAG, "resumePage: showing component $id, current visibility=${component.view.visibility}, parent=${component.view.parent}")

            // Ensure view is attached to host (may have been detached during page navigation)
            if (component.view.parent != host && host != null) {
                (component.view.parent as? ViewGroup)?.removeView(component.view)
                host.addView(component.view)
                Log.d(TAG, "resumePage: re-attached component $id to host")
            }

            component.view.visibility = View.VISIBLE
            component.focus()
            Log.d(TAG, "resumePage: after set visibility=${component.view.visibility}")
        }
    }

    private fun unmountComponent(id: String, pageId: String?) {
        val component = components.remove(id) ?: return
        component.unmount()
        pageId?.let { pid ->
            pageComponents[pid]?.remove(id)
            if (pageComponents[pid]?.isEmpty() == true) {
                pageComponents.remove(pid)
            }
        }
        componentPage.remove(id)
    }
}
