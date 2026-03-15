package com.lingxia.lxapp.NativeComponents.Components

import android.content.Context
import android.content.res.ColorStateList
import android.graphics.Color
import android.graphics.PorterDuff
import android.graphics.RectF
import android.graphics.drawable.GradientDrawable
import android.os.Build
import android.text.Editable
import android.text.InputFilter
import android.text.InputType
import android.text.Spanned
import android.text.TextWatcher
import android.text.method.DigitsKeyListener
import android.text.method.KeyListener
import android.text.method.PasswordTransformationMethod
import android.view.KeyEvent
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.EditorInfo
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.TextView
import com.lingxia.lxapp.LxApp
import com.lingxia.lxapp.NativeComponents.LxNativeComponent
import com.lingxia.lxapp.NativeComponents.LxNativeComponentFactory
import android.view.inputmethod.InputMethodManager
import java.util.Locale
import kotlin.math.max
import kotlin.math.roundToInt

class InputComponentFactory : LxNativeComponentFactory {
    override fun make(id: String, initialProps: Map<String, Any?>, eventSink: (Map<String, Any>) -> Unit): LxNativeComponent {
        return InputComponent(id, initialProps, eventSink)
    }
}

private enum class InputMode {
    TEXT,
    NUMBER,
    DIGIT,
    PASSWORD
}

