package com.lingxia.lxapp.chrome

import android.app.Activity
import android.content.res.Configuration
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.view.Gravity
import android.view.KeyEvent
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import com.lingxia.app.NativeApi

/**
 * The tab items a compact strip could not fit, shown as a panel directly above
 * the bar so the "more" slot reads as an extension of it rather than a modal.
 */
internal object TabBarOverflowSheet {
    private const val COLUMNS = 5
    private const val PANEL_CORNER_RADIUS_DP = 16f
    private const val PANEL_HORIZONTAL_MARGIN_DP = 12
    private const val PANEL_BOTTOM_GAP_DP = 8
    private const val PANEL_PADDING_DP = 8
    private const val CELL_VERTICAL_PADDING_DP = 6
    private const val CELL_ICON_SIZE_DP = 24
    private const val CELL_ICON_TEXT_SPACING_DP = 4
    private const val CELL_TEXT_SIZE_SP = 10f
    private const val ENTER_DURATION_MS = 160L
    private const val CELL_INDICATOR_ALPHA = 0x33
    private var activeAnchor: View? = null
    private var activeDismiss: (() -> Unit)? = null
    private var activeRefresh: ((TabBarState, List<Int>) -> Unit)? = null

    fun refresh(anchor: View, state: TabBarState, indices: List<Int>) {
        if (activeAnchor === anchor) {
            activeRefresh?.invoke(state, indices)
        }
    }

    fun dismiss(anchor: View) {
        if (activeAnchor === anchor) {
            activeDismiss?.invoke()
        }
    }

    /**
     * @param anchor the tab strip; the panel floats just above it.
     * @param indices positions in `state.list` to offer, in order.
     * @param onPick receives the picked item's declaration index.
     * @param onDismiss mirrors every exit path back into the strip's state.
     */
    fun show(
        activity: Activity,
        anchor: View,
        state: TabBarState,
        indices: List<Int>,
        onPick: (Int) -> Unit,
        onDismiss: () -> Unit
    ) {
        if (indices.isEmpty()) {
            onDismiss()
            return
        }
        val palette = OverlayPalette.of(activity)
        val rootView = activity.window.decorView as ViewGroup
        val gapBelowAnchor = gapBelowAnchorTop(rootView, anchor)

        val container = FrameLayout(activity).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            // Take focus so the hardware/gesture back key closes the panel
            // instead of leaving the lxapp behind it.
            isFocusableInTouchMode = true
        }
        var dismissed = false
        val dismiss = {
            if (!dismissed) {
                dismissed = true
                rootView.removeView(container)
                if (activeAnchor === anchor) {
                    activeAnchor = null
                    activeDismiss = null
                    activeRefresh = null
                }
                onDismiss()
            }
        }
        activeDismiss?.invoke()
        activeAnchor = anchor
        activeDismiss = dismiss

