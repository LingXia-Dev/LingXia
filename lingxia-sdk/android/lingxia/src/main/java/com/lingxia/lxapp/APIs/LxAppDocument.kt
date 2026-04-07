package com.lingxia.lxapp.APIs

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.webkit.MimeTypeMap
import android.widget.Toast
import com.lingxia.lxapp.LxApp
import com.lingxia.lxapp.APIs.document.PdfViewerActivity
import com.lingxia.lxapp.APIs.document.LingxiaDocumentProvider
import java.io.File
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.Locale

internal object LxAppDocument {
    private const val TAG = "LingXia.LxAppDocument"
    private val WPS_PACKAGES = listOf(
        "cn.wps.moffice_eng",
        "cn.wps.moffice_i18n",
        "cn.wps.moffice_ent"
    )

    @JvmStatic
    fun openDocument(filePath: String, mimeType: String?, showMenu: Boolean): Boolean {
        val resolvedMime = mimeType?.takeIf { it.isNotBlank() } ?: guessMimeType(filePath)
        val isPdf = resolvedMime.equals("application/pdf", true) || filePath.lowercase().endsWith(".pdf")
        return if (isPdf) {
            reviewDocument(filePath, mimeType, showMenu)
        } else {
            openDocumentExternal(filePath, mimeType, showMenu)
        }
    }

    private data class ValidatedRequest(val activity: Activity, val file: File, val resolvedMime: String?)

    private fun validateRequest(caller: String, filePath: String, mimeType: String?): ValidatedRequest? {
        val activity = LxApp.getCurrentActivity()
        if (activity == null) {
            Log.w(TAG, "$caller: current activity is null")
            return null
        }
        if (filePath.isBlank()) {
            Log.w(TAG, "$caller: empty file path")
            return null
        }
        val file = File(filePath)
        val resolvedMime = mimeType?.takeIf { it.isNotBlank() } ?: guessMimeType(filePath)
        return ValidatedRequest(activity, file, resolvedMime)
    }

    @JvmStatic
    fun reviewDocument(filePath: String, mimeType: String?, showMenu: Boolean): Boolean {
        val req = validateRequest("reviewDocument", filePath, mimeType) ?: return false

        val lowerPath = filePath.lowercase()
        if (req.resolvedMime.equals("application/pdf", true) || lowerPath.endsWith(".pdf")) {
            return launchInternalPdfViewer(req.activity, req.file, showMenu)
        }

        Log.i(TAG, "reviewDocument: no native review handler for $filePath")
        return false
    }

    @JvmStatic
    fun openDocumentExternal(filePath: String, mimeType: String?, showMenu: Boolean): Boolean {
        val req = validateRequest("openDocumentExternal", filePath, mimeType) ?: return false

        val wpsPackageName = resolveWpsPackage(req.activity)
        val isChineseLocale = isChineseLanguageLocale(req.activity)
        if (isChineseLocale && wpsPackageName == null) {
            Log.w(TAG, "openDocumentExternal: WPS is required for domestic users but not installed")
            promptInstallWps(req.activity)
            return true
        }

        val contentUri: Uri = LingxiaDocumentProvider.uriForFile(req.activity, req.file)

        val intent = Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(contentUri, req.resolvedMime ?: "*/*")
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            if (isChineseLocale && wpsPackageName != null) {
                `package` = wpsPackageName
            }
        }

        val latch = CountDownLatch(1)
        var success = false

        req.activity.runOnUiThread {
            try {
                req.activity.startActivity(intent)
                success = true
            } catch (error: ActivityNotFoundException) {
                Log.e(TAG, "openDocumentExternal: no activity found to handle document", error)
            } catch (error: Exception) {
                Log.e(TAG, "openDocumentExternal: failed to launch viewer", error)
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
                Log.e(TAG, "reviewDocument: failed to start PdfViewerActivity", error)
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
        val extension = MimeTypeMap.getFileExtensionFromUrl(path)?.lowercase()
        if (!extension.isNullOrEmpty()) {
            val mapped = MimeTypeMap.getSingleton().getMimeTypeFromExtension(extension)
            if (!mapped.isNullOrEmpty()) {
                return mapped
            }
        }
        return null
    }

    private fun resolveWpsPackage(context: Context): String? {
        val packageManager = context.packageManager
        WPS_PACKAGES.forEach { pkg ->
            try {
                packageManager.getPackageInfo(pkg, 0)
                return pkg
            } catch (_: Exception) {
                // Continue checking other package names
            }
        }
        return null
    }

    private fun isChineseLanguageLocale(context: Context): Boolean {
        val configuration = context.resources.configuration
        val locale = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            configuration.locales[0]
        } else {
            @Suppress("DEPRECATION")
            configuration.locale
        }

        if (locale == null) {
            return false
        }

        val language = locale.language.lowercase(Locale.US)
        return language == "zh"
    }

    private fun promptInstallWps(context: Context) {
        val showToast = {
            Toast.makeText(
                context,
                "请安装 WPS Office 以打开此文档",
                Toast.LENGTH_LONG
            ).show()
        }

        if (context is Activity) {
            context.runOnUiThread { showToast() }
        } else {
            Handler(Looper.getMainLooper()).post { showToast() }
        }
    }
}
