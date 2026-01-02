package com.lingxia.lxapp.APIs

import android.content.Context
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.RectF
import android.graphics.drawable.GradientDrawable
import android.os.Build
import android.util.AttributeSet
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.widget.LinearLayout
import android.widget.TextView
import androidx.core.content.ContextCompat
import java.time.temporal.WeekFields
import java.text.SimpleDateFormat
import java.util.*

class CustomCalendarView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
    defStyleAttr: Int = 0
) : LinearLayout(context, attrs, defStyleAttr) {

    private val calendar = Calendar.getInstance()
    private val dateFormat = SimpleDateFormat("yyyy-MM", Locale.getDefault())
    private var currentMonth = Calendar.getInstance()

    var selectedDate: Date? = null
        set(value) {
            field = value
            if (value != null) {
                currentMonth.time = value
            }
            refreshCalendar()
        }

    var selectedRange: Pair<Date, Date>? = null
        set(value) {
            field = value
            if (value != null) {
                currentMonth.time = value.first
            }
            refreshCalendar()
        }

    var minimumDate: Date? = null
    var maximumDate: Date? = null
    var isRangeMode = false
        set(value) {
            field = value
            // Auto-show quick select buttons for range mode
            setQuickSelect(value)
        }
    var onDateSelected: ((Date) -> Unit)? = null
    var onRangeSelected: ((Date, Date) -> Unit)? = null

    private var isSelectingStart = true
    private var tempStartDate: Date? = null

    private val headerLayout: LinearLayout
    private val weekdayLayout: LinearLayout
    private val calendarGrid: ViewGroup
    private val monthLabel: TextView

    var showQuickSelect = false
    private var quickSelectLayout: LinearLayout? = null

    init {
        orientation = VERTICAL
        setPadding(dp(12), dp(6), dp(12), dp(4))

        // Month header with navigation
        headerLayout = LinearLayout(context).apply {
            orientation = HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = LayoutParams(LayoutParams.MATCH_PARENT, dp(36))
        }

        val prevYearBtn = createNavButton("<<") { changeYear(-1) }
        val prevMonthBtn = createNavButton("<") { changeMonth(-1) }

        monthLabel = TextView(context).apply {
            textSize = 16f
            setTextColor(android.graphics.Color.parseColor("#333333"))
            typeface = android.graphics.Typeface.DEFAULT_BOLD
            gravity = Gravity.CENTER
            layoutParams = LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f)
        }

        val nextMonthBtn = createNavButton(">") { changeMonth(1) }
        val nextYearBtn = createNavButton(">>") { changeYear(1) }

        headerLayout.addView(prevYearBtn)
        headerLayout.addView(prevMonthBtn)
        headerLayout.addView(monthLabel)
        headerLayout.addView(nextMonthBtn)
        headerLayout.addView(nextYearBtn)

        addView(headerLayout)

        // Weekday headers
        weekdayLayout = LinearLayout(context).apply {
            orientation = HORIZONTAL
            layoutParams = LayoutParams(LayoutParams.MATCH_PARENT, dp(24))
        }

        val locale = Locale.getDefault()
        val cal = Calendar.getInstance(locale)
        val firstDow = resolveFirstDayOfWeek(locale, cal)
        val weekdays = Array(7) { i ->
            val dow = ((firstDow - 1 + i) % 7) + 1
            cal.set(Calendar.DAY_OF_WEEK, dow)
            cal.getDisplayName(Calendar.DAY_OF_WEEK, Calendar.SHORT, locale)?.take(3) ?: ""
        }

        for (day in weekdays) {
            val label = TextView(context).apply {
                text = day
                textSize = 11f
                setTextColor(android.graphics.Color.parseColor("#999999"))
                gravity = Gravity.CENTER
                layoutParams = LayoutParams(0, LayoutParams.MATCH_PARENT, 1f)
            }
            weekdayLayout.addView(label)
        }

        addView(weekdayLayout)

        // Calendar grid (6 rows x 7 columns = 42 cells)
        calendarGrid = LinearLayout(context).apply {
            orientation = VERTICAL
            layoutParams = LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT)
        }

        for (row in 0 until 6) {
            val rowLayout = LinearLayout(context).apply {
                orientation = HORIZONTAL
                layoutParams = LayoutParams(LayoutParams.MATCH_PARENT, dp(32))
            }

            for (col in 0 until 7) {
                val dayCell = DayCell(context)
                dayCell.layoutParams = LayoutParams(0, LayoutParams.MATCH_PARENT, 1f)
                rowLayout.addView(dayCell)
            }

            calendarGrid.addView(rowLayout)
        }

        addView(calendarGrid)

        updateMonthLabel()
        refreshCalendar()
    }

    fun setQuickSelect(enabled: Boolean) {
        showQuickSelect = enabled
        if (enabled && quickSelectLayout == null) {
            quickSelectLayout = createQuickSelectButtons()
            addView(quickSelectLayout, 0)
        } else if (!enabled && quickSelectLayout != null) {
            removeView(quickSelectLayout)
            quickSelectLayout = null
        }
    }

    private fun createQuickSelectButtons(): LinearLayout {
        return LinearLayout(context).apply {
            orientation = VERTICAL
            setPadding(0, dp(2), 0, dp(4))
            layoutParams = LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT)

            val row1Options = arrayOf("last7days", "last30days", "thisweek")
            val row2Options = arrayOf("lastweek", "thismonth", "lastmonth")

            fun createButtonRow(options: Array<String>): LinearLayout {
                return LinearLayout(context).apply {
                    orientation = HORIZONTAL
                    layoutParams = LayoutParams(LayoutParams.MATCH_PARENT, dp(28))

                    options.forEach { option ->
                        val btn = TextView(context).apply {
                            text = getQuickSelectText(option)
                            textSize = 12f
                            setTextColor(android.graphics.Color.parseColor("#007AFF"))
                            gravity = Gravity.CENTER
                            setBackgroundColor(android.graphics.Color.parseColor("#E3F2FD"))
                            val drawable = GradientDrawable().apply {
                                setColor(android.graphics.Color.parseColor("#E3F2FD"))
                                cornerRadius = dp(14).toFloat()
                            }
                            background = drawable
                            layoutParams = LayoutParams(0, LayoutParams.MATCH_PARENT, 1f).apply {
                                if (option != options.first()) {
                                    leftMargin = dp(6)
                                }
                            }
                            isClickable = true
                            isFocusable = true
                            setOnClickListener { handleQuickSelect(option) }
                        }
                        addView(btn)
                    }
                }
            }

            addView(createButtonRow(row1Options))
            addView(createButtonRow(row2Options).apply {
                (layoutParams as MarginLayoutParams).topMargin = dp(4)
            })
        }
    }

    private fun getQuickSelectText(type: String): String {
        val resId = context.resources.getIdentifier("lx_date_$type", "string", context.packageName)
        return if (resId != 0) {
            context.getString(resId)
        } else {
            when (type) {
                "last7days" -> "近7日"
                "last30days" -> "近30日"
                "thisweek" -> "本周"
                "lastweek" -> "上周"
                "thismonth" -> "本月"
                "lastmonth" -> "上月"
                else -> type
            }
        }
    }

    private fun handleQuickSelect(type: String) {
        val calendar = Calendar.getInstance()
        val today = calendar.time
        val endDate: Date
        val startDate: Date

        when (type) {
            "last7days" -> {
                endDate = today
                calendar.add(Calendar.DAY_OF_YEAR, -6)
                startDate = calendar.time
            }
            "last30days" -> {
                endDate = today
                calendar.add(Calendar.DAY_OF_YEAR, -29)
                startDate = calendar.time
            }
            "thisweek" -> {
                calendar.set(Calendar.DAY_OF_WEEK, resolveFirstDayOfWeek(Locale.getDefault(), calendar))
                startDate = calendar.time
                endDate = today
            }
            "lastweek" -> {
                calendar.set(Calendar.DAY_OF_WEEK, resolveFirstDayOfWeek(Locale.getDefault(), calendar))
                calendar.add(Calendar.WEEK_OF_YEAR, -1)
                startDate = calendar.time
                calendar.add(Calendar.DAY_OF_YEAR, 6)
                endDate = calendar.time
            }
            "thismonth" -> {
                calendar.set(Calendar.DAY_OF_MONTH, 1)
                startDate = calendar.time
                endDate = today
            }
            "lastmonth" -> {
                calendar.add(Calendar.MONTH, -1)
                calendar.set(Calendar.DAY_OF_MONTH, 1)
                startDate = calendar.time
                calendar.set(Calendar.DAY_OF_MONTH, calendar.getActualMaximum(Calendar.DAY_OF_MONTH))
                endDate = calendar.time
            }
            else -> return
        }

        selectedRange = Pair(startDate, endDate)
        isSelectingStart = true
        tempStartDate = null
        onRangeSelected?.invoke(startDate, endDate)
    }

    private fun createNavButton(text: String, onClick: () -> Unit): TextView {
        return TextView(context).apply {
            this.text = text
            textSize = 16f
            setTextColor(android.graphics.Color.parseColor("#007AFF"))
            gravity = Gravity.CENTER
            layoutParams = LayoutParams(dp(36), LayoutParams.MATCH_PARENT)
            isClickable = true
            isFocusable = true
            setOnClickListener { onClick() }
        }
    }

    private fun changeMonth(delta: Int) {
        currentMonth.add(Calendar.MONTH, delta)
        updateMonthLabel()
        refreshCalendar()
    }

    private fun changeYear(delta: Int) {
        currentMonth.add(Calendar.YEAR, delta)
        updateMonthLabel()
        refreshCalendar()
    }

    private fun updateMonthLabel() {
        val year = currentMonth.get(Calendar.YEAR)
        val month = currentMonth.get(Calendar.MONTH) + 1
        monthLabel.text = String.format("%d-%02d", year, month)
    }

    private fun refreshCalendar() {
        val year = currentMonth.get(Calendar.YEAR)
        val month = currentMonth.get(Calendar.MONTH)

        val firstDayOfMonth = Calendar.getInstance().apply {
            set(year, month, 1)
        }

        val firstDayOfWeek = resolveFirstDayOfWeek(Locale.getDefault(), firstDayOfMonth)
        val firstDayIndex =
            (firstDayOfMonth.get(Calendar.DAY_OF_WEEK) - firstDayOfWeek + 7) % 7
        val daysInMonth = firstDayOfMonth.getActualMaximum(Calendar.DAY_OF_MONTH)

        val startDate = Calendar.getInstance().apply {
            time = firstDayOfMonth.time
            add(Calendar.DAY_OF_MONTH, -firstDayIndex)
        }

        val minDay = minimumDate?.let { stripTime(it) }
        val maxDay = maximumDate?.let { stripTime(it) }

        var dayIndex = 0
        for (row in 0 until 6) {
            val rowLayout = calendarGrid.getChildAt(row) as LinearLayout
            for (col in 0 until 7) {
                val dayCell = rowLayout.getChildAt(col) as DayCell
                val cellDate = Calendar.getInstance().apply {
                    time = startDate.time
                    add(Calendar.DAY_OF_MONTH, dayIndex)
                }

                val isCurrentMonth = cellDate.get(Calendar.MONTH) == month
                val isToday = isSameDay(cellDate.time, Date())
                val cellDay = stripTime(cellDate.time)
                val isDisabled = (minDay != null && cellDay.before(minDay)) ||
                                 (maxDay != null && cellDay.after(maxDay))

                dayCell.setDate(
                    cellDate.time,
                    isCurrentMonth,
                    isToday,
                    isDisabled,
                    isSelected(cellDate.time),
                    isInRange(cellDate.time)
                )

                dayCell.setOnClickListener {
                    if (!isDisabled) {
                        handleDayClick(cellDate.time)
                    }
                }

                dayIndex++
            }
        }
    }

    private fun handleDayClick(date: Date) {
        if (isRangeMode) {
            if (isSelectingStart) {
                tempStartDate = date
                selectedRange = Pair(date, date)
                isSelectingStart = false
                // Trigger onScroll for first date selection
                onRangeSelected?.invoke(date, date)
            } else {
                val start = tempStartDate ?: date
                val end = date
                if (end.before(start)) {
                    selectedRange = Pair(end, start)
                    onRangeSelected?.invoke(end, start)
                } else {
                    selectedRange = Pair(start, end)
                    onRangeSelected?.invoke(start, end)
                }
                isSelectingStart = true
                tempStartDate = null
            }
        } else {
            selectedDate = date
            onDateSelected?.invoke(date)
        }
        refreshCalendar()
    }

    private fun isSelected(date: Date): Boolean {
        return if (isRangeMode) {
            selectedRange?.let { range ->
                isSameDay(date, range.first) || isSameDay(date, range.second)
            } ?: false
        } else {
            selectedDate?.let { isSameDay(date, it) } ?: false
        }
    }

    private fun isInRange(date: Date): Boolean {
        return if (isRangeMode) {
            selectedRange?.let { range ->
                val dateDay = stripTime(date)
                val startDay = stripTime(range.first)
                val endDay = stripTime(range.second)
                !dateDay.before(startDay) && !dateDay.after(endDay)
            } ?: false
        } else {
            false
        }
    }

    private fun stripTime(date: Date): Date {
        val cal = Calendar.getInstance()
        cal.time = date
        cal.set(Calendar.HOUR_OF_DAY, 0)
        cal.set(Calendar.MINUTE, 0)
        cal.set(Calendar.SECOND, 0)
        cal.set(Calendar.MILLISECOND, 0)
        return cal.time
    }

    private fun isSameDay(date1: Date, date2: Date): Boolean {
        val cal1 = Calendar.getInstance().apply { time = date1 }
        val cal2 = Calendar.getInstance().apply { time = date2 }
        return cal1.get(Calendar.YEAR) == cal2.get(Calendar.YEAR) &&
               cal1.get(Calendar.DAY_OF_YEAR) == cal2.get(Calendar.DAY_OF_YEAR)
    }

    private fun resolveFirstDayOfWeek(locale: Locale, calendar: Calendar): Int {
        if (locale.language.lowercase(Locale.ROOT) == "zh") {
            return Calendar.MONDAY
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val first = WeekFields.of(locale).firstDayOfWeek.value // 1=Mon..7=Sun
            return if (first == 7) Calendar.SUNDAY else first + 1
        }
        return calendar.firstDayOfWeek
    }

    private fun dp(value: Int): Int {
        return (value * resources.displayMetrics.density).toInt()
    }

    private inner class DayCell(context: Context) : View(context) {
        private var date: Date? = null
        private var isCurrentMonth = true
        private var isToday = false
        private var isDisabled = false
        private var isSelected = false
        private var isInRange = false

        private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
        private val textPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            textAlign = Paint.Align.CENTER
            textSize = sp(16f)
        }

        fun setDate(
            date: Date,
            isCurrentMonth: Boolean,
            isToday: Boolean,
            isDisabled: Boolean,
            isSelected: Boolean,
            isInRange: Boolean
        ) {
            this.date = date
            this.isCurrentMonth = isCurrentMonth
            this.isToday = isToday
            this.isDisabled = isDisabled
            this.isSelected = isSelected
            this.isInRange = isInRange
            invalidate()
        }

        override fun onDraw(canvas: Canvas) {
            super.onDraw(canvas)

            val date = this.date ?: return
            val cal = Calendar.getInstance().apply { time = date }
            val day = cal.get(Calendar.DAY_OF_MONTH)

            val centerX = width / 2f
            val centerY = height / 2f
            val radius = minOf(width, height) / 2f - dp(4)

            // Draw range background with rounded corners
            if (isInRange) {
                paint.color = android.graphics.Color.parseColor("#E3F2FD")
                paint.style = Paint.Style.FILL

                val rect = RectF(0f, dp(2).toFloat(), width.toFloat(), height - dp(2).toFloat())

                if (isSelected) {
                    // Determine if this is start or end date
                    val range = selectedRange
                    val isStart = range != null && isSameDay(date, range.first)
                    val isEnd = range != null && isSameDay(date, range.second)
                    val isSingleDay = isStart && isEnd

                    if (isSingleDay) {
                        // Single day selection - no background extension
                    } else if (isStart) {
                        // Start date - extended from center to right edge
                        canvas.drawRect(centerX, rect.top, width.toFloat(), rect.bottom, paint)
                    } else if (isEnd) {
                        // End date - extended from left edge to center
                        canvas.drawRect(0f, rect.top, centerX, rect.bottom, paint)
                    }
                } else {
                    // Middle dates - full width background
                    canvas.drawRect(0f, rect.top, width.toFloat(), rect.bottom, paint)
                }
            }

            // Draw selection circle
            if (isSelected) {
                paint.color = android.graphics.Color.parseColor("#007AFF")
                paint.style = Paint.Style.FILL
                canvas.drawCircle(centerX, centerY, radius, paint)
            }

            // Draw today indicator circle (even if selected in range mode)
            if (isToday && !isSelected) {
                paint.color = android.graphics.Color.parseColor("#007AFF")
                paint.style = Paint.Style.STROKE
                paint.strokeWidth = 2f * resources.displayMetrics.density
                canvas.drawCircle(centerX, centerY, radius, paint)
            }

            // Draw day number with appropriate styling
            textPaint.color = when {
                isDisabled -> android.graphics.Color.parseColor("#CCCCCC")  // Disabled: light gray
                isSelected -> android.graphics.Color.WHITE  // Selected: white
                !isCurrentMonth -> android.graphics.Color.parseColor("#666666")  // Not in month: lighter gray (brighter)
                else -> android.graphics.Color.parseColor("#333333")  // Normal: dark gray
            }

            // Apply alpha: disabled dates are more faded
            textPaint.alpha = if (isDisabled) (255 * 0.3).toInt() else 255

            val textY = centerY - (textPaint.descent() + textPaint.ascent()) / 2
            canvas.drawText(day.toString(), centerX, textY, textPaint)
        }

        private fun sp(value: Float): Float {
            return value * resources.displayMetrics.scaledDensity
        }
    }
}
