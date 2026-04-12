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
import com.lingxia.lxapp.NativeApi
import com.lingxia.lxapp.APIs.document.PdfViewerActivity
import com.lingxia.lxapp.APIs.document.LingxiaDocumentProvider
import java.io.File
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.Locale
import org.json.JSONArray
import org.json.JSONObject

internal object LxAppFile {
    private const val TAG = "LingXia.LxAppFile"
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

    @JvmStatic
    fun chooseFile(
        multiple: Boolean,
        title: String?,
        defaultPath: String?,
        filtersJson: String?,
        callbackId: Long
    ): Boolean {
        val activity = LxApp.getCurrentActivity()
        if (activity == null) {
            Log.w(TAG, "chooseFile: current activity is null")
            NativeApi.onCallback(callbackId, false, "1000")
            return false
        }

        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
            addCategory(Intent.CATEGORY_OPENABLE)
            type = "*/*"
            putExtra(Intent.EXTRA_ALLOW_MULTIPLE, multiple)
            title?.takeIf { it.isNotBlank() }?.let { putExtra(Intent.EXTRA_TITLE, it) }
        }

        val mimeTypes = parseMimeTypes(filtersJson)
        if (mimeTypes.isNotEmpty()) {
            intent.type = mimeTypes.first()
            if (mimeTypes.size > 1 && Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                intent.putExtra(Intent.EXTRA_MIME_TYPES, mimeTypes.toTypedArray())
            }
        }

        val launched = activity.openHostFileDialog(intent) { paths ->
            val payload = JSONObject().apply {
                put("canceled", paths == null || paths.isEmpty())
                put("paths", JSONArray(paths ?: emptyList<String>()))
            }
            NativeApi.onCallback(callbackId, true, payload.toString())
        }
        if (!launched) {
            NativeApi.onCallback(callbackId, false, "1000")
        }
        return launched
    }

    @JvmStatic
    fun chooseDirectory(
        title: String?,
        _defaultPath: String?,
        callbackId: Long
    ): Boolean {
        val activity = LxApp.getCurrentActivity()
        if (activity == null) {
            Log.w(TAG, "chooseDirectory: current activity is null")
            NativeApi.onCallback(callbackId, false, "1000")
            return false
        }

        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).apply {
            title?.takeIf { it.isNotBlank() }?.let { putExtra(Intent.EXTRA_TITLE, it) }
        }

        val launched = activity.openHostFileDialog(intent) { paths ->
            val payload = JSONObject().apply {
                put("canceled", paths == null || paths.isEmpty())
                put("paths", JSONArray(paths ?: emptyList<String>()))
            }
            NativeApi.onCallback(callbackId, true, payload.toString())
        }
        if (!launched) {
            NativeApi.onCallback(callbackId, false, "1000")
        }
        return launched
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

    private fun parseMimeTypes(filtersJson: String?): List<String> {
        if (filtersJson.isNullOrBlank()) {
            return emptyList()
        }
        return try {
            val array = JSONArray(filtersJson)
            buildList {
                for (index in 0 until array.length()) {
                    val raw = array.optString(index).trim()
                    if (raw.isEmpty()) continue
                    if (raw.contains('/')) {
                        add(raw)
                        continue
                    }
                    val ext = raw.trimStart('.')
                    if (ext.isEmpty()) continue
                    val mime = MimeTypeMap.getSingleton()
                        .getMimeTypeFromExtension(ext.lowercase(Locale.US))
                    if (!mime.isNullOrBlank()) {
                        add(mime)
                    }
                }
            }.distinct()
        } catch (error: Exception) {
            Log.w(TAG, "parseMimeTypes failed: ${error.message}")
            emptyList()
        }
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
