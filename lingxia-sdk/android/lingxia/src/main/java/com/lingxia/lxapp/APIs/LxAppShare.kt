package com.lingxia.lxapp.APIs

import android.content.ActivityNotFoundException
import android.content.ClipData
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import android.webkit.MimeTypeMap
import com.lingxia.app.LxLog
import com.lingxia.app.NativeApi
import com.lingxia.lxapp.LxApp
import com.lingxia.lxapp.APIs.document.LingxiaDocumentProvider
import java.io.File
import java.io.FileOutputStream
import java.util.Locale
import java.util.UUID
import org.json.JSONArray

internal object LxAppShare {
    private const val TAG = "LingXia.LxAppShare"

    @JvmStatic
    fun share(
        title: String,
        text: String,
        url: String,
        filesJson: String,
        callbackId: Long,
    ): Boolean {
        val activity = LxApp.getCurrentActivity()
        if (activity == null) {
            LxLog.w(TAG, "share: current activity is null")
            NativeApi.onCallback(callbackId, false, "1000")
            return false
        }

        val files = try {
            parseFiles(filesJson)
        } catch (error: Throwable) {
            LxLog.w(TAG, "share: failed to parse filesJson", error)
            NativeApi.onCallback(callbackId, false, "1002")
            return false
        }

        val normalizedTitle = title.trim()
        val normalizedText = text.trim()
        val normalizedUrl = url.trim()
        val shareText = combinedText(normalizedText, normalizedUrl)
        if (normalizedTitle.isEmpty() && shareText.isEmpty() && files.isEmpty()) {
            NativeApi.onCallback(callbackId, false, "1002")
            return false
        }

        val uris = ArrayList<Uri>()
        val mimeTypes = ArrayList<String>()
        for (path in files) {
            val shareFile = resolveShareFile(activity, path)
            if (shareFile == null) {
                LxLog.w(TAG, "share: file is not readable: $path")
                NativeApi.onCallback(callbackId, false, "1000")
                return false
            }
            uris.add(shareFile.uri)
            mimeTypes.add(shareFile.mimeType)
        }

        val intent = Intent(
            if (uris.size > 1) Intent.ACTION_SEND_MULTIPLE else Intent.ACTION_SEND
        ).apply {
            type = resolveIntentMimeType(mimeTypes, hasText = shareText.isNotEmpty())
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            if (mimeTypes.size > 1) {
                putExtra(Intent.EXTRA_MIME_TYPES, mimeTypes.distinct().toTypedArray())
            }
            if (normalizedTitle.isNotEmpty()) {
                putExtra(Intent.EXTRA_TITLE, normalizedTitle)
                putExtra(Intent.EXTRA_SUBJECT, normalizedTitle)
            }
            if (shareText.isNotEmpty()) {
                putExtra(Intent.EXTRA_TEXT, shareText)
            }
            when (uris.size) {
                0 -> Unit
                1 -> {
                    putExtra(Intent.EXTRA_STREAM, uris[0])
                    clipData = ClipData.newRawUri(normalizedTitle.ifEmpty { "share" }, uris[0])
                }
                else -> {
                    putParcelableArrayListExtra(Intent.EXTRA_STREAM, uris)
                    clipData = ClipData.newRawUri(normalizedTitle.ifEmpty { "share" }, uris[0]).apply {
                        for (index in 1 until uris.size) {
                            addItem(ClipData.Item(uris[index]))
                        }
                    }
                }
            }
        }

        val chooserTitle = normalizedTitle.ifEmpty { null }
        val chooser = Intent.createChooser(intent, chooserTitle).apply {
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            if (uris.isNotEmpty()) {
                clipData = intent.clipData
            }
        }
        return try {
            activity.runOnUiThread {
                try {
                    grantReadPermissions(intent, uris)
                    grantReadPermissions(chooser, uris)
                    activity.startActivity(chooser)
                    NativeApi.onCallback(callbackId, true, "{}")
                } catch (error: ActivityNotFoundException) {
                    LxLog.e(TAG, "share: no activity found", error)
                    NativeApi.onCallback(callbackId, false, "1000")
                } catch (error: Throwable) {
                    LxLog.e(TAG, "share: startActivity failed", error)
                    NativeApi.onCallback(callbackId, false, "1000")
                }
            }
            true
        } catch (error: Throwable) {
            LxLog.e(TAG, "share: dispatch failed", error)
            NativeApi.onCallback(callbackId, false, "1000")
            false
        }
    }

    private fun parseFiles(filesJson: String): List<String> {
        if (filesJson.isBlank()) return emptyList()
        val array = JSONArray(filesJson)
        return buildList {
            for (index in 0 until array.length()) {
                val path = array.optString(index).trim()
                if (path.isEmpty()) continue
                add(path)
            }
        }
    }

    private fun combinedText(text: String, url: String): String {
        return listOf(text, url)
            .filter { it.isNotEmpty() }
            .joinToString("\n")
    }

    private data class ShareFile(
        val uri: Uri,
        val mimeType: String,
    )

