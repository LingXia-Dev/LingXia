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
import android.widget.FrameLayout
import android.view.ContextThemeWrapper
import com.google.android.material.badge.BadgeDrawable
import com.google.android.material.badge.BadgeUtils

data class TabBarConfig(
    val backgroundColor: Int? = null,            // Background color, default white
    val selectedColor: Int? = null,              // Selected item color, default tech blue
    val color: Int? = null,                      // Unselected item color, default gray
    val borderStyle: Int? = null,                // Top border color, default light gray
    val dimension: Int? = null,                  // Dimension in dp: height for top/bottom, width for left/right
    val position: Position = Position.BOTTOM,    // Position: 0=Bottom, 1=Top, 2=Left, 3=Right
    val list: List<TabBarItem> = emptyList(),   // List of tab items
    val visible: Boolean = true                  // TabBar visibility, default true
) {
    companion object {
        // ⚠️  CRITICAL: FFI Alignment Required ⚠️
        // These values MUST match exactly with Rust TabBarPosition enum!
        // See: lingxia-miniapp/src/miniapp/config/tabbar.rs

        /** Tab bar at the bottom (default) - MUST match Rust Bottom = 0 */
        const val POSITION_BOTTOM = 0

        /** Tab bar at the top - MUST match Rust Top = 1 */
        const val POSITION_TOP = 1

        /** Tab bar at the left - MUST match Rust Left = 2 */
        const val POSITION_LEFT = 2

        /** Tab bar at the right - MUST match Rust Right = 3 */
        const val POSITION_RIGHT = 3

        // Modern UI color scheme
        val DEFAULT_SELECTED_COLOR = Color.parseColor("#1677FF")    // Primary blue
        val DEFAULT_UNSELECTED_COLOR = Color.parseColor("#666666")  // Dark gray
        val DEFAULT_BORDER_COLOR = Color.parseColor("#F0F0F0")      // Light gray
        val DEFAULT_BACKGROUND_COLOR = Color.WHITE                   // White
    }

    // Position enum for backward compatibility, with int values matching constants
    enum class Position(val value: Int) {
        BOTTOM(POSITION_BOTTOM),
        TOP(POSITION_TOP),
        LEFT(POSITION_LEFT),
        RIGHT(POSITION_RIGHT);

        companion object {
            fun fromInt(value: Int): Position = when(value) {
                POSITION_TOP -> TOP
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
    val visible: Boolean = true           // Whether this tab is visible
)

// Unique view IDs for dot notification
private const val RED_DOT_ID = 1001

// Map to store active BadgeDrawables associated with their anchor views (iconContainers)
private val badgeDrawables = mutableMapOf<View, BadgeDrawable>()

/**
 * TabBar component for mini apps, supporting:
 * - Customizable tab items with icons and text
 * - Top/bottom positioning
 * - Notification badges (red dot and text)
 * - Dynamic styling and content updates
 */
class TabBar(context: Context) : LinearLayout(context) {
    companion object {
        private const val TAG = "LingXia.TabBar"
        private const val DEFAULT_TAB_BAR_SIZE_DP = 56
        private const val VERTICAL_TAB_BAR_WIDTH_MULTIPLIER = 1.0f
        // Constants for vertical TabBar item styling
        private const val VERTICAL_ITEM_MAX_HEIGHT_DP = 70
        private const val VERTICAL_ITEM_MIN_HEIGHT_DP = 48
        private const val VERTICAL_ITEM_PADDING_HORIZONTAL_DP = 6
        private const val VERTICAL_ITEM_PADDING_VERTICAL_DP = 8
        private const val VERTICAL_ITEM_ICON_SIZE_DP = 22
        private const val HORIZONTAL_ITEM_ICON_SIZE_DP = 24
        private const val VERTICAL_ITEM_TEXT_SIZE_SP = 12f
        private const val HORIZONTAL_ITEM_TEXT_SIZE_SP = 12f
        private const val ITEM_ICON_TOP_MARGIN_DP = 1
        private const val ITEM_TEXT_TOP_MARGIN_DP = 1
        private const val ITEM_TEXT_BOTTOM_MARGIN_DP = 1
        private const val ITEM_BORDER_THICKNESS_DP = 1f

        // Padding for individual TabBarItems when TabBar is horizontal
        private const val HORIZONTAL_ITEM_PADDING_SIDES_DP = 4
        private const val HORIZONTAL_ITEM_PADDING_VERTICAL_DP = 2

        private val VERTICAL_BORDER_COLOR = Color.parseColor("#E0E0E0")
        private val VERTICAL_TABBAR_BACKGROUND_COLOR = Color.parseColor("#F8F8F8")
        private val VERTICAL_SELECTED_ITEM_BACKGROUND_COLOR = Color.parseColor("#E6F0FF")
        private const val SELECTED_ITEM_CORNER_RADIUS_DP = 12f // Unified corner radius for updateTabState
        private const val INITIAL_SELECTED_ITEM_CORNER_RADIUS_DP = 8f // For createTabItem
    }

    internal var config = TabBarConfig()
    private var items = listOf<TabBarItem>()
    private var tabViews = mutableListOf<LinearLayout>()
    private var itemsContainer: LinearLayout? = null
    private var selectedPosition = -1
    private var onTabSelectedListener: ((Int, String) -> Unit)? = null
    private var onVisibilityChangedListener: ((Boolean) -> Unit)? = null

    init {
        orientation = when (config.position.value) {
            TabBarConfig.POSITION_LEFT, TabBarConfig.POSITION_RIGHT -> HORIZONTAL
            else -> VERTICAL
        }
        visibility = View.GONE

        itemsContainer = LinearLayout(context)
        updateItemsContainerLayoutOnly(this.config)
        performLayoutForPosition()
    }

    private fun updateItemsContainerLayout(currentConfig: TabBarConfig) {
        itemsContainer?.apply {
            orientation = when (currentConfig.position.value) {
                TabBarConfig.POSITION_LEFT, TabBarConfig.POSITION_RIGHT -> VERTICAL
                else -> HORIZONTAL
            }

            val isVerticalTabBar = currentConfig.position.value == TabBarConfig.POSITION_LEFT || currentConfig.position.value == TabBarConfig.POSITION_RIGHT

            if (isVerticalTabBar) {
                // For vertical TabBar, itemsContainer has fixed width and wraps content height
                layoutParams = LayoutParams(
                    (DEFAULT_TAB_BAR_SIZE_DP * VERTICAL_TAB_BAR_WIDTH_MULTIPLIER * resources.displayMetrics.density).toInt(),
                    ViewGroup.LayoutParams.WRAP_CONTENT
                )
            } else {
                // For horizontal TabBar, itemsContainer matches parent width and has fixed height
                layoutParams = LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    (DEFAULT_TAB_BAR_SIZE_DP * resources.displayMetrics.density).toInt()
                )
            }

            // Gravity for aligning children (tab items) within itemsContainer
            gravity = if (orientation == VERTICAL) { // itemsContainer is vertical (TabBar is LEFT/RIGHT)
                Gravity.TOP or Gravity.CENTER_HORIZONTAL
            } else { // itemsContainer is horizontal (TabBar is TOP/BOTTOM)
                Gravity.CENTER
            }

            // Set background for itemsContainer to match TabBar background
            val backgroundColor = when {
                currentConfig.backgroundColor == Color.TRANSPARENT -> Color.TRANSPARENT
                currentConfig.backgroundColor != null -> currentConfig.backgroundColor!!
                isVerticalTabBar -> VERTICAL_TABBAR_BACKGROUND_COLOR
                else -> TabBarConfig.DEFAULT_BACKGROUND_COLOR
            }
            setBackgroundColor(backgroundColor)
        }
    }

    private fun updateItemsContainerLayoutOnly(currentConfig: TabBarConfig) {
        itemsContainer?.apply {
            orientation = when (currentConfig.position.value) {
                TabBarConfig.POSITION_LEFT, TabBarConfig.POSITION_RIGHT -> VERTICAL
                else -> HORIZONTAL
            }

            val isVerticalTabBar = currentConfig.position.value == TabBarConfig.POSITION_LEFT || currentConfig.position.value == TabBarConfig.POSITION_RIGHT

            if (isVerticalTabBar) {
                // For vertical TabBar, itemsContainer has fixed width and wraps content height
                layoutParams = LayoutParams(
                    (DEFAULT_TAB_BAR_SIZE_DP * VERTICAL_TAB_BAR_WIDTH_MULTIPLIER * resources.displayMetrics.density).toInt(),
                    ViewGroup.LayoutParams.WRAP_CONTENT
                )
            } else {
                // For horizontal TabBar, itemsContainer matches parent width and has fixed height
                layoutParams = LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    (DEFAULT_TAB_BAR_SIZE_DP * resources.displayMetrics.density).toInt()
                )
            }

            // Gravity for aligning children (tab items) within itemsContainer
            gravity = if (orientation == VERTICAL) { // itemsContainer is vertical (TabBar is LEFT/RIGHT)
                Gravity.TOP or Gravity.CENTER_HORIZONTAL
            } else { // itemsContainer is horizontal (TabBar is TOP/BOTTOM)
                Gravity.CENTER
            }
        }
    }

    private fun performLayoutForPosition() {
        removeAllViews()

        val isBackgroundTransparent = config.backgroundColor == Color.TRANSPARENT ||
                                     (config.backgroundColor != null && Color.alpha(config.backgroundColor!!) < 255)

        when (config.position.value) {
            TabBarConfig.POSITION_TOP -> {
                addView(itemsContainer)
                if (!isBackgroundTransparent) {
                    addView(View(context).apply {
                        setBackgroundColor(config.borderStyle ?: TabBarConfig.DEFAULT_BORDER_COLOR)
                        layoutParams = LayoutParams(
                            ViewGroup.LayoutParams.MATCH_PARENT,
                            (ITEM_BORDER_THICKNESS_DP * resources.displayMetrics.density).toInt()
                        )
                    })
                }
            }
            TabBarConfig.POSITION_BOTTOM -> {
                if (!isBackgroundTransparent) {
                    addView(View(context).apply {
                        setBackgroundColor(config.borderStyle ?: TabBarConfig.DEFAULT_BORDER_COLOR)
                        layoutParams = LayoutParams(
                            ViewGroup.LayoutParams.MATCH_PARENT,
                            (ITEM_BORDER_THICKNESS_DP * resources.displayMetrics.density).toInt()
                        )
                    })
                }
                addView(itemsContainer)
            }
            TabBarConfig.POSITION_LEFT -> {
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
            TabBarConfig.POSITION_RIGHT -> {
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

    fun setConfig(newConfig: TabBarConfig) {
        if (!isValidConfig(newConfig)) {
            Log.w(TAG, "Invalid TabBar config provided")
            return
        }

        config = newConfig

        val isBackgroundTransparent = config.backgroundColor == Color.TRANSPARENT ||
                                     (config.backgroundColor != null && Color.alpha(config.backgroundColor!!) < 255)

        val tabBarBackgroundColor = when {
            config.backgroundColor == Color.TRANSPARENT -> Color.TRANSPARENT
            config.backgroundColor != null -> config.backgroundColor!!
            config.position.value == TabBarConfig.POSITION_LEFT || config.position.value == TabBarConfig.POSITION_RIGHT -> VERTICAL_TABBAR_BACKGROUND_COLOR
            else -> TabBarConfig.DEFAULT_BACKGROUND_COLOR
        }

        setBackgroundColor(tabBarBackgroundColor)
        elevation = if (isBackgroundTransparent) 0f else 8f * resources.displayMetrics.density

        updateItemsContainerLayout(this.config)
        performLayoutForPosition()
        setItems(newConfig.list)
        visibility = View.VISIBLE
    }

    fun setItems(newItems: List<TabBarItem>) {
        items = newItems.filter { it.visible }  // Only show items where visible is true

        itemsContainer?.let { container ->
            container.removeAllViews()
            tabViews.clear()

            if (items.isNotEmpty()) {
                val isVertical = config.position.value == TabBarConfig.POSITION_LEFT || config.position.value == TabBarConfig.POSITION_RIGHT

                // Get the size of the space for each item
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

                // Find selected item index (default to 0 if none specified)
                val initialSelectedIdx = items.indexOfFirst { it.selected }.takeIf { it >= 0 } ?: 0
                selectedPosition = initialSelectedIdx

                items.forEachIndexed { index, item ->
                    createTabView(item, config, index == selectedPosition).also { view ->
                        tabViews.add(view)
                        container.addView(view)
                    }
                }
            }
        }
    }

    /**
     * Show the tabBar
     * @param animation Whether to use animation
     */
    fun showTabBar(animation: Boolean = false) {
        setVisible(true, animation)
    }

    /**
     * Hide the tabBar
     * @param animation Whether to use animation
     */
    fun hideTabBar(animation: Boolean = false) {
        setVisible(false, animation)
    }

    /**
     * Show or hide the TabBar
     * @param visible Whether to show the TabBar
     * @param animation Whether to use animation
     */
    fun setVisible(visible: Boolean, animation: Boolean = false) {
        if (animation) {
            if (visible) {
                alpha = 0f
                visibility = View.VISIBLE
                animate().alpha(1f).setDuration(200).start()
            } else {
                animate().alpha(0f).setDuration(200).withEndAction {
                    visibility = View.GONE
                }.start()
            }
        } else {
            visibility = if (visible) View.VISIBLE else View.GONE
        }
    }

    fun setSelectedIndex(index: Int, notifyListener: Boolean = true) {
        if (index < 0 || index >= items.size || index >= tabViews.size) {
            return
        }

        if (index != selectedPosition) {
            val previousIndex = selectedPosition
            selectedPosition = index

            // Update UI state
            if (previousIndex >= 0 && previousIndex < tabViews.size) {
                updateTabState(tabViews[previousIndex], items[previousIndex], false)
            }
            updateTabState(tabViews[index], items[index], true)

            // Notify listener
            onTabSelectedListener?.invoke(index, items[index].pagePath)
        }
    }

    fun setOnTabSelectedListener(listener: (Int, String) -> Unit) {
        onTabSelectedListener = listener
    }

    fun setOnVisibilityChangedListener(listener: (Boolean) -> Unit) {
        onVisibilityChangedListener = listener
    }

    override fun onVisibilityChanged(changedView: View, visibility: Int) {
        super.onVisibilityChanged(changedView, visibility)
        if (changedView == this) {
            onVisibilityChangedListener?.invoke(visibility == View.VISIBLE)
        }
    }

    private fun createTabView(item: TabBarItem, config: TabBarConfig, isSelected: Boolean): LinearLayout {
        val isVertical = config.position.value == TabBarConfig.POSITION_LEFT || config.position.value == TabBarConfig.POSITION_RIGHT

        return LinearLayout(context).apply {
            orientation = VERTICAL
            gravity = Gravity.CENTER

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
                )
            }

            // Add icon
            val iconContainer = FrameLayout(context).apply {
                val iconSize = if (isVertical) VERTICAL_ITEM_ICON_SIZE_DP else HORIZONTAL_ITEM_ICON_SIZE_DP
                val iconSizePx = (iconSize * resources.displayMetrics.density).toInt()
                layoutParams = LayoutParams(iconSizePx, iconSizePx).apply {
                    gravity = Gravity.CENTER_HORIZONTAL
                }
            }

            val icon = ImageView(context).apply {
                layoutParams = FrameLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.MATCH_PARENT
                )
                scaleType = ImageView.ScaleType.FIT_CENTER
                setImageDrawable(getIconDrawable(item, isSelected))
            }

            iconContainer.addView(icon)
            addView(iconContainer)

            if (!item.text.isNullOrBlank()) {
                val textView = TextView(context).apply {
                    text = item.text
                    setTextColor(if (isSelected)
                        config.selectedColor ?: TabBarConfig.DEFAULT_SELECTED_COLOR
                        else config.color ?: TabBarConfig.DEFAULT_UNSELECTED_COLOR)
                    textSize = if (isVertical) VERTICAL_ITEM_TEXT_SIZE_SP else HORIZONTAL_ITEM_TEXT_SIZE_SP
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
                }
                addView(textView)
            }

            setOnClickListener {
                onTabSelectedListener?.invoke(items.indexOf(item), item.pagePath)
            }
        }
    }

    private fun updateTabStates() {
        items.forEachIndexed { index, item ->
            tabViews.getOrNull(index)?.let { view ->
                updateTabState(view, item, item.selected)
            }
        }
    }

    private fun updateSelection(selectedIndex: Int) {
        items.forEachIndexed { index, item ->
            tabViews.getOrNull(index)?.let { view ->
                updateTabState(view, item, index == selectedIndex)
            }
        }
    }

    private fun updateTabState(tabView: LinearLayout, item: TabBarItem, selected: Boolean) {
        val iconContainer = tabView.getChildAt(0) as FrameLayout
        val icon = iconContainer.getChildAt(0) as ImageView
        icon.setImageDrawable(getIconDrawable(item, selected))

        if (tabView.childCount > 1) {
            (tabView.getChildAt(1) as? TextView)?.setTextColor(
                if (selected)
                    config.selectedColor ?: TabBarConfig.DEFAULT_SELECTED_COLOR
                    else config.color ?: TabBarConfig.DEFAULT_UNSELECTED_COLOR
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
            setColor(if (selected)
                config.selectedColor ?: TabBarConfig.DEFAULT_SELECTED_COLOR
                else config.color ?: TabBarConfig.DEFAULT_UNSELECTED_COLOR)
            val size = (24 * resources.displayMetrics.density).toInt()
            setSize(size, size)
        }
    }

    /**
     * Show a red dot notification badge on a specific tab item
     * @param index The index of the tab item (counting from left)
     */
    fun showTabBarRedDot(index: Int) {
        if (index < 0 || index >= tabViews.size) {
            Log.d(TAG, "Invalid index for red dot: $index, tabViews size: ${tabViews.size}")
            return
        }

        val tabView = tabViews[index]
        val iconContainer = tabView.getChildAt(0) as? FrameLayout ?: return

        // Remove existing red dot if any
        iconContainer.findViewById<View>(RED_DOT_ID)?.let {
            (it.parent as? ViewGroup)?.removeView(it)
        }

        val dotSize = (8 * resources.displayMetrics.density).toInt()
        val redDot = View(context).apply {
            id = RED_DOT_ID
            layoutParams = FrameLayout.LayoutParams(dotSize, dotSize).apply {
                gravity = Gravity.TOP or Gravity.END
            }
            background = GradientDrawable().apply {
                shape = GradientDrawable.OVAL
                setColor(Color.RED)
                setSize(dotSize, dotSize)
            }
            visibility = View.VISIBLE
            setLayerType(View.LAYER_TYPE_SOFTWARE, null)
        }

        iconContainer.addView(redDot)
    }

    /**
     * Hide the red dot notification badge on a specific tab item
     * @param index The index of the tab item (counting from left)
     */
    fun hideTabBarRedDot(index: Int) {
        if (index < 0 || index >= tabViews.size) return
        tabViews[index].findViewById<View>(RED_DOT_ID)?.visibility = View.GONE
    }

    /**
     * Add a text badge to a specific tab item
     * @param index The index of the tab item (counting from left)
     * @param text The text to display, must not be empty
     */
    fun setTabBarBadge(index: Int, text: String) {
        if (index < 0 || index >= tabViews.size || text.isEmpty()) {
            Log.d(TAG, "Invalid index or empty text for badge: $index, text: '$text', tabViews size: ${tabViews.size}")
            return
        }

        val tabView = tabViews[index]
        // The anchor for the badge is the FrameLayout containing the icon
        val iconContainer = tabView.getChildAt(0) as? FrameLayout ?: return

        // Post the badge creation and attachment to the view's message queue
        // This ensures it runs after the layout pass
        iconContainer.post {
            // Wrap the original context with a Material Components theme
            val materialContext = ContextThemeWrapper(context, com.google.android.material.R.style.Theme_MaterialComponents_DayNight) // Or Theme_MaterialComponents_Light etc.

            // Create and configure the BadgeDrawable inside the post block using the themed context
            val badgeDrawable = BadgeDrawable.create(materialContext).apply {
                backgroundColor = Color.RED
                badgeTextColor = Color.WHITE
                // Add a positive vertical offset to shift the badge down
                verticalOffset = (6 * resources.displayMetrics.density).toInt()
                // horizontalOffset = (1 * resources.displayMetrics.density).toInt() // Adjust horizontal if needed too
                badgeGravity = BadgeDrawable.TOP_END // Position at top-end of the anchor

                // Set text or number based on content
                val number = text.toIntOrNull()
                if (number != null) {
                    this.number = number
                } else {
                    this.text = text // Use the 'text' property setter for non-numeric strings
                }
                isVisible = true
            }

            // Store the drawable for later removal (also inside post)
            badgeDrawables[iconContainer] = badgeDrawable

            // Attach the badge to the icon container (inside post)
            BadgeUtils.attachBadgeDrawable(badgeDrawable, iconContainer)
        }
    }

    /**
     * Remove the text badge (BadgeDrawable) from a specific tab item
     * @param index The index of the tab item (counting from left)
     */
    fun removeTabBarBadge(index: Int) {
        if (index < 0 || index >= tabViews.size) return
        val tabView = tabViews[index]
        val iconContainer = tabView.getChildAt(0) as? FrameLayout ?: return

        // Also post the removal to handle potential race conditions
        iconContainer.post {
            // Retrieve the stored BadgeDrawable for this container
            badgeDrawables.remove(iconContainer)?.let { badgeToRemove ->
                // Detach the specific BadgeDrawable from the icon container
                BadgeUtils.detachBadgeDrawable(badgeToRemove, iconContainer)
            }
        }
    }

    /**
     * Dynamically set the overall style of the TabBar
     * @param color Default text color for tabs
     * @param selectedColor Text color for selected tab
     * @param backgroundColor Background color of the TabBar
     * @param borderStyle Color of the TabBar's top border, only supports black/white
     */
    fun setTabBarStyle(
        color: String? = null,
        selectedColor: String? = null,
        backgroundColor: String? = null,
        borderStyle: String? = null
    ) {
        var updatedConfig = config
        color?.let { updatedConfig = updatedConfig.copy(color = Color.parseColor(it)) }
        selectedColor?.let { updatedConfig = updatedConfig.copy(selectedColor = Color.parseColor(it)) }
        backgroundColor?.let {
            val bgColor = Color.parseColor(it)
            updatedConfig = updatedConfig.copy(backgroundColor = bgColor)
            setBackgroundColor(bgColor)
        }
        borderStyle?.let {
            val borderColor = when(it.lowercase()) {
                "black" -> Color.BLACK
                "white" -> Color.WHITE
                else -> TabBarConfig.DEFAULT_BORDER_COLOR
            }
            updatedConfig = updatedConfig.copy(borderStyle = borderColor)
        }

        config = updatedConfig
        updateTabStates()
    }

    /**
     * Dynamically set the content of a specific tab item
     * @param index The index of the tab item (counting from left)
     * @param text Text label for the tab (null to hide text)
     * @param iconPath Path to the icon image
     * @param selectedIconPath Path to the selected state icon image
     */
    fun setTabBarItem(
        index: Int,
        text: String? = null,
        iconPath: String? = null,
        selectedIconPath: String? = null
    ) {
        if (index < 0 || index >= items.size) return

        val item = items[index]
        val newItem = item.copy(
            text = text ?: item.text,
            iconPath = iconPath ?: item.iconPath,
            selectedIconPath = selectedIconPath ?: item.selectedIconPath
        )

        items = items.toMutableList().apply {
            set(index, newItem)
        }

        // Need to recreate the tab items if text visibility changed
        setItems(items)
    }

    // Gets the index of the currently selected tab item
    fun getSelectedIndex(): Int {
        return selectedPosition
    }

    // Finds the index of a tab item by its pagePath
    fun findTabIndexByPath(path: String): Int {
        return items.indexOfFirst { it.pagePath == path }
    }

    private fun isValidConfig(config: TabBarConfig): Boolean {
        return config.list.isNotEmpty()
    }
}
