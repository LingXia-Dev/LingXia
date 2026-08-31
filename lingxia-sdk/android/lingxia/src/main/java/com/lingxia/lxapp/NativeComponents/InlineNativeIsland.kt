package com.lingxia.lxapp.NativeComponents

import android.graphics.Color
import android.graphics.Rect
import android.graphics.Typeface
import android.content.res.ColorStateList
import android.graphics.drawable.GradientDrawable
import android.os.Build
import android.text.TextUtils
import android.view.Gravity
import android.view.MotionEvent
import android.view.TouchDelegate
import android.view.View
import android.view.ViewGroup
import android.view.accessibility.AccessibilityNodeInfo
import android.widget.FrameLayout
import android.widget.SeekBar
import android.widget.TextView
import com.lingxia.lxapp.NativeComponents.Components.VideoComponent
import com.lingxia.app.NativeApi
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
    private val appId: String,
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
    private val touchDelegates = CompositeTouchDelegate(container).also { container.touchDelegate = it }
    private val revisions = mutableMapOf<String, Long>()
    private val roots = mutableMapOf<String, Map<String, Any?>>()
    private val rootOrders = mutableMapOf<String, Int>()
    private var scrollXPx = 0
    private var scrollYPx = 0
    private data class RootLease(
        val root: Map<String, Any?>,
        val id: String,
        val sequence: Long = 1,
        var active: Boolean = false
    )
    private val leases = mutableMapOf<String, RootLease>()
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
            "root.destroy" -> {
                destroyRoot(message)
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

    fun lastAppliedRevision(): Long = revisions.values.maxOrNull() ?: 0L

    fun videoComponent(componentId: String): VideoComponent? =
        nodes.values.firstOrNull { it.authorId == componentId }?.video

    fun videoIds(): Set<String> = nodes.values.filter { it.video != null }.map { it.authorId }.toSet()

    fun teardown() {
        nodes.keys.toList().forEach(::removeNode)
        (container.parent as? ViewGroup)?.removeView(container)
        pendingOutgoing.clear()
        revisions.clear()
        roots.clear()
        rootOrders.clear()
        leases.clear()
    }

    private fun applyCommit(message: Map<String, Any?>) {
        val root = asMap(message["root"]) ?: return
        val rootKey = root["rootKey"] as? String ?: return
        val base = (message["baseRevision"] as? Number)?.toLong() ?: return
        val revision = (message["revision"] as? Number)?.toLong() ?: return
        if (revision <= base || revision == 0L) {
            pendingOutgoing += mapOf(
                "action" to "root.error",
                "id" to rootKey,
                "root" to root,
                "message" to "commit revision must be greater than baseRevision",
                "recoverable" to true
            )
            return
        }
        val policyError = NativeApi.validateInlineNativeMediaUrls(
            appId,
            JSONArray(mediaUrlsFromCommit(message)).toString()
        )
        if (policyError != null) {
            pendingOutgoing += mapOf(
                "action" to "root.error",
                "id" to rootKey,
                "root" to root,
                "message" to policyError,
                "recoverable" to true
            )
            return
        }
        val last = revisions[rootKey] ?: 0L
        val existingRoot = roots[rootKey]
        if (existingRoot != null && !sameRootGeneration(existingRoot, root)) {
            pendingOutgoing += mapOf(
                "action" to "root.error",
                "id" to rootKey,
                "root" to root,
                "message" to "root generation changed without destroy",
                "recoverable" to true
            )
            return
        }
        if (base == 0L) {
            nodes.values.filter { it.rootKey == rootKey }.map { it.key }.forEach(::removeNode)
            revisions.remove(rootKey)
            leases.remove(rootKey)
        } else if (base != last) {
            pendingOutgoing += mapOf(
                "action" to "root.resyncRequired",
                "id" to rootKey,
                "root" to root,
                "lastAppliedRevision" to last
            )
            return
        }
        val operations = message["operations"] as? List<*> ?: return
        for (raw in operations) {
            val op = asMap(raw) ?: continue
            when (op["op"] as? String) {
                "mount" -> mount(asMap(op["node"]) ?: continue, rootKey)
                "update" -> update(op)
                "unmount" -> {
                    val node = asMap(op["node"]) ?: continue
                    if ((asMap(node["ref"]) ?: node)["rootKey"] != rootKey) continue
                    val key = nodeKey(node) ?: continue
                    removeNode(key)
                }
                "reorder" -> {
                    val node = asMap(op["node"]) ?: continue
                    if ((asMap(node["ref"]) ?: node)["rootKey"] != rootKey) continue
                    val key = nodeKey(node) ?: continue
                    val order = (op["order"] as? Number)?.toInt() ?: continue
                    nodes[key]?.order = order
                }
                "reparent" -> {
                    val node = asMap(op["node"]) ?: continue
                    if ((asMap(node["ref"]) ?: node)["rootKey"] != rootKey) continue
                    val key = nodeKey(node) ?: continue
                    val parent = asMap(op["parent"])
                    if (parent != null && parent["rootKey"] != rootKey) continue
                    nodes[key]?.parentKey = parent?.get("nodeKey") as? String
                }
            }
        }
        restack()
        revisions[rootKey] = revision
        roots[rootKey] = root
        pendingOutgoing += mapOf(
            "action" to "root.applied",
            "id" to rootKey,
            "root" to root,
            "revision" to revision
        )
        grantLeaseIfNeeded(root)
    }

    private fun destroyRoot(message: Map<String, Any?>) {
        val root = asMap(message["root"]) ?: return
        val rootKey = root["rootKey"] as? String ?: return
        val current = roots[rootKey] ?: return
        if (!sameRootGeneration(current, root)) return
        nodes.values.filter { it.rootKey == rootKey }.map { it.key }.forEach(::removeNode)
        revisions.remove(rootKey)
        roots.remove(rootKey)
        rootOrders.remove(rootKey)
        leases.remove(rootKey)
        restack()
    }

    private fun applyGeometry(message: Map<String, Any?>) {
        (message["roots"] as? List<*>)?.forEach { raw ->
            val entry = asMap(raw) ?: return@forEach
            val ref = asMap(entry["ref"]) ?: return@forEach
            val rootKey = ref["rootKey"] as? String ?: return@forEach
            if (roots[rootKey]?.let { sameRootGeneration(it, ref) } == true) {
                rootOrders[rootKey] = (entry["rootOrder"] as? Number)?.toInt() ?: Int.MAX_VALUE
            }
        }
        val entries = message["nodes"] as? List<*> ?: return
        for (raw in entries) {
            val entry = asMap(raw) ?: continue
            val ref = asMap(entry["ref"]) ?: continue
            val key = ref["nodeKey"] as? String ?: continue
            val node = nodes[key] ?: continue
            if ((ref["rootKey"] as? String) != node.rootKey) continue
            val rect = asMap(entry["contentRect"]) ?: continue
            node.rectX = number(rect["x"])
            node.rectY = number(rect["y"])
            node.rectW = number(rect["width"])
            node.rectH = number(rect["height"])
            node.visible = entry["visible"] as? Boolean ?: true
            applyFrame(node)
        }
        restack()
    }

    private fun acceptLease(message: Map<String, Any?>) {
        val rootKey = (message["id"] as? String)
            ?: asMap(message["root"])?.get("rootKey") as? String
            ?: return
        val lease = leases[rootKey] ?: return
        if (lease.active) return
        val incomingId = message["leaseId"] as? String ?: ""
        val incomingSeq = (message["sequence"] as? Number)?.toLong() ?: 0L
        if (incomingId.isNotEmpty() && incomingId != lease.id) return
        if (incomingSeq > 0 && incomingSeq != lease.sequence) return
        lease.active = true
        pendingOutgoing += mapOf(
            "action" to "root.leaseActive",
            "id" to rootKey,
            "root" to lease.root,
            "leaseId" to lease.id,
            "sequence" to lease.sequence
        )
    }

    private fun grantLeaseIfNeeded(root: Map<String, Any?>) {
        val rootKey = root["rootKey"] as? String ?: "island"
        if (leases.containsKey(rootKey)) return
        val lease = RootLease(root, "lease-$rootKey")
        leases[rootKey] = lease
        pendingOutgoing += mapOf(
            "action" to "root.leaseGranted",
            "id" to rootKey,
            "root" to root,
            "leaseId" to lease.id,
            "sequence" to lease.sequence,
            "leaseDurationMs" to 8000
        )
    }

    private fun applyVideoCommand(message: Map<String, Any?>) {
        val owner = asMap(message["owner"])
        val key = owner?.get("nodeKey") as? String
        val node = key?.let { nodes[it] } ?: nodes.values.firstOrNull { it.kind == "video" }
        val video = node?.video ?: return
        val command = asMap(message["command"]) ?: return
        val policyError = NativeApi.validateInlineNativeMediaUrls(
            appId,
            JSONArray(mediaUrlsFromValue(command)).toString()
        )
        if (policyError != null) return
        val name = command["name"] as? String ?: return
        val params = when (name) {
            "seek" -> mapOf("time" to number(command["seconds"]))
            "setStreamSource" -> asMap(command["options"]) ?: emptyMap()
            else -> emptyMap()
        }
        video.handleCommand(name, params)
    }

    private fun mount(node: Map<*, *>, expectedRootKey: String) {
        val ref = asMap(node["ref"]) ?: return
        val key = ref["nodeKey"] as? String ?: return
        val rootKey = ref["rootKey"] as? String ?: return
        if (rootKey != expectedRootKey) return
        val kind = node["kind"] as? String ?: return
        if (kind !in ALLOWED_KINDS) return
        val authorId = (node["authorId"] as? String)?.takeIf { it.isNotEmpty() } ?: key
        val automationId = (node["automationId"] as? String)?.takeIf { it.isNotEmpty() }
        val props = asMap(node["props"]) ?: emptyMap()
        val order = (node["order"] as? Number)?.toInt() ?: 0
        val parentKey = asMap(node["parent"])?.get("nodeKey") as? String
        if (asMap(node["parent"])?.get("rootKey")?.let { it != rootKey } == true) return
        if (nodes[key]?.rootKey?.let { it != rootKey } == true) return
        if (nodes.containsKey(key)) {
            removeNode(key)
        }
        val item = factoryNode(key, rootKey, kind, authorId, automationId, parentKey, order, props)
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
        if ((ref["rootKey"] as? String) != existing.rootKey) return
        val patch = asMap(op["patch"]) ?: return
        existing.props = existing.props + patch
        applyProps(existing)
    }

    private fun factoryNode(
        key: String,
        rootKey: String,
        kind: String,
        authorId: String,
        automationId: String?,
        parentKey: String?,
        order: Int,
        props: Map<String, Any?>
    ): IslandNode {
        val item = IslandNode(
            key,
            rootKey,
            kind,
            authorId,
            automationId,
            parentKey,
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
                    isClickable = false
                    includeFontPadding = false
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
                            val source = if (isInTouchMode) "pointer" else "keyboard"
                            eventSink(authorId, "press", mapOf("source" to source))
                        }
                    }
                    isFocusable = true
                }
                installInteractiveEvents(item.view, authorId)
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
                installInteractiveEvents(item.view, authorId)
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
            "text" -> applyText(node)
            "tappable" -> applyButton(node)
            "slider" -> applySlider(node)
            "view" -> applyScrim(node)
        }
        applyNativeStyle(node)
        applyAccessibility(node)
        applyPointerEvents(node)
    }

    private fun applyText(node: IslandNode) {
        val text = node.view as? TextView ?: return
        text.text = node.props["text"]?.toString().orEmpty()
        text.setTextColor(readColor(node.props, "color", Color.WHITE))
        text.textSize = cssNumber(node.props["fontSize"], 12f)
        text.typeface = Typeface.create(
            Typeface.DEFAULT,
            if (fontWeight(node.props["fontWeight"]) >= 600) Typeface.BOLD else Typeface.NORMAL
        )
        val maxLines = number(node.props["maxLines"]).roundToInt()
        text.maxLines = if (maxLines > 0) maxLines else Integer.MAX_VALUE
        text.ellipsize = if (maxLines > 0) TextUtils.TruncateAt.END else null
        text.gravity = when (node.props["textAlign"]?.toString()) {
            "center" -> Gravity.CENTER_HORIZONTAL or Gravity.CENTER_VERTICAL
            "end" -> Gravity.END or Gravity.CENTER_VERTICAL
            else -> Gravity.START or Gravity.CENTER_VERTICAL
        }
        text.textDirection = when (node.props["dir"]?.toString()) {
            "rtl" -> View.TEXT_DIRECTION_RTL
            "ltr" -> View.TEXT_DIRECTION_LTR
            else -> View.TEXT_DIRECTION_FIRST_STRONG
        }
        val lineHeight = cssNumber(node.props["lineHeight"], 0f)
        if (lineHeight > 0f) {
            text.setLineSpacing((lineHeight - text.textSize / density).coerceAtLeast(0f) * density, 1f)
        } else {
            text.setLineSpacing(0f, 1f)
        }
    }

    private fun applyButton(node: IslandNode) {
        val content = asMap(node.props["content"])
        val text = content?.get("text")?.toString()
            ?: node.props["label"]?.toString()
            ?: ""
        val icon = content?.let { asMap(it["icon"]) }?.get("name")?.toString()
            ?: node.props["icon"]?.toString()
        val loading = boolProp(node.props, "loading")
        val iconText = semanticIcon(icon)
        val iconPosition = node.props["iconPosition"]?.toString() ?: "start"
        node.label?.text = when {
            loading -> "…"
            iconText.isNullOrEmpty() -> text
            text.isEmpty() -> iconText
            iconPosition == "end" -> "$text  $iconText"
            else -> "$iconText  $text"
        }
        val disabled = boolProp(node.props, "disabled")
        val pressed = boolProp(node.props, "pressed")
        node.view.isEnabled = !disabled && !loading
        node.view.isSelected = pressed
        node.view.isActivated = boolProp(node.props, "expanded")

        val intent = node.props["intent"]?.toString() ?: "neutral"
        val emphasis = node.props["emphasis"]?.toString() ?: "secondary"
        val foreground = buttonForeground(intent, emphasis, disabled)
        val background = buttonBackground(intent, emphasis, pressed, disabled)
        node.label?.setTextColor(readColor(node.props, "color", foreground))
        node.label?.textSize = cssNumber(
            styleValue(node.props, "fontSize"),
            if (node.props["size"]?.toString() == "compact") 12f else 14f
        )
        node.label?.typeface = Typeface.create(Typeface.DEFAULT, Typeface.BOLD)
        node.view.background = GradientDrawable().apply {
            setColor(readStyleColor(node.props, "backgroundColor", background))
            cornerRadius = cssNumber(styleValue(node.props, "borderRadius"), 10f) * density
            val borderWidth = cssNumber(styleValue(node.props, "borderWidth"), 0f).roundToInt()
            if (borderWidth > 0) {
                setStroke(borderWidth.coerceAtLeast(1), readStyleColor(node.props, "borderColor", foreground))
            }
        }
    }

    private fun applySlider(node: IslandNode) {
        val seek = node.seek ?: return
        if (node.dragging) return
        val min = number(node.props["min"])
        val max = number(node.props["max"]).let { if (it <= min) min + 1.0 else it }
        val value = number(node.props["value"]).coerceIn(min, max)
        val t = ((value - min) / (max - min)).coerceIn(0.0, 1.0)
        seek.progress = (t * 1000.0).roundToInt()
        val buffered = number(node.props["bufferedValue"]).coerceIn(min, max)
        seek.secondaryProgress = (((buffered - min) / (max - min)).coerceIn(0.0, 1.0) * 1000.0).roundToInt()
        seek.isEnabled = !boolProp(node.props, "disabled")
        val accent = readStyleColor(node.props, "accentColor", 0xFF3B82F6.toInt())
        seek.progressTintList = ColorStateList.valueOf(accent)
        seek.thumbTintList = ColorStateList.valueOf(accent)
        seek.secondaryProgressTintList = ColorStateList.valueOf(withAlpha(accent, 112))
        seek.progressBackgroundTintList = ColorStateList.valueOf(0x665F6368)
        val label = formatSliderValue(node.props, value)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            seek.stateDescription = label
        }
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

    private fun applyNativeStyle(node: IslandNode) {
        val style = asMap(node.props["nativeStyle"]) ?: return
        if (style.containsKey("opacity")) {
            node.view.alpha = number(style["opacity"]).toFloat().coerceIn(0f, 1f)
        } else {
            node.view.alpha = 1f
        }
        if ((node.kind == "view" || node.kind == "tappable") && style["backgroundColor"] != null) {
            val background = parseCssColor(style["backgroundColor"], Color.TRANSPARENT)
            val radius = cssNumber(style["borderRadius"], 0f) * density
            val width = cssNumber(style["borderWidth"], 0f).roundToInt()
            node.view.background = GradientDrawable().apply {
                setColor(background)
                cornerRadius = radius
                if (width > 0) setStroke(width, parseCssColor(style["borderColor"], Color.TRANSPARENT))
            }
        }
    }

    private fun applyAccessibility(node: IslandNode) {
        val hidden = boolProp(node.props, "aria-hidden") || boolProp(node.props, "ariaHidden")
        node.view.importantForAccessibility = when {
            hidden -> View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS
            node.kind in setOf("tappable", "slider", "video", "text") -> View.IMPORTANT_FOR_ACCESSIBILITY_YES
            else -> View.IMPORTANT_FOR_ACCESSIBILITY_AUTO
        }
        val ariaLabel = node.props["aria-label"]?.toString()
            ?: node.props["ariaLabel"]?.toString()
        node.view.contentDescription = ariaLabel?.takeIf { it.isNotBlank() }
        node.automationId?.let { node.view.tag = it }
        val description = node.props["aria-description"]?.toString()
            ?: node.props["ariaDescription"]?.toString()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            if (!description.isNullOrBlank()) {
                node.view.stateDescription = description
            } else if (node.kind != "slider") {
                node.view.stateDescription = null
            }
        }
        node.view.accessibilityDelegate = object : View.AccessibilityDelegate() {
            override fun onInitializeAccessibilityNodeInfo(host: View, info: AccessibilityNodeInfo) {
                super.onInitializeAccessibilityNodeInfo(host, info)
                info.className = when (node.kind) {
                    "tappable" -> "android.widget.Button"
                    "slider" -> "android.widget.SeekBar"
                    "text" -> "android.widget.TextView"
                    else -> info.className
                }
                info.isEnabled = node.view.isEnabled
                info.isClickable = node.kind == "tappable" && node.view.isEnabled
            }
        }
    }

    private fun installInteractiveEvents(view: View, authorId: String) {
        view.setOnFocusChangeListener { _, focused ->
            eventSink(authorId, if (focused) "focus" else "blur", mapOf("source" to "keyboard"))
        }
        view.setOnHoverListener { _, event ->
            when (event.actionMasked) {
                MotionEvent.ACTION_HOVER_ENTER -> eventSink(authorId, "pointerenter", mapOf("source" to "pointer"))
                MotionEvent.ACTION_HOVER_EXIT -> eventSink(authorId, "pointerleave", mapOf("source" to "pointer"))
            }
            false
        }
    }

    private fun applyPointerEvents(node: IslandNode) {
        val mode = node.props["pointerEvents"] as? String
            ?: if (node.kind == "text" || node.kind == "view") "box-none" else "auto"
        val interactive = node.kind == "tappable" || node.kind == "slider" || node.kind == "video"
        val acceptsPointer = mode == "auto" || mode == "box-only"
        node.view.isClickable = interactive && acceptsPointer
        if (interactive) {
            val unavailable = boolProp(node.props, "disabled") ||
                (node.kind == "tappable" && boolProp(node.props, "loading"))
            node.view.isEnabled = acceptsPointer && !unavailable
        }
    }

    private fun applyFrame(node: IslandNode) {
        val left = (node.rectX * density - scrollXPx).roundToInt()
        val top = (node.rectY * density - scrollYPx).roundToInt()
        val width = (node.rectW * density).roundToInt().coerceAtLeast(1)
        val height = (node.rectH * density).roundToInt().coerceAtLeast(1)
        node.view.visibility = if (node.visible && node.rectW > 0 && node.rectH > 0) View.VISIBLE else View.GONE
        if (node.video != null) {
            // Player positions via translationX/Y; layout margins on the same
            // view would double-offset the picture away from the CSS rect.
            node.video?.setFrame(
                android.graphics.RectF(
                    left.toFloat(),
                    top.toFloat(),
                    (left + width).toFloat(),
                    (top + height).toFloat()
                )
            )
            return
        }
        val params = (node.view.layoutParams as? FrameLayout.LayoutParams)
            ?: FrameLayout.LayoutParams(width, height)
        params.width = width
        params.height = height
        params.leftMargin = left
        params.topMargin = top
        node.view.layoutParams = params
        updateHitSlop(node)
    }

    private fun updateHitSlop(node: IslandNode) {
        val hitSlop = cssNumber(node.props["hitSlop"], 0f)
        if (hitSlop <= 0f || node.kind != "tappable") {
            touchDelegates.remove(node.key)
            return
        }
        node.view.post {
            val rect = Rect()
            node.view.getHitRect(rect)
            val extra = (hitSlop * density).roundToInt()
            rect.inset(-extra, -extra)
            touchDelegates.put(node.key, TouchDelegate(rect, node.view))
        }
    }

    private fun restack() {
        val ordered = mutableListOf<IslandNode>()
        fun append(parentKey: String?) {
            nodes.values.filter { it.parentKey == parentKey }
                .sortedWith(compareBy(
                    { if (parentKey == null) rootOrders[it.rootKey] ?: Int.MAX_VALUE else 0 },
                    { it.order },
                    { it.key }
                ))
                .forEach { node ->
                    ordered += node
                    append(node.key)
                }
        }
        append(null)
        ordered.forEachIndexed { index, node ->
            container.bringChildToFront(node.view)
            node.view.z = index.toFloat()
        }
    }

    private fun removeNode(key: String) {
        val node = nodes.remove(key) ?: return
        touchDelegates.remove(key)
        node.video?.unmount()
        container.removeView(node.view)
    }

    private fun nodeKey(node: Map<String, Any?>): String? {
        return (asMap(node["ref"])?.get("nodeKey") as? String) ?: node["nodeKey"] as? String
    }

    private fun sameRootGeneration(a: Map<String, Any?>, b: Map<String, Any?>): Boolean =
        a["surfaceInstanceId"] == b["surfaceInstanceId"] &&
            a["pageInstanceId"] == b["pageInstanceId"] &&
            a["documentInstanceId"] == b["documentInstanceId"] &&
            a["rootKey"] == b["rootKey"] &&
            (a["rootEpoch"] as? Number)?.toLong() == (b["rootEpoch"] as? Number)?.toLong()

    private fun mediaUrlsFromCommit(message: Map<String, Any?>): List<String> {
        val urls = mutableListOf<String>()
        val operations = message["operations"] as? List<*> ?: return urls
        for (raw in operations) {
            val operation = asMap(raw) ?: continue
            mediaUrlsFromValue(operation["node"]).forEach(urls::add)
            mediaUrlsFromValue(operation["patch"]).forEach(urls::add)
        }
        return urls
    }

    private fun mediaUrlsFromValue(value: Any?): List<String> {
        val urls = mutableListOf<String>()
        fun walk(current: Any?, key: String? = null) {
            when (current) {
                is Map<*, *> -> current.forEach { (childKey, child) -> walk(child, childKey?.toString()) }
                is List<*> -> current.forEach { walk(it, key) }
                is String -> if (key in setOf("src", "url", "uri", "poster")) urls += current
            }
        }
        walk(value)
        return urls
    }

    private class IslandNode(
        val key: String,
        val rootKey: String,
        val kind: String,
        val authorId: String,
        val automationId: String?,
        var parentKey: String?,
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

    private class CompositeTouchDelegate(owner: View) : TouchDelegate(Rect(), owner) {
        private val delegates = linkedMapOf<String, TouchDelegate>()

        fun put(key: String, delegate: TouchDelegate) {
            delegates[key] = delegate
        }

        fun remove(key: String) {
            delegates.remove(key)
        }

        override fun onTouchEvent(event: MotionEvent): Boolean =
            delegates.values.any { it.onTouchEvent(event) }
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

private fun styleValue(props: Map<String, Any?>, key: String): Any? =
    asMap(props["nativeStyle"])?.get(key)

private fun cssNumber(value: Any?, fallback: Float): Float {
    return when (value) {
        is Number -> value.toFloat()
        is String -> Regex("-?[0-9]+(?:\\.[0-9]+)?")
            .find(value.trim())
            ?.value
            ?.toFloatOrNull()
            ?: fallback
        else -> fallback
    }
}

private fun fontWeight(value: Any?): Int = when (value) {
    is Number -> value.toInt()
    is String -> when (value.lowercase()) {
        "bold", "bolder" -> 700
        "normal", "lighter" -> 400
        else -> value.toIntOrNull() ?: 400
    }
    else -> 400
}

private fun readColor(props: Map<String, Any?>, key: String, fallback: Int): Int {
    val direct = props[key]
    if (direct != null) return parseCssColor(direct, fallback)
    return readStyleColor(props, key, fallback)
}

private fun readStyleColor(props: Map<String, Any?>, key: String, fallback: Int): Int =
    parseCssColor(styleValue(props, key), fallback)

private fun parseCssColor(value: Any?, fallback: Int): Int {
    val raw = value?.toString()?.trim().orEmpty()
    if (raw.isEmpty()) return fallback
    val rgb = Regex("rgba?\\(([^)]+)\\)", RegexOption.IGNORE_CASE).matchEntire(raw)
    if (rgb != null) {
        val fields = rgb.groupValues[1].split(',').map { it.trim() }
        if (fields.size >= 3) {
            val red = cssColorChannel(fields[0])
            val green = cssColorChannel(fields[1])
            val blue = cssColorChannel(fields[2])
            val alpha = fields.getOrNull(3)?.toFloatOrNull()?.let {
                if (it <= 1f) (it * 255f).roundToInt() else it.roundToInt()
            } ?: 255
            return Color.argb(alpha.coerceIn(0, 255), red, green, blue)
        }
    }
    return try {
        Color.parseColor(raw)
    } catch (_: IllegalArgumentException) {
        fallback
    }
}

private fun cssColorChannel(value: String): Int {
    if (value.endsWith('%')) {
        return ((value.dropLast(1).toFloatOrNull() ?: 0f) * 2.55f).roundToInt().coerceIn(0, 255)
    }
    return (value.toFloatOrNull() ?: 0f).roundToInt().coerceIn(0, 255)
}

private fun withAlpha(color: Int, alpha: Int): Int =
    Color.argb(alpha.coerceIn(0, 255), Color.red(color), Color.green(color), Color.blue(color))

private fun buttonBackground(intent: String, emphasis: String, pressed: Boolean, disabled: Boolean): Int {
    if (emphasis == "quiet") return Color.TRANSPARENT
    if (disabled) return 0xFF9CA3AF.toInt()
    val base = when (intent) {
        "accent" -> 0xFF2563EB.toInt()
        "destructive" -> 0xFFDC2626.toInt()
        else -> 0xFF374151.toInt()
    }
    if (emphasis == "secondary") return withAlpha(base, if (pressed) 112 else 80)
    return if (pressed) withAlpha(base, 210) else base
}

private fun buttonForeground(intent: String, emphasis: String, disabled: Boolean): Int {
    if (disabled) return 0xFFE5E7EB.toInt()
    if (emphasis != "quiet") return Color.WHITE
    return when (intent) {
        "accent" -> 0xFF2563EB.toInt()
        "destructive" -> 0xFFDC2626.toInt()
        else -> 0xFF111827.toInt()
    }
}

private fun semanticIcon(name: String?): String? = when (name) {
    "close" -> "×"
    "play" -> "▶"
    "pause" -> "Ⅱ"
    "mute" -> "🔇"
    "unmute" -> "🔊"
    "fullscreen" -> "⛶"
    "more" -> "⋯"
    else -> null
}

private fun formatSliderValue(props: Map<String, Any?>, value: Double): String? {
    return when (props["valueLabel"]?.toString()) {
        "value" -> if (value % 1.0 == 0.0) value.roundToInt().toString() else "%.1f".format(value)
        "time" -> {
            val total = value.coerceAtLeast(0.0).roundToInt()
            val hours = total / 3600
            val minutes = (total % 3600) / 60
            val seconds = total % 60
            if (hours > 0) "%d:%02d:%02d".format(hours, minutes, seconds)
            else "%d:%02d".format(minutes, seconds)
        }
        else -> null
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
