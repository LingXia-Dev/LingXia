package com.lingxia.lxapp.chrome

import android.app.Activity
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
import java.io.File

/**
 * The tab items a compact strip could not fit, shown as a panel directly above
 * the bar so the "more" slot reads as an extension of it rather than a modal.
 */
internal object TabBarOverflowSheet {
    private const val COLUMNS = 5
    private const val PANEL_CORNER_RADIUS_DP = 16f
    private const val PANEL_PADDING_DP = 8
    private const val CELL_VERTICAL_PADDING_DP = 12
    private const val CELL_ICON_SIZE_DP = 24
    private const val CELL_ICON_TEXT_SPACING_DP = 4
    private const val CELL_TEXT_SIZE_SP = 11f
    private const val ENTER_DURATION_MS = 160L
    private const val CELL_INDICATOR_WIDTH_DP = 40
    private const val CELL_INDICATOR_HEIGHT_DP = 28
    private const val CELL_INDICATOR_ALPHA = 0x33

    /**
     * @param anchor the tab strip; the panel sits flush on top of it.
     * @param indices item indices to offer, in declaration order.
     * @param onPick receives the picked item's index in the full item list.
     */
    fun show(
        activity: Activity,
        anchor: View,
        state: TabBarState,
        indices: List<Int>,
        onPick: (Int) -> Unit
    ) {
        if (indices.isEmpty()) {
            return
        }
        val palette = OverlayPalette.of(activity)
        val rootView = activity.window.decorView as ViewGroup

        val container = FrameLayout(activity).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            // Take focus so the hardware/gesture back key closes the panel
            // instead of leaving the lxapp behind it.
            isFocusableInTouchMode = true
        }
        val dismiss = { rootView.removeView(container) }

        container.addView(View(activity).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setBackgroundColor(palette.scrim)
            alpha = 0f
            setOnClickListener { dismiss() }
            animate().alpha(1f).setDuration(ENTER_DURATION_MS).start()
        })

        val panel = buildPanel(activity, palette, state, indices) { index ->
            dismiss()
            onPick(index)
        }
        panel.layoutParams = FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.WRAP_CONTENT
        ).apply {
            gravity = Gravity.BOTTOM
            bottomMargin = gapBelowAnchorTop(rootView, anchor)
        }
        container.addView(panel)

        container.setOnKeyListener { _, keyCode, event ->
            val back = keyCode == KeyEvent.KEYCODE_BACK && event.action == KeyEvent.ACTION_UP
            if (back) dismiss()
            back
        }

        rootView.addView(container)
        container.requestFocus()

        // The slide-up offset needs a measured height, so the panel stays hidden
        // for the layout pass that produces one.
        panel.alpha = 0f
        panel.post {
            panel.alpha = 1f
            panel.translationY = panel.height.toFloat()
            panel.animate().translationY(0f).setDuration(ENTER_DURATION_MS).start()
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
                setColor(palette.surface)
                val radius = PANEL_CORNER_RADIUS_DP * density
                cornerRadii = floatArrayOf(radius, radius, radius, radius, 0f, 0f, 0f, 0f)
            }
            elevation = 8f * density
            val padding = (PANEL_PADDING_DP * density).toInt()
            setPadding(padding, padding, padding, padding)
            // The panel is the modal surface; taps must not fall through to the scrim.
            isClickable = true

            indices.chunked(COLUMNS).forEach { row ->
                addView(buildRow(activity, palette, state, row, onPick))
            }
        }
    }

    /**
     * A short final row keeps the column width of a full one, so cells stay
     * aligned in a grid instead of spreading across the panel.
     */
    private fun buildRow(
        activity: Activity,
        palette: OverlayPalette,
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
                addView(buildCell(activity, palette, state, item, index == state.selectedIndex) {
                    onPick(index)
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
        palette: OverlayPalette,
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
            val vertical = (CELL_VERTICAL_PADDING_DP * density).toInt()
            setPadding(0, vertical, 0, vertical)
            clipChildren = false
            clipToPadding = false
            setOnClickListener { onClick() }

            val iconSize = (CELL_ICON_SIZE_DP * density).toInt()
            val badgeSpace = (10 * density).toInt()
            val iconWrapper = FrameLayout(activity).apply {
                layoutParams = LinearLayout.LayoutParams(
                    iconSize + badgeSpace,
                    iconSize + (4 * density).toInt()
                )
                clipChildren = false
                clipToPadding = false
            }
            // Mirrors the strip: a single-icon item needs chrome to read as
            // selected, since there is no second drawable to swap in.
            if (selected && !item.hasSelectedIcon) {
                iconWrapper.addView(View(activity).apply {
                    layoutParams = FrameLayout.LayoutParams(
                        (CELL_INDICATOR_WIDTH_DP * density).toInt(),
                        (CELL_INDICATOR_HEIGHT_DP * density).toInt()
                    ).apply { gravity = Gravity.CENTER }
                    background = GradientDrawable().apply {
                        shape = GradientDrawable.RECTANGLE
                        setColor((state.selectedColor and 0x00FFFFFF) or (CELL_INDICATOR_ALPHA shl 24))
                        cornerRadius = CELL_INDICATOR_HEIGHT_DP * density / 2f
                    }
                })
            }
            iconWrapper.addView(ImageView(activity).apply {
                layoutParams = FrameLayout.LayoutParams(iconSize, iconSize).apply {
                    gravity = Gravity.CENTER
                }
                scaleType = ImageView.ScaleType.FIT_CENTER
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
                    setTextColor(if (selected) state.selectedColor else palette.body)
                    setTextSize(android.util.TypedValue.COMPLEX_UNIT_SP, CELL_TEXT_SIZE_SP)
                    gravity = Gravity.CENTER
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
    }

    private fun loadIcon(
        item: TabBarItem,
        selected: Boolean,
        state: TabBarState,
        density: Float
    ): android.graphics.drawable.Drawable {
        val path = if (selected && item.selectedIconPath.isNotEmpty()) {
            item.selectedIconPath
        } else {
            item.iconPath
        }
        val file = File(path)
        val loaded = if (file.exists()) {
            android.graphics.drawable.Drawable.createFromPath(file.absolutePath)
        } else {
            null
        }
        return loaded ?: GradientDrawable().apply {
            shape = GradientDrawable.OVAL
            setColor(if (selected) state.selectedColor else state.color)
            val size = (CELL_ICON_SIZE_DP * density).toInt()
            setSize(size, size)
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
