package com.lingxia.lxapp.APIs.document

import android.content.ContentProvider
import android.content.ContentValues
import android.content.Context
import android.database.MatrixCursor
import android.database.Cursor
import android.net.Uri
import android.os.ParcelFileDescriptor
import android.provider.OpenableColumns
import android.util.Base64
import android.webkit.MimeTypeMap
import java.io.File
import java.io.FileNotFoundException
import kotlin.text.Charsets

internal class LingxiaDocumentProvider : ContentProvider() {

    override fun onCreate(): Boolean = true

    override fun getType(uri: Uri): String? {
        val file = resolveFile(context, uri) ?: return null
        val extension = file.extension.lowercase()
        return if (extension.isNotEmpty()) {
            MimeTypeMap.getSingleton().getMimeTypeFromExtension(extension)
        } else {
            "application/octet-stream"
        }
    }

    override fun openFile(uri: Uri, mode: String): ParcelFileDescriptor {
        if (mode != "r") {
            throw FileNotFoundException("Write mode not supported")
        }
        val file = resolveFile(context, uri)
            ?: throw FileNotFoundException("File not found for uri: $uri")
        if (!file.exists() || !file.canRead()) {
            throw FileNotFoundException("Cannot read ${file.absolutePath}")
        }
        return ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY)
    }

    override fun insert(uri: Uri, values: ContentValues?): Uri? = null
    override fun delete(uri: Uri, selection: String?, selectionArgs: Array<out String>?): Int = 0
    override fun update(
        uri: Uri,
        values: ContentValues?,
        selection: String?,
        selectionArgs: Array<out String>?
    ): Int = 0
    override fun query(
        uri: Uri,
        projection: Array<out String>?,
        selection: String?,
        selectionArgs: Array<out String>?,
        sortOrder: String?
    ): Cursor? {
        val file = resolveFile(context, uri) ?: return null
        val columns = projection?.takeIf { it.isNotEmpty() } ?: arrayOf(
            OpenableColumns.DISPLAY_NAME,
            OpenableColumns.SIZE,
        )
        return MatrixCursor(columns).apply {
            addRow(columns.map { column ->
                when (column) {
                    OpenableColumns.DISPLAY_NAME -> file.name
                    OpenableColumns.SIZE -> file.length()
                    else -> null
                }
            }.toTypedArray())
        }
    }

    companion object {
        private const val AUTHORITY_SUFFIX = ".lingxia.documents"
        private const val PATH_SHARE = "share"

        fun authority(context: Context): String {
            return context.applicationContext.packageName + AUTHORITY_SUFFIX
        }

        fun uriForFile(context: Context, file: File): Uri {
            val encoded = Base64.encodeToString(
                file.absolutePath.toByteArray(Charsets.UTF_8),
                Base64.NO_WRAP or Base64.URL_SAFE
            )
            return Uri.Builder()
                .scheme("content")
                .authority(authority(context))
                .appendPath(PATH_SHARE)
                .appendPath(encoded)
                .build()
        }

        private fun resolveFile(context: Context?, uri: Uri): File? {
            context ?: return null
            if (uri.authority != authority(context)) {
                return null
            }
            val segments = uri.pathSegments
            if (segments.size != 2 || segments[0] != PATH_SHARE) {
                return null
            }
            val encodedPath = segments[1]
            val decoded = try {
                val bytes = Base64.decode(encodedPath, Base64.NO_WRAP or Base64.URL_SAFE)
                String(bytes, Charsets.UTF_8)
            } catch (error: IllegalArgumentException) {
                return null
            }
            return File(decoded)
        }
    }
}
