package com.lingxia.lxapp.NativeComponents

import android.graphics.RectF
import android.view.TextureView
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import org.json.JSONArray
import org.json.JSONObject

/**
 * Sibling island host for inline native nodes.
 *
 * The container sits next to the page WebView (not inside it). Video uses
 * [TextureView] only — SurfaceView hole-punch is forbidden. Child order is
 * committed sibling / document order from `root.commit`.
 */
internal class InlineNativeIsland(
    private val host: ViewGroup
) {
    private val container = FrameLayout(host.context).apply {
        id = View.generateViewId()
        isClickable = false
        clipToPadding = true
        clipChildren = true
    }
    private val nodes = linkedMapOf<String, IslandNode>()
    private var lastAppliedRevision = 0L

    init {
        if (container.parent == null) {
            host.addView(
                container,
                ViewGroup.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.MATCH_PARENT
                )
            )
        }
    }

    fun handle(message: Map<String, Any?>): Boolean {
        return when (message["action"] as? String) {
            "root.commit" -> {
                applyCommit(message)
                true
            }
            "geometry.snapshot" -> true
            "video.command" -> true
            else -> false
        }
    }

    private fun applyCommit(message: Map<String, Any?>) {
        val revision = (message["revision"] as? Number)?.toLong() ?: return
        val operations = message["operations"] as? List<*> ?: return
        for (raw in operations) {
            val op = raw as? Map<*, *> ?: continue
            when (op["op"] as? String) {
                "mount" -> mount(op["node"] as? Map<*, *> ?: continue)
                "unmount" -> {
                    val node = op["node"] as? Map<*, *> ?: continue
                    val key = node["nodeKey"] as? String ?: continue
                    nodes.remove(key)?.view?.let { container.removeView(it) }
                }
                "reorder" -> {
                    val node = op["node"] as? Map<*, *> ?: continue
                    val key = node["nodeKey"] as? String ?: continue
                    val order = (op["order"] as? Number)?.toInt() ?: continue
                    nodes[key]?.order = order
                }
            }
        }
        restack()
        lastAppliedRevision = revision
    }

    private fun mount(node: Map<*, *>) {
        val ref = node["ref"] as? Map<*, *> ?: return
        val key = ref["nodeKey"] as? String ?: return
        val kind = node["kind"] as? String ?: return
        if (kind !in ALLOWED_KINDS) return
        val view = factoryView(kind)
        val order = (node["order"] as? Number)?.toInt() ?: 0
        nodes[key] = IslandNode(key, kind, order, view)
        container.addView(view)
    }

    private fun factoryView(kind: String): View {
        return when (kind) {
            "video" -> TextureView(host.context)
            else -> View(host.context)
        }
    }

    private fun restack() {
        val ordered = nodes.values.sortedWith(compareBy({ it.order }, { it.key }))
        ordered.forEachIndexed { index, node ->
            container.bringChildToFront(node.view)
            node.view.z = index.toFloat()
        }
    }

    fun lastAppliedRevision(): Long = lastAppliedRevision

    private data class IslandNode(
        val key: String,
        val kind: String,
        var order: Int,
        val view: View
    )

    companion object {
        val ALLOWED_KINDS = setOf("root", "view", "text", "tappable", "slider", "video")
    }
}

internal fun jsonObjectToMap(value: JSONObject): Map<String, Any?> {
    val out = mutableMapOf<String, Any?>()
    val keys = value.keys()
    while (keys.hasNext()) {
        val key = keys.next()
        out[key] = when (val item = value.get(key)) {
            JSONObject.NULL -> null
            is JSONObject -> jsonObjectToMap(item)
            is JSONArray -> (0 until item.length()).map { item.get(it) }
            else -> item
        }
    }
    return out
}

