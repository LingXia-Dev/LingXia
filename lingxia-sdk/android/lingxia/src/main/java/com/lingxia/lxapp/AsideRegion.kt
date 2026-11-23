package com.lingxia.lxapp

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Path
import android.graphics.drawable.GradientDrawable
import android.text.TextUtils
import android.util.TypedValue
import android.view.GestureDetector
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.HorizontalScrollView
import android.widget.ImageButton
import android.widget.LinearLayout
import android.widget.PopupMenu
import android.widget.TextView
import com.lingxia.app.Lingxia
import com.lingxia.app.NativeApi
import com.lingxia.lxapp.chrome.OverlayPalette
import org.json.JSONObject

/**
 * The side panel's lxapps, as the core's plan places them: docked beside the main on a large
 * screen, over it otherwise. Each child lxapp is an [LxAppPane]; a tab strip switches and
 * closes them, a docked slot's divider drags its width, and a collapsed slot leaves a tongue
 * at the edge.
 */
internal class AsideRegion(private val activity: LxAppActivity, private val host: FrameLayout) {
    /** The core's `panel` aside slot. */
    data class Slot(
        val children: List<String>,
        val active: String?,
        val visible: Boolean,
        val overlay: Boolean,
        val collapsed: Boolean,
        val maximized: Boolean,
    ) {
        companion object {
            fun parse(plan: JSONObject): Slot? {
                val slots = plan.optJSONArray("asideSlots") ?: return null
                for (index in 0 until slots.length()) {
                    val slot = slots.optJSONObject(index) ?: continue
                    if (slot.optString("kind") != "panel") continue
                    val array = slot.optJSONArray("children")
                    val children = (0 until (array?.length() ?: 0)).mapNotNull { array?.optString(it) }
                    return Slot(
                        children,
                        slot.optString("activeChild").takeIf { it.isNotEmpty() && !slot.isNull("activeChild") },
                        slot.optBoolean("visible"),
                        slot.optBoolean("overlay"),
                        slot.optBoolean("collapsed"),
                        slot.optBoolean("maximized"),
                    )
                }
                return null
            }
        }
    }

    private val density = activity.resources.displayMetrics.density
    private fun dp(value: Float) = (value * density).toInt()

    val panes = LinkedHashMap<String, LxAppPane>()
    /** Non-lxapp children, such as a URL surface: a view the surface layer owns and closes. */
    private class Foreign(val view: View, val title: () -> String, val close: () -> Unit)
    private val foreign = LinkedHashMap<String, Foreign>()
    private fun hosts(id: String?) = id != null && (panes.containsKey(id) || foreign.containsKey(id))
    /** Panes a plan has listed: one missing from a later plan was taken out of the slot. */
    private val placed = HashSet<String>()
    var slot: Slot? = null
        private set

