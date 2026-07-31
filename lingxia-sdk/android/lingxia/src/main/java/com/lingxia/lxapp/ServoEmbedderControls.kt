package com.lingxia.lxapp

import android.app.Dialog
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import android.util.Log
import android.webkit.MimeTypeMap
import android.widget.ArrayAdapter
import android.widget.EditText
import android.widget.ListView
import androidx.appcompat.app.AlertDialog
import com.lingxia.webview.LingXiaServoView
import com.lingxia.webview.LingXiaWebViewHost
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.lang.ref.WeakReference

internal object ServoEmbedderControls {
    private const val TAG = "ServoEmbedderControls"

    fun attachIfNeeded(webView: LingXiaWebViewHost, activity: LxAppActivity) {
        if (webView !is LingXiaServoView) return
        webView.setEmbedderControlHandler(Handler(webView, activity))
    }

    private class Handler(
        webView: LingXiaServoView,
        activity: LxAppActivity,
    ) : LingXiaServoView.EmbedderControlHandler {
        private val webView = WeakReference(webView)
        private val activity = WeakReference(activity)
        private val dialogs = mutableMapOf<Long, Dialog>()
        private val pending = mutableSetOf<Long>()
        private var destroyed = false

        override fun show(requestId: Long, kind: String, payload: String) {
            if (destroyed) return
            pending += requestId
            val host = activity.get() ?: return cancel(requestId)
            val data = runCatching { JSONObject(payload) }.getOrElse { return cancel(requestId) }
            when (kind) {
                "select" -> showSelect(host, requestId, data)
                "file" -> showFile(host, requestId, data)
                "alert", "confirm", "prompt" -> showDialog(host, requestId, kind, data)
                else -> cancel(requestId)
            }
        }

        override fun hide(requestId: Long) {
            pending -= requestId
            dialogs.remove(requestId)?.dismiss()
        }

        override fun onDestroyed() {
            destroyed = true
            pending.clear()
            dialogs.values.toList().forEach(Dialog::dismiss)
            dialogs.clear()
        }

        private fun complete(requestId: Long, value: String) {
            dialogs.remove(requestId)
            if (destroyed || !pending.remove(requestId)) return
            webView.get()?.completeEmbedderControl(requestId, true, value)
        }

        private fun cancel(requestId: Long) {
            dialogs.remove(requestId)
            if (destroyed || !pending.remove(requestId)) return
            webView.get()?.completeEmbedderControl(requestId, false, "")
        }

        private fun showDialog(
            host: LxAppActivity,
            requestId: Long,
            kind: String,
            data: JSONObject,
        ) {
            val input = if (kind == "prompt") {
                EditText(host).apply {
                    setText(data.optString("default"))
                    setSelection(text.length)
                }
            } else {
                null
            }
            val dialog = AlertDialog.Builder(host)
                .setTitle("Web page")
                .setMessage(data.optString("message"))
                .apply { if (input != null) setView(input) }
                .setPositiveButton(android.R.string.ok) { _, _ ->
                    complete(requestId, input?.text?.toString().orEmpty())
                }
                .apply {
                    if (kind != "alert") {
                        setNegativeButton(android.R.string.cancel) { _, _ -> cancel(requestId) }
                    }
                }
                .setOnCancelListener { cancel(requestId) }
                .create()
            dialogs[requestId] = dialog
            dialog.show()
        }

        private fun showSelect(host: LxAppActivity, requestId: Long, data: JSONObject) {
            val options = data.optJSONArray("options") ?: JSONArray()
            val multiple = data.optBoolean("multiple")
            val selected = data.optJSONArray("selected")?.toIntSet() ?: emptySet()
            val labels = ArrayList<String>(options.length())
            val indices = IntArray(options.length())
            val enabled = BooleanArray(options.length())
            for (position in 0 until options.length()) {
                val option = options.optJSONObject(position) ?: JSONObject()
                val group = if (option.isNull("group")) {
                    null
                } else {
                    option.optString("group").takeIf { it.isNotBlank() }
                }
                val label = option.optString("label")
                labels += if (group == null) label else "$group / $label"
                indices[position] = option.optInt("index", position)
                enabled[position] = !option.optBoolean("disabled")
            }

            val list = ListView(host).apply {
                choiceMode = if (multiple) ListView.CHOICE_MODE_MULTIPLE else ListView.CHOICE_MODE_SINGLE
                adapter = object : ArrayAdapter<String>(
                    host,
                    if (multiple) android.R.layout.simple_list_item_multiple_choice
                    else android.R.layout.simple_list_item_single_choice,
                    labels,
                ) {
                    override fun isEnabled(position: Int): Boolean = enabled[position]
                }
                for (position in indices.indices) {
                    setItemChecked(position, indices[position] in selected)
                }
            }
            val dialog = AlertDialog.Builder(host)
                .setTitle("Select")
                .setView(list)
                .setPositiveButton(android.R.string.ok) { _, _ ->
                    val result = JSONArray()
                    for (position in indices.indices) {
                        if (list.isItemChecked(position)) result.put(indices[position])
                    }
                    complete(requestId, result.toString())
                }
                .setNegativeButton(android.R.string.cancel) { _, _ -> cancel(requestId) }
                .setOnCancelListener { cancel(requestId) }
                .create()
            dialogs[requestId] = dialog
            dialog.show()
        }

        private fun showFile(host: LxAppActivity, requestId: Long, data: JSONObject) {
            val filters = data.optJSONArray("filters")?.toStringList().orEmpty()
            val mimeTypes = filters.mapNotNull { extension ->
                MimeTypeMap.getSingleton().getMimeTypeFromExtension(extension.removePrefix("."))
            }.distinct()
            val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                addCategory(Intent.CATEGORY_OPENABLE)
                type = mimeTypes.singleOrNull() ?: "*/*"
                putExtra(Intent.EXTRA_ALLOW_MULTIPLE, data.optBoolean("multiple"))
                if (mimeTypes.size > 1) putExtra(Intent.EXTRA_MIME_TYPES, mimeTypes.toTypedArray())
            }
            if (!host.openHostFileDialog(intent) { uris ->
                    if (uris == null) {
                        cancel(requestId)
                        return@openHostFileDialog
                    }
                    Thread {
                        val paths = materializeFiles(host, requestId, uris)
                        host.runOnUiThread {
                            if (paths.isEmpty()) cancel(requestId)
                            else complete(requestId, JSONArray(paths).toString())
                        }
                    }.start()
                }) {
                cancel(requestId)
            }
        }

