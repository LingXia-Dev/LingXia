package com.lingxia.lxapp.APIs

import android.content.ClipData
import android.content.ClipDescription
import android.content.ClipboardManager
import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.os.Build
import com.lingxia.app.Lingxia
import com.lingxia.app.LxLog
import com.lingxia.lxapp.LxApp
import com.lingxia.lxapp.APIs.document.LingxiaDocumentProvider
import java.io.File
import java.io.FileOutputStream
import org.json.JSONArray
import org.json.JSONObject

internal object LxAppClipboard {
    private const val TAG = "LingXia.Clipboard"

    @JvmStatic
    fun write(kind: String, payload: String): String {
        return try {
            val clipboard = clipboard() ?: return fail(4000, "clipboard service unavailable")
            when (kind) {
                "text" -> {
                    clipboard.setPrimaryClip(ClipData.newPlainText("text", payload))
                    ok()
                }
                "image" -> {
                    val context = context() ?: return fail(4000, "clipboard context unavailable")
                    val uri = resolveImageUri(context, payload)
                        ?: return fail(1002, "clipboard image filePath is not a readable image")
                    clipboard.setPrimaryClip(ClipData.newUri(context.contentResolver, "image", uri))
                    ok()
                }
                else -> fail(1002, "unknown clipboard type")
            }
        } catch (error: SecurityException) {
            LxLog.w(TAG, "clipboard write denied", error)
            fail(3008, "clipboard permission denied")
        } catch (error: Throwable) {
            LxLog.e(TAG, "clipboard write failed", error)
            fail(1001, error.message ?: "clipboard write failed")
        }
    }

    @JvmStatic
    fun read(kind: String, imageOutputPath: String): String {
        return try {
            val clipboard = clipboard() ?: return fail(4000, "clipboard service unavailable")
            if (!clipboard.hasPrimaryClip()) {
                return encodeRead(null, null)
            }
            val clip = clipboard.primaryClip ?: return encodeRead(null, null)
            val description = clipboard.primaryClipDescription
            val wantText = kind.isEmpty() || kind == "text"
            val wantImage = kind.isEmpty() || kind == "image"
            var text: String? = null
            var imagePath: String? = null
            if (wantText && (description == null || isTextClip(description))) {
                text = clip.getItemAt(0).coerceNotNullText()
            }
            if (wantImage && imageOutputPath.isNotEmpty()) {
                imagePath = copyImage(clip, imageOutputPath)
            }
            encodeRead(text, imagePath)
        } catch (error: SecurityException) {
            LxLog.w(TAG, "clipboard read denied", error)
            fail(3008, "clipboard permission denied")
        } catch (error: Throwable) {
            LxLog.e(TAG, "clipboard read failed", error)
            fail(1001, error.message ?: "clipboard read failed")
        }
    }

