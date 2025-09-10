package com.lingxia.lxapp

import android.content.Context
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import java.io.File
import android.util.Log
import android.util.TypedValue
import android.widget.FrameLayout

data class TabBarState(
    val backgroundColor: Int = Color.WHITE,          // Background color, default white
    val selectedColor: Int = 0xFF1677FF.toInt(),     // Selected item color, default tech blue
    val color: Int = 0xFF666666.toInt(),             // Unselected item color, default gray
    val borderStyle: Int = 0xFFF0F0F0.toInt(),       // Top border color, default light gray
    val dimension: Int = 64,                         // Dimension in dp: height for bottom, width for left/right
    val position: Position = Position.BOTTOM,        // Position: 0=Bottom, 1=Left, 2=Right
    val list: List<TabBarItem> = emptyList(),        // List of tab items
    val visible: Boolean = true,                     // TabBar visibility (kept for FFI compatibility)
    val selectedIndex: Int = 0                       // Selected tab index managed by Rust
) {
    /**
     * Check if background color is transparent.
     * Uses bit operations to check alpha channel
     */
    fun isBackgroundTransparent(): Boolean {
        return (backgroundColor ushr 24) and 0xFF == 0
    }

    companion object {
        // These values MUST match exactly with Rust TabBarPosition enum!
        // See: lingxia-lxapp/src/lxapp/config/tabbar.rs

        /** Tab bar at the bottom (default) - MUST match Rust Bottom = 0 */
        const val POSITION_BOTTOM = 0

        /** Tab bar at the left - MUST match Rust Left = 1 */
        const val POSITION_LEFT = 1

        /** Tab bar at the right - MUST match Rust Right = 2 */
        const val POSITION_RIGHT = 2
    }

    // Position enum for backward compatibility, with int values matching constants
    enum class Position(val value: Int) {
        BOTTOM(POSITION_BOTTOM),
        LEFT(POSITION_LEFT),
        RIGHT(POSITION_RIGHT);

        companion object {
            fun fromInt(value: Int): Position = when(value) {
                POSITION_LEFT -> LEFT
                POSITION_RIGHT -> RIGHT
                else -> BOTTOM // default
            }
        }
    }
}

data class TabBarItem(
    val pagePath: String,                 // Page path to navigate to
    val text: String?,                    // Tab text label (optional, null means no text)
    val iconPath: String,                 // Absolute path to the icon file
    val selectedIconPath: String,         // Absolute path to the selected state icon file
    val selected: Boolean = false,        // Whether this tab is selected
    val group: Int = 0,                   // Group positioning: 0=center (default), 1=start, 2=end
    val badge: String? = null,            // Badge text (optional)
    val hasRedDot: Boolean = false        // Whether to show red dot indicator
)

/**
 * TabBar component for mini apps, supporting:
 * - Customizable tab items with icons and text
 * - Bottom/left/right positioning
 * - Notification badges (red dot and text)
 * - Dynamic styling and content updates
 */
class TabBar(context: Context) : LinearLayout(context) {
    companion object {
        private const val TAG = "LingXia.TabBar"
        private const val VERTICAL_TAB_BAR_WIDTH_MULTIPLIER = 1.0f
        // Constants for vertical TabBar item styling
        private const val VERTICAL_ITEM_MAX_HEIGHT_DP = 70
        private const val VERTICAL_ITEM_MIN_HEIGHT_DP = 48
        private const val VERTICAL_ITEM_PADDING_HORIZONTAL_DP = 6
        private const val VERTICAL_ITEM_PADDING_VERTICAL_DP = 8
        private const val VERTICAL_ITEM_ICON_SIZE_DP = 22
        private const val HORIZONTAL_ITEM_ICON_SIZE_DP = 24
        private const val VERTICAL_ITEM_TEXT_SIZE_SP = 12f
        private const val HORIZONTAL_ITEM_TEXT_SIZE_SP = 11f  // Slightly smaller for horizontal
        private const val ITEM_ICON_TOP_MARGIN_DP = 1
        private const val ITEM_TEXT_TOP_MARGIN_DP = 2  // More space between icon and text
        private const val ITEM_TEXT_BOTTOM_MARGIN_DP = 2  // More bottom margin
        private const val ITEM_BORDER_THICKNESS_DP = 1f

        // Padding for individual TabBarItems when TabBar is horizontal
        private const val HORIZONTAL_ITEM_PADDING_SIDES_DP = 4
        private const val HORIZONTAL_ITEM_PADDING_VERTICAL_DP = 2

        private val VERTICAL_BORDER_COLOR = 0xFFE0E0E0.toInt()
        private val VERTICAL_TABBAR_BACKGROUND_COLOR = 0xFFF8F8F8.toInt()
        private val VERTICAL_SELECTED_ITEM_BACKGROUND_COLOR = 0xFFE6F0FF.toInt()
        private const val SELECTED_ITEM_CORNER_RADIUS_DP = 12f // Unified corner radius for updateTabState
        private const val INITIAL_SELECTED_ITEM_CORNER_RADIUS_DP = 8f // For createTabItem
    }

