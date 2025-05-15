package com.lingxia.miniapp

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
import org.json.JSONObject
import com.google.android.material.badge.BadgeDrawable
import com.google.android.material.badge.BadgeUtils

data class TabBarConfig(
    val backgroundColor: Int? = null,            // Background color, default white
    val selectedColor: Int? = null,              // Selected item color, default tech blue
    val color: Int? = null,                      // Unselected item color, default gray
    val borderStyle: Int? = null,                // Top border color, default light gray
    val height: Int? = null,                     // Height in dp, default 56dp
    val position: Position = Position.BOTTOM,    // Position, default bottom
    val list: List<TabBarItem> = emptyList(),   // List of tab items
    val visible: Boolean = true                  // TabBar visibility, default true
) {
    enum class Position {
        TOP, BOTTOM, LEFT, RIGHT
    }

    companion object {
        // Modern UI color scheme
        val DEFAULT_SELECTED_COLOR = Color.parseColor("#1677FF")    // Primary blue
        val DEFAULT_UNSELECTED_COLOR = Color.parseColor("#666666")  // Dark gray
        val DEFAULT_BORDER_COLOR = Color.parseColor("#F0F0F0")      // Light gray
        val DEFAULT_BACKGROUND_COLOR = Color.WHITE                   // White

        fun fromJson(json: String?): TabBarConfig? {
            if (json.isNullOrEmpty()) return null

            return try {
                val jsonObject = JSONObject(json)
                val list = jsonObject.optJSONArray("list")?.let { array ->
                    (0 until array.length()).mapNotNull { i ->
                        try {
                            val item = array.getJSONObject(i)
                            TabBarItem(
                                pagePath = item.optString("pagePath", ""),
                                text = item.optString("text", ""),
                                iconPath = item.optString("iconPath", ""),
                                selectedIconPath = item.optString("selectedIconPath", ""),
                                selected = item.optBoolean("selected", false)
                            )
                        } catch (e: Exception) {
                            null
                        }
                    }
                } ?: emptyList()

                TabBarConfig(
                    backgroundColor = if (jsonObject.has("backgroundColor")) parseColor(jsonObject.optString("backgroundColor"), null) else null,
                    selectedColor = if (jsonObject.has("selectedColor")) parseColor(jsonObject.optString("selectedColor"), null) else null,
                    color = if (jsonObject.has("color")) parseColor(jsonObject.optString("color"), null) else null,
                    borderStyle = if (jsonObject.has("borderStyle")) parseColor(jsonObject.optString("borderStyle"), null) else null,
                    position = when (jsonObject.optString("position", "bottom").lowercase()) {
                        "top" -> Position.TOP
                        "left" -> Position.LEFT
                        "right" -> Position.RIGHT
                        else -> Position.BOTTOM
                    },
                    list = list
                )
            } catch (e: Exception) {
                Log.e("TabBar", "Error parsing TabBar config: ${e.message}")
                null
            }
        }

        private fun parseColor(colorString: String?, defaultColor: Int?): Int? {
            if (colorString.isNullOrEmpty()) return defaultColor
            return try {
                Color.parseColor(colorString)
            } catch (e: Exception) {
                defaultColor
            }
        }
    }
}

