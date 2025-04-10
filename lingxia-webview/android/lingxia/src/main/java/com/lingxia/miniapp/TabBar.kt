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
import org.json.JSONObject

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

class TabBar(context: Context) : LinearLayout(context) {
    companion object {
        private const val TAG = "LingXia.TabBar"
    }

    private var config = TabBarConfig()
    private var items = listOf<TabBarItem>()
    private var tabViews = mutableListOf<LinearLayout>()
    private var onTabSelectedListener: ((Int, String) -> Unit)? = null
    private var itemsContainer: LinearLayout? = null
    private var selectedPosition = -1

    init {
        orientation = VERTICAL
        setBackgroundColor(config.backgroundColor ?: TabBarConfig.DEFAULT_BACKGROUND_COLOR)
        elevation = 8f * resources.displayMetrics.density
        visibility = View.GONE  // 默认隐藏，直到设置有效的配置

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

    fun setVisible(visible: Boolean) {
        visibility = if (visible) View.VISIBLE else View.GONE
        // Update WebView container margin when visibility changes
        (parent as? FrameLayout)?.let { parentFrame ->
            (parentFrame.getChildAt(0) as? FrameLayout)?.let { webViewContainer ->
                webViewContainer.layoutParams = (webViewContainer.layoutParams as? FrameLayout.LayoutParams)?.apply {
                    if (config.position == TabBarConfig.Position.TOP) {
                        topMargin = if (visible) (56 * resources.displayMetrics.density).toInt() else 0
                    } else {
                        bottomMargin = if (visible) (56 * resources.displayMetrics.density).toInt() else 0
                    }
                }
            }
        }
    }

    fun setSelectedIndex(index: Int) {
        if (index in items.indices) {
            updateSelection(index)
            onTabSelectedListener?.invoke(index, items[index].pagePath)
        }
    }

    fun setOnTabSelectedListener(listener: (Int, String) -> Unit) {
        onTabSelectedListener = listener
    }

    private fun createTabItem(item: TabBarItem, width: Int): LinearLayout {
        return LinearLayout(context).apply {
            orientation = VERTICAL
            gravity = Gravity.CENTER
            layoutParams = LayoutParams(width, ViewGroup.LayoutParams.MATCH_PARENT)

            val iconSize = (28 * resources.displayMetrics.density).toInt()
            val icon = ImageView(context).apply {
                layoutParams = LayoutParams(iconSize, iconSize).apply {
                    topMargin = (4 * resources.displayMetrics.density).toInt()
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
            addView(icon)

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

            setOnClickListener {
                val clickedIndex = tabViews.indexOf(this)
                if (clickedIndex >= 0) {
                    if (clickedIndex == 1) {
                        setVisible(false)
                        return@setOnClickListener
                    }
                    updateSelection(clickedIndex)
                    onTabSelectedListener?.invoke(clickedIndex, item.pagePath)
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
        (tabView.getChildAt(0) as? ImageView)?.apply {
            val iconDrawable = getIconDrawable(item, selected)
            setImageDrawable(iconDrawable)
            // Apply color filter only for default icon
            if (iconDrawable is GradientDrawable) {
                setColorFilter(if (selected)
                    config.selectedColor ?: TabBarConfig.DEFAULT_SELECTED_COLOR
                    else config.color ?: TabBarConfig.DEFAULT_UNSELECTED_COLOR)
            } else {
                colorFilter = null
            }
        }

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

        // Hide TabBar when second item is clicked
        if (position == 1) {
            visibility = View.GONE
            return
        }

        selectedPosition = position
        updateTabStates()
        onTabSelectedListener?.invoke(position, items[position].pagePath)
    }
}