    internal var config = TabBarState()
    private var items = listOf<TabBarItem>()
    private var tabViews = mutableListOf<LinearLayout>()
    private var itemsContainer: LinearLayout? = null
    private var onTabSelectedListener: ((Int, String) -> Unit)? = null


    init {
        orientation = when (config.position.value) {
            TabBarState.POSITION_LEFT, TabBarState.POSITION_RIGHT -> HORIZONTAL
            else -> VERTICAL
        }
        visibility = View.GONE

        // Ensure TabBar doesn't clip badge views
        clipChildren = false
        clipToPadding = false

        itemsContainer = LinearLayout(context).apply {
            // Ensure items container also doesn't clip
            clipChildren = false
            clipToPadding = false
        }
        updateItemsContainerLayoutOnly(this.config)
        performLayoutForPosition()
    }

    private fun updateItemsContainerLayout(currentConfig: TabBarState) {
        itemsContainer?.apply {
            orientation = when (currentConfig.position.value) {
                TabBarState.POSITION_LEFT, TabBarState.POSITION_RIGHT -> VERTICAL
                else -> HORIZONTAL
            }

            val isVerticalTabBar = currentConfig.position.value == TabBarState.POSITION_LEFT || currentConfig.position.value == TabBarState.POSITION_RIGHT

            // Use configured dimension (Rust provides default value, but Android FFI might be nullable)
            val tabBarDimension = currentConfig.dimension

            if (isVerticalTabBar) {
                // For vertical TabBar, itemsContainer has fixed width and wraps content height
                layoutParams = LayoutParams(
                    (tabBarDimension * VERTICAL_TAB_BAR_WIDTH_MULTIPLIER * resources.displayMetrics.density).toInt(),
                    ViewGroup.LayoutParams.WRAP_CONTENT
                )
            } else {
                // For horizontal TabBar, itemsContainer matches parent width and has fixed height
                layoutParams = LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    (tabBarDimension * resources.displayMetrics.density).toInt()
                )
            }

            // Gravity for aligning children (tab items) within itemsContainer
            gravity = if (orientation == VERTICAL) { // itemsContainer is vertical (TabBar is LEFT/RIGHT)
                Gravity.TOP or Gravity.CENTER_HORIZONTAL
            } else { // itemsContainer is horizontal (TabBar is BOTTOM)
                Gravity.CENTER
            }

            // Set background for itemsContainer to match TabBar background
            val backgroundColor = when {
                currentConfig.backgroundColor == Color.TRANSPARENT -> Color.TRANSPARENT
                isVerticalTabBar -> VERTICAL_TABBAR_BACKGROUND_COLOR
                else -> currentConfig.backgroundColor
            }
            setBackgroundColor(backgroundColor)
        }
    }

    private fun updateItemsContainerLayoutOnly(currentConfig: TabBarState) {
        itemsContainer?.apply {
            orientation = when (currentConfig.position.value) {
                TabBarState.POSITION_LEFT, TabBarState.POSITION_RIGHT -> VERTICAL
                else -> HORIZONTAL
            }

            val isVerticalTabBar = currentConfig.position.value == TabBarState.POSITION_LEFT || currentConfig.position.value == TabBarState.POSITION_RIGHT

            // Use configured dimension (Rust provides default value, but Android FFI might be nullable)
            val tabBarDimension = currentConfig.dimension

            if (isVerticalTabBar) {
                // For vertical TabBar, itemsContainer has fixed width and wraps content height
                layoutParams = LayoutParams(
                    (tabBarDimension * VERTICAL_TAB_BAR_WIDTH_MULTIPLIER * resources.displayMetrics.density).toInt(),
                    ViewGroup.LayoutParams.WRAP_CONTENT
                )
            } else {
                // For horizontal TabBar, itemsContainer matches parent width and has fixed height
                layoutParams = LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    (tabBarDimension * resources.displayMetrics.density).toInt()
                )
            }

            // Gravity for aligning children (tab items) within itemsContainer
            gravity = if (orientation == VERTICAL) { // itemsContainer is vertical (TabBar is LEFT/RIGHT)
                Gravity.TOP or Gravity.CENTER_HORIZONTAL
            } else { // itemsContainer is horizontal (TabBar is BOTTOM)
                Gravity.CENTER
            }
        }
    }

    private fun performLayoutForPosition() {
        removeAllViews()

        val isBackgroundTransparent = config.isBackgroundTransparent()

        when (config.position.value) {
            TabBarState.POSITION_BOTTOM -> {
                if (!isBackgroundTransparent) {
                    addView(View(context).apply {
                        setBackgroundColor(config.borderStyle)
                        layoutParams = LayoutParams(
                            ViewGroup.LayoutParams.MATCH_PARENT,
                            (ITEM_BORDER_THICKNESS_DP * resources.displayMetrics.density).toInt()
                        )
                    })
                }
                addView(itemsContainer)
            }
            TabBarState.POSITION_LEFT -> {
                orientation = HORIZONTAL
                addView(itemsContainer)
                if (!isBackgroundTransparent) {
                    addView(View(context).apply {
                        setBackgroundColor(VERTICAL_BORDER_COLOR)
                        layoutParams = LayoutParams(
                            (ITEM_BORDER_THICKNESS_DP * resources.displayMetrics.density).toInt(),
                            ViewGroup.LayoutParams.MATCH_PARENT
                        )
                    })
                }
            }
            TabBarState.POSITION_RIGHT -> {
                orientation = HORIZONTAL
                if (!isBackgroundTransparent) {
                    addView(View(context).apply {
                        setBackgroundColor(VERTICAL_BORDER_COLOR)
                        layoutParams = LayoutParams(
                            (ITEM_BORDER_THICKNESS_DP * resources.displayMetrics.density).toInt(),
                            ViewGroup.LayoutParams.MATCH_PARENT
                        )
                    })
                }
                addView(itemsContainer)
            }
        }
    }

    fun setConfig(newConfig: TabBarState) {
        if (!isValidConfig(newConfig)) {
            Log.w(TAG, "Invalid TabBar config provided")
            return
        }

        config = newConfig

        val isBackgroundTransparent = config.isBackgroundTransparent()

        val tabBarBackgroundColor = when {
            config.backgroundColor == Color.TRANSPARENT -> Color.TRANSPARENT
            config.position.value == TabBarState.POSITION_LEFT || config.position.value == TabBarState.POSITION_RIGHT -> VERTICAL_TABBAR_BACKGROUND_COLOR
            else -> config.backgroundColor
        }

        setBackgroundColor(tabBarBackgroundColor)
        elevation = if (isBackgroundTransparent) 0f else 8f * resources.displayMetrics.density

        updateItemsContainerLayout(this.config)
        performLayoutForPosition()
        setItems(newConfig.list)

        // Set visibility based on Rust config
        visibility = if (newConfig.visible) View.VISIBLE else View.GONE
    }

    fun setItems(newItems: List<TabBarItem>) {
        Log.d(TAG, "setItems called with ${newItems.size} items")
        newItems.forEachIndexed { index, item ->
            Log.d(TAG, "Item $index: text='${item.text}', pagePath='${item.pagePath}'")
        }
        items = newItems  // Use all items from configuration

        itemsContainer?.let { container ->
            container.removeAllViews()
            tabViews.clear()

            if (items.isNotEmpty()) {

                // Check if ANY item has group field (grouped mode vs centered mode)
                val hasAnyGroupField = items.any { it.group != 0 }

                if (hasAnyGroupField) {
                    // Grouped Mode: Distribute items to start/end
                    setupGroupedLayout(container)
                } else {
                    // Centered Mode: All items centered (original behavior)
                    setupCenteredLayout(container)
                }
            }
        }
    }

    /**
     * Setup grouped layout: distribute items to start/end positions
     */
    private fun setupGroupedLayout(container: LinearLayout) {
        val isVertical = config.position.value == TabBarState.POSITION_LEFT || config.position.value == TabBarState.POSITION_RIGHT

        // Group items by their group value
        val startItems = mutableListOf<TabBarItem>()
        val endItems = mutableListOf<TabBarItem>()

        items.forEachIndexed { index, item ->
            when (item.group) {
                2 -> endItems.add(item) // end
                else -> startItems.add(item) // 0 (no group) or 1 (start) → all treated as start
            }
        }

        // Create start container
        if (startItems.isNotEmpty()) {
            val startContainer = createGroupContainer(startItems, isVertical)
            container.addView(startContainer)
        }

        // Add flexible spacer to push end items to bottom/right
        val spacer = View(context).apply {
            layoutParams = LinearLayout.LayoutParams(
                if (isVertical) LinearLayout.LayoutParams.MATCH_PARENT else 0,
                if (isVertical) 0 else LinearLayout.LayoutParams.MATCH_PARENT,
                1f
            )
        }
        container.addView(spacer)

        // Create end container
        if (endItems.isNotEmpty()) {
            val endContainer = createGroupContainer(endItems, isVertical)
            container.addView(endContainer)
        }
    }

    /**
     * Setup centered layout: all items centered (original behavior)
     */
    private fun setupCenteredLayout(container: LinearLayout) {
        val isVertical = config.position.value == TabBarState.POSITION_LEFT || config.position.value == TabBarState.POSITION_RIGHT

        // Get the size of the space for each item (original logic)
        val itemSize = if (isVertical) {
            // For vertical layout, use more compact spacing
            (resources.displayMetrics.heightPixels / Math.max(items.size, 4)).coerceAtMost(
                (VERTICAL_ITEM_MAX_HEIGHT_DP * resources.displayMetrics.density).toInt()
            ).coerceAtLeast(
                (VERTICAL_ITEM_MIN_HEIGHT_DP * resources.displayMetrics.density).toInt()
            )
        } else {
            resources.displayMetrics.widthPixels / items.size
        }

        items.forEachIndexed { index, item ->
            createTabView(item, config, index == config.selectedIndex, index).also { view ->
                tabViews.add(view)
                container.addView(view)
            }
        }
    }

    /**
     * Create a container for a group of tab items
     */
    private fun createGroupContainer(groupItems: List<TabBarItem>, isVertical: Boolean): LinearLayout {
        val container = LinearLayout(context).apply {
            orientation = if (isVertical) LinearLayout.VERTICAL else LinearLayout.HORIZONTAL
            layoutParams = LinearLayout.LayoutParams(
                if (isVertical) LinearLayout.LayoutParams.MATCH_PARENT else LinearLayout.LayoutParams.WRAP_CONTENT,
                if (isVertical) LinearLayout.LayoutParams.WRAP_CONTENT else LinearLayout.LayoutParams.MATCH_PARENT
            )

            // Add spacing between items in the group
            val spacing = (4 * resources.displayMetrics.density).toInt()
            setPadding(
                if (isVertical) 0 else spacing,
                if (isVertical) spacing else 0,
                if (isVertical) 0 else spacing,
                if (isVertical) spacing else 0
            )
        }

        groupItems.forEachIndexed { index, item ->
            // Find the global index of this item
            val globalIndex = items.indexOf(item)
            if (globalIndex >= 0) {
                val tabView = createTabView(item, config, globalIndex == config.selectedIndex, globalIndex)

                // Add margin between items (except for the first item)
                if (index > 0) {
                    val marginPx = (12 * resources.displayMetrics.density).toInt() // Increased spacing
                    (tabView.layoutParams as? LinearLayout.LayoutParams)?.apply {
                        if (isVertical) {
                            topMargin = marginPx
                        } else {
                            leftMargin = marginPx
                        }
                    }
                }

                tabViews.add(tabView)
                container.addView(tabView)
            }
        }

        return container
    }

    fun setSelectedIndex(index: Int, notifyListener: Boolean = true) {
        if (index < 0 || index >= items.size) {
            return
        }

        if (index != config.selectedIndex) {
            val previousIndex = config.selectedIndex
            config = config.copy(selectedIndex = index)

            // Update UI state for all tab views
            tabViews.forEachIndexed { viewIndex, tabView ->
                // Find which item this view represents by checking the tag or by position
                val itemIndex = findItemIndexForTabView(tabView, viewIndex)
                if (itemIndex >= 0 && itemIndex < items.size) {
                    val isSelected = itemIndex == index
                    updateTabState(tabView, items[itemIndex], isSelected)
                }
            }

            // Notify listener
            if (notifyListener) {
                onTabSelectedListener?.invoke(index, items[index].pagePath)
            }
        }
    }

    /**
     * Find the item index for a given tab view using its tag
     */
    private fun findItemIndexForTabView(tabView: LinearLayout, viewIndex: Int): Int {
        return (tabView.tag as? Int) ?: viewIndex.takeIf { it < items.size } ?: -1
    }

    fun setOnTabSelectedListener(listener: (Int, String) -> Unit) {
        onTabSelectedListener = listener
    }

    private fun createTabView(item: TabBarItem, config: TabBarState, isSelected: Boolean, itemIndex: Int = -1): LinearLayout {
        val isVertical = config.position.value == TabBarState.POSITION_LEFT || config.position.value == TabBarState.POSITION_RIGHT

        return LinearLayout(context).apply {
            orientation = VERTICAL
            gravity = Gravity.CENTER

            // Set tag to store the item index for grouped layout
            if (itemIndex >= 0) {
                tag = itemIndex
            }

            layoutParams = if (isVertical) {
                LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    0,
                    1f
                ).apply {
                    val margin = (VERTICAL_ITEM_PADDING_HORIZONTAL_DP * resources.displayMetrics.density).toInt()
                    setMargins(margin, margin, margin, margin)
                }
            } else {
                LayoutParams(
                    0,
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    1f
                ).apply {
                    // Add padding for better spacing in horizontal layout
                    val padding = (2 * resources.displayMetrics.density).toInt()
                    setMargins(padding, padding, padding, padding)
                }
            }

            // Create a wrapper container for icon + badge
            val iconWrapper = FrameLayout(context).apply {
                val iconSize = if (isVertical) VERTICAL_ITEM_ICON_SIZE_DP else HORIZONTAL_ITEM_ICON_SIZE_DP
                val iconSizePx = (iconSize * resources.displayMetrics.density).toInt()

                // Minimal extension to accommodate small badge
                val badgeSpace = (10 * resources.displayMetrics.density).toInt()
                val wrapperWidth = iconSizePx + badgeSpace
                val wrapperHeight = iconSizePx + (4 * resources.displayMetrics.density).toInt()  // Minimal vertical extension

                layoutParams = LayoutParams(wrapperWidth, wrapperHeight).apply {
                    gravity = Gravity.CENTER_HORIZONTAL
                }

                // Ensure container doesn't clip children
                clipChildren = false
                clipToPadding = false
            }

            // Create icon with proper centering
            val iconSize = if (isVertical) VERTICAL_ITEM_ICON_SIZE_DP else HORIZONTAL_ITEM_ICON_SIZE_DP
            val iconSizePx = (iconSize * resources.displayMetrics.density).toInt()

            val icon = ImageView(context).apply {
                layoutParams = FrameLayout.LayoutParams(iconSizePx, iconSizePx).apply {
                    gravity = Gravity.CENTER
                }
                scaleType = ImageView.ScaleType.FIT_CENTER
                setImageDrawable(getIconDrawable(item, isSelected))
            }

            iconWrapper.addView(icon)

            // Add badge if present - positioned outside icon bounds
            if (!item.badge.isNullOrBlank()) {
                val badgeView = createBadgeView(item.badge!!)
                iconWrapper.addView(badgeView)
            }

            // Add red dot if present - positioned outside icon bounds
            if (item.hasRedDot) {
                val redDotView = createRedDotView()
                iconWrapper.addView(redDotView)
            }

            addView(iconWrapper)

            if (!item.text.isNullOrBlank()) {
                Log.d(TAG, "Creating TextView for item: text='${item.text}', pagePath='${item.pagePath}'")
                val textView = TextView(context).apply {
                    text = item.text

                    // Use config colors with proper alpha handling
                    setTextColor(if (isSelected) config.selectedColor else config.color)
                    setTextSize(android.util.TypedValue.COMPLEX_UNIT_SP,
                        if (isVertical) VERTICAL_ITEM_TEXT_SIZE_SP else HORIZONTAL_ITEM_TEXT_SIZE_SP)
                    layoutParams = LayoutParams(
                        ViewGroup.LayoutParams.WRAP_CONTENT,
                        ViewGroup.LayoutParams.WRAP_CONTENT
                    ).apply {
                        topMargin = (ITEM_TEXT_TOP_MARGIN_DP * resources.displayMetrics.density).toInt()
                        bottomMargin = (ITEM_TEXT_BOTTOM_MARGIN_DP * resources.displayMetrics.density).toInt()
                    }
                    gravity = Gravity.CENTER
                    isSingleLine = true
                    ellipsize = android.text.TextUtils.TruncateAt.END
                    // Ensure text has enough space
                    minWidth = (40 * resources.displayMetrics.density).toInt()

                }
                addView(textView)
                Log.d(TAG, "Added TextView to LinearLayout: text='${textView.text}', textColor=${textView.currentTextColor}, textSize=${textView.textSize}")
            }

            setOnClickListener {
                onTabSelectedListener?.invoke(items.indexOf(item), item.pagePath)
            }
        }
    }

    private fun updateTabState(tabView: LinearLayout, item: TabBarItem, selected: Boolean) {
        val iconContainer = tabView.getChildAt(0) as FrameLayout
        val icon = iconContainer.getChildAt(0) as ImageView
        icon.setImageDrawable(getIconDrawable(item, selected))

        if (tabView.childCount > 1) {
            (tabView.getChildAt(1) as? TextView)?.setTextColor(
                if (selected) config.selectedColor else config.color
            )
        }
    }

    private fun getIconDrawable(item: TabBarItem, selected: Boolean): android.graphics.drawable.Drawable {
        val iconPath = if (selected && item.selectedIconPath.isNotEmpty()) {
            item.selectedIconPath
        } else {
            item.iconPath
        }

        return try {
            val iconFile = File(iconPath)
            if (iconFile.exists()) {
                android.graphics.drawable.Drawable.createFromPath(iconFile.absolutePath)
                    ?: createDefaultIcon(selected)
            } else {
                createDefaultIcon(selected)
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to load icon: ${e.message}")
            createDefaultIcon(selected)
        }
    }

    private fun createDefaultIcon(selected: Boolean): android.graphics.drawable.Drawable {
        return GradientDrawable().apply {
            shape = GradientDrawable.OVAL
            setColor(if (selected) config.selectedColor else config.color)
            val size = (24 * resources.displayMetrics.density).toInt()
            setSize(size, size)
        }
    }

    // Gets the index of the currently selected tab item
    fun getSelectedIndex(): Int {
        return config.selectedIndex
    }

    // Finds the index of a tab item by its pagePath
    fun findTabIndexByPath(path: String): Int {
        return items.indexOfFirst { it.pagePath == path }
    }

    private fun isValidConfig(config: TabBarState): Boolean {
        return config.list.isNotEmpty()
    }

    /**
     * Create a badge view with text - compact design
     */
    private fun createBadgeView(badgeText: String): TextView {
        return TextView(context).apply {
            text = badgeText
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 7f)  // Even smaller text
            gravity = Gravity.CENTER
            isSingleLine = true
            includeFontPadding = false  // Remove extra font padding

            // Create very compact rounded background
            background = GradientDrawable().apply {
                shape = GradientDrawable.RECTANGLE
                setColor(0xFFFF4444.toInt())  // Bright red
                cornerRadius = (6 * resources.displayMetrics.density)  // Smaller radius
            }

            // Position at top-right within wrapper bounds
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT
            ).apply {
                gravity = Gravity.TOP or Gravity.END
                // Use small positive margins to keep badge fully visible
                val margin = (2 * resources.displayMetrics.density).toInt()
                setMargins(0, margin, margin, 0)
            }

            // Very minimal padding for ultra-compact appearance
            val horizontalPadding = (3 * resources.displayMetrics.density).toInt()
            val verticalPadding = (1 * resources.displayMetrics.density).toInt()
            setPadding(horizontalPadding, verticalPadding, horizontalPadding, verticalPadding)

            // Very small minimum size
            minWidth = (12 * resources.displayMetrics.density).toInt()
            minHeight = (12 * resources.displayMetrics.density).toInt()
        }
    }

    /**
     * Create a red dot indicator - small and positioned outside icon
     */
    private fun createRedDotView(): View {
        return View(context).apply {
            // Create small red circle
            background = GradientDrawable().apply {
                shape = GradientDrawable.OVAL
                setColor(0xFFFF4444.toInt())  // Bright red
            }

            // Position at top-right within wrapper bounds
            val dotSize = (6 * resources.displayMetrics.density).toInt()
            layoutParams = FrameLayout.LayoutParams(dotSize, dotSize).apply {
                gravity = Gravity.TOP or Gravity.END
                // Use small positive margins to keep dot fully visible
                val margin = (3 * resources.displayMetrics.density).toInt()
                setMargins(0, margin, margin, 0)
            }
        }
    }
}
