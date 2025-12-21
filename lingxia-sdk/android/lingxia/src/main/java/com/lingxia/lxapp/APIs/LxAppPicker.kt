package com.lingxia.lxapp.APIs

import android.app.Activity
import android.content.Context
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.util.Log
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.*
import androidx.core.view.ViewCompat
import com.lingxia.lxapp.LxApp
import com.lingxia.lxapp.NativeApi
import org.json.JSONArray
import org.json.JSONObject

/**
 * Picker configuration data class
 */
data class PickerConfig(
    val mode: String, // "selector" or "multiSelector"
    val range: List<Any>, // List<String> for selector, List<List<String>> for multiSelector
    val value: List<Int>, // Initial selected indices
    val cascading: Boolean = false, // Whether dual column is cascading
    val cancelText: String,
    val cancelButtonColor: String,
    val cancelTextColor: String,
    val confirmText: String,
    val confirmButtonColor: String,
    val confirmTextColor: String
)

/**
 * LingXia Picker implementation for Android
 */
object LxAppPicker {
    private const val TAG = "LingXia.LxAppPicker"

    private var currentPickerView: View? = null
    private var currentMaskView: View? = null
    private var currentSelectedIndices = mutableListOf<Int>()
    private var currentMode: String = "selector"
    private var cascadingData: Map<String, List<String>>? = null
    private var firstColumnItems: List<String>? = null
    private var secondColumnPicker: View? = null // Track current picker mode

    @JvmStatic
    fun showSingleColumnPicker(
        options: Array<String>,
        cancelText: String,
        cancelButtonColor: String,
        cancelTextColor: String,
        confirmText: String,
        confirmButtonColor: String,
        confirmTextColor: String,
        callbackId: Long
    ) {
        val activity = LxApp.getCurrentActivity()
        if (activity == null) {
            Log.e(TAG, "showSingleColumnPicker: current activity is null")
            sendPickerResultCancel(callbackId)
            return
        }

        val config = PickerConfig(
            mode = "selector",
            range = options.toList(),
            value = listOf(0),
            cascading = false,
            cancelText = cancelText,
            cancelButtonColor = cancelButtonColor,
            cancelTextColor = cancelTextColor,
            confirmText = confirmText,
            confirmButtonColor = confirmButtonColor,
            confirmTextColor = confirmTextColor
        )

        currentSelectedIndices.clear()
        currentSelectedIndices.addAll(config.value)
        currentMode = config.mode
        cascadingData = null
        firstColumnItems = null

        activity.runOnUiThread {
            showPickerInternal(activity, config, callbackId)
        }
    }

    @JvmStatic
    fun showDualColumnPicker(
        firstColumn: Array<String>,
        secondColumn: Array<String>,
        cancelText: String,
        cancelButtonColor: String,
        cancelTextColor: String,
        confirmText: String,
        confirmButtonColor: String,
        confirmTextColor: String,
        callbackId: Long
    ) {
        val activity = LxApp.getCurrentActivity()
        if (activity == null) {
            Log.e(TAG, "showDualColumnPicker: current activity is null")
            sendPickerResultCancel(callbackId)
            return
        }

        val config = PickerConfig(
            mode = "multiSelector",
            range = listOf(firstColumn.toList(), secondColumn.toList()),
            value = listOf(0, 0),
            cascading = false,
            cancelText = cancelText,
            cancelButtonColor = cancelButtonColor,
            cancelTextColor = cancelTextColor,
            confirmText = confirmText,
            confirmButtonColor = confirmButtonColor,
            confirmTextColor = confirmTextColor
        )

        currentSelectedIndices.clear()
        currentSelectedIndices.addAll(config.value)
        currentMode = config.mode
        cascadingData = null
        firstColumnItems = null

        activity.runOnUiThread {
            showPickerInternal(activity, config, callbackId)
        }
    }

