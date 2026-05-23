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
import com.lingxia.app.LxApp
import com.lingxia.lxapp.R
import org.json.JSONArray
import org.json.JSONObject
import java.text.SimpleDateFormat
import java.util.*

internal data class DatePickerConfig(
    val mode: String,
    val fields: String,
    val value: Any?,
    val start: String?,
    val end: String?,
    val cancelText: String,
    val cancelButtonColor: String,
    val cancelTextColor: String,
    val confirmText: String,
    val confirmButtonColor: String,
    val confirmTextColor: String
)

internal object LxAppDatePicker {
    private const val TAG = "LingXia.DatePicker"

    private var currentPickerView: View? = null
    private var currentMaskView: View? = null
    private var currentValue: Any? = null

    @JvmStatic
    fun showDatePicker(
        mode: String,
        fields: String,
        value: String?,
        start: String?,
        end: String?,
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
            Log.e(TAG, "showDatePicker: current activity is null")
            sendPickerResultCancel(callbackId)
            return
        }

        val config = DatePickerConfig(
            mode = mode,
            fields = fields,
            value = parseValue(fields, value),
            start = start,
            end = end,
            cancelText = cancelText,
            cancelButtonColor = cancelButtonColor,
            cancelTextColor = cancelTextColor,
            confirmText = confirmText,
            confirmButtonColor = confirmButtonColor,
            confirmTextColor = confirmTextColor
        )

