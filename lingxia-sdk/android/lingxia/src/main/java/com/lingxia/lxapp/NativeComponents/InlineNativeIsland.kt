package com.lingxia.lxapp.NativeComponents

import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.SeekBar
import android.widget.TextView
import com.lingxia.lxapp.NativeComponents.Components.VideoComponent
import org.json.JSONArray
import org.json.JSONObject
import kotlin.math.roundToInt

/**
 * Sibling island host for inline native nodes.
 *
 * The container sits next to the page WebView (not inside it). Video uses
 * [TextureView] via [VideoComponent] — SurfaceView hole-punch is forbidden.
 * Child order is committed sibling / document order from `root.commit`.
 */
internal class InlineNativeIsland(
    private val host: ViewGroup,
    private val eventSink: (componentId: String, event: String, detail: Map<String, Any?>) -> Unit
) {
    private val density = host.resources.displayMetrics.density
    private val container = FrameLayout(host.context).apply {
        id = View.generateViewId()
        isClickable = false
        clipToPadding = true
        clipChildren = true
    }
    private val nodes = linkedMapOf<String, IslandNode>()
    private var lastAppliedRevision = 0L
    private var scrollXPx = 0
    private var scrollYPx = 0
    private var leaseGranted = false
    private var leaseActive = false
    private var leaseId = ""
    private var leaseSequence = 1L
    private var lastRoot: Map<String, Any?>? = null
    private val pendingOutgoing = mutableListOf<Map<String, Any?>>()

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
            "geometry.snapshot" -> {
                applyGeometry(message)
                true
            }
            "root.leaseAccept" -> {
                acceptLease(message)
                true
            }
            "video.command" -> {
                applyVideoCommand(message)
                true
            }
            else -> false
        }
    }

    fun drainOutgoing(): List<Map<String, Any?>> {
        val outgoing = pendingOutgoing.toList()
        pendingOutgoing.clear()
        return outgoing
    }

    fun onWebViewScroll(scrollX: Int, scrollY: Int) {
        scrollXPx = scrollX
        scrollYPx = scrollY
        nodes.values.forEach { applyFrame(it) }
    }

    fun lastAppliedRevision(): Long = lastAppliedRevision

    private fun applyCommit(message: Map<String, Any?>) {
        val operations = message["operations"] as? List<*> ?: return
        for (raw in operations) {
            val op = asMap(raw) ?: continue
            when (op["op"] as? String) {
                "mount" -> mount(asMap(op["node"]) ?: continue)
                "update" -> update(op)
                "unmount" -> {
                    val node = asMap(op["node"]) ?: continue
                    val key = nodeKey(node) ?: continue
                    removeNode(key)
                }
                "reorder" -> {
                    val node = asMap(op["node"]) ?: continue
                    val key = nodeKey(node) ?: continue
                    val order = (op["order"] as? Number)?.toInt() ?: continue
                    nodes[key]?.order = order
                }
            }
        }
        restack()
        lastAppliedRevision = (message["revision"] as? Number)?.toLong() ?: lastAppliedRevision
        val root = asMap(message["root"])
        if (root != null) {
            lastRoot = root
            grantLeaseIfNeeded(root)
        }
    }

    private fun applyGeometry(message: Map<String, Any?>) {
        val entries = message["nodes"] as? List<*> ?: return
        for (raw in entries) {
            val entry = asMap(raw) ?: continue
            val ref = asMap(entry["ref"]) ?: continue
            val key = ref["nodeKey"] as? String ?: continue
            val node = nodes[key] ?: continue
            val rect = asMap(entry["contentRect"]) ?: continue
            node.rectX = number(rect["x"])
            node.rectY = number(rect["y"])
            node.rectW = number(rect["width"])
            node.rectH = number(rect["height"])
            node.visible = entry["visible"] as? Boolean ?: true
            applyFrame(node)
        }
    }

    private fun acceptLease(message: Map<String, Any?>) {
        if (!leaseGranted || leaseActive) return
        val incomingId = message["leaseId"] as? String ?: ""
        val incomingSeq = (message["sequence"] as? Number)?.toLong() ?: 0L
        if (incomingId.isNotEmpty() && incomingId != leaseId) return
        if (incomingSeq > 0 && incomingSeq != leaseSequence) return
        leaseActive = true
        val rootKey = lastRoot?.get("rootKey") as? String ?: "island"
        pendingOutgoing += mapOf(
            "action" to "root.leaseActive",
            "id" to rootKey,
            "root" to lastRoot,
            "leaseId" to leaseId,
            "sequence" to leaseSequence
        )
    }

    private fun grantLeaseIfNeeded(root: Map<String, Any?>) {
        if (leaseGranted) return
        val rootKey = root["rootKey"] as? String ?: "island"
        leaseGranted = true
        leaseId = "lease-$rootKey"
        leaseSequence = 1
        pendingOutgoing += mapOf(
            "action" to "root.leaseGranted",
            "id" to rootKey,
            "root" to root,
            "leaseId" to leaseId,
            "sequence" to leaseSequence,
            "leaseDurationMs" to 8000
        )
    }

    private fun applyVideoCommand(message: Map<String, Any?>) {
        val owner = asMap(message["owner"])
        val key = owner?.get("nodeKey") as? String
        val node = key?.let { nodes[it] } ?: nodes.values.firstOrNull { it.kind == "video" }
        val video = node?.video ?: return
        val command = asMap(message["command"]) ?: return
        val name = command["name"] as? String ?: return
        val params = when (name) {
            "seek" -> mapOf("time" to number(command["seconds"]))
            "setStreamSource" -> asMap(command["options"]) ?: emptyMap()
            else -> emptyMap()
        }
        video.handleCommand(name, params)
    }

    private fun mount(node: Map<*, *>) {
        val ref = asMap(node["ref"]) ?: return
        val key = ref["nodeKey"] as? String ?: return
        val kind = node["kind"] as? String ?: return
        if (kind !in ALLOWED_KINDS) return
        val authorId = (node["authorId"] as? String)?.takeIf { it.isNotEmpty() } ?: key
        val props = asMap(node["props"]) ?: emptyMap()
        val order = (node["order"] as? Number)?.toInt() ?: 0
        val item = factoryNode(key, kind, authorId, order, props)
        nodes[key] = item
        if (item.view.parent == null) {
            container.addView(item.view)
        }
        applyFrame(item)
    }

    private fun update(op: Map<String, Any?>) {
        val ref = asMap(op["node"]) ?: return
        val key = nodeKey(ref) ?: return
        val existing = nodes[key] ?: return
        val patch = asMap(op["patch"]) ?: return
        existing.props = existing.props + patch
        applyProps(existing)
    }

    private fun factoryNode(
        key: String,
        kind: String,
        authorId: String,
        order: Int,
        props: Map<String, Any?>
    ): IslandNode {
        val item = IslandNode(
            key,
            kind,
            authorId,
            order,
            props,
            FrameLayout(host.context)
        )
        when (kind) {
            "video" -> {
                val video = VideoComponent(authorId, props) { event ->
                    val name = event["event"] as? String ?: return@VideoComponent
                    @Suppress("UNCHECKED_CAST")
                    val detail = (event["detail"] as? Map<String, Any?>) ?: emptyMap()
                    eventSink(authorId, name, detail)
                }
                video.mount(container)
                item.video = video
                item.view = video.view
            }
            "text" -> {
                item.view = TextView(host.context).apply {
                    setTextColor(Color.WHITE)
                    textSize = 12f
                    isClickable = false
                }
            }
            "tappable" -> {
                val label = TextView(host.context).apply {
                    setTextColor(Color.WHITE)
                    gravity = Gravity.CENTER
                    textSize = 12f
                }
                item.label = label
                item.view = FrameLayout(host.context).apply {
                    setBackgroundColor(0xCC2A2A2A.toInt())
                    addView(
                        label,
                        FrameLayout.LayoutParams(
                            ViewGroup.LayoutParams.MATCH_PARENT,
                            ViewGroup.LayoutParams.MATCH_PARENT
                        )
                    )
                    setOnClickListener {
                        if (!boolProp(item.props, "disabled")) {
                            eventSink(authorId, "press", mapOf("source" to "pointer"))
                        }
                    }
                }
            }
            "slider" -> {
                val seek = SeekBar(host.context).apply {
                    max = 1000
                    setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
                        override fun onProgressChanged(bar: SeekBar?, progress: Int, fromUser: Boolean) {
                            if (!fromUser || !item.dragging) return
                            eventSink(authorId, "valuechange", mapOf("value" to sliderValue(item, progress)))
                        }
                        override fun onStartTrackingTouch(bar: SeekBar?) {
                            item.dragging = true
                        }
                        override fun onStopTrackingTouch(bar: SeekBar?) {
                            item.dragging = false
                            eventSink(
                                authorId,
                                "valuecommit",
                                mapOf("value" to sliderValue(item, bar?.progress ?: 0))
                            )
                        }
                    })
                }
                item.seek = seek
                item.view = seek
            }
            else -> {
                item.view = FrameLayout(host.context).apply { isClickable = false }
            }
        }
        applyProps(item)
        return item
    }

    private fun applyProps(node: IslandNode) {
        when (node.kind) {
            "video" -> node.video?.update(node.props)
            "text" -> (node.view as? TextView)?.text = node.props["text"] as? String ?: ""
            "tappable" -> {
                val content = asMap(node.props["content"])
                node.label?.text = (content?.get("text") as? String)
                    ?: (content?.let { asMap(it["icon"]) }?.get("name") as? String)
                    ?: (node.props["label"] as? String)
                    ?: (node.props["icon"] as? String)
                    ?: ""
                node.view.isEnabled = !boolProp(node.props, "disabled")
            }
            "slider" -> applySlider(node)
            "view" -> applyScrim(node)
        }
        applyPointerEvents(node)
    }

    private fun applySlider(node: IslandNode) {
        val seek = node.seek ?: return
        if (node.dragging) return
        val min = number(node.props["min"])
        val max = number(node.props["max"]).let { if (it <= min) min + 1.0 else it }
        val value = number(node.props["value"]).coerceIn(min, max)
        val t = ((value - min) / (max - min)).coerceIn(0.0, 1.0)
        seek.progress = (t * 1000.0).roundToInt()
        seek.isEnabled = !boolProp(node.props, "disabled")
    }

    private fun sliderValue(node: IslandNode, progress: Int): Double {
        val min = number(node.props["min"])
        val max = number(node.props["max"]).let { if (it <= min) min + 1.0 else it }
        val step = number(node.props["step"])
        val raw = min + (progress / 1000.0) * (max - min)
        val snapped = if (step > 0) min + kotlin.math.round((raw - min) / step) * step else raw
        return snapped.coerceIn(min, max)
    }

    private fun applyScrim(node: IslandNode) {
        val view = node.view
        val paint = asMap(node.props["scrimPaint"]) ?: return
        val scrim = paint["scrim"] as? String ?: "none"
        val opacity = number(paint["opacity"]).toFloat().coerceIn(0f, 1f)
        val alpha = (opacity * 255f).roundToInt().coerceIn(0, 255)
        view.background = when (scrim) {
            "full" -> GradientDrawable().apply { setColor(Color.argb(alpha, 0, 0, 0)) }
            "bottom" -> GradientDrawable(
                GradientDrawable.Orientation.TOP_BOTTOM,
                intArrayOf(Color.TRANSPARENT, Color.argb(alpha, 0, 0, 0))
            )
            "top" -> GradientDrawable(
                GradientDrawable.Orientation.BOTTOM_TOP,
                intArrayOf(Color.TRANSPARENT, Color.argb(alpha, 0, 0, 0))
            )
            else -> null
        }
    }

    private fun applyPointerEvents(node: IslandNode) {
        val mode = node.props["pointerEvents"] as? String
            ?: if (node.kind == "text" || node.kind == "view") "box-none" else "auto"
        val interactive = node.kind == "tappable" || node.kind == "slider" || node.kind == "video"
        node.view.isClickable = interactive && mode != "none"
        if (mode == "none" || mode == "box-none") {
            if (!interactive) {
                node.view.isClickable = false
            }
        }
    }

    private fun applyFrame(node: IslandNode) {
        val left = (node.rectX * density - scrollXPx).roundToInt()
        val top = (node.rectY * density - scrollYPx).roundToInt()
        val width = (node.rectW * density).roundToInt().coerceAtLeast(1)
        val height = (node.rectH * density).roundToInt().coerceAtLeast(1)
        val params = (node.view.layoutParams as? FrameLayout.LayoutParams)
            ?: FrameLayout.LayoutParams(width, height)
        params.width = width
        params.height = height
        params.leftMargin = left
        params.topMargin = top
        node.view.layoutParams = params
        node.view.visibility = if (node.visible && node.rectW > 0 && node.rectH > 0) View.VISIBLE else View.GONE
        node.video?.setFrame(
            android.graphics.RectF(
                left.toFloat(),
                top.toFloat(),
                (left + width).toFloat(),
                (top + height).toFloat()
            )
        )
    }

    private fun restack() {
        val ordered = nodes.values.sortedWith(compareBy({ it.order }, { it.key }))
        ordered.forEachIndexed { index, node ->
            container.bringChildToFront(node.view)
            node.view.z = index.toFloat()
        }
    }

    private fun removeNode(key: String) {
        val node = nodes.remove(key) ?: return
        node.video?.unmount()
        container.removeView(node.view)
    }

    private fun nodeKey(node: Map<String, Any?>): String? {
        return (asMap(node["ref"])?.get("nodeKey") as? String) ?: node["nodeKey"] as? String
    }

    private class IslandNode(
        val key: String,
        val kind: String,
        val authorId: String,
        var order: Int,
        var props: Map<String, Any?>,
        var view: View,
        var video: VideoComponent? = null,
        var label: TextView? = null,
        var seek: SeekBar? = null,
        var rectX: Double = 0.0,
        var rectY: Double = 0.0,
        var rectW: Double = 0.0,
        var rectH: Double = 0.0,
        var visible: Boolean = true,
        var dragging: Boolean = false
    )

    companion object {
        val ALLOWED_KINDS = setOf("root", "view", "text", "tappable", "slider", "video")
    }
}

internal fun asMap(value: Any?): Map<String, Any?>? {
    return when (value) {
        is Map<*, *> -> value.entries.associate { it.key.toString() to it.value }
        else -> null
    }
}

private fun number(value: Any?): Double {
    return when (value) {
        is Number -> value.toDouble()
        is String -> value.toDoubleOrNull() ?: 0.0
        else -> 0.0
    }
}

private fun boolProp(props: Map<String, Any?>, key: String): Boolean {
    return when (val value = props[key]) {
        is Boolean -> value
        is String -> value == "true" || value == "1"
        is Number -> value.toDouble() != 0.0
        else -> false
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