class InputComponent(
    override val id: String,
    private val initialProps: Map<String, Any?>,
    private val eventSink: (Map<String, Any>) -> Unit
) : LxNativeComponent {

    private val resolvedProps = initialProps.toMutableMap()
    private var container: FrameLayout? = null
    private var editText: EditText? = null
    private var suppressInputEvent = false
    private var lastFocusedValue: String = ""
    private var maxLength: Int = -1
    private var confirmHold: Boolean = false
    private var holdKeyboard: Boolean = false
    private var inputMode: InputMode = InputMode.TEXT
    private var lastAppliedInputType: Int = Int.MIN_VALUE
    private var lastAppliedImeOptions: Int = Int.MIN_VALUE
    private var lastAppliedCursorColor: String? = null
    private var lastAppliedPlaceholderStyle: String? = null
    private var defaultKeyListener: KeyListener? = null
    private val numberKeyListener: KeyListener = DigitsKeyListener.getInstance(Locale.US, false, false)
    private val digitKeyListener: KeyListener = DigitsKeyListener.getInstance(Locale.US, false, true)
    private var defaultHintTextColors: ColorStateList? = null
    private var context: android.content.Context? = null
    private var lastFocusPropValue: Boolean = false
    private var didApplyInitialFocusState: Boolean = false

    override val view: View
        get() = container ?: FrameLayout(
            context ?: LxApp.getCurrentActivity()
            ?: throw IllegalStateException("InputComponent is not mounted")
        )

    override fun mount(host: ViewGroup) {
        val ctx = host.context
        context = ctx
        val root = FrameLayout(ctx).apply {
            setBackgroundColor(android.graphics.Color.TRANSPARENT)
        }
        val density = ctx.resources.displayMetrics.density
        val hPad = (8 * density).roundToInt()
        val input = EditText(ctx).apply {
            setBackgroundColor(android.graphics.Color.TRANSPARENT)
            isSingleLine = true
            includeFontPadding = false
            setPadding(hPad, 0, hPad, 0)
            imeOptions = EditorInfo.IME_ACTION_DONE
        }
        defaultHintTextColors = input.hintTextColors
        defaultKeyListener = input.keyListener
        lastAppliedInputType = input.inputType
        lastAppliedImeOptions = input.imeOptions

        root.addView(
            input,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
        )
        host.addView(root)

        container = root
        editText = input
        bindListeners(input)
        update(initialProps)
    }

    override fun update(props: Map<String, Any?>) {
        val input = editText ?: return
        if (props.isNotEmpty()) {
            resolvedProps.putAll(props)
        }
        val state = resolvedProps

        val value = state["value"]?.toString()
        if (value != null && value != input.text?.toString()) {
            suppressInputEvent = true
            input.setText(value)
            input.setSelection(value.length.coerceAtLeast(0))
            suppressInputEvent = false
        }

        val placeholder = state["placeholder"]?.toString()
        if (placeholder != null) {
            input.hint = placeholder
        }
        val placeholderStyle = state["placeholderStyle"]?.toString() ?: state["placeholder-style"]?.toString()
        applyPlaceholderStyleIfNeeded(input, placeholderStyle)

        val disabled = state.bool("disabled", false)
        input.isEnabled = !disabled

        val type = (state["type"]?.toString() ?: "text").trim().lowercase()
        val password = state.bool("password", false) || type == "password" || type == "safe-password"
        inputMode = resolveInputMode(type, password)
        applyInputMode(input, type, password)

        val max = state.int("maxlength", -1)
        maxLength = max
        applyInputFilters(input)
        sanitizeAndApplyIfNeeded(input)

        val cursorColor = state["cursorColor"]?.toString() ?: state["cursor-color"]?.toString()
        applyCursorColorIfNeeded(input, cursorColor)

        val confirmType = (state["confirmType"]?.toString() ?: state["confirm-type"]?.toString() ?: "done")
            .trim()
            .lowercase()
        val nextImeOptions = resolveImeOptions(confirmType)
        if (nextImeOptions != lastAppliedImeOptions) {
            input.imeOptions = nextImeOptions
            lastAppliedImeOptions = nextImeOptions
        }

        confirmHold = state.bool("confirmHold", state.bool("confirm-hold", false))
        holdKeyboard = state.bool("holdKeyboard", state.bool("hold-keyboard", false))

        val selectionStart = state.int("selectionStart", state.int("selection-start", -1))
        val selectionEnd = state.int("selectionEnd", state.int("selection-end", -1))
        if (selectionStart >= 0 && selectionEnd >= selectionStart) {
            val textLength = input.text?.length ?: 0
            val start = selectionStart.coerceIn(0, textLength)
            val end = selectionEnd.coerceIn(start, textLength)
            input.setSelection(start, end)
        } else {
            val cursor = state.int("cursor", -1)
            if (cursor >= 0) {
                val textLength = input.text?.length ?: 0
                input.setSelection(cursor.coerceIn(0, textLength))
            }
        }

        val cornerRadius = state.double("cornerRadius", 0.0).toFloat()
        applyCornerRadius(cornerRadius)

        val shouldFocus = state.bool("focus", false) || state.bool("autoFocus", state.bool("auto-focus", false))
        if (shouldFocus && !lastFocusPropValue && !input.hasFocus()) {
            input.post {
                input.requestFocus()
                input.setSelection(input.text?.length ?: 0)
            }
        } else if (!shouldFocus && input.hasFocus() && (lastFocusPropValue || !didApplyInitialFocusState)) {
            blur()
        }
        lastFocusPropValue = shouldFocus
        didApplyInitialFocusState = true
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
        val input = editText ?: return
        input.clearFocus()
        hideSoftKeyboard(input)
    }

    override fun handleCommand(name: String, params: Map<String, Any?>?) {
        when (name) {
            "focus" -> focus()
            "blur" -> blur()
        }
    }

    override fun unmount() {
        val input = editText
        if (input != null) {
            input.setOnEditorActionListener(null)
            input.onFocusChangeListener = null
        }
        container?.let { root ->
            (root.parent as? ViewGroup)?.removeView(root)
        }
        editText = null
        container = null
    }

    private fun bindListeners(input: EditText) {
        input.addTextChangedListener(object : TextWatcher {
            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {
            }

            override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {
                if (suppressInputEvent) return
                val raw = s?.toString() ?: ""
                var next = sanitizeForInputMode(raw, inputMode)
                if (maxLength >= 0 && next.length > maxLength) {
                    next = next.take(maxLength)
                }
                if (next != raw) {
                    suppressInputEvent = true
                    input.setText(next)
                    input.setSelection(next.length.coerceIn(0, input.text?.length ?: 0))
                    suppressInputEvent = false
                }
                emit("input", detailWithValue(next, input))
            }

            override fun afterTextChanged(s: Editable?) {
            }
        })

        input.setOnEditorActionListener { _, actionId, keyEvent ->
            val isEnterKey = keyEvent?.keyCode == KeyEvent.KEYCODE_ENTER && keyEvent.action == KeyEvent.ACTION_DOWN
            val isImeAction = actionId == EditorInfo.IME_ACTION_DONE ||
                actionId == EditorInfo.IME_ACTION_GO ||
                actionId == EditorInfo.IME_ACTION_NEXT ||
                actionId == EditorInfo.IME_ACTION_SEARCH ||
                actionId == EditorInfo.IME_ACTION_SEND
            if (isEnterKey || isImeAction) {
                emit("confirm", detailWithValue(input.text?.toString() ?: "", input))
                if (!confirmHold && !holdKeyboard) {
                    blur()
                }
                return@setOnEditorActionListener true
            }
            false
        }

        input.onFocusChangeListener = View.OnFocusChangeListener { _, hasFocus ->
            val value = input.text?.toString() ?: ""
            if (hasFocus) {
                lastFocusedValue = value
                emit("focus", detailWithValue(value, input))
            } else {
                lastFocusPropValue = false
                emit("blur", detailWithValue(value, input))
                if (value != lastFocusedValue) {
                    emit("change", detailWithValue(value, input))
                }
            }
        }
    }

    private fun detailWithValue(value: String, input: EditText): Map<String, Any> {
        return mapOf(
            "value" to value,
            "cursor" to input.selectionStart.coerceAtLeast(0)
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

    private fun resolveInputType(type: String, password: Boolean): Int {
        if (password || type == "password" || type == "safe-password") {
            return InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD or InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
        }
        return when (type) {
            "number" -> InputType.TYPE_CLASS_NUMBER
            "digit" -> InputType.TYPE_CLASS_NUMBER or InputType.TYPE_NUMBER_FLAG_DECIMAL
            else -> InputType.TYPE_CLASS_TEXT
        }
    }

    private fun resolveInputMode(type: String, password: Boolean): InputMode {
        if (password || type == "password" || type == "safe-password") {
            return InputMode.PASSWORD
        }
        return when (type) {
            "number" -> InputMode.NUMBER
            "digit" -> InputMode.DIGIT
            else -> InputMode.TEXT
        }
    }

    private fun sanitizeAndApplyIfNeeded(input: EditText) {
        val current = input.text?.toString() ?: ""
        var next = sanitizeForInputMode(current, inputMode)
        if (maxLength >= 0 && next.length > maxLength) {
            next = next.take(maxLength)
        }
        if (next == current) return
        suppressInputEvent = true
        input.setText(next)
        input.setSelection(next.length.coerceIn(0, input.text?.length ?: 0))
        suppressInputEvent = false
    }

    private fun applyInputMode(input: EditText, type: String, password: Boolean) {
        val oldStart = input.selectionStart.coerceAtLeast(0)
        val oldEnd = input.selectionEnd.coerceAtLeast(0)
        val nextType = resolveInputType(type, password)
        var changed = false

        if (nextType != lastAppliedInputType) {
            val oldTypeface = input.typeface
            input.inputType = nextType
            input.typeface = oldTypeface
            lastAppliedInputType = nextType
            changed = true
        }

        val nextKeyListener = when (inputMode) {
            InputMode.NUMBER -> numberKeyListener
            InputMode.DIGIT -> digitKeyListener
            InputMode.TEXT, InputMode.PASSWORD -> defaultKeyListener
        }
        if (input.keyListener !== nextKeyListener) {
            input.keyListener = nextKeyListener
            changed = true
        }

        val desiredTransformation = if (password) PasswordTransformationMethod.getInstance() else null
        if (password) {
            if (input.transformationMethod !== desiredTransformation) {
                input.transformationMethod = desiredTransformation
                changed = true
            }
        } else if (input.transformationMethod != null) {
            input.transformationMethod = null
            changed = true
        }

        val length = input.text?.length ?: 0
        if (oldStart >= 0 && oldEnd >= oldStart) {
            input.setSelection(oldStart.coerceIn(0, length), oldEnd.coerceIn(0, length))
        }

        if (changed && input.hasFocus()) {
            restartInputMethod(input)
        }
    }

    private fun applyInputFilters(input: EditText) {
        val filters = mutableListOf<InputFilter>()
        when (inputMode) {
            InputMode.NUMBER -> filters.add(NumberOnlyFilter)
            InputMode.DIGIT -> filters.add(DigitFilter)
            InputMode.TEXT, InputMode.PASSWORD -> {}
        }
        if (maxLength >= 0) {
            filters.add(InputFilter.LengthFilter(maxLength))
        }
        input.filters = filters.toTypedArray()
    }

    private fun sanitizeForInputMode(value: String, mode: InputMode): String {
        return when (mode) {
            InputMode.NUMBER -> {
                val builder = StringBuilder(value.length)
                value.forEach { ch ->
                    if (isAsciiDigit(ch)) builder.append(ch)
                }
                builder.toString()
            }
            InputMode.DIGIT -> {
                val builder = StringBuilder(value.length)
                var hasDot = false
                value.forEach { ch ->
                    when {
                        isAsciiDigit(ch) -> builder.append(ch)
                        ch == '.' && !hasDot -> {
                            hasDot = true
                            builder.append(ch)
                        }
                    }
                }
                builder.toString()
            }
            InputMode.TEXT, InputMode.PASSWORD -> value
        }
    }

    private fun isAsciiDigit(ch: Char): Boolean {
        return ch in '0'..'9'
    }

    private fun resolveImeOptions(confirmType: String): Int {
        return when (confirmType) {
            "send" -> EditorInfo.IME_ACTION_SEND
            "search" -> EditorInfo.IME_ACTION_SEARCH
            "next" -> EditorInfo.IME_ACTION_NEXT
            "go" -> EditorInfo.IME_ACTION_GO
            else -> EditorInfo.IME_ACTION_DONE
        }
    }

    private fun applyPlaceholderStyleIfNeeded(input: EditText, styleValue: String?) {
        val style = styleValue?.trim()?.takeIf { it.isNotEmpty() }
        if (style == lastAppliedPlaceholderStyle) return
        if (style == null) {
            defaultHintTextColors?.let { input.setHintTextColor(it) }
            lastAppliedPlaceholderStyle = null
            return
        }
        val colorExpr = extractColorFromStyle(style) ?: style
        val color = parseColorCompat(colorExpr) ?: run {
            defaultHintTextColors?.let { input.setHintTextColor(it) }
            lastAppliedPlaceholderStyle = style
            return
        }
        input.setHintTextColor(color)
        lastAppliedPlaceholderStyle = style
    }

    private fun applyCursorColorIfNeeded(input: EditText, colorValue: String?) {
        val raw = colorValue?.trim()?.takeIf { it.isNotEmpty() }
        if (raw == null) {
            clearCursorColorIfNeeded(input)
            return
        }
        if (raw == lastAppliedCursorColor) return
        val parsed = parseColorCompat(raw) ?: return

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            val width = max(2, (input.resources.displayMetrics.density * 2f).roundToInt())
            val cursor = GradientDrawable().apply {
                shape = GradientDrawable.RECTANGLE
                setColor(parsed)
                setSize(width, max(1, input.lineHeight))
            }
            input.textCursorDrawable = cursor
        } else {
            tryTintLegacyCursor(input, parsed)
        }
        lastAppliedCursorColor = raw
    }

    private fun clearCursorColorIfNeeded(input: EditText) {
        if (lastAppliedCursorColor == null) return
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            input.textCursorDrawable = null
        }
        lastAppliedCursorColor = null
    }

    private fun restartInputMethod(input: EditText) {
        val imm = input.context.getSystemService(Context.INPUT_METHOD_SERVICE) as? InputMethodManager ?: return
        imm.restartInput(input)
    }

    private fun hideSoftKeyboard(input: EditText) {
        val imm = input.context.getSystemService(Context.INPUT_METHOD_SERVICE) as? InputMethodManager ?: return
        imm.hideSoftInputFromWindow(input.windowToken, 0)
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
            (alphaFloat.coerceIn(0f, 1f) * 255f).roundToInt()
        }
        return Color.argb(a, r, g, b)
    }

    private fun extractColorFromStyle(style: String): String? {
        val match = STYLE_COLOR_REGEX.find(style) ?: return null
        val value = match.groupValues.getOrNull(1)?.trim().orEmpty()
        return value.takeIf { it.isNotEmpty() }
    }

    private fun tryTintLegacyCursor(input: EditText, color: Int) {
        try {
            val cursorResField = TextView::class.java.getDeclaredField("mCursorDrawableRes")
            cursorResField.isAccessible = true
            val cursorRes = cursorResField.getInt(input)
            if (cursorRes == 0) return

            val editorField = TextView::class.java.getDeclaredField("mEditor")
            editorField.isAccessible = true
            val editor = editorField.get(input) ?: return

            val drawable = input.context.getDrawable(cursorRes)?.mutate() ?: return
            drawable.setColorFilter(color, PorterDuff.Mode.SRC_IN)
            val drawables = arrayOf(drawable, drawable)

            val cursorField = editor.javaClass.getDeclaredField("mCursorDrawable")
            cursorField.isAccessible = true
            cursorField.set(editor, drawables)
        } catch (_: Exception) {
        }
    }

    private companion object {
        val STYLE_COLOR_REGEX = Regex("""(?:^|;)\s*color\s*:\s*([^;]+)""", RegexOption.IGNORE_CASE)
        val RGB_REGEX = Regex(
            """rgba?\(\s*([0-9]{1,3})\s*,\s*([0-9]{1,3})\s*,\s*([0-9]{1,3})(?:\s*,\s*([0-9]*\.?[0-9]+))?\s*\)""",
            RegexOption.IGNORE_CASE
        )
    }
}