        container.addView(View(activity).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            ).apply {
                gravity = Gravity.TOP
                bottomMargin = gapBelowAnchor
            }
            setBackgroundColor(Color.TRANSPARENT)
            setOnClickListener { dismiss() }
        })

        var panel = buildPanel(activity, palette, state, indices) { index ->
            onPick(index)
            dismiss()
        }
        panel.layoutParams = FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.WRAP_CONTENT
        ).apply {
            gravity = Gravity.BOTTOM
            val density = activity.resources.displayMetrics.density
            leftMargin = (PANEL_HORIZONTAL_MARGIN_DP * density).toInt()
            rightMargin = (PANEL_HORIZONTAL_MARGIN_DP * density).toInt()
            bottomMargin = gapBelowAnchor + (PANEL_BOTTOM_GAP_DP * density).toInt()
        }
        container.addView(panel)
        activeRefresh = { nextState, nextIndices ->
            if (!nextState.visible || nextIndices.isEmpty()) {
                dismiss()
            } else {
                // The open panel is part of the bar: runtime style, label and
                // badge patches must not leave it showing the previous state.
                val replacement = buildPanel(activity, OverlayPalette.of(activity), nextState, nextIndices) { index ->
                    onPick(index)
                    dismiss()
                }
                replacement.layoutParams = panel.layoutParams
                panel.animate().cancel()
                container.removeView(panel)
                container.addView(replacement)
                panel = replacement
            }
        }

        container.setOnKeyListener { _, keyCode, event ->
            val back = keyCode == KeyEvent.KEYCODE_BACK && event.action == KeyEvent.ACTION_UP
            if (back) dismiss()
            back
        }

        rootView.addView(container)
        container.requestFocus()

        // The slide-up offset needs a measured height, so the panel stays hidden
        // for the layout pass that produces one.
        val enteringPanel = panel
        enteringPanel.alpha = 0f
        enteringPanel.post {
            if (!dismissed && panel === enteringPanel) {
                enteringPanel.alpha = 1f
                enteringPanel.translationY = enteringPanel.height.toFloat()
                enteringPanel.animate().translationY(0f).setDuration(ENTER_DURATION_MS).start()
            }
        }
    }

    /** Height of the strip plus whatever sits below it, in root coordinates. */
    private fun gapBelowAnchorTop(rootView: ViewGroup, anchor: View): Int {
        val anchorLocation = IntArray(2)
        val rootLocation = IntArray(2)
        anchor.getLocationOnScreen(anchorLocation)
        rootView.getLocationOnScreen(rootLocation)
        return (rootLocation[1] + rootView.height - anchorLocation[1]).coerceAtLeast(0)
    }

    /**
     * The plate behind the folded items.
     *
     * The bar itself is drawn with the lxapp's declared colour, so the panel has
     * to follow it or the two disagree whenever the app and the system do — a
     * light bar with a dark panel over it. Where the bar is transparent there is
     * nothing to follow and the panel floats over the page, so the page's own
     * colour is what makes it read as one surface; the overlay palette is the
     * last resort, for a host that declares neither.
     */
    private fun panelSurface(
        activity: Activity,
        palette: OverlayPalette,
        state: TabBarState
    ): Int {
        if (!state.isBackgroundTransparent()) return state.backgroundColor
        val dark = (activity.resources.configuration.uiMode and
            Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES
        runCatching { NativeApi.pageBackgroundColor(dark) }
            .getOrNull()
            ?.takeIf { it.isNotEmpty() }
            ?.let { hex -> runCatching { Color.parseColor(hex) }.getOrNull() }
            ?.let { return it }
        return palette.surface
    }

    private fun buildPanel(
        activity: Activity,
        palette: OverlayPalette,
        state: TabBarState,
        indices: List<Int>,
        onPick: (Int) -> Unit
    ): LinearLayout {
        val density = activity.resources.displayMetrics.density
        return LinearLayout(activity).apply {
            orientation = LinearLayout.VERTICAL
            background = GradientDrawable().apply {
                setColor(panelSurface(activity, palette, state))
                val radius = PANEL_CORNER_RADIUS_DP * density
                cornerRadius = radius
            }
            elevation = 8f * density
            val padding = (PANEL_PADDING_DP * density).toInt()
            setPadding(padding, padding, padding, padding)
            // The panel is the modal surface; taps must not fall through to the scrim.
            isClickable = true

            indices.chunked(COLUMNS).forEach { row ->
                addView(buildRow(activity, state, row, onPick))
            }
        }
    }

    /**
     * A short final row keeps the column width of a full one, so cells stay
     * aligned in a grid instead of spreading across the panel.
     */
    private fun buildRow(
        activity: Activity,
        state: TabBarState,
        indices: List<Int>,
        onPick: (Int) -> Unit
    ): LinearLayout = LinearLayout(activity).apply {
        orientation = LinearLayout.HORIZONTAL
        layoutParams = LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.WRAP_CONTENT
        )
        indices.forEach { index ->
            state.list.getOrNull(index)?.let { item ->
                val selected = item.index == state.selectedIndex
                addView(buildCell(activity, state, item, selected) {
                    onPick(item.index)
                })
            }
        }
        repeat(COLUMNS - indices.size) {
            addView(View(activity).apply {
                layoutParams = LinearLayout.LayoutParams(0, 1, 1f)
            })
        }
    }

    private fun buildCell(
        activity: Activity,
        state: TabBarState,
        item: TabBarItem,
        selected: Boolean,
        onClick: () -> Unit
    ): LinearLayout {
        val density = activity.resources.displayMetrics.density
        return LinearLayout(activity).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER
            layoutParams = LinearLayout.LayoutParams(
                0,
                ViewGroup.LayoutParams.WRAP_CONTENT,
                1f
            )
            clipChildren = false
            clipToPadding = false
            setOnClickListener { onClick() }
            val cellInset = (2 * density).toInt()
            setPadding(cellInset, cellInset, cellInset, cellInset)

            val vertical = (CELL_VERTICAL_PADDING_DP * density).toInt()
            val content = LinearLayout(activity).apply {
                orientation = LinearLayout.VERTICAL
                gravity = Gravity.CENTER_HORIZONTAL
                layoutParams = LinearLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT
                )
                setPadding(0, vertical, 0, vertical)
                clipChildren = false
                clipToPadding = false

                val iconSize = (CELL_ICON_SIZE_DP * density).toInt()
                val badgeSpace = (8 * density).toInt()
                val iconWrapper = FrameLayout(activity).apply {
                    layoutParams = LinearLayout.LayoutParams(
                        iconSize + badgeSpace,
                        iconSize + badgeSpace
                    )
                    clipChildren = false
                    clipToPadding = false
                    if (selected) {
                        background = GradientDrawable().apply {
                            shape = GradientDrawable.OVAL
                            setColor((state.selectedColor and 0x00FFFFFF) or (CELL_INDICATOR_ALPHA shl 24))
                        }
                    }
                }
                iconWrapper.addView(ImageView(activity).apply {
                    layoutParams = FrameLayout.LayoutParams(iconSize, iconSize).apply {
                        gravity = Gravity.CENTER
                    }
                    ChromeIcon.applyTo(this, item.iconPath)
                    setImageDrawable(loadIcon(item, selected, state, density))
                })
                if (!item.badge.isNullOrBlank()) {
                    iconWrapper.addView(badgeView(activity, item.badge, density))
                } else if (item.hasRedDot) {
                    iconWrapper.addView(redDotView(activity, density))
                }
                addView(iconWrapper)

                if (!item.text.isNullOrBlank()) {
                    addView(TextView(activity).apply {
                        text = item.text
                        setTextColor(if (selected) state.selectedColor else state.color)
                        setTextSize(android.util.TypedValue.COMPLEX_UNIT_SP, CELL_TEXT_SIZE_SP)
                        gravity = Gravity.CENTER
                        includeFontPadding = false
                        isSingleLine = true
                        ellipsize = android.text.TextUtils.TruncateAt.END
                        layoutParams = LinearLayout.LayoutParams(
                            ViewGroup.LayoutParams.MATCH_PARENT,
                            ViewGroup.LayoutParams.WRAP_CONTENT
                        ).apply {
                            topMargin = (CELL_ICON_TEXT_SPACING_DP * density).toInt()
                        }
                    })
                }
            }
            addView(content)
        }
    }

    private fun loadIcon(
        item: TabBarItem,
        selected: Boolean,
        state: TabBarState,
        density: Float
    ): android.graphics.drawable.Drawable {
        val tint = if (selected) state.selectedColor else state.color
        return ChromeIcon.load(item.iconPath, tint) {
            GradientDrawable().apply {
                shape = GradientDrawable.OVAL
                setColor(tint)
                val size = (CELL_ICON_SIZE_DP * density).toInt()
                setSize(size, size)
            }
        }
    }

    private fun badgeView(activity: Activity, text: String, density: Float): TextView =
        TextView(activity).apply {
            this.text = text
            setTextColor(Color.WHITE)
            setTextSize(android.util.TypedValue.COMPLEX_UNIT_SP, 7f)
            gravity = Gravity.CENTER
            isSingleLine = true
            includeFontPadding = false
            background = GradientDrawable().apply {
                shape = GradientDrawable.RECTANGLE
                setColor(0xFFFA5151.toInt())  // Badge red, unified across platforms
                cornerRadius = 6 * density
            }
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT
            ).apply {
                gravity = Gravity.TOP or Gravity.END
                setMargins(0, (2 * density).toInt(), (2 * density).toInt(), 0)
            }
            setPadding((3 * density).toInt(), (1 * density).toInt(), (3 * density).toInt(), (1 * density).toInt())
            minWidth = (12 * density).toInt()
            minHeight = (12 * density).toInt()
        }

    private fun redDotView(activity: Activity, density: Float): View = View(activity).apply {
        background = GradientDrawable().apply {
            shape = GradientDrawable.OVAL
            setColor(0xFFFA5151.toInt())  // Badge red, unified across platforms
        }
        val size = (6 * density).toInt()
        layoutParams = FrameLayout.LayoutParams(size, size).apply {
            gravity = Gravity.TOP or Gravity.END
            setMargins(0, (3 * density).toInt(), (3 * density).toInt(), 0)
        }
    }
}
