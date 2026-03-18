package com.lingxia.lxapp.NativeComponents.Components

import android.graphics.RectF
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import com.lingxia.lxapp.APIs.LxAppPicker
import com.lingxia.lxapp.APIs.LxAppDatePicker
import com.lingxia.lxapp.LxApp
import com.lingxia.lxapp.NativeComponents.LxNativeComponent
import com.lingxia.lxapp.NativeComponents.LxNativeComponentFactory
import org.json.JSONArray
import org.json.JSONObject

internal class PickerComponentFactory : LxNativeComponentFactory {
    override fun make(id: String, initialProps: Map<String, Any?>, eventSink: (Map<String, Any>) -> Unit) =
        PickerComponent(id, initialProps, eventSink)
}

internal class PickerComponent(
    override val id: String,
    private val initialProps: Map<String, Any?>,
    private val eventSink: (Map<String, Any>) -> Unit
) : LxNativeComponent {

    private var context: android.content.Context? = null
    private var placeholderView: FrameLayout? = null
    private var currentCallbackId: Long = 0L

    override val view: View get() = placeholderView ?: FrameLayout(context!!)

    companion object {
        private var nextCallbackId: Long = 1L
    }

    override fun mount(host: ViewGroup) {
        context = LxApp.getCurrentActivity() ?: host.context
        placeholderView = FrameLayout(context!!).apply {
            setBackgroundColor(android.graphics.Color.TRANSPARENT)
            isClickable = false
            isFocusable = false
        }
        host.addView(placeholderView)

        // Auto-show picker on mount
        showPickerWithProps(initialProps)
    }

    override fun update(props: Map<String, Any?>) {
        // Picker doesn't need updates once shown
    }

    override fun setFrame(frame: RectF) {
        placeholderView?.let { view ->
            view.layoutParams = (view.layoutParams as? FrameLayout.LayoutParams)?.apply {
                leftMargin = frame.left.toInt()
                topMargin = frame.top.toInt()
                width = frame.width().toInt()
                height = frame.height().toInt()
            } ?: FrameLayout.LayoutParams(frame.width().toInt(), frame.height().toInt()).apply {
                leftMargin = frame.left.toInt()
                topMargin = frame.top.toInt()
            }
        }
    }

    override fun focus() { }
    override fun blur() { }

    override fun handleCommand(name: String, params: Map<String, Any?>?) {
        // Commands not needed for picker
    }

    override fun unmount() {
        val mode = initialProps["mode"] as? String ?: "selector"
        if (mode == "date" || mode == "time") {
            LxAppDatePicker.hideDatePicker()
        } else {
            LxAppPicker.hidePicker()
        }
        if (currentCallbackId != 0L) {
            LxAppPicker.localCallbacks.remove(currentCallbackId)
        }
        placeholderView?.let { view ->
            (view.parent as? ViewGroup)?.removeView(view)
        }
        placeholderView = null
    }

    private fun showPickerWithProps(props: Map<String, Any?>) {
        val mode = props["mode"] as? String ?: "selector"

        nextCallbackId++
        currentCallbackId = nextCallbackId

        // Register local callback - only remove on terminal events (confirm/cancel), not scroll
        LxAppPicker.localCallbacks[currentCallbackId] = { success, data ->
            val isTerminal = !success || data.contains("\"confirm\"") || data.contains("\"cancel\"")
            if (isTerminal) {
                LxAppPicker.localCallbacks.remove(currentCallbackId)
            }
            handlePickerCallback(success, data, mode)
        }

        // Date/Time picker
        if (mode == "date" || mode == "time") {
            val fields = props["fields"] as? String ?: "day"
            val value = when (val v = props["value"]) {
                is String -> v
                is List<*> -> JSONArray(v).toString()
                else -> ""
            }

            LxAppDatePicker.showDatePicker(
                mode = mode,
                fields = fields,
                value = value,
                start = props["start"] as? String ?: "",
                end = props["end"] as? String ?: "",
                cancelText = props["cancelText"] as? String ?: "",
                cancelButtonColor = props["cancelButtonColor"] as? String ?: "#F2F2F2",
                cancelTextColor = props["cancelTextColor"] as? String ?: "#007AFF",
                confirmText = props["confirmText"] as? String ?: "",
                confirmButtonColor = props["confirmButtonColor"] as? String ?: "#007AFF",
                confirmTextColor = props["confirmTextColor"] as? String ?: "#FFFFFF",
                callbackId = currentCallbackId
            )
            return
        }

        // Regular picker
        val columnsJSON: String = when (val columns = props["columns"]) {
            is String -> columns
            is List<*> -> JSONArray(columns).toString()
            else -> "[]"
        }

        // Parse columns to determine picker type
        try {
            val parsedData = JSONArray(columnsJSON)

            when {
                // Cascading picker: [[...], {...}]
                parsedData.length() == 2 &&
                parsedData.optJSONArray(0) != null &&
                parsedData.optJSONObject(1) != null -> {
                    val firstColumn = parsedData.getJSONArray(0)
                    val cascadingMap = parsedData.getJSONObject(1)

                    val firstColumnArray = Array(firstColumn.length()) { firstColumn.getString(it) }
                    // Use firstColumnArray order to ensure keys/values align correctly
                    val values = firstColumnArray.map { key ->
                        val valuesArray = cascadingMap.optJSONArray(key) ?: JSONArray()
                        Array(valuesArray.length()) { valuesArray.getString(it) }
                    }

                    LxAppPicker.showCascadingPicker(
                        firstColumn = firstColumnArray,
                        keys = firstColumnArray,
                        values = values.toTypedArray(),
                        cancelText = props["cancelText"] as? String ?: "",
                        cancelButtonColor = props["cancelButtonColor"] as? String ?: "#F2F2F2",
                        cancelTextColor = props["cancelTextColor"] as? String ?: "#007AFF",
                        confirmText = props["confirmText"] as? String ?: "",
                        confirmButtonColor = props["confirmButtonColor"] as? String ?: "#007AFF",
                        confirmTextColor = props["confirmTextColor"] as? String ?: "#FFFFFF",
                        callbackId = currentCallbackId
                    )
                }

                // Multi column picker: [[...], [...], ...]
                parsedData.length() >= 2 &&
                parsedData.optJSONArray(0) != null &&
                parsedData.optJSONArray(1) != null -> {
                    val firstColumn = parsedData.getJSONArray(0)
                    val secondColumn = parsedData.getJSONArray(1)

                    val firstArray = Array(firstColumn.length()) { firstColumn.getString(it) }
                    val secondArray = Array(secondColumn.length()) { secondColumn.getString(it) }

                    LxAppPicker.showDualColumnPicker(
                        firstColumn = firstArray,
                        secondColumn = secondArray,
                        cancelText = props["cancelText"] as? String ?: "",
                        cancelButtonColor = props["cancelButtonColor"] as? String ?: "#F2F2F2",
                        cancelTextColor = props["cancelTextColor"] as? String ?: "#007AFF",
                        confirmText = props["confirmText"] as? String ?: "",
                        confirmButtonColor = props["confirmButtonColor"] as? String ?: "#007AFF",
                        confirmTextColor = props["confirmTextColor"] as? String ?: "#FFFFFF",
                        callbackId = currentCallbackId
                    )
                }

                // Single column picker: [[...]]
                parsedData.length() >= 1 && parsedData.optJSONArray(0) != null -> {
                    val column = parsedData.getJSONArray(0)
                    val columnArray = Array(column.length()) { column.getString(it) }

                    LxAppPicker.showSingleColumnPicker(
                        options = columnArray,
                        cancelText = props["cancelText"] as? String ?: "",
                        cancelButtonColor = props["cancelButtonColor"] as? String ?: "#F2F2F2",
                        cancelTextColor = props["cancelTextColor"] as? String ?: "#007AFF",
                        confirmText = props["confirmText"] as? String ?: "",
                        confirmButtonColor = props["confirmButtonColor"] as? String ?: "#007AFF",
                        confirmTextColor = props["confirmTextColor"] as? String ?: "#FFFFFF",
                        callbackId = currentCallbackId
                    )
                }
            }
        } catch (e: Exception) {
            android.util.Log.e("PickerComponent", "Failed to parse columns: $columnsJSON", e)
        }
    }

    private fun sendEvent(event: String, detail: Map<String, Any>) {
        eventSink(mapOf("event" to event, "detail" to detail))
    }

    private fun handlePickerCallback(success: Boolean, data: String, mode: String) {
        if (!success) {
            sendEvent("change", mapOf("cancelled" to true))
            return
        }

        try {
            val result = JSONObject(data)
            val detail = mutableMapOf<String, Any>()

            if (mode == "date" || mode == "time") {
                if (result.has("value")) {
                    val value = result.get("value")
                    detail["value"] = when (value) {
                        is JSONArray -> {
                            val list = mutableListOf<String>()
                            for (i in 0 until value.length()) {
                                list.add(value.getString(i))
                            }
                            list
                        }
                        else -> value
                    }
                }
            } else {
                if (result.has("index")) {
                    val index = result.get("index")
                    detail["index"] = when (index) {
                        is JSONArray -> {
                            val list = mutableListOf<Int>()
                            for (i in 0 until index.length()) {
                                list.add(index.getInt(i))
                            }
                            list
                        }
                        else -> index
                    }
                }
            }

            if (result.optBoolean("confirm", false)) {
                detail["confirmed"] = true
                sendEvent("change", detail)
            } else if (result.optBoolean("cancel", false)) {
                detail["cancelled"] = true
                sendEvent("change", detail)
            } else {
                sendEvent("scroll", detail)
            }
        } catch (e: Exception) {
            android.util.Log.e("PickerComponent", "Failed to parse callback data: $data", e)
        }
    }
}