private object NumberOnlyFilter : InputFilter {
    override fun filter(
        source: CharSequence,
        start: Int,
        end: Int,
        dest: Spanned,
        dstart: Int,
        dend: Int
    ): CharSequence? {
        if (start >= end) return null
        val builder = StringBuilder(end - start)
        var changed = false
        for (index in start until end) {
            val ch = source[index]
            if (ch in '0'..'9') {
                builder.append(ch)
            } else {
                changed = true
            }
        }
        return if (!changed) null else builder.toString()
    }
}

private object DigitFilter : InputFilter {
    override fun filter(
        source: CharSequence,
        start: Int,
        end: Int,
        dest: Spanned,
        dstart: Int,
        dend: Int
    ): CharSequence? {
        if (start >= end) return null
        val existing = StringBuilder(dest).replace(dstart, dend, "").toString()
        var hasDot = existing.indexOf('.') >= 0
        val builder = StringBuilder(end - start)
        var changed = false
        for (index in start until end) {
            val ch = source[index]
            when {
                ch in '0'..'9' -> builder.append(ch)
                ch == '.' && !hasDot -> {
                    hasDot = true
                    builder.append(ch)
                }
                else -> changed = true
            }
        }
        return if (!changed) null else builder.toString()
    }
}

fun Map<String, Any?>.bool(key: String, default: Boolean): Boolean {
    val value = this[key] ?: return default
    return when (value) {
        is Boolean -> value
        is Number -> value.toInt() != 0
        is String -> value.equals("true", ignoreCase = true) || value == "1"
        else -> default
    }
}

fun Map<String, Any?>.int(key: String, default: Int): Int {
    val value = this[key] ?: return default
    return when (value) {
        is Int -> value
        is Number -> value.toInt()
        is String -> value.toIntOrNull() ?: default
        else -> default
    }
}

fun Map<String, Any?>.double(key: String, default: Double): Double {
    val value = this[key] ?: return default
    return when (value) {
        is Double -> value
        is Number -> value.toDouble()
        is String -> value.toDoubleOrNull() ?: default
        else -> default
    }
}