    private fun resolveShareFile(context: Context, path: String): ShareFile? {
        val parsed = runCatching { Uri.parse(path) }.getOrNull()
        if (parsed != null && parsed.scheme != null) {
            when (parsed.scheme?.lowercase(Locale.US)) {
                "content" -> {
                    val ourAuthority = LingxiaDocumentProvider.authority(context)
                    if (parsed.authority == ourAuthority) {
                        return ShareFile(
                            uri = parsed,
                            mimeType = guessMimeType(context, parsed, path),
                        )
                    }
                    val materialized = materializeContentUri(context, parsed) ?: return null
                    return ShareFile(
                        uri = LingxiaDocumentProvider.uriForFile(context, materialized),
                        mimeType = guessMimeType(context, parsed, materialized.name),
                    )
                }
                "file" -> {
                    val localFile = parsed.path?.let { File(it) } ?: return null
                    if (!localFile.exists() || !localFile.isFile || !localFile.canRead()) {
                        return null
                    }
                    return ShareFile(
                        uri = LingxiaDocumentProvider.uriForFile(context, localFile),
                        mimeType = guessMimeType(context, parsed, localFile.name),
                    )
                }
            }
        }

        val localFile = File(path)
        if (!localFile.exists() || !localFile.isFile || !localFile.canRead()) {
            return null
        }
        return ShareFile(
            uri = LingxiaDocumentProvider.uriForFile(context, localFile),
            mimeType = guessMimeType(context, null, localFile.name),
        )
    }

    private fun materializeContentUri(context: Context, uri: Uri): File? {
        val displayName = queryDisplayName(context, uri)
            ?.takeIf { it.isNotBlank() }
            ?: "shared"
        val mimeType = runCatching { context.contentResolver.getType(uri) }.getOrNull()
        val safeName = sanitizeShareFileName(displayName, mimeType)
        val dir = File(context.cacheDir, "lx_share").apply { mkdirs() }
        val dest = File(dir, "${UUID.randomUUID()}-$safeName")
        return try {
            context.contentResolver.openInputStream(uri).use { input ->
                if (input == null) {
                    LxLog.w(TAG, "materializeContentUri: openInputStream returned null for $uri")
                    return null
                }
                FileOutputStream(dest).use { output ->
                    input.copyTo(output)
                }
            }
            dest
        } catch (error: Throwable) {
            LxLog.w(TAG, "materializeContentUri failed for $uri", error)
            runCatching { dest.delete() }
            null
        }
    }

    private fun sanitizeShareFileName(displayName: String, mimeType: String?): String {
        val safeName = displayName.replace(Regex("[/\\\\]"), "_").trim().ifEmpty { "shared" }
        if (File(safeName).extension.isNotBlank()) {
            return safeName
        }
        val extension = mimeType
            ?.takeIf { it.isNotBlank() }
            ?.let { MimeTypeMap.getSingleton().getExtensionFromMimeType(it) }
            ?.takeIf { it.isNotBlank() }
        return if (extension != null) "$safeName.$extension" else safeName
    }

    private fun guessMimeType(context: Context, uri: Uri?, nameHint: String): String {
        if (uri != null) {
            val fromResolver = runCatching {
                context.contentResolver.getType(uri)
            }.getOrNull()
            if (!fromResolver.isNullOrBlank()) {
                return fromResolver
            }
        }

        val displayName = uri?.let { queryDisplayName(context, it) }.orEmpty()
        val extension = listOf(displayName, nameHint)
            .firstNotNullOfOrNull { value ->
                MimeTypeMap.getFileExtensionFromUrl(value)
                    .takeIf { it.isNotBlank() }
                    ?.lowercase(Locale.US)
            }
            ?: File(nameHint).extension.lowercase(Locale.US)
        if (extension.isNotEmpty()) {
            val mapped = MimeTypeMap.getSingleton().getMimeTypeFromExtension(extension)
            if (!mapped.isNullOrBlank()) {
                return mapped
            }
        }
        return "application/octet-stream"
    }

    private fun queryDisplayName(context: Context, uri: Uri): String? {
        return runCatching {
            context.contentResolver.query(
                uri,
                arrayOf(OpenableColumns.DISPLAY_NAME),
                null,
                null,
                null,
            )?.use { cursor ->
                if (!cursor.moveToFirst()) {
                    return@use null
                }
                val index = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                if (index >= 0) cursor.getString(index) else null
            }
        }.getOrNull()
    }

    private fun resolveIntentMimeType(mimeTypes: List<String>, hasText: Boolean): String {
        val normalized = mimeTypes
            .map { it.trim().lowercase(Locale.US) }
            .filter { it.isNotEmpty() }
            .distinct()
        if (normalized.isEmpty()) {
            return if (hasText) "text/plain" else "*/*"
        }
        if (normalized.size == 1) {
            return normalized[0]
        }
        val topLevels = normalized.map { it.substringBefore('/') }.distinct()
        return if (topLevels.size == 1) "${topLevels[0]}/*" else "*/*"
    }

    private fun grantReadPermissions(intent: Intent, uris: List<Uri>) {
        if (uris.isEmpty()) return
        val activity = LxApp.getCurrentActivity() ?: return
        val targets = activity.packageManager.queryIntentActivities(intent, 0)
        for (target in targets) {
            val packageName = target.activityInfo?.packageName ?: continue
            for (uri in uris) {
                activity.grantUriPermission(
                    packageName,
                    uri,
                    Intent.FLAG_GRANT_READ_URI_PERMISSION
                )
            }
        }
    }
}
