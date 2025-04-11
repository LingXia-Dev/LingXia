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
        TOP, BOTTOM
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
                    position = if (jsonObject.optString("position", "bottom") == "top") Position.TOP else Position.BOTTOM,
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
    }

    private var config = TabBarConfig()
    private var items = listOf<TabBarItem>()
    private var tabViews = mutableListOf<LinearLayout>()
    private var itemsContainer: LinearLayout? = null
    private var selectedPosition = -1
    private var tabSelectedListener: ((Int, String) -> Unit)? = null
    private var onVisibilityChangedListener: ((Boolean) -> Unit)? = null

    init {
        orientation = VERTICAL
        setBackgroundColor(config.backgroundColor ?: TabBarConfig.DEFAULT_BACKGROUND_COLOR)
        elevation = 8f * resources.displayMetrics.density
        visibility = View.GONE  // Hidden by default until valid config is set

        itemsContainer = LinearLayout(context).apply {
            orientation = HORIZONTAL
            layoutParams = LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            gravity = Gravity.CENTER
            setBackgroundColor(config.backgroundColor ?: TabBarConfig.DEFAULT_BACKGROUND_COLOR)
        }
        addView(itemsContainer)
    }

    private fun updateLayoutForPosition() {
        removeAllViews()

        if (config.position == TabBarConfig.Position.BOTTOM) {
            addView(View(context).apply {
                setBackgroundColor(config.borderStyle ?: TabBarConfig.DEFAULT_BORDER_COLOR)
                layoutParams = LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    (1f * resources.displayMetrics.density).toInt()
                )
            })
        }

        itemsContainer?.let { container ->
            addView(container)
        }
    }

    fun setConfig(newConfig: TabBarConfig?) {
        if (newConfig == null) {
            visibility = View.GONE
            return
        }

        config = newConfig
        setBackgroundColor(config.backgroundColor ?: TabBarConfig.DEFAULT_BACKGROUND_COLOR)
        updateLayoutForPosition()
        setItems(config.list)
        visibility = if (config.visible) View.VISIBLE else View.GONE
    }

    fun setItems(newItems: List<TabBarItem>) {
        items = newItems.filter { it.visible }  // Only show items where visible is true

        itemsContainer?.let { container ->
            container.removeAllViews()
            tabViews.clear()

            if (items.isNotEmpty()) {
                val itemWidth = resources.displayMetrics.widthPixels / items.size

                items.forEachIndexed { _, item ->
                    createTabItem(item, itemWidth).also { view ->
                        tabViews.add(view)
                        container.addView(view)
                    }
                }
                updateTabStates()
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
        if (index in items.indices && index != selectedPosition) {
            selectedPosition = index
            updateSelection(index)
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

    private fun createTabItem(item: TabBarItem, width: Int): LinearLayout {
        return LinearLayout(context).apply {
            orientation = VERTICAL
            gravity = Gravity.CENTER
            layoutParams = LayoutParams(width, ViewGroup.LayoutParams.MATCH_PARENT)

            // Create a FrameLayout to wrap the icon and allow for badge overlay
            val iconContainer = FrameLayout(context).apply {
                layoutParams = LayoutParams(
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT
                ).apply {
                    gravity = Gravity.CENTER_HORIZONTAL
                    topMargin = (4 * resources.displayMetrics.density).toInt()
                    clipChildren = false
                    clipToPadding = false
                }
            }

            // Add icon to the container
            val iconSize = (28 * resources.displayMetrics.density).toInt()
            val icon = ImageView(context).apply {
                layoutParams = FrameLayout.LayoutParams(iconSize, iconSize).apply {
                    gravity = Gravity.CENTER
                }

                val iconDrawable = getIconDrawable(item, item.selected)
                setImageDrawable(iconDrawable)
                if (iconDrawable is GradientDrawable) {
                    setColorFilter(if (item.selected)
                        config.selectedColor ?: TabBarConfig.DEFAULT_SELECTED_COLOR
                        else config.color ?: TabBarConfig.DEFAULT_UNSELECTED_COLOR)
                }
                scaleType = ImageView.ScaleType.FIT_CENTER
            }
            iconContainer.addView(icon)
            addView(iconContainer)

            // Add text label
            addView(TextView(context).apply {
                text = item.text
                setTextColor(if (item.selected)
                    config.selectedColor ?: TabBarConfig.DEFAULT_SELECTED_COLOR
                    else config.color ?: TabBarConfig.DEFAULT_UNSELECTED_COLOR)
                textSize = 13f
                gravity = Gravity.CENTER
                layoutParams = LayoutParams(
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT
                ).apply {
                    topMargin = (2 * resources.displayMetrics.density).toInt()
                    bottomMargin = (6 * resources.displayMetrics.density).toInt()
                }
            })

            // Set click listener for the whole item
            setOnClickListener {
                val clickedIndex = tabViews.indexOf(this)
                if (clickedIndex >= 0) {
                    onTabItemClick(clickedIndex)
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

        // Update Text Color (This part was likely correct)
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

    private fun onTabItemClick(position: Int) {
        if (position == selectedPosition) return

        selectedPosition = position
        updateSelection(position)

        tabSelectedListener?.invoke(position, items[position].pagePath)
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