    private val tabs = LinearLayout(activity).apply { orientation = LinearLayout.HORIZONTAL }
    private val tabScroll = HorizontalScrollView(activity).apply {
        isHorizontalScrollBarEnabled = false
        addView(tabs)
    }
    private val leading = icon(R.drawable.icon_back, R.string.lx_aside_collapse) { collapse() }
    private val maximize = icon(R.drawable.icon_sidebar_expand, R.string.lx_aside_maximize) { toggleMaximized() }
    private val trailing = icon(R.drawable.icon_aside_collapse, R.string.lx_aside_collapse) { collapse() }
    private val strip = LinearLayout(activity).apply {
        orientation = LinearLayout.HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL
        addView(leading)
        addView(tabScroll, LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.MATCH_PARENT, 1f))
        addView(maximize)
        addView(trailing)
    }
    /** The docked share of the window; dragging the divider sets it. */
    private var ratio = activity.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        .getFloat(PREF_RATIO, DEFAULT_RATIO)
    private val divider = DividerView(activity)
    private val paneHost = FrameLayout(activity)
    val view = LinearLayout(activity).apply {
        orientation = LinearLayout.VERTICAL
        visibility = View.GONE
        isClickable = true
        addView(strip, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, dp(STRIP_DP)))
        addView(paneHost, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f))
    }
    /** The tongue a collapsed slot leaves at the edge; a tap or a pull brings the slot back. */
    private val ear = TongueView(activity).apply {
        visibility = View.GONE
        onRestore = ::restore
    }

    init {
        host.addView(view, FrameLayout.LayoutParams(0, ViewGroup.LayoutParams.MATCH_PARENT))
        host.addView(divider, FrameLayout.LayoutParams(dp(DIVIDER_HIT_DP), ViewGroup.LayoutParams.MATCH_PARENT))
        host.addView(ear, FrameLayout.LayoutParams(
            dp(TONGUE_HIT_W_DP), dp(TONGUE_HIT_H_DP), Gravity.END or Gravity.CENTER_VERTICAL))
        divider.visibility = View.GONE
    }

    /** Whether a large screen keeps the slot beside the main; a phone shows it over the main. */
    private fun canDock(): Boolean =
        activity.resources.configuration.smallestScreenWidthDp >= com.lingxia.view.GpuFoldAdapter.COMPACT_MAX_DP

    private fun showing(): Boolean = !activity.isMediaFullscreen &&
        slot?.let { it.visible && !it.collapsed && it.active != null } == true &&
        hosts(slot?.active)

    /** Shown over the main: a phone's slot, or a maximized one. */
    val overlay: Boolean get() = showing() && (slot!!.overlay || slot!!.maximized || !canDock())

    val docked: Boolean get() = showing() && !overlay

    /** The docked width: [ratio] of the window, leaving each side at least [SIDE_MIN_DP]. */
    fun dockedWidth(): Int = clampWidth((windowWidth() * ratio).toInt())

    private fun windowWidth() = host.width - host.paddingLeft - host.paddingRight

    private fun clampWidth(width: Int): Int {
        val window = windowWidth()
        val min = dp(SIDE_MIN_DP)
        if (window < 2 * min) return window / 2
        return width.coerceIn(min, window - min)
    }

    /** A drag puts the divider at [x]; it snaps to the middle near it. */
    private fun dragDividerTo(x: Float) {
        val window = windowWidth()
        if (window <= 0) return
        var width = clampWidth((window - x).toInt())
        if (kotlin.math.abs(width - window / 2) < dp(SNAP_DP)) width = window / 2
        ratio = width.toFloat() / window
        layout()
    }

    private fun saveRatio() {
        activity.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().putFloat(PREF_RATIO, ratio).apply()
    }

    fun open(appId: String, sessionId: Long) {
        if (panes.containsKey(appId)) return
        val root = FrameLayout(activity)
        val pane = LxAppPane(activity, root, isMain = false)
        pane.appId = appId
        pane.currentSessionId = sessionId
        pane.setupWebViewContainer()
        pane.createNavBar()
        pane.observeGpuChromeInsets()
        root.visibility = View.GONE
        paneHost.addView(root, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        panes[appId] = pane
        pane.openLxApp(appId, sessionId)
        apply(slot)
    }

    /** Takes [appId]'s pane out of the region; its pages stay alive in the runtime. */
    fun drop(appId: String): LxAppPane? {
        val pane = panes.remove(appId) ?: return null
        placed.remove(appId)
        pane.detach()
        apply(slot)
        return pane
    }

    /** Hosts a surface's [view] as the child [id]; the core's next plan places it. */
    fun adopt(id: String, view: View, title: () -> String, close: () -> Unit) {
        release(id)
        view.visibility = View.GONE
        paneHost.addView(view, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        foreign[id] = Foreign(view, title, close)
        apply(slot)
    }

    /** Lets go of a surface child; the surface layer tears its view down. */
    fun release(id: String) {
        val child = foreign.remove(id) ?: return
        placed.remove(id)
        (child.view.parent as? ViewGroup)?.removeView(child.view)
        apply(slot)
    }

    /** The activity left or came back; only the shown pane's page was running. */
    fun onHostPaused(paused: Boolean) {
        val pane = slot?.active?.let(panes::get)?.takeIf { it.root.visibility == View.VISIBLE } ?: return
        if (paused) pane.currentWebView?.pause() else pane.currentWebView?.resume()
    }

    fun apply(next: Slot?) {
        slot = next
        val children = next?.children.orEmpty()
        next?.let { placed.addAll(children.filter(::hosts)) }
        // The surface layer closes a surface child the plan dropped.
        (placed - children.toSet()).forEach { if (panes.containsKey(it)) drop(it) else release(it) }
        val active = next?.active
        foreign.forEach { (id, child) ->
            child.view.visibility = if (id == active && showing()) View.VISIBLE else View.GONE
        }
        panes.forEach { (id, pane) ->
            val shown = id == active && showing()
            if (shown && pane.root.visibility != View.VISIBLE) {
                pane.root.visibility = View.VISIBLE
                pane.currentWebView?.resume()
            } else if (!shown && pane.root.visibility == View.VISIBLE) {
                pane.root.visibility = View.GONE
                pane.currentWebView?.pause()
            }
        }
        bindTabs()
        layout()
    }

    /** Places the region, and the ear of a collapsed slot, for the window as it is now. */
    fun layout() {
        val palette = OverlayPalette.of(activity)
        val status = LxAppActivity.getStatusBarHeight(activity)
        val shown = showing()
        val over = overlay
        view.visibility = if (shown) View.VISIBLE else View.GONE
        (view.layoutParams as FrameLayout.LayoutParams).apply {
            width = if (over) ViewGroup.LayoutParams.MATCH_PARENT else dockedWidth()
            gravity = Gravity.END
            topMargin = 0
            view.layoutParams = this
        }
        // The region starts below the status bar; docked, the bar over it continues the main's band.
        view.setPadding(0, status, 0, 0)
        view.setBackgroundColor(if (over) palette.surface else activity.mainPane.statusBandColor())
        view.elevation = if (over) dp(8f).toFloat() else 0f
        strip.setBackgroundColor(palette.surface)
        leading.visibility = if (over) View.VISIBLE else View.GONE
        trailing.visibility = if (over) View.GONE else View.VISIBLE
        maximize.visibility = if (canDock() && slot?.overlay != true || slot?.maximized == true) View.VISIBLE else View.GONE
        maximize.setImageResource(if (slot?.maximized == true) R.drawable.icon_sidebar_collapse else R.drawable.icon_sidebar_expand)
        listOf(leading, maximize, trailing).forEach { it.setColorFilter(palette.body) }

        (divider.layoutParams as FrameLayout.LayoutParams).apply {
            gravity = Gravity.END
            topMargin = status
            // The hit area straddles the boundary; its line sits on it.
            rightMargin = dockedWidth() - dp(DIVIDER_HIT_DP) / 2
            divider.layoutParams = this
        }
        divider.lineColor = palette.separator
        divider.gripColor = palette.secondaryText
        divider.visibility = if (shown && !over) View.VISIBLE else View.GONE

        val title = slot?.let { (it.active ?: it.children.lastOrNull()) }?.let(::titleOf)
        val earShown = !shown && slot?.active != null && (slot?.collapsed == true || slot?.visible == false)
        ear.visibility = if (earShown) View.VISIBLE else View.GONE
        if (earShown) {
            ear.contentDescription = String.format(
                Lingxia.localizedString(activity, R.string.lx_aside_show_content), title.orEmpty())
            ear.color = palette.body
            ear.fill = palette.surface
            ear.stroke = palette.separator
            // Beside a tab column, not over it.
            (ear.layoutParams as FrameLayout.LayoutParams).apply {
                rightMargin = activity.mainPane.tabColumnWidth()
                ear.layoutParams = this
            }
        }
        listOf(view, divider, ear).forEach { host.bringChildToFront(it) }
        activity.onAsideLayoutChanged()
    }

    private fun bindTabs() {
        val palette = OverlayPalette.of(activity)
        tabs.removeAllViews()
        val active = slot?.active
        slot?.children.orEmpty().filter(::hosts).forEach { id ->
            val selected = id == active
            val tab = LinearLayout(activity).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
                setPadding(dp(12f), 0, dp(4f), 0)
                background = GradientDrawable().apply {
                    cornerRadius = dp(8f).toFloat()
                    setColor(if (selected) palette.secondaryFill else Color.TRANSPARENT)
                }
                setOnClickListener { if (!selected) command("focus", id) }
                setOnLongClickListener { showTabMenu(this, id); true }
            }
            tab.addView(TextView(activity).apply {
                text = titleOf(id)
                maxLines = 1
                ellipsize = TextUtils.TruncateAt.END
                maxWidth = dp(120f)
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 14f)
                setTextColor(if (selected) palette.title else palette.secondaryText)
            })
            tab.addView(icon(R.drawable.icon_close_x, R.string.lx_common_close, size = 28f) { close(id) }.apply {
                setColorFilter(palette.secondaryText)
            })
            tabs.addView(tab, LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, dp(34f)).apply {
                marginStart = dp(4f)
            })
            if (selected) tabScroll.post { tabScroll.requestChildRectangleOnScreen(tabs, android.graphics.Rect(tab.left, 0, tab.right, tab.height), false) }
        }
    }

    private fun showTabMenu(anchor: View, appId: String) {
        PopupMenu(activity, anchor).apply {
            if (panes.containsKey(appId)) {
                menu.add(Lingxia.localizedString(activity, R.string.lx_aside_open_in_main)).setOnMenuItemClickListener {
                    NativeApi.asideCommand(appId, "promote", "")
                    true
                }
            }
            menu.add(Lingxia.localizedString(activity, R.string.lx_common_close)).setOnMenuItemClickListener {
                close(appId)
                true
            }
        }.show()
    }

    /** Closes [appId]: the slot lets it go and its session ends. */
    fun close(appId: String) {
        foreign[appId]?.let { it.close(); return }
        val pane = panes[appId] ?: return
        command("unregister", appId)
        pane.notifyLxAppClosed(pane.currentSessionId)
        drop(appId)
    }

    fun collapse() {
        if (slot?.maximized == true) command("unmaximize", "panel")
        command("collapse", "panel")
    }

    fun restore() {
        command("restore", "panel")
    }

    private fun toggleMaximized() {
        command(if (slot?.maximized == true) "unmaximize" else "maximize", "panel")
    }

    /** Runs [name] on the window's graph, which any lxapp of the window reaches. */
    private fun command(name: String, arg: String) {
        val graph = activity.mainPane.takeIf { it.hasAppId && it.appId.isNotBlank() }?.appId
            ?: panes.keys.firstOrNull() ?: return
        NativeApi.asideCommand(graph, name, arg)
    }

    private fun titleOf(id: String): String = foreign[id]?.title?.invoke() ?: appName(id)

    private fun appName(appId: String): String =
        NativeApi.getLxAppInfo(appId)?.appName?.takeIf { it.isNotBlank() } ?: appId

    private fun icon(drawable: Int, label: Int, size: Float = 44f, onClick: () -> Unit) =
        ImageButton(activity).apply {
            setImageResource(drawable)
            background = null
            contentDescription = Lingxia.localizedString(activity, label)
            scaleType = android.widget.ImageView.ScaleType.CENTER_INSIDE
            val pad = dp(size / 4.4f)
            setPadding(pad, pad, pad, pad)
            layoutParams = LinearLayout.LayoutParams(dp(size), dp(size))
            setOnClickListener { onClick() }
        }

    /** The docked divider: a hairline with a grip, dragged to size the slot. */
    @SuppressLint("ClickableViewAccessibility")
    private inner class DividerView(context: Context) : View(context) {
        var lineColor = Color.LTGRAY
            set(value) { field = value; invalidate() }
        var gripColor = Color.GRAY
            set(value) { field = value; invalidate() }
        private var dragging = false
        private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
        private val doubleTap = GestureDetector(context, object : GestureDetector.SimpleOnGestureListener() {
            override fun onDoubleTap(e: MotionEvent): Boolean {
                ratio = DEFAULT_RATIO
                saveRatio()
                layout()
                return true
            }
        })

        override fun onTouchEvent(event: MotionEvent): Boolean {
            doubleTap.onTouchEvent(event)
            when (event.actionMasked) {
                MotionEvent.ACTION_DOWN -> {
                    dragging = true
                    parent?.requestDisallowInterceptTouchEvent(true)
                    invalidate()
                }
                MotionEvent.ACTION_MOVE -> if (dragging) {
                    val location = IntArray(2)
                    host.getLocationOnScreen(location)
                    dragDividerTo(event.rawX - location[0] - host.paddingLeft)
                }
                MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                    dragging = false
                    saveRatio()
                    invalidate()
                }
            }
            return true
        }

        override fun onDraw(canvas: Canvas) {
            val cx = width / 2f
            paint.color = lineColor
            canvas.drawRect(cx - density / 2, 0f, cx + density / 2, height.toFloat(), paint)
            paint.color = gripColor
            paint.alpha = if (dragging) 200 else 110
            val w = dp(4f).toFloat()
            val h = dp(36f).toFloat()
            val cy = height / 2f
            canvas.drawRoundRect(cx - w / 2, cy - h / 2, cx + w / 2, cy + h / 2, w / 2, w / 2, paint)
        }
    }

    /** The desktop's collapsed-slot tongue: a trapezoid whose long side lies on the edge. */
    @SuppressLint("ClickableViewAccessibility")
    private inner class TongueView(context: Context) : View(context) {
        var onRestore: (() -> Unit)? = null
        var color = Color.GRAY
            set(value) { field = value; invalidate() }
        var fill = Color.WHITE
            set(value) { field = value; invalidate() }
        var stroke = Color.LTGRAY
            set(value) { field = value; invalidate() }
        private var pressed = false
        private var downX = 0f
        private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
        private val path = Path()

        init {
            isClickable = true
            setLayerType(LAYER_TYPE_SOFTWARE, null)
            importantForAccessibility = IMPORTANT_FOR_ACCESSIBILITY_YES
            setOnClickListener { onRestore?.invoke() }
        }

        override fun onTouchEvent(event: MotionEvent): Boolean {
            when (event.actionMasked) {
                MotionEvent.ACTION_DOWN -> { pressed = true; downX = event.x; invalidate() }
                MotionEvent.ACTION_MOVE -> if (pressed && downX - event.x > dp(PULL_DP)) {
                    // Pulled toward the main: the slot comes out.
                    pressed = false
                    invalidate()
                    onRestore?.invoke()
                    return true
                }
                MotionEvent.ACTION_UP -> if (pressed) { pressed = false; invalidate(); performClick() }
                MotionEvent.ACTION_CANCEL -> { pressed = false; invalidate() }
            }
            return true
        }

        override fun onDraw(canvas: Canvas) {
            val outer = width.toFloat()
            val inner = outer - dp(TONGUE_W_DP)
            val top = (height - dp(TONGUE_H_DP)) / 2f
            val bottom = top + dp(TONGUE_H_DP)
            val slope = dp(TONGUE_SLOPE_DP).toFloat()
            path.reset()
            path.moveTo(outer, top)
            path.lineTo(inner, top + slope)
            path.lineTo(inner, bottom - slope)
            path.lineTo(outer, bottom)
            path.close()
            // Over arbitrary page content it needs its own ground to read.
            paint.style = Paint.Style.FILL
            paint.color = fill
            paint.setShadowLayer(dp(3f).toFloat(), 0f, 0f, Color.argb(48, 0, 0, 0))
            canvas.drawPath(path, paint)
            paint.clearShadowLayer()
            paint.style = Paint.Style.STROKE
            paint.strokeWidth = density
            paint.color = stroke
            canvas.drawPath(path, paint)
            paint.style = Paint.Style.FILL
            paint.color = color
            paint.alpha = if (pressed) 200 else 120
            val gx = (inner + outer) / 2f
            val gh = dp(TONGUE_H_DP - 2 * TONGUE_SLOPE_DP - 16f) / 2f
            val gw = density * 1.5f
            canvas.drawRoundRect(gx - gw, height / 2f - gh, gx + gw, height / 2f + gh, gw, gw, paint)
        }
    }

    companion object {
        private const val STRIP_DP = 44f
        private const val SIDE_MIN_DP = 320f
        private const val DEFAULT_RATIO = 0.5f
        private const val SNAP_DP = 24f
        private const val DIVIDER_HIT_DP = 24f
        private const val TONGUE_W_DP = 12f
        private const val TONGUE_H_DP = 56f
        private const val TONGUE_SLOPE_DP = 8f
        private const val PULL_DP = 16f
        private const val TONGUE_HIT_W_DP = 28f
        private const val TONGUE_HIT_H_DP = 88f
        private const val PREFS = "lingxia_aside"
        private const val PREF_RATIO = "dock_ratio"
    }
}
