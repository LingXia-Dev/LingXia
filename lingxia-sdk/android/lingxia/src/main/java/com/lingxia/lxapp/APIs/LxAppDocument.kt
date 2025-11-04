package com.lingxia.lxapp.APIs

import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import android.util.Log
import android.webkit.MimeTypeMap
import com.lingxia.lxapp.LxApp
import com.lingxia.lxapp.APIs.document.PdfViewerActivity
import com.lingxia.lxapp.APIs.document.LingxiaDocumentProvider
import java.io.File
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

internal object LxAppDocument {
    private const val TAG = "LingXia.LxAppDocument"

    @JvmStatic
    fun openDocument(filePath: String, mimeType: String?, showMenu: Boolean): Boolean {
        val activity = LxApp.getCurrentActivity()
        if (activity == null) {
            Log.w(TAG, "openDocument: current activity is null")
            return false
        }

        if (filePath.isBlank()) {
            Log.w(TAG, "openDocument: empty file path")
            return false
        }

        val file = File(filePath)
        if (!file.exists()) {
            Log.w(TAG, "openDocument: file does not exist -> $filePath")
            return false
        }

        val lowerPath = filePath.lowercase()
        val resolvedMime = mimeType?.takeIf { it.isNotBlank() } ?: guessMimeType(filePath)

        if (resolvedMime.equals("application/pdf", true) || lowerPath.endsWith(".pdf")) {
            return launchInternalPdfViewer(activity, file, showMenu)
        }

        val contentUri: Uri = LingxiaDocumentProvider.uriForFile(activity, file)

        val intent = Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(contentUri, resolvedMime ?: "*/*")
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }

        val latch = CountDownLatch(1)
        var success = false

        activity.runOnUiThread {
            try {
                if (showMenu) {
                    val chooser = Intent.createChooser(intent, file.name)
                    activity.startActivity(chooser)
                } else {
                    activity.startActivity(intent)
                }
                success = true
            } catch (error: ActivityNotFoundException) {
                Log.e(TAG, "openDocument: no activity found to handle document", error)
            } catch (error: Exception) {
                Log.e(TAG, "openDocument: failed to launch viewer", error)
            } finally {
                latch.countDown()
            }
        }

        return try {
            latch.await(5, TimeUnit.SECONDS) && success
        } catch (interrupted: InterruptedException) {
            Thread.currentThread().interrupt()
            false
        }
    }

    private fun launchInternalPdfViewer(activity: android.app.Activity, file: File, showMenu: Boolean): Boolean {
        val intent = Intent(activity, PdfViewerActivity::class.java).apply {
            putExtra(PdfViewerActivity.EXTRA_FILE_PATH, file.absolutePath)
            putExtra(PdfViewerActivity.EXTRA_DISPLAY_NAME, file.name)
            putExtra(PdfViewerActivity.EXTRA_SHOW_MENU, showMenu)
        }

        val latch = CountDownLatch(1)
        var success = false

        activity.runOnUiThread {
            try {
                activity.startActivity(intent)
                success = true
            } catch (error: Exception) {
                Log.e(TAG, "openDocument: failed to start PdfViewerActivity", error)
            } finally {
                latch.countDown()
            }
        }

        return try {
            latch.await(5, TimeUnit.SECONDS) && success
        } catch (interrupted: InterruptedException) {
            Thread.currentThread().interrupt()
            false
        }
    }

    private fun guessMimeType(path: String): String? {
        // Note: Rust layer (open.rs) already handles common document types
        val extension = MimeTypeMap.getFileExtensionFromUrl(path)?.lowercase()
        if (!extension.isNullOrEmpty()) {
            val mapped = MimeTypeMap.getSingleton().getMimeTypeFromExtension(extension)
            if (!mapped.isNullOrEmpty()) {
                return mapped
            }
        }
        return null
    }
}