    @JvmStatic
    fun clear(): String {
        return try {
            val clipboard = clipboard() ?: return fail(4000, "clipboard service unavailable")
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                clipboard.clearPrimaryClip()
            } else {
                clipboard.setPrimaryClip(ClipData.newPlainText("", ""))
            }
            ok()
        } catch (error: SecurityException) {
            fail(3008, "clipboard permission denied")
        } catch (error: Throwable) {
            LxLog.e(TAG, "clipboard clear failed", error)
            fail(1001, error.message ?: "clipboard clear failed")
        }
    }

    @JvmStatic
    fun types(): String {
        return try {
            val clipboard = clipboard() ?: return fail(4000, "clipboard service unavailable")
            val types = JSONArray()
            val description = clipboard.primaryClipDescription
            if (description != null) {
                if (description.hasMimeType(ClipDescription.MIMETYPE_TEXT_PLAIN)
                    || description.hasMimeType(ClipDescription.MIMETYPE_TEXT_HTML)
                ) {
                    types.put("text")
                }
                if (hasImageMime(description)) {
                    types.put("image")
                }
            }
            JSONObject()
                .put("ok", true)
                .put("canceled", false)
                .put("types", types)
                .toString()
        } catch (error: SecurityException) {
            fail(3008, "clipboard permission denied")
        } catch (error: Throwable) {
            LxLog.e(TAG, "clipboard types failed", error)
            fail(1001, error.message ?: "clipboard types failed")
        }
    }

    private fun ClipData.Item.coerceNotNullText(): String? {
        val context = context() ?: return text?.toString()
        val value = coerceToText(context)?.toString()
        return value?.takeIf { it.isNotEmpty() } ?: text?.toString()
    }

    private fun resolveImageUri(context: Context, payload: String): Uri? {
        val parsed = runCatching { Uri.parse(payload) }.getOrNull()
        when (parsed?.scheme?.lowercase()) {
            "content", "datashare" -> return parsed.takeIf { isDecodableImage(context, it) }
            "file" -> {
                val file = parsed.path?.let(::File) ?: return null
                if (!file.isFile || !file.canRead() || !isDecodableImage(file)) return null
                return LingxiaDocumentProvider.uriForFile(context, file)
            }
        }
        val file = File(payload)
        if (!file.isFile || !file.canRead() || !isDecodableImage(file)) return null
        return LingxiaDocumentProvider.uriForFile(context, file)
    }

    // Bounds-only decode: rejects a non-image before it lands on the clipboard,
    // so every host answers the same 1002 for a bad `filePath`.
    private fun isDecodableImage(file: File): Boolean {
        val options = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(file.absolutePath, options)
        return options.outWidth > 0 && options.outHeight > 0
    }

    private fun isDecodableImage(context: Context, uri: Uri): Boolean {
        return runCatching {
            context.contentResolver.openInputStream(uri).use { input ->
                if (input == null) return false
                val options = BitmapFactory.Options().apply { inJustDecodeBounds = true }
                BitmapFactory.decodeStream(input, null, options)
                options.outWidth > 0 && options.outHeight > 0
            }
        }.getOrDefault(false)
    }

    // The clip carries a URI, not pixels. Only a URI the resolver reports as
    // `image/*` counts as an image — a copied text document is not one — and
    // the bytes are re-encoded so the `.png` the runtime hands out is a PNG
    // whatever the source format was.
    private fun copyImage(clip: ClipData, dest: String): String? {
        val context = context() ?: return null
        val resolver = context.contentResolver
        val uri = (0 until clip.itemCount)
            .mapNotNull { clip.getItemAt(it).uri }
            .firstOrNull { candidate ->
                val mime = runCatching { resolver.getType(candidate) }.getOrNull()
                mime != null && mime.lowercase().startsWith("image/")
            }
            ?: return null
        return encodeUriAsPng(context, uri, dest)
    }

    private fun encodeUriAsPng(context: Context, uri: Uri, dest: String): String? {
        val out = File(dest)
        out.parentFile?.mkdirs()
        return try {
            val bitmap = context.contentResolver.openInputStream(uri).use { input ->
                if (input == null) return null
                BitmapFactory.decodeStream(input)
            } ?: return null
            FileOutputStream(out).use { output ->
                if (!bitmap.compress(Bitmap.CompressFormat.PNG, 100, output)) {
                    throw IllegalStateException("PNG encode failed")
                }
            }
            dest
        } catch (error: Throwable) {
            LxLog.w(TAG, "clipboard image copy failed", error)
            runCatching { out.delete() }
            null
        }
    }

    private fun isTextClip(description: ClipDescription): Boolean {
        return description.hasMimeType(ClipDescription.MIMETYPE_TEXT_PLAIN)
            || description.hasMimeType(ClipDescription.MIMETYPE_TEXT_HTML)
    }

    private fun hasImageMime(description: ClipDescription): Boolean {
        for (index in 0 until description.mimeTypeCount) {
            val mime = description.getMimeType(index).lowercase()
            if (mime.startsWith("image/") || mime == ClipDescription.MIMETYPE_TEXT_URILIST) {
                return true
            }
        }
        return false
    }

    private fun clipboard(): ClipboardManager? {
        val context = context() ?: return null
        return context.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
    }

    private fun context(): Context? {
        return LxApp.getCurrentActivity() ?: Lingxia.applicationContext()
    }

    private fun ok(): String = JSONObject().put("ok", true).toString()

    private fun fail(code: Int, detail: String): String {
        return JSONObject()
            .put("ok", false)
            .put("error", code)
            .put("detail", detail)
            .toString()
    }

    private fun encodeRead(text: String?, imagePath: String?): String {
        val payload = JSONObject()
            .put("ok", true)
            .put("canceled", false)
        if (text != null) {
            payload.put("text", text)
        }
        if (imagePath != null) {
            payload.put("imagePath", imagePath)
            payload.put("imageMime", "image/png")
        }
        return payload.toString()
    }
}