    @JvmStatic
    fun showCascadingPicker(
        firstColumn: Array<String>,
        keys: Array<String>,
        values: Array<Array<String>>,
        cancelText: String,
        cancelButtonColor: String,
        cancelTextColor: String,
        confirmText: String,
        confirmButtonColor: String,
        confirmTextColor: String,
        callbackId: Long
    ) {
        val activity = LxApp.getCurrentActivity()
        if (activity == null) {
            Log.e(TAG, "showCascadingPicker: current activity is null")
            sendPickerResultCancel(callbackId)
            return
        }

        val cascadingMap = mutableMapOf<String, List<String>>().apply {
            for (index in keys.indices) {
                val key = keys[index]
                val columnValues = values.getOrNull(index)?.toList() ?: emptyList()
                put(key, columnValues)
            }
        }

        val config = PickerConfig(
            mode = "multiSelector",
            range = listOf(firstColumn.toList(), cascadingMap as Map<String, List<String>>),
            value = listOf(0, 0),
            cascading = true,
            cancelText = cancelText,
            cancelButtonColor = cancelButtonColor,
            cancelTextColor = cancelTextColor,
            confirmText = confirmText,
            confirmButtonColor = confirmButtonColor,
            confirmTextColor = confirmTextColor
        )

        currentSelectedIndices.clear()
        currentSelectedIndices.addAll(config.value)
        currentMode = config.mode
        cascadingData = cascadingMap
        firstColumnItems = firstColumn.toList()

        activity.runOnUiThread {
            showPickerInternal(activity, config, callbackId)
        }
    }

    @JvmStatic
    fun hidePicker() {
        LxApp.getCurrentActivity()?.runOnUiThread {
            hidePickerInternal()
        } ?: hidePickerInternal()
    }

    private fun showPickerInternal(context: Context, config: PickerConfig, callbackId: Long) {

        val activity = context as? Activity ?: return
        val rootView = activity.findViewById<ViewGroup>(android.R.id.content) ?: return

        // Hide any existing picker first
        hidePickerInternal()

        // Create mask
        currentMaskView = createMaskView(activity) {
            // Cancel on mask click
            sendPickerResultCancel(callbackId)
            hidePickerInternal()
        }
        rootView.addView(currentMaskView)

        // Create picker view
        currentPickerView = createPickerView(activity, config, callbackId)
        rootView.addView(currentPickerView)
    }

    private fun createMaskView(context: Context, onCancel: () -> Unit): View {
        return View(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
            setBackgroundColor(Color.parseColor("#80000000"))
            isClickable = true
            setOnClickListener { onCancel() }
        }
    }