data class TabBarItem(
    val pagePath: String,                 // Page path to navigate to
    val text: String,                     // Tab text label
    val iconPath: String,                 // Absolute path to the icon file
    val selectedIconPath: String,         // Absolute path to the selected state icon file
    val selected: Boolean = false,        // Whether this tab is selected
    val visible: Boolean = true          // Whether this tab is visible
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
        private const val VERTICAL_ITEM_ICON_SIZE_DP = 24
        private const val HORIZONTAL_ITEM_ICON_SIZE_DP = 28
        private const val VERTICAL_ITEM_TEXT_SIZE_SP = 12f
        private const val HORIZONTAL_ITEM_TEXT_SIZE_SP = 13f
        private const val ITEM_ICON_TOP_MARGIN_DP = 2
        private const val ITEM_TEXT_TOP_MARGIN_DP = 2
        private const val ITEM_TEXT_BOTTOM_MARGIN_DP = 2
        private const val ITEM_BORDER_THICKNESS_DP = 1f

        // Padding for individual TabBarItems when TabBar is horizontal
        private const val HORIZONTAL_ITEM_PADDING_SIDES_DP = 4
        private const val HORIZONTAL_ITEM_PADDING_VERTICAL_DP = 3

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
    private var tabSelectedListener: ((Int, String) -> Unit)? = null
    private var onVisibilityChangedListener: ((Boolean) -> Unit)? = null

    init {
        orientation = when (config.position) {
            TabBarConfig.Position.LEFT, TabBarConfig.Position.RIGHT -> HORIZONTAL
            else -> VERTICAL
        }
        setBackgroundColor(config.backgroundColor ?: TabBarConfig.DEFAULT_BACKGROUND_COLOR)
        elevation = 8f * resources.displayMetrics.density
        visibility = View.GONE  // Hidden by default until valid config is set

        itemsContainer = LinearLayout(context)
        // Configure itemsContainer with the initial default config
        updateItemsContainerLayout(this.config)

        // Perform initial layout of TabBar's direct children (border, itemsContainer)
        // This will use the default config for the first pass.
        updateLayoutForPosition()
    }

    private fun updateItemsContainerLayout(currentConfig: TabBarConfig) {
        itemsContainer?.apply {
            orientation = when (currentConfig.position) {
                TabBarConfig.Position.LEFT, TabBarConfig.Position.RIGHT -> VERTICAL
                else -> HORIZONTAL
            }

            val isVerticalTabBar = currentConfig.position == TabBarConfig.Position.LEFT || currentConfig.position == TabBarConfig.Position.RIGHT

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
            // Set background for itemsContainer itself
            setBackgroundColor(currentConfig.backgroundColor ?: TabBarConfig.DEFAULT_BACKGROUND_COLOR)
        }
    }

    private fun updateLayoutForPosition() {
        removeAllViews()

        when (config.position) {
            TabBarConfig.Position.TOP -> {
                addView(itemsContainer)
                addView(View(context).apply {
                    setBackgroundColor(config.borderStyle ?: TabBarConfig.DEFAULT_BORDER_COLOR)
                    layoutParams = LayoutParams(
                        ViewGroup.LayoutParams.MATCH_PARENT,
                        (ITEM_BORDER_THICKNESS_DP * resources.displayMetrics.density).toInt()
                    )
                })
            }
            TabBarConfig.Position.BOTTOM -> {
                addView(View(context).apply {
                    setBackgroundColor(config.borderStyle ?: TabBarConfig.DEFAULT_BORDER_COLOR)
                    layoutParams = LayoutParams(
                        ViewGroup.LayoutParams.MATCH_PARENT,
                        (ITEM_BORDER_THICKNESS_DP * resources.displayMetrics.density).toInt()
                    )
                })
                addView(itemsContainer)
            }
            TabBarConfig.Position.LEFT -> {
                orientation = HORIZONTAL
                addView(itemsContainer)
                // Add darker border for better visual separation
                addView(View(context).apply {
                    setBackgroundColor(VERTICAL_BORDER_COLOR)
                    layoutParams = LayoutParams(
                        (ITEM_BORDER_THICKNESS_DP * resources.displayMetrics.density).toInt(),
                        ViewGroup.LayoutParams.MATCH_PARENT
                    )
                })
            }
            TabBarConfig.Position.RIGHT -> {
                orientation = HORIZONTAL
                // Add darker border for better visual separation
                addView(View(context).apply {
                    setBackgroundColor(VERTICAL_BORDER_COLOR)
                    layoutParams = LayoutParams(
                        (ITEM_BORDER_THICKNESS_DP * resources.displayMetrics.density).toInt(),
                        ViewGroup.LayoutParams.MATCH_PARENT
                    )
                })
                addView(itemsContainer)
            }
        }
    }

    fun setConfig(newConfig: TabBarConfig?) {
        if (newConfig == null) {
            visibility = View.GONE
            return
        }

        config = newConfig

        // Set appropriate background color for TabBar itself based on newConfig
        if (config.position == TabBarConfig.Position.LEFT || config.position == TabBarConfig.Position.RIGHT) {
            setBackgroundColor(config.backgroundColor ?: VERTICAL_TABBAR_BACKGROUND_COLOR)
        } else {
            setBackgroundColor(config.backgroundColor ?: TabBarConfig.DEFAULT_BACKGROUND_COLOR)
        }

        // Reconfigure itemsContainer based on the new config BEFORE adding it to TabBar layout
        updateItemsContainerLayout(this.config)

        updateLayoutForPosition() // This will arrange TabBar's children, including the updated itemsContainer
        setItems(config.list)
        visibility = if (config.visible) View.VISIBLE else View.GONE
    }

    fun setItems(newItems: List<TabBarItem>) {
        items = newItems.filter { it.visible }  // Only show items where visible is true

        itemsContainer?.let { container ->
            container.removeAllViews()
            tabViews.clear()

            if (items.isNotEmpty()) {
                val isVertical = config.position == TabBarConfig.Position.LEFT || config.position == TabBarConfig.Position.RIGHT

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
                    createTabItem(item, index == selectedPosition, itemSize).also { view ->
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

            // Optionally notify listener
            if (notifyListener) {
                tabSelectedListener?.invoke(index, items[index].pagePath)
            }
        }
    }

    fun setOnTabSelectedListener(listener: (Int, String) -> Unit) {
        tabSelectedListener = listener
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

    private fun createTabItem(item: TabBarItem, isSelected: Boolean, width: Int): LinearLayout {
        val isVertical = config.position == TabBarConfig.Position.LEFT || config.position == TabBarConfig.Position.RIGHT

        return LinearLayout(context).apply {
            // Always use VERTICAL orientation (icon on top, text below) regardless of TabBar position
            orientation = VERTICAL
            gravity = Gravity.CENTER

            // Add padding based on position
            if (isVertical) {
                setPadding(
                    (VERTICAL_ITEM_PADDING_HORIZONTAL_DP * resources.displayMetrics.density).toInt(),
                    (VERTICAL_ITEM_PADDING_VERTICAL_DP * resources.displayMetrics.density).toInt(),
                    (VERTICAL_ITEM_PADDING_HORIZONTAL_DP * resources.displayMetrics.density).toInt(),
                    (VERTICAL_ITEM_PADDING_VERTICAL_DP * resources.displayMetrics.density).toInt()
                )
            } else {
                setPadding(
                    (HORIZONTAL_ITEM_PADDING_SIDES_DP * resources.displayMetrics.density).toInt(),
                    (HORIZONTAL_ITEM_PADDING_VERTICAL_DP * resources.displayMetrics.density).toInt(),
                    (HORIZONTAL_ITEM_PADDING_SIDES_DP * resources.displayMetrics.density).toInt(),
                    (HORIZONTAL_ITEM_PADDING_VERTICAL_DP * resources.displayMetrics.density).toInt()
                )
            }

            layoutParams = if (isVertical) {
                LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, width)
            } else {
                LayoutParams(width, ViewGroup.LayoutParams.MATCH_PARENT)
            }

            // Change background for selected state in vertical layout
            if (isVertical && isSelected) {
                background = GradientDrawable().apply {
                    cornerRadius = INITIAL_SELECTED_ITEM_CORNER_RADIUS_DP * resources.displayMetrics.density
                    setColor(VERTICAL_SELECTED_ITEM_BACKGROUND_COLOR)
                }
            }

            // Create a FrameLayout to wrap the icon and allow for badge overlay
            val iconContainer = FrameLayout(context).apply {
                layoutParams = LayoutParams(
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT
                ).apply {
                    // Center horizontally and put at top for all positions
                    gravity = Gravity.CENTER_HORIZONTAL
                    topMargin = (ITEM_ICON_TOP_MARGIN_DP * resources.displayMetrics.density).toInt()
                    clipChildren = false
                    clipToPadding = false
                }
            }

            // Add icon to the container
            val iconSize = if (isVertical) {
                (VERTICAL_ITEM_ICON_SIZE_DP * resources.displayMetrics.density).toInt()  // Smaller icon for vertical layout
            } else {
                (HORIZONTAL_ITEM_ICON_SIZE_DP * resources.displayMetrics.density).toInt()
            }

            val icon = ImageView(context).apply {
                layoutParams = FrameLayout.LayoutParams(iconSize, iconSize).apply {
                    gravity = Gravity.CENTER
                }

                val iconDrawable = getIconDrawable(item, isSelected)
                setImageDrawable(iconDrawable)
                if (iconDrawable is GradientDrawable) {
                    setColorFilter(if (isSelected)
                        config.selectedColor ?: TabBarConfig.DEFAULT_SELECTED_COLOR
                        else config.color ?: TabBarConfig.DEFAULT_UNSELECTED_COLOR)
                }
                scaleType = ImageView.ScaleType.FIT_CENTER
            }
            iconContainer.addView(icon)
            addView(iconContainer)

            // Add text label
            val textView = TextView(context).apply {
                text = item.text
                setTextColor(if (isSelected)
                    config.selectedColor ?: TabBarConfig.DEFAULT_SELECTED_COLOR
                    else config.color ?: TabBarConfig.DEFAULT_UNSELECTED_COLOR)

                // Different text sizes based on position
                textSize = if (isVertical) VERTICAL_ITEM_TEXT_SIZE_SP else HORIZONTAL_ITEM_TEXT_SIZE_SP

                // Always place text below icon with consistent layout
                layoutParams = LayoutParams(
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT
                ).apply {
                    topMargin = (ITEM_TEXT_TOP_MARGIN_DP * resources.displayMetrics.density).toInt()
                    bottomMargin = (ITEM_TEXT_BOTTOM_MARGIN_DP * resources.displayMetrics.density).toInt()
                }

                gravity = Gravity.CENTER
                isSingleLine = true
                // Add ellipsis if text is too long
                ellipsize = android.text.TextUtils.TruncateAt.END
            }
            addView(textView)

            // Set click listener for the whole item
            setOnClickListener {
                val clickedIndex = tabViews.indexOf(this)
                if (clickedIndex >= 0 && clickedIndex != selectedPosition) {
                    // Prevent flashing by updating UI before notifying listener
                    val previousSelected = selectedPosition
                    selectedPosition = clickedIndex

                    // First update visual state for immediate feedback
                    if (previousSelected >= 0 && previousSelected < tabViews.size) {
                        // Update previous selected tab to unselected state
                        updateTabState(tabViews[previousSelected], items[previousSelected], false)
                    }
                    // Update new tab to selected state
                    updateTabState(this, items[clickedIndex], true)

                    // Then notify listener after visual update
                    tabSelectedListener?.invoke(clickedIndex, items[clickedIndex].pagePath)
                }
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
        val isVertical = config.position == TabBarConfig.Position.LEFT || config.position == TabBarConfig.Position.RIGHT

        // Update background for vertical layout
        if (isVertical) {
            if (selected) {
                tabView.background = GradientDrawable().apply {
                    cornerRadius = SELECTED_ITEM_CORNER_RADIUS_DP * resources.displayMetrics.density
                    setColor(VERTICAL_SELECTED_ITEM_BACKGROUND_COLOR)
                }
            } else {
                tabView.background = null
            }
        }

        // Update Icon inside the FrameLayout container
        (tabView.getChildAt(0) as? FrameLayout)?.let { iconContainer ->
            (iconContainer.getChildAt(0) as? ImageView)?.apply {
                val iconDrawable = getIconDrawable(item, selected)
                setImageDrawable(iconDrawable)
                // Apply color filter only for default icon (GradientDrawable)
                if (iconDrawable is GradientDrawable) {
                    setColorFilter(if (selected)
                        config.selectedColor ?: TabBarConfig.DEFAULT_SELECTED_COLOR
                        else config.color ?: TabBarConfig.DEFAULT_UNSELECTED_COLOR)
                } else {
                    // Clear any previous filter if using a custom icon
                    clearColorFilter()
                }
            }
        }

        // Update Text Color - The text view index is 1 regardless of orientation
        (tabView.getChildAt(1) as? TextView)?.setTextColor(
            if (selected)
                config.selectedColor ?: TabBarConfig.DEFAULT_SELECTED_COLOR
                else config.color ?: TabBarConfig.DEFAULT_UNSELECTED_COLOR
        )
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
            val size = (28 * resources.displayMetrics.density).toInt()
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
     * @param text Text label for the tab
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

        updateTabStates()
    }

    // Gets the index of the currently selected tab item
    fun getSelectedIndex(): Int {
        return selectedPosition
    }

    // Finds the index of a tab item by its pagePath
    fun findTabIndexByPath(path: String): Int {
        return items.indexOfFirst { it.pagePath == path }
    }
}