        activity.runOnUiThread {
            showDatePickerInternal(activity, config, callbackId)
        }
    }

    @JvmStatic
    fun hideDatePicker() {
        LxApp.getCurrentActivity()?.runOnUiThread {
            hideDatePickerInternal()
        } ?: hideDatePickerInternal()
    }

    private fun parseValue(fields: String, value: String?): Any? {
        if (value == null || value.isEmpty()) return null

        return when (fields) {
            "range" -> {
                try {
                    val arr = JSONArray(value)
                    listOf(arr.getString(0), arr.getString(1))
                } catch (e: Exception) {
                    null
                }
            }
            else -> value
        }
    }

    private fun NumberPicker.hideDividers() {
        try {
            val dividerField = NumberPicker::class.java.declaredFields
                .find { it.name == "mSelectionDivider" }
            dividerField?.isAccessible = true
            dividerField?.set(this, null)
        } catch (e: Exception) {
            Log.w(TAG, "Failed to hide dividers: ${e.message}")
        }
    }

    private fun showDatePickerInternal(context: Context, config: DatePickerConfig, callbackId: Long) {
        val activity = context as? Activity ?: return
        val rootView = activity.findViewById<ViewGroup>(android.R.id.content) ?: return

        hideDatePickerInternal()

        currentMaskView = createMaskView(activity) {
            sendPickerResultCancel(callbackId)
            hideDatePickerInternal()
        }
        rootView.addView(currentMaskView)

        currentPickerView = createDatePickerView(activity, config, callbackId)
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

    private fun createDatePickerView(context: Context, config: DatePickerConfig, callbackId: Long): View {
        val container = FrameLayout(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
        }

        val pickerContent = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(Color.WHITE)

            val cornerRadius = (12 * context.resources.displayMetrics.density).toInt()
            val drawable = GradientDrawable().apply {
                setColor(Color.WHITE)
                cornerRadii = floatArrayOf(
                    cornerRadius.toFloat(), cornerRadius.toFloat(),
                    cornerRadius.toFloat(), cornerRadius.toFloat(),
                    0f, 0f,
                    0f, 0f
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

        val pickerView = if (config.mode == "time") {
            createTimePicker(context, config, callbackId)
        } else {
            createDatePickerContent(context, config, callbackId)
        }

        val pickerHeight = when {
            config.mode == "time" -> 140
            config.fields == "year" || config.fields == "month" -> 140
            config.fields == "range" -> 350
            else -> 290
        }

        pickerView.layoutParams = LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT,
            (pickerHeight * context.resources.displayMetrics.density).toInt()
        ).apply {
            topMargin = 0
        }
        pickerContent.addView(pickerView)

        val separator = View(context).apply {
            setBackgroundColor(Color.parseColor("#F0F0F0"))
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                (0.5 * context.resources.displayMetrics.density).toInt()
            )
        }
        pickerContent.addView(separator)

        val buttonsContainer = createButtonsContainer(context, config, callbackId)
        pickerContent.addView(buttonsContainer)

        container.addView(pickerContent)
        com.lingxia.util.ActivityInsets.applyBottomMargin(container, pickerContent, 0)

        return container
    }

    private fun createTimePicker(context: Context, config: DatePickerConfig, callbackId: Long): View {
        val density = context.resources.displayMetrics.density
        val pickerItemHeight = (28 * density).toInt()
        val container = LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER
            val paddingPx = 0
            setPadding(paddingPx, paddingPx, paddingPx, paddingPx)
        }

        val timeFormat = SimpleDateFormat("HH:mm", Locale.getDefault())
        val startTime = config.start?.let {
            try { timeFormat.parse(it)?.let { d -> Calendar.getInstance().apply { time = d } } }
            catch (e: Exception) { null }
        } ?: Calendar.getInstance().apply { set(Calendar.HOUR_OF_DAY, 0); set(Calendar.MINUTE, 0) }

        val endTime = config.end?.let {
            try { timeFormat.parse(it)?.let { d -> Calendar.getInstance().apply { time = d } } }
            catch (e: Exception) { null }
        } ?: Calendar.getInstance().apply { set(Calendar.HOUR_OF_DAY, 23); set(Calendar.MINUTE, 59) }

        val initialTime = (config.value as? String)?.let {
            try { timeFormat.parse(it)?.let { d -> Calendar.getInstance().apply { time = d } } }
            catch (e: Exception) { null }
        } ?: Calendar.getInstance()

        val hourPicker = NumberPicker(context).apply {
            minValue = startTime.get(Calendar.HOUR_OF_DAY)
            maxValue = endTime.get(Calendar.HOUR_OF_DAY)
            value = initialTime.get(Calendar.HOUR_OF_DAY).coerceIn(minValue, maxValue)
            setFormatter { String.format("%02d", it) }
            layoutParams = LinearLayout.LayoutParams(0, pickerItemHeight * 5).apply {
                weight = 1f
            }
            hideDividers()
        }
        tuneNumberPicker(hourPicker, pickerItemHeight)

        val minutePicker = NumberPicker(context).apply {
            minValue = 0
            maxValue = 59
            value = initialTime.get(Calendar.MINUTE)
            setFormatter { String.format("%02d", it) }
            layoutParams = LinearLayout.LayoutParams(0, pickerItemHeight * 5).apply {
                weight = 1f
                leftMargin = (16 * context.resources.displayMetrics.density).toInt()
            }
            hideDividers()
        }
        tuneNumberPicker(minutePicker, pickerItemHeight)

        val updateCurrentValue = {
            currentValue = String.format("%02d:%02d", hourPicker.value, minutePicker.value)
            sendPickerResultScroll(callbackId, currentValue as String)
        }

        hourPicker.setOnValueChangedListener { _, _, _ -> updateCurrentValue() }
        minutePicker.setOnValueChangedListener { _, _, _ -> updateCurrentValue() }

        currentValue = String.format("%02d:%02d", hourPicker.value, minutePicker.value)

        container.addView(hourPicker)
        container.addView(TextView(context).apply {
            text = ":"
            textSize = 22f
            setTextColor(Color.parseColor("#666666"))
            gravity = Gravity.CENTER
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                leftMargin = 0
                rightMargin = 0
            }
        })
        container.addView(minutePicker)

        return container
    }

    private fun createDatePickerContent(context: Context, config: DatePickerConfig, callbackId: Long): View {
        return when (config.fields) {
            "year" -> createYearPicker(context, config, callbackId)
            "month" -> createMonthPicker(context, config, callbackId)
            "range" -> createDateRangePicker(context, config, callbackId)
            else -> createDayPicker(context, config, callbackId)
        }
    }

    private fun createYearPicker(context: Context, config: DatePickerConfig, callbackId: Long): View {
        val density = context.resources.displayMetrics.density
        val pickerItemHeight = (28 * density).toInt()
        val minYear = config.start?.substring(0, 4)?.toIntOrNull() ?: 1970
        val maxYear = config.end?.substring(0, 4)?.toIntOrNull() ?: 2100
        val initialYear = (config.value as? String)?.toIntOrNull() ?: Calendar.getInstance().get(Calendar.YEAR)

        val container = FrameLayout(context).apply {
            val paddingPx = 0
            setPadding(paddingPx, paddingPx, paddingPx, paddingPx)
        }

        val yearPicker = NumberPicker(context).apply {
            minValue = minYear
            maxValue = maxYear
            value = initialYear.coerceIn(minValue, maxValue)
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                pickerItemHeight * 5
            ).apply {
                gravity = Gravity.CENTER
            }
            setOnValueChangedListener { _, _, newVal ->
                currentValue = newVal.toString()
                sendPickerResultScroll(callbackId, currentValue as String)
            }
            hideDividers()
        }
        tuneNumberPicker(yearPicker, pickerItemHeight)

        currentValue = yearPicker.value.toString()
        container.addView(yearPicker)

        return container
    }

    private fun createMonthPicker(context: Context, config: DatePickerConfig, callbackId: Long): View {
        val density = context.resources.displayMetrics.density
        val pickerItemHeight = (28 * density).toInt()
        val dateFormat = SimpleDateFormat("yyyy-MM", Locale.getDefault())
        val minDate = config.start?.let {
            try { dateFormat.parse(it)?.let { d -> Calendar.getInstance().apply { time = d } } }
            catch (e: Exception) { null }
        } ?: Calendar.getInstance().apply { set(1970, 0, 1) }

        val maxDate = config.end?.let {
            try { dateFormat.parse(it)?.let { d -> Calendar.getInstance().apply { time = d } } }
            catch (e: Exception) { null }
        } ?: Calendar.getInstance().apply { set(2100, 11, 31) }

        val initialDate = (config.value as? String)?.let {
            try { dateFormat.parse(it)?.let { d -> Calendar.getInstance().apply { time = d } } }
            catch (e: Exception) { null }
        } ?: Calendar.getInstance()

        val container = LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER
            val paddingPx = 0
            setPadding(paddingPx, paddingPx, paddingPx, paddingPx)
        }

        val yearPicker = NumberPicker(context).apply {
            minValue = minDate.get(Calendar.YEAR)
            maxValue = maxDate.get(Calendar.YEAR)
            value = initialDate.get(Calendar.YEAR).coerceIn(minValue, maxValue)
            layoutParams = LinearLayout.LayoutParams(0, pickerItemHeight * 5).apply {
                weight = 1f
            }
            hideDividers()
        }
        tuneNumberPicker(yearPicker, pickerItemHeight)

        val monthPicker = NumberPicker(context).apply {
            minValue = 1
            maxValue = 12
            value = initialDate.get(Calendar.MONTH) + 1
            setFormatter { String.format("%02d", it) }
            layoutParams = LinearLayout.LayoutParams(0, pickerItemHeight * 5).apply {
                weight = 1f
                leftMargin = 0
            }
            hideDividers()
        }
        tuneNumberPicker(monthPicker, pickerItemHeight)

        val updateCurrentValue = {
            currentValue = String.format("%04d-%02d", yearPicker.value, monthPicker.value)
            sendPickerResultScroll(callbackId, currentValue as String)
        }

        yearPicker.setOnValueChangedListener { _, _, _ -> updateCurrentValue() }
        monthPicker.setOnValueChangedListener { _, _, _ -> updateCurrentValue() }

        currentValue = String.format("%04d-%02d", yearPicker.value, monthPicker.value)

        container.addView(yearPicker)
        container.addView(monthPicker)

        return container
    }

    private fun createDayPicker(context: Context, config: DatePickerConfig, callbackId: Long): View {
        val dateFormat = SimpleDateFormat("yyyy-MM-dd", Locale.getDefault())
        val minDate = config.start?.let {
            try { dateFormat.parse(it) }
            catch (e: Exception) { null }
        }
        val maxDate = config.end?.let {
            try { dateFormat.parse(it) }
            catch (e: Exception) { null }
        }
        val initialDate = (config.value as? String)?.let {
            try { dateFormat.parse(it) }
            catch (e: Exception) { null }
        } ?: Date()

        val calendarView = CustomCalendarView(context).apply {
            this.minimumDate = minDate
            this.maximumDate = maxDate
            this.selectedDate = initialDate
            this.isRangeMode = false
            this.onDateSelected = { date ->
                currentValue = dateFormat.format(date)
                sendPickerResultScroll(callbackId, currentValue as String)
            }
        }

        currentValue = dateFormat.format(calendarView.selectedDate ?: Date())

        return calendarView
    }

    private fun createDateRangePicker(context: Context, config: DatePickerConfig, callbackId: Long): View {
        val dateFormat = SimpleDateFormat("yyyy-MM-dd", Locale.getDefault())
        val today = Date()

        val minDate = config.start?.let {
            try { dateFormat.parse(it) }
            catch (e: Exception) { null }
        }
        val maxDate = config.end?.let {
            try { dateFormat.parse(it) }
            catch (e: Exception) { null }
        }

        val initialRange = (config.value as? List<*>)?.let {
            if (it.size == 2) {
                val start = (it[0] as? String)?.let { s ->
                    try { dateFormat.parse(s) } catch (e: Exception) { null }
                } ?: today
                val end = (it[1] as? String)?.let { s ->
                    try { dateFormat.parse(s) } catch (e: Exception) { null }
                } ?: today
                Pair(start, end)
            } else null
        } ?: Pair(today, today)

        val calendarView = CustomCalendarView(context).apply {
            this.minimumDate = minDate
            this.maximumDate = maxDate
            this.selectedRange = initialRange
            this.isRangeMode = true
            this.onRangeSelected = { start, end ->
                currentValue = listOf(dateFormat.format(start), dateFormat.format(end))
                sendPickerResultScroll(callbackId, currentValue!!)
            }
        }

        currentValue = listOf(dateFormat.format(initialRange.first), dateFormat.format(initialRange.second))

        return calendarView
    }

    private fun createButtonsContainer(
        context: Context,
        config: DatePickerConfig,
        callbackId: Long
    ): LinearLayout {
        return LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER
            val paddingPx = (6 * context.resources.displayMetrics.density).toInt()
            setPadding(paddingPx, paddingPx, paddingPx, paddingPx + (16 * context.resources.displayMetrics.density).toInt())
            setBackgroundColor(Color.WHITE)
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            )

            val cancelButton = TextView(context).apply {
                text = if (config.cancelText.isNotEmpty()) config.cancelText else context.getString(R.string.lx_common_cancel)
                textSize = 18f
                try {
                    setTextColor(Color.parseColor(config.cancelTextColor))
                } catch (e: Exception) {
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
                        setColor(Color.parseColor("#F2F2F2"))
                    }
                    cornerRadius = (8 * context.resources.displayMetrics.density)
                }
                background = drawable
                layoutParams = LinearLayout.LayoutParams(
                    (120 * context.resources.displayMetrics.density).toInt(),
                    (44 * context.resources.displayMetrics.density).toInt()
                ).apply {
                    weight = 0f
                }
                setOnClickListener {
                    sendPickerResultCancel(callbackId)
                    hideDatePickerInternal()
                }
            }
            addView(cancelButton)

            addView(View(context).apply {
                layoutParams = LinearLayout.LayoutParams(
                    (16 * context.resources.displayMetrics.density).toInt(),
                    0
                )
            })

            val confirmButton = TextView(context).apply {
                text = if (config.confirmText.isNotEmpty()) config.confirmText else context.getString(R.string.lx_common_confirm)
                textSize = 18f
                try {
                    setTextColor(Color.parseColor(config.confirmTextColor))
                } catch (e: Exception) {
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
                        setColor(Color.parseColor("#007AFF"))
                    }
                    cornerRadius = (8 * context.resources.displayMetrics.density)
                }
                background = drawable
                layoutParams = LinearLayout.LayoutParams(
                    (120 * context.resources.displayMetrics.density).toInt(),
                    (44 * context.resources.displayMetrics.density).toInt()
                ).apply {
                    weight = 0f
                }
                setOnClickListener {
                    sendPickerResultConfirm(callbackId)
                    hideDatePickerInternal()
                }
            }
            addView(confirmButton)
        }
    }

    private fun tuneNumberPicker(picker: NumberPicker, itemHeightPx: Int) {
        try {
            val selectorHeightField = NumberPicker::class.java.getDeclaredField("mSelectorElementHeight")
            selectorHeightField.isAccessible = true
            selectorHeightField.setInt(picker, itemHeightPx)
        } catch (_: Exception) {
        }

        try {
            val dividerHeightField = NumberPicker::class.java.getDeclaredField("mSelectionDividerHeight")
            dividerHeightField.isAccessible = true
            dividerHeightField.setInt(picker, 0)
        } catch (_: Exception) {
        }
    }

    private fun sendPickerResultCancel(callbackId: Long) {
        val localCallback = LxAppPicker.localCallbacks[callbackId]
        if (localCallback != null) {
            localCallback(false, "2000")
            return
        }
        com.lingxia.app.NativeApi.onCallback(callbackId, false, "2000")
    }

    private fun sendPickerResultConfirm(callbackId: Long) {
        val result = JSONObject().apply {
            put("value", currentValue ?: "")
            put("confirm", true)
        }
        val resultStr = result.toString()
        val localCallback = LxAppPicker.localCallbacks[callbackId]
        if (localCallback != null) {
            localCallback(true, resultStr)
            return
        }
        com.lingxia.app.NativeApi.onCallback(callbackId, true, resultStr)
    }

    private fun sendPickerResultScroll(callbackId: Long, value: Any) {
        val result = JSONObject().apply {
            when (value) {
                is List<*> -> put("value", JSONArray(value))
                else -> put("value", value)
            }
        }
        val resultStr = result.toString()
        val localCallback = LxAppPicker.localCallbacks[callbackId]
        if (localCallback != null) {
            localCallback(true, resultStr)
            return
        }
        com.lingxia.app.NativeApi.onCallback(callbackId, true, resultStr)
    }

    private fun hideDatePickerInternal() {
        currentPickerView?.let { pickerView ->
            (pickerView.parent as? ViewGroup)?.removeView(pickerView)
            currentPickerView = null
        }
        currentMaskView?.let { maskView ->
            (maskView.parent as? ViewGroup)?.removeView(maskView)
            currentMaskView = null
        }
        currentValue = null
    }
}
