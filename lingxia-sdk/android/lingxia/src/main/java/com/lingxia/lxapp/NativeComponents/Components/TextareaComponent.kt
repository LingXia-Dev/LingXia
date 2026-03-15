package com.lingxia.lxapp.NativeComponents.Components

import android.content.Context
import android.content.res.ColorStateList
import android.graphics.Color
import android.graphics.RectF
import android.graphics.drawable.GradientDrawable
import android.text.Editable
import android.text.InputFilter
import android.text.InputType
import android.text.TextWatcher
import android.view.KeyEvent
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.EditText
import android.widget.FrameLayout
import com.lingxia.lxapp.LxApp
import com.lingxia.lxapp.NativeComponents.LxNativeComponent
import com.lingxia.lxapp.NativeComponents.LxNativeComponentFactory
import kotlin.math.abs
import kotlin.math.max
import kotlin.math.roundToInt

class TextareaComponentFactory : LxNativeComponentFactory {
    override fun make(id: String, initialProps: Map<String, Any?>, eventSink: (Map<String, Any>) -> Unit): LxNativeComponent {
        return TextareaComponent(id, initialProps, eventSink)
    }
}

class TextareaComponent(
    override val id: String,
    private val initialProps: Map<String, Any?>,
    private val eventSink: (Map<String, Any>) -> Unit
) : LxNativeComponent {

    private val resolvedProps = initialProps.toMutableMap()
    private var container: FrameLayout? = null
    private var editText: EditText? = null
    private var suppressInputEvent = false
    private var maxLength: Int = -1
    private var confirmHold: Boolean = false
    private var holdKeyboard: Boolean = false
    private var confirmTypeValue: String = "return"
    private var autoHeightEnabled: Boolean = false
    private var lastAppliedImeOptions: Int = Int.MIN_VALUE
    private var lastAppliedPlaceholderStyle: String? = null
    private var lastAppliedTextStyle: String? = null
    private var defaultHintTextColors: ColorStateList? = null
    private var defaultTextColors: ColorStateList? = null
    private var lineSyncPosted: Boolean = false
    private var lastLineCount: Int = 1
    private var lastContentHeight: Int = 0
    private var lastFocusedValue: String = ""
    private var lastFocusPropValue: Boolean = false
    private var didApplyInitialFocusState: Boolean = false
    private var context: android.content.Context? = null

    override val view: View
        get() = container ?: FrameLayout(
            context ?: LxApp.getCurrentActivity()
            ?: throw IllegalStateException("TextareaComponent is not mounted")
        )

    override fun mount(host: ViewGroup) {
        val ctx = host.context
        context = ctx
        val root = FrameLayout(ctx).apply {
            setBackgroundColor(android.graphics.Color.TRANSPARENT)
        }
        val density = ctx.resources.displayMetrics.density
        val pad = (4 * density).roundToInt()
        val textarea = EditText(ctx).apply {
            setBackgroundColor(android.graphics.Color.TRANSPARENT)
            setPadding(pad, pad, pad, pad)
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE
            isSingleLine = false
            minLines = 1
            maxLines = Int.MAX_VALUE
            imeOptions = EditorInfo.IME_ACTION_DONE
            gravity = android.view.Gravity.TOP or android.view.Gravity.START
        }
        defaultHintTextColors = textarea.hintTextColors
        defaultTextColors = textarea.textColors
        lastAppliedImeOptions = textarea.imeOptions
        root.addView(
            textarea,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
        )
        host.addView(root)

        container = root
        editText = textarea
        bindListeners(textarea)
        update(initialProps)
    }

    override fun update(props: Map<String, Any?>) {
        val textarea = editText ?: return
        if (props.isNotEmpty()) {
            resolvedProps.putAll(props)
        }
        val state = resolvedProps

        val max = state.int("maxlength", -1)
        maxLength = max
        applyMaxLengthFilter(textarea)

        val rawValue = state["value"]?.toString()
        val value = if (rawValue != null && maxLength >= 0 && rawValue.length > maxLength) {
            rawValue.take(maxLength)
        } else {
            rawValue
        }
        if (value != null && value != textarea.text?.toString()) {
            suppressInputEvent = true
            textarea.setText(value)
            textarea.setSelection(value.length.coerceAtLeast(0))
            suppressInputEvent = false
            emitLineChangeIfNeeded(textarea)
            syncAutoHeightScroll(textarea)
            requestPostLayoutSync(textarea)
        }

        val placeholder = state["placeholder"]?.toString()
        if (placeholder != null) {
            textarea.hint = placeholder
        }
        val placeholderStyle = state["placeholderStyle"]?.toString() ?: state["placeholder-style"]?.toString()
        applyPlaceholderStyleIfNeeded(textarea, placeholderStyle)
        val textStyle = state["textColor"]?.toString() ?: state["text-color"]?.toString()
        applyTextColorIfNeeded(textarea, textStyle)

        val disabled = state.bool("disabled", false)
        textarea.isEnabled = !disabled

        autoHeightEnabled = state.bool("autoHeight", state.bool("auto-height", false))
        applyAutoHeightBehavior(textarea)

        val confirmType = (state["confirmType"]?.toString() ?: state["confirm-type"]?.toString() ?: "return")
            .trim()
            .lowercase()
        confirmTypeValue = confirmType
        val nextImeOptions = resolveImeOptions(confirmType, autoHeightEnabled)
        if (nextImeOptions != lastAppliedImeOptions) {
            textarea.imeOptions = nextImeOptions
            lastAppliedImeOptions = nextImeOptions
        }
        confirmHold = state.bool("confirmHold", state.bool("confirm-hold", false))
        holdKeyboard = state.bool("holdKeyboard", state.bool("hold-keyboard", false))

        val selectionStart = state.int("selectionStart", state.int("selection-start", -1))
        val selectionEnd = state.int("selectionEnd", state.int("selection-end", -1))
        if (selectionStart >= 0 && selectionEnd >= selectionStart) {
            val textLength = textarea.text?.length ?: 0
            val start = selectionStart.coerceIn(0, textLength)
            val end = selectionEnd.coerceIn(start, textLength)
            textarea.setSelection(start, end)
        } else {
            val cursor = state.int("cursor", -1)
            if (cursor >= 0) {
                val textLength = textarea.text?.length ?: 0
                textarea.setSelection(cursor.coerceIn(0, textLength))
            }
        }

        val cornerRadius = state.double("cornerRadius", 0.0).toFloat()
        applyCornerRadius(cornerRadius)

        val shouldFocus = state.bool("focus", false) || state.bool("autoFocus", state.bool("auto-focus", false))
        if (shouldFocus && !lastFocusPropValue && !textarea.hasFocus()) {
            textarea.post { textarea.requestFocus() }
        } else if (!shouldFocus && textarea.hasFocus() && (lastFocusPropValue || !didApplyInitialFocusState)) {
            blur()
        }
        lastFocusPropValue = shouldFocus
        didApplyInitialFocusState = true
        emitLineChangeIfNeeded(textarea)
        requestPostLayoutSync(textarea)
    }

    override fun setFrame(frame: RectF) {
        val root = container ?: return
        root.layoutParams = (root.layoutParams as? FrameLayout.LayoutParams)?.apply {
            leftMargin = frame.left.roundToInt()
            topMargin = frame.top.roundToInt()
            width = max(0, frame.width().roundToInt())
            height = max(0, frame.height().roundToInt())
        } ?: FrameLayout.LayoutParams(
            max(0, frame.width().roundToInt()),
            max(0, frame.height().roundToInt())
        ).apply {
            leftMargin = frame.left.roundToInt()
            topMargin = frame.top.roundToInt()
        }
    }

    override fun focus() {
        editText?.requestFocus()
    }

    override fun blur() {
        val textarea = editText ?: return
        textarea.clearFocus()
        hideSoftKeyboard(textarea)
    }

    override fun handleCommand(name: String, params: Map<String, Any?>?) {
        when (name) {
            "focus" -> focus()
            "blur" -> blur()
        }
    }

    override fun unmount() {
        val textarea = editText
        if (textarea != null) {
            textarea.setOnEditorActionListener(null)
            textarea.onFocusChangeListener = null
        }
        container?.let { root ->
            (root.parent as? ViewGroup)?.removeView(root)
        }
        editText = null
        container = null
    }

    private fun bindListeners(textarea: EditText) {
        textarea.addTextChangedListener(object : TextWatcher {
            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {
            }

            override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {
                if (suppressInputEvent) return
                val text = s?.toString() ?: ""
                emit("input", detailWithValue(text, textarea))
                emitLineChangeIfNeeded(textarea)
                syncAutoHeightScroll(textarea)
                requestPostLayoutSync(textarea)
            }

            override fun afterTextChanged(s: Editable?) {
            }
        })

        textarea.setOnEditorActionListener { _, actionId, keyEvent ->
            val isImeAction = actionId == EditorInfo.IME_ACTION_DONE ||
                actionId == EditorInfo.IME_ACTION_GO ||
                actionId == EditorInfo.IME_ACTION_NEXT ||
                actionId == EditorInfo.IME_ACTION_SEARCH ||
                actionId == EditorInfo.IME_ACTION_SEND
            val isPlainEnterAction = actionId == EditorInfo.IME_NULL ||
                actionId == EditorInfo.IME_ACTION_NONE ||
                actionId == EditorInfo.IME_ACTION_UNSPECIFIED
            val isHardwareEnter = keyEvent?.keyCode == KeyEvent.KEYCODE_ENTER && keyEvent.action == KeyEvent.ACTION_DOWN
            if (isHardwareEnter) {
                // Keep Enter for newline in multiline textarea.
                return@setOnEditorActionListener false
            }
            val shouldPreferNewline = autoHeightEnabled &&
                (confirmTypeValue == "done" || confirmTypeValue == "return") &&
                (isPlainEnterAction || actionId == EditorInfo.IME_ACTION_DONE)
            if (shouldPreferNewline) {
                // Keep IME Enter as newline for auto-height textarea.
                return@setOnEditorActionListener false
            }
            if (isImeAction && confirmTypeValue != "return") {
                emit("confirm", detailWithValue(textarea.text?.toString() ?: "", textarea))
                if (!confirmHold && !holdKeyboard) {
                    blur()
                }
                return@setOnEditorActionListener true
            }
            false
        }

        textarea.onFocusChangeListener = View.OnFocusChangeListener { _, hasFocus ->
            val value = textarea.text?.toString() ?: ""
            if (hasFocus) {
                lastFocusedValue = value
                emit("focus", detailWithValue(value, textarea))
            } else {
                lastFocusPropValue = false
                emit("blur", detailWithValue(value, textarea))
                if (value != lastFocusedValue) {
                    emit("change", detailWithValue(value, textarea))
                }
            }
        }
    }

    private fun emitLineChangeIfNeeded(textarea: EditText) {
        val count = max(1, textarea.layout?.lineCount ?: 1)
        val contentHeightPx = measureContentHeight(textarea)
        val heightChanged = abs(contentHeightPx - lastContentHeight) >= 1
        if (count == lastLineCount && !heightChanged) return
        lastLineCount = count
        lastContentHeight = contentHeightPx
        // measureContentHeight returns physical pixels; JS interprets height as CSS pixels,
        // so divide by density to match the coordinate space JS uses for layout.
        val density = textarea.resources?.displayMetrics?.density ?: 1f
        val contentHeightCss = contentHeightPx / density
        val detail = mapOf(
            "lineCount" to count,
            "height" to contentHeightCss,
            "heightRpx" to contentHeightCss
        )
        emit("linechange", detail)
    }

    private fun measureContentHeight(textarea: EditText): Int {
        val layout = textarea.layout
        val contentWithoutPadding = if (layout != null) {
            layout.getLineTop(layout.lineCount)
        } else {
            max(1, textarea.lineCount) * textarea.lineHeight
        }
        return max(
            textarea.lineHeight + textarea.compoundPaddingTop + textarea.compoundPaddingBottom,
            contentWithoutPadding + textarea.compoundPaddingTop + textarea.compoundPaddingBottom
        )
    }

    private fun applyAutoHeightBehavior(textarea: EditText) {
        textarea.isVerticalScrollBarEnabled = !autoHeightEnabled
        textarea.overScrollMode = if (autoHeightEnabled) View.OVER_SCROLL_NEVER else View.OVER_SCROLL_IF_CONTENT_SCROLLS
        textarea.setHorizontallyScrolling(false)
    }

    private fun applyMaxLengthFilter(textarea: EditText) {
        textarea.filters = if (maxLength >= 0) {
            arrayOf(InputFilter.LengthFilter(maxLength))
        } else {
            emptyArray()
        }
    }

    private fun syncAutoHeightScroll(textarea: EditText) {
        if (!autoHeightEnabled) return
        textarea.post {
            // Auto-height textarea should expand outer host, not internally scroll.
            if (textarea.scrollY != 0) {
                textarea.scrollTo(0, 0)
            }
        }
    }

    private fun requestPostLayoutSync(textarea: EditText) {
        if (lineSyncPosted) return
        lineSyncPosted = true
        textarea.post {
            lineSyncPosted = false
            emitLineChangeIfNeeded(textarea)
            syncAutoHeightScroll(textarea)
        }
    }

    private fun detailWithValue(value: String, textarea: EditText): Map<String, Any> {
        val selectionStart = textarea.selectionStart.coerceAtLeast(0)
        val selectionEnd = textarea.selectionEnd.coerceAtLeast(selectionStart)
        return mapOf(
            "value" to value,
            "cursor" to selectionStart,
            "selectionStart" to selectionStart,
            "selectionEnd" to selectionEnd
        )
    }

    private fun emit(event: String, detail: Map<String, Any>) {
        eventSink(
            mapOf(
                "event" to event,
                "detail" to detail
            )
        )
    }

    private fun applyCornerRadius(radius: Float) {
        val root = container ?: return
        val current = root.background as? GradientDrawable ?: GradientDrawable().also {
            it.setColor(android.graphics.Color.TRANSPARENT)
            root.background = it
        }
        current.cornerRadius = radius
    }

    private fun resolveImeOptions(confirmType: String, autoHeight: Boolean): Int {
        val action = when (confirmType) {
            "send" -> EditorInfo.IME_ACTION_SEND
            "search" -> EditorInfo.IME_ACTION_SEARCH
            "next" -> EditorInfo.IME_ACTION_NEXT
            "go" -> EditorInfo.IME_ACTION_GO
            "return" -> EditorInfo.IME_ACTION_NONE
            else -> EditorInfo.IME_ACTION_DONE
        }
        return if (autoHeight) action or EditorInfo.IME_FLAG_NO_ENTER_ACTION else action
    }

    private fun applyPlaceholderStyleIfNeeded(textarea: EditText, styleValue: String?) {
        val style = styleValue?.trim()?.takeIf { it.isNotEmpty() }
        if (style == lastAppliedPlaceholderStyle) return
        if (style == null) {
            defaultHintTextColors?.let { textarea.setHintTextColor(it) }
            lastAppliedPlaceholderStyle = null
            return
        }
        val colorExpr = extractColorFromStyle(style) ?: style
        val color = parseColorCompat(colorExpr) ?: run {
            defaultHintTextColors?.let { textarea.setHintTextColor(it) }
            lastAppliedPlaceholderStyle = style
            return
        }
        textarea.setHintTextColor(color)
        lastAppliedPlaceholderStyle = style
    }

    private fun applyTextColorIfNeeded(textarea: EditText, styleValue: String?) {
        val style = styleValue?.trim()?.takeIf { it.isNotEmpty() }
        if (style == lastAppliedTextStyle) return
        if (style == null) {
            defaultTextColors?.let { textarea.setTextColor(it) }
            lastAppliedTextStyle = null
            return
        }
        val colorExpr = extractColorFromStyle(style) ?: style
        val color = parseColorCompat(colorExpr) ?: run {
            defaultTextColors?.let { textarea.setTextColor(it) }
            lastAppliedTextStyle = style
            return
        }
        textarea.setTextColor(color)
        lastAppliedTextStyle = style
    }

    private fun hideSoftKeyboard(textarea: EditText) {
        val imm = textarea.context.getSystemService(Context.INPUT_METHOD_SERVICE) as? InputMethodManager ?: return
        imm.hideSoftInputFromWindow(textarea.windowToken, 0)
    }

    private fun parseColorCompat(value: String): Int? {
        return try {
            Color.parseColor(value)
        } catch (_: IllegalArgumentException) {
            parseRgbaColor(value)
        }
    }

    private fun parseRgbaColor(value: String): Int? {
        val match = RGB_REGEX.matchEntire(value.trim()) ?: return null
        val r = (match.groupValues[1].toIntOrNull() ?: return null).coerceIn(0, 255)
        val g = (match.groupValues[2].toIntOrNull() ?: return null).coerceIn(0, 255)
        val b = (match.groupValues[3].toIntOrNull() ?: return null).coerceIn(0, 255)
        val alphaGroup = match.groupValues.getOrNull(4).orEmpty()
        val a = if (alphaGroup.isEmpty()) {
            255
        } else {
            val alphaFloat = alphaGroup.toFloatOrNull() ?: return null
            (alphaFloat.coerceIn(0f, 1f) * 255f).toInt()
        }
        return Color.argb(a, r, g, b)
    }

    private fun extractColorFromStyle(style: String): String? {
        val match = STYLE_COLOR_REGEX.find(style) ?: return null
        val value = match.groupValues.getOrNull(1)?.trim().orEmpty()
        return value.takeIf { it.isNotEmpty() }
    }

    private companion object {
        val STYLE_COLOR_REGEX = Regex("""(?:^|;)\s*color\s*:\s*([^;]+)""", RegexOption.IGNORE_CASE)
        val RGB_REGEX = Regex(
            """rgba?\(\s*([0-9]{1,3})\s*,\s*([0-9]{1,3})\s*,\s*([0-9]{1,3})(?:\s*,\s*([0-9]*\.?[0-9]+))?\s*\)""",
            RegexOption.IGNORE_CASE
        )
    }
}