        private fun materializeFiles(
            host: LxAppActivity,
            requestId: Long,
            values: List<String>,
        ): List<String> {
            val directory = File(host.cacheDir, "servo-file-chooser").apply { mkdirs() }
            return values.mapIndexedNotNull { index, raw ->
                runCatching {
                    val uri = Uri.parse(raw)
                    val displayName = host.contentResolver.query(
                        uri,
                        arrayOf(OpenableColumns.DISPLAY_NAME),
                        null,
                        null,
                        null,
                    )?.use { cursor ->
                        if (cursor.moveToFirst()) cursor.getString(0) else null
                    } ?: "file-$index"
                    val safeName = displayName.replace(Regex("[^A-Za-z0-9._-]"), "_")
                    val target = File(directory, "$requestId-$index-$safeName")
                    host.contentResolver.openInputStream(uri).use { input ->
                        requireNotNull(input) { "Unable to open $uri" }
                        target.outputStream().use(input::copyTo)
                    }
                    target.absolutePath
                }.getOrElse { error ->
                    Log.w(TAG, "Unable to materialize selected file $raw", error)
                    null
                }
            }
        }
    }

    private fun JSONArray.toIntSet(): Set<Int> = buildSet {
        for (index in 0 until length()) add(optInt(index))
    }

    private fun JSONArray.toStringList(): List<String> = buildList {
        for (index in 0 until length()) optString(index).takeIf(String::isNotBlank)?.let(::add)
    }
}