    private fun createPickerView(context: Context, config: PickerConfig, callbackId: Long): View {
        val container = FrameLayout(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
        }

        val pickerContent = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(Color.WHITE)

            // Add rounded corners
            val cornerRadius = (12 * context.resources.displayMetrics.density).toInt()
            val drawable = GradientDrawable().apply {
                setColor(Color.WHITE)
                cornerRadii = floatArrayOf(
                    cornerRadius.toFloat(), cornerRadius.toFloat(), // top-left
                    cornerRadius.toFloat(), cornerRadius.toFloat(), // top-right
                    0f, 0f, // bottom-right
                    0f, 0f  // bottom-left
                )
            }
            background = drawable

            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                gravity = Gravity.BOTTOM
            }
        }

        // Add picker wheels first (WeChat style - wheels at top)
        val wheelContainer = createWheelContainer(context, config, callbackId)
        pickerContent.addView(wheelContainer)

        // Add subtle separator line
        val separator = View(context).apply {
            setBackgroundColor(Color.parseColor("#F0F0F0"))
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                (0.5 * context.resources.displayMetrics.density).toInt()
            )
        }
        pickerContent.addView(separator)

        // Add buttons at bottom (WeChat style)
        val buttonsContainer = createButtonsContainer(context, config, callbackId)
        pickerContent.addView(buttonsContainer)

        container.addView(pickerContent)

        // Use Activity-provided bottom inset (encapsulated helper)
        com.lingxia.lxapp.util.ActivityInsets.applyBottomMargin(container, pickerContent, 0)
        return container
    }

    private fun createButtonsContainer(
        context: Context,
        config: PickerConfig,
        callbackId: Long
    ): LinearLayout {
        return LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER
            val paddingPx = (20 * context.resources.displayMetrics.density).toInt()
            setPadding(paddingPx, paddingPx, paddingPx, paddingPx)

            setBackgroundColor(Color.WHITE)

            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                (80 * context.resources.displayMetrics.density).toInt()
            )

            val cancelButton = TextView(context).apply {
                text = config.cancelText
                textSize = 18f

                try {
                    setTextColor(Color.parseColor(config.cancelTextColor))
                } catch (e: Exception) {
                    Log.w(TAG, "Failed to parse cancel text color: ${config.cancelTextColor}, using default")
                    setTextColor(Color.parseColor("#007AFF"))
                }

                gravity = Gravity.CENTER
                isClickable = true
                isFocusable = true

                val buttonHeight = (44 * context.resources.displayMetrics.density).toInt()
                val buttonPadding = (16 * context.resources.displayMetrics.density).toInt()
                setPadding(buttonPadding, 0, buttonPadding, 0)
                minHeight = buttonHeight

                val drawable = GradientDrawable().apply {
                    try {
                        setColor(Color.parseColor(config.cancelButtonColor))
                    } catch (e: Exception) {
                        Log.w(TAG, "Failed to parse cancel button color: ${config.cancelButtonColor}, using default")
                        setColor(Color.parseColor("#F2F2F2"))
                    }
                    cornerRadius = (8 * context.resources.displayMetrics.density)
                }
                background = drawable

                // Set explicit layout params with minimum width
                layoutParams = LinearLayout.LayoutParams(
                    (120 * context.resources.displayMetrics.density).toInt(),
                    (44 * context.resources.displayMetrics.density).toInt()
                ).apply {
                    weight = 0f
                }

                setOnClickListener {
                    sendPickerResultCancel(callbackId)
                    hidePickerInternal()
                }
            }
            addView(cancelButton)

            val spacer = View(context).apply {
                layoutParams = LinearLayout.LayoutParams(
                    (16 * context.resources.displayMetrics.density).toInt(), // 16dp fixed spacing
                    0
                )
            }
            addView(spacer)

            val confirmButton = TextView(context).apply {
                text = config.confirmText
                textSize = 18f

                try {
                    setTextColor(Color.parseColor(config.confirmTextColor))
                } catch (e: Exception) {
                    Log.w(TAG, "Failed to parse confirm text color: ${config.confirmTextColor}, using default")
                    setTextColor(Color.parseColor("#FFFFFF"))
                }

                gravity = Gravity.CENTER
                isClickable = true
                isFocusable = true

                val buttonHeight = (44 * context.resources.displayMetrics.density).toInt()
                val buttonPadding = (16 * context.resources.displayMetrics.density).toInt()
                setPadding(buttonPadding, 0, buttonPadding, 0)
                minHeight = buttonHeight

                val drawable = GradientDrawable().apply {
                    try {
                        setColor(Color.parseColor(config.confirmButtonColor))
                    } catch (e: Exception) {
                        Log.w(TAG, "Failed to parse confirm button color: ${config.confirmButtonColor}, using default")
                        setColor(Color.parseColor("#007AFF"))
                    }
                    cornerRadius = (8 * context.resources.displayMetrics.density)
                }
                background = drawable

                // Set explicit layout params with minimum width
                layoutParams = LinearLayout.LayoutParams(
                    (120 * context.resources.displayMetrics.density).toInt(),
                    (44 * context.resources.displayMetrics.density).toInt()
                ).apply {
                    weight = 0f
                    leftMargin = (8 * context.resources.displayMetrics.density).toInt()
                }

                setOnClickListener {
                    sendPickerResultConfirm(callbackId)
                    hidePickerInternal()
                }
            }
            addView(confirmButton)
        }
    }

    private fun createWheelContainer(context: Context, config: PickerConfig, callbackId: Long): FrameLayout {
        return FrameLayout(context).apply {
            val paddingPx = (20 * context.resources.displayMetrics.density).toInt()
            setPadding(paddingPx, paddingPx, paddingPx, paddingPx)

            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                (200 * context.resources.displayMetrics.density).toInt()
            )

            // Add picker container
            val pickerContainer = LinearLayout(context).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER
                layoutParams = FrameLayout.LayoutParams(
                    FrameLayout.LayoutParams.MATCH_PARENT,
                    FrameLayout.LayoutParams.MATCH_PARENT
                )
            }
            addView(pickerContainer)

            // Parse columns based on mode and cascading flag
            val columns = when (config.mode) {
                "selector" -> {
                    // Single column: range is List<String>
                    listOf(config.range.mapNotNull { it as? String })
                }
                "multiSelector" -> {
                    if (config.cascading && config.range.size >= 2) {
                        // Cascading mode: first column is List<String>, second is HashMap<String, List<String>>
                        val firstColumn = (config.range[0] as? List<*>)?.mapNotNull { it as? String } ?: emptyList()
                        val cascadingMap = config.range[1] as? Map<*, *> ?: emptyMap<String, List<String>>()

                        // Store cascading data for later use
                        firstColumnItems = firstColumn
                        cascadingData = cascadingMap.mapNotNull { (key, value) ->
                            val keyStr = key as? String
                            val valueList = (value as? List<*>)?.mapNotNull { it as? String }
                            if (keyStr != null && valueList != null) keyStr to valueList else null
                        }.toMap()

                        // For cascading, show the first column and the corresponding second column
                        val secondColumn = if (firstColumn.isNotEmpty()) {
                            val selectedFirstIndex = currentSelectedIndices.getOrNull(0) ?: 0
                            val firstKey = firstColumn.getOrNull(selectedFirstIndex) ?: firstColumn[0]
                            cascadingData?.get(firstKey) ?: emptyList()
                        } else {
                            emptyList()
                        }

                        listOf(firstColumn, secondColumn)
                    } else {
                        // Regular multi column: range is List<List<String>>
                        cascadingData = null
                        firstColumnItems = null
                        config.range.mapNotNull { column ->
                            (column as? List<*>)?.mapNotNull { it as? String }
                        }
                    }
                }
                else -> listOf(config.range.mapNotNull { it as? String })
            }

            // Ensure currentSelectedIndices has enough elements to cover all columns
            while (currentSelectedIndices.size < columns.size) {
                currentSelectedIndices.add(0)
            }

            columns.forEachIndexed { columnIndex, columnItems ->
                val customPicker = createCustomScrollPicker(context, columnItems, columnIndex, callbackId)
                pickerContainer.addView(customPicker)

                // Store reference to second column picker for cascading updates
                if (columnIndex == 1 && config.cascading) {
                    secondColumnPicker = customPicker
                }
            }
        }
    }

    private fun sendPickerResultCancel(callbackId: Long) {
        NativeApi.onCallback(callbackId, false, "2000")
    }

    private fun sendPickerResultConfirm(callbackId: Long) {
        val result = JSONObject().apply {
            if (currentMode == "selector") {
                // Single column: return single number
                put("index", currentSelectedIndices.firstOrNull() ?: 0)
            } else {
                // Multi column: return array
                put("index", JSONArray(currentSelectedIndices))
            }
            put("confirm", true)
        }
        NativeApi.onCallback(callbackId, true, result.toString())
    }

    private fun sendPickerResultScroll(callbackId: Long) {
        val result = JSONObject().apply {
            if (currentMode == "selector") {
                // Single column: return single number
                put("index", currentSelectedIndices.firstOrNull() ?: 0)
            } else {
                // Multi column: return array
                put("index", JSONArray(currentSelectedIndices))
            }
        }
        NativeApi.onCallback(callbackId, true, result.toString())
    }

    private fun hidePickerInternal() {
        currentPickerView?.let { pickerView ->
            removePickerFromParent(pickerView)
            currentPickerView = null
        }

        currentMaskView?.let { maskView ->
            removePickerFromParent(maskView)
            currentMaskView = null
        }
    }

    private fun removePickerFromParent(view: View) {
        (view.parent as? ViewGroup)?.removeView(view)
    }

    private fun createCustomScrollPicker(context: Context, items: List<String>, columnIndex: Int, callbackId: Long): FrameLayout {
        return FrameLayout(context).apply {
            layoutParams = LinearLayout.LayoutParams(
                0,
                LinearLayout.LayoutParams.MATCH_PARENT
            ).apply {
                weight = 1f
                if (columnIndex > 0) {
                    leftMargin = (16 * context.resources.displayMetrics.density).toInt()
                }
            }

            val itemHeight = (40 * context.resources.displayMetrics.density).toInt()
            val centerOffset = (80 * context.resources.displayMetrics.density).toInt() // Center position offset

            // Create ScrollView and LinearLayout
            val scrollView = android.widget.ScrollView(context).apply {
                isVerticalScrollBarEnabled = false
                isSmoothScrollingEnabled = true
            }

            val linearLayout = LinearLayout(context).apply {
                orientation = LinearLayout.VERTICAL
                gravity = Gravity.CENTER_HORIZONTAL

                // Add top padding - ensure first option can scroll to center position
                addView(View(context).apply {
                    layoutParams = LinearLayout.LayoutParams(
                        LinearLayout.LayoutParams.MATCH_PARENT,
                        centerOffset
                    )
                })

                // Add options
                items.forEachIndexed { index, item ->
                    val textView = android.widget.TextView(context).apply {
                        text = item
                        textSize = 18f
                        setTextColor(Color.parseColor("#333333"))
                        gravity = Gravity.CENTER
                        layoutParams = LinearLayout.LayoutParams(
                            LinearLayout.LayoutParams.MATCH_PARENT,
                            itemHeight
                        )
                    }
                    addView(textView)
                }

                // Add bottom padding - ensure last option can scroll to center position
                addView(View(context).apply {
                    layoutParams = LinearLayout.LayoutParams(
                        LinearLayout.LayoutParams.MATCH_PARENT,
                        centerOffset
                    )
                })
            }
            scrollView.addView(linearLayout)

            // Set initial position
            val initialIndex = if (columnIndex < currentSelectedIndices.size) {
                currentSelectedIndices[columnIndex].coerceIn(0, items.size - 1)
            } else 0

            // Improved scroll handling with proper debouncing - only trigger when scrolling stops
            var scrollEndRunnable: Runnable? = null
            var lastScrollTime = 0L
            var lastLoggedIndex = -1 // Prevent duplicate logging

            scrollView.post {
                val initialScrollY = initialIndex * itemHeight
                scrollView.scrollTo(0, initialScrollY)
                // Initialize text colors
                updateTextColors(linearLayout, initialIndex, itemHeight, centerOffset)

                lastLoggedIndex = initialIndex
                currentSelectedIndices[columnIndex] = initialIndex
            }

            scrollView.setOnScrollChangeListener { _, _, scrollY, _, _ ->
                val currentTime = System.currentTimeMillis()
                lastScrollTime = currentTime

                // Calculate the current center position corresponding to the option index
                val centerIndex = (scrollY + itemHeight / 2) / itemHeight
                val validIndex = centerIndex.coerceIn(0, items.size - 1)

                // Update selected index and text colors immediately for visual feedback
                updateSelectedIndex(columnIndex, validIndex)
                updateTextColors(linearLayout, validIndex, itemHeight, centerOffset)

                // Cancel previous scroll end detection
                scrollEndRunnable?.let { scrollView.handler.removeCallbacks(it) }

                // Set new scroll end detection with proper debouncing
                scrollEndRunnable = Runnable {
                    // Double-check if scrolling has actually stopped
                    if (System.currentTimeMillis() - lastScrollTime >= 300) {
                        // Recalculate current position when runnable executes
                        val currentScrollY = scrollView.scrollY
                        val currentCenterIndex = (currentScrollY + itemHeight / 2) / itemHeight
                        val currentValidIndex = currentCenterIndex.coerceIn(0, items.size - 1)

                        // Auto-align to nearest option
                        val targetScrollY = currentValidIndex * itemHeight
                        if (Math.abs(currentScrollY - targetScrollY) > 5) {
                            scrollView.smoothScrollTo(0, targetScrollY)
                        }

                        // Send scroll selection update if index changed
                        if (currentValidIndex != lastLoggedIndex) {
                            lastLoggedIndex = currentValidIndex
                            currentSelectedIndices[columnIndex] = currentValidIndex

                            // Handle cascading update for first column
                            if (columnIndex == 0 && cascadingData != null && firstColumnItems != null) {
                                updateSecondColumnForCascading(currentValidIndex)
                            }

                            sendPickerResultScroll(callbackId)
                        }
                    }
                }
                scrollView.handler.postDelayed(scrollEndRunnable!!, 300) // 300ms debounce for scroll end detection
            }
            addView(scrollView)

            // Add selection area indicator lines - positioned precisely at option area boundaries
            val topLine = View(context).apply {
                setBackgroundColor(Color.parseColor("#E0E0E0"))
                layoutParams = FrameLayout.LayoutParams(
                    FrameLayout.LayoutParams.MATCH_PARENT,
                    (1 * context.resources.displayMetrics.density).toInt()
                ).apply {
                    topMargin = centerOffset // Top of selection area
                }
            }
            addView(topLine)

            val bottomLine = View(context).apply {
                setBackgroundColor(Color.parseColor("#E0E0E0"))
                layoutParams = FrameLayout.LayoutParams(
                    FrameLayout.LayoutParams.MATCH_PARENT,
                    (1 * context.resources.displayMetrics.density).toInt()
                ).apply {
                    topMargin = centerOffset + itemHeight // Bottom of selection area
                }
            }
            addView(bottomLine)
        }
    }

    // Helper function to update selected index
    private fun updateSelectedIndex(columnIndex: Int, newIndex: Int) {
        while (currentSelectedIndices.size <= columnIndex) {
            currentSelectedIndices.add(0)
        }
        if (currentSelectedIndices[columnIndex] != newIndex) {
            currentSelectedIndices[columnIndex] = newIndex
        }
    }

    // Helper function to update text colors
    private fun updateTextColors(linearLayout: LinearLayout, selectedIndex: Int, itemHeight: Int, centerOffset: Int) {
        // Skip first blank view, start from index 1
        for (i in 1 until linearLayout.childCount - 1) { // Exclude last blank view
            val textView = linearLayout.getChildAt(i) as? TextView
            textView?.let {
                val itemIndex = i - 1 // Subtract top blank view
                if (itemIndex == selectedIndex) {
                    // Selected item: soft dark color, slightly larger font
                    it.setTextColor(Color.parseColor("#333333"))
                    it.textSize = 20f
                    it.alpha = 1.0f
                } else {
                    // Non-selected item: light color, normal font
                    it.setTextColor(Color.parseColor("#999999"))
                    it.textSize = 18f
                    it.alpha = 0.6f
                }
            }
        }
    }

    // Helper function to update second column when first column selection changes (cascading)
    private fun updateSecondColumnForCascading(firstColumnIndex: Int) {
        val firstItems = firstColumnItems ?: return
        val cascading = cascadingData ?: return
        val secondPicker = secondColumnPicker ?: return

        if (firstColumnIndex >= 0 && firstColumnIndex < firstItems.size) {
            val selectedFirstItem = firstItems[firstColumnIndex]
            val newSecondItems = cascading[selectedFirstItem] ?: emptyList()

            // Find the ScrollView and LinearLayout in the second column picker
            val scrollView = findScrollViewInPicker(secondPicker)
            val linearLayout = scrollView?.getChildAt(0) as? LinearLayout

            if (scrollView != null && linearLayout != null) {
                // Clear existing items
                linearLayout.removeAllViews()

                // Add new items with proper spacing (blank views at top and bottom)
                val itemHeight = (40 * secondPicker.context.resources.displayMetrics.density).toInt()
                val centerOffset = (80 * secondPicker.context.resources.displayMetrics.density).toInt()

                // Add top blank space
                val topBlankView = View(secondPicker.context).apply {
                    layoutParams = LinearLayout.LayoutParams(
                        LinearLayout.LayoutParams.MATCH_PARENT,
                        centerOffset
                    )
                }
                linearLayout.addView(topBlankView)

                // Add items
                newSecondItems.forEachIndexed { index, item ->
                    val textView = TextView(secondPicker.context).apply {
                        text = item
                        textSize = 18f
                        gravity = Gravity.CENTER
                        setTextColor(Color.parseColor("#999999"))
                        alpha = 0.6f
                        layoutParams = LinearLayout.LayoutParams(
                            LinearLayout.LayoutParams.MATCH_PARENT,
                            itemHeight
                        )
                    }
                    linearLayout.addView(textView)
                }

                // Add bottom blank space
                val bottomBlankView = View(secondPicker.context).apply {
                    layoutParams = LinearLayout.LayoutParams(
                        LinearLayout.LayoutParams.MATCH_PARENT,
                        centerOffset
                    )
                }
                linearLayout.addView(bottomBlankView)

                // Reset second column selection to 0
                currentSelectedIndices[1] = 0
                scrollView.scrollTo(0, 0)
                updateTextColors(linearLayout, 0, itemHeight, centerOffset)
            }
        }
    }

    // Helper function to find ScrollView in picker
    private fun findScrollViewInPicker(picker: View): android.widget.ScrollView? {
        if (picker is android.widget.ScrollView) return picker
        if (picker is ViewGroup) {
            for (i in 0 until picker.childCount) {
                val found = findScrollViewInPicker(picker.getChildAt(i))
                if (found != null) return found
            }
        }
        return null
    }
}
