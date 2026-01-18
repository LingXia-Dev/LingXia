package com.lingxia.lxapp.APIs

import android.content.ContentResolver
import android.content.ContentValues
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import android.util.Log
import androidx.appcompat.app.AppCompatActivity
import com.lingxia.lxapp.APIs.media.ImageOps
import com.lingxia.lxapp.APIs.media.MediaCaptureFragment
import com.lingxia.lxapp.APIs.media.MediaPickerFragment
import com.lingxia.lxapp.APIs.media.MediaPreviewFragment
import com.lingxia.lxapp.APIs.media.PreviewMediaPayload
import com.lingxia.lxapp.APIs.media.ScanCodeFragment
import com.lingxia.lxapp.LxApp
import com.lingxia.lxapp.NativeApi
import org.json.JSONObject
import java.io.File
import java.io.IOException
import java.io.OutputStream

internal object LxAppMedia {
    private const val TAG = "LingXia.LxAppMedia"

    @JvmStatic
    fun previewMedia(items: Array<PreviewMediaPayload>) {
        val activity = LxApp.getCurrentActivity()
        if (activity == null) {
            Log.w(TAG, "previewMedia: current activity is null")
            return
        }
        if (items.isEmpty()) {
            Log.w(TAG, "previewMedia: invalid media payload")
            return
        }
        val appCompat = activity as? AppCompatActivity
        if (appCompat == null) {
            Log.w(TAG, "previewMedia: activity is not AppCompatActivity")
            return
        }
        appCompat.runOnUiThread {
            MediaPreviewFragment.show(appCompat, items)
        }
    }

    /**
     * Retrieve basic metadata for an image URI (width/height/mime), akin to wx.getImageInfo.
     */
    @JvmStatic
    fun getImageInfo(uri: String): String {
        val ctx = LxApp.applicationContext() ?: return JSONObject().apply {
            put("success", false)
            put("error", "Application context unavailable")
        }.toString()
        val sourceFile = resolveLocalFile(uri) ?: return JSONObject().apply {
            put("success", false)
            put("error", "Only local file paths are supported")
        }.toString()

        val result = try {
            val info = ImageOps.readInfo(ctx, Uri.fromFile(sourceFile))
            if (info == null) {
                JSONObject().apply {
                    put("success", false)
                    put("error", "Failed to read image info")
                }
            } else {
                JSONObject().apply {
                    put("success", true)
                    put("width", info.width)
                    put("height", info.height)
                    put("mimeType", info.mimeType ?: "")
                }
            }
        } catch (e: Exception) {
            JSONObject().apply {
                put("success", false)
                put("error", e.message ?: "getImageInfo failed")
            }
        }

        return result.toString()
    }

    /**
     * Copy an album/content URI into a concrete file path via the ContentResolver.
     * For JPEG/JPG destinations, transcodes to 80% quality while guarding against OOM.
     * For videos and other files, streams bytes as-is.
     */
    @JvmStatic
    fun copyAlbumMediaToFile(uri: String, destPath: String): Boolean {
        return try {
            val ctx = LxApp.getApplicationContext()
            val contentResolver = ctx.contentResolver
            val outFile = File(destPath)
            outFile.parentFile?.let { if (!it.exists()) it.mkdirs() }

            // Check if destination is JPEG (image compression required)
            val ext = outFile.extension.lowercase()
            val isJpeg = ext == "jpg" || ext == "jpeg"

            val parsed = android.net.Uri.parse(uri)
            if (isJpeg) {
                if (ImageOps.transcodeToJpeg(contentResolver, parsed, outFile)) {
                    true
                } else {
                    Log.w(TAG, "transcodeToJpeg failed, streaming fallback for $uri")
                    streamCopy(contentResolver, parsed, outFile)
                }
            } else {
                streamCopy(contentResolver, parsed, outFile)
            }
        } catch (oom: OutOfMemoryError) {
            Log.e(TAG, "copyAlbumMediaToFile OOM for $uri, falling back to stream", oom)
            val ctx = LxApp.getApplicationContext()
            streamCopy(ctx.contentResolver, android.net.Uri.parse(uri), File(destPath))
        } catch (e: Exception) {
            Log.e(TAG, "copyAlbumMediaToFile failed: ${e.message}", e)
            false
        }
    }

    private fun streamCopy(resolver: ContentResolver, uri: android.net.Uri, dest: File): Boolean {
        return try {
            resolver.openInputStream(uri)?.use { input ->
                dest.outputStream().use { output ->
                    input.copyTo(output)
                }
                true
            } ?: false
        } catch (e: Exception) {
            Log.e(TAG, "streamCopy failed: ${e.message}", e)
            false
        }
    }

    /**
     * Compress an image URI into the provided output file path and return the resulting file path.
     */
    @JvmStatic
    fun compressImage(
        uri: String,
        outputPath: String,
        quality: Int,
        targetWidth: Int,
        targetHeight: Int
    ): String {
        return try {
            val ctx = LxApp.getApplicationContext()
            val resolver = ctx.contentResolver
            val sourceFile = resolveLocalFile(uri)
                ?: return errorResult("Only local file paths are supported")
            if (!sourceFile.exists()) {
                return errorResult("Source file does not exist")
            }
            val normalizedQuality = quality.coerceIn(0, 100)
            val width = targetWidth.takeIf { it > 0 }
            val height = targetHeight.takeIf { it > 0 }
            val maxDimension = listOfNotNull(width, height).maxOrNull() ?: 4096
            val outputFile = File(outputPath)
            outputFile.parentFile?.let { parent ->
                if (!parent.exists() && !parent.mkdirs()) {
                    Log.e(TAG, "compressImage: failed to create parent for $outputPath")
                    return ""
                }
            }
            val success = ImageOps.transcodeToJpeg(
                resolver,
                Uri.fromFile(sourceFile),
                outputFile,
                normalizedQuality,
                maxDimension,
                width,
                height
            )
            if (success) {
                outputFile.absolutePath
            } else {
                outputFile.delete()
                errorResult("Transcode failed")
            }
        } catch (oom: OutOfMemoryError) {
            Log.e(TAG, "compressImage OOM for $uri", oom)
            errorResult("Out of memory during compression")
        } catch (e: Exception) {
            Log.e(TAG, "compressImage failed: ${e.message}", e)
            errorResult(e.message ?: "compressImage failed")
        }
    }

    private fun resolveLocalFile(uri: String): File? {
        return when {
            uri.startsWith("file://", ignoreCase = true) -> {
                val parsed = Uri.parse(uri)
                parsed.path?.let { File(it) }
            }
            uri.startsWith("content://", ignoreCase = true) || uri.startsWith("phasset:", ignoreCase = true) -> null
            else -> File(uri)
        }
    }

    private fun errorResult(message: String): String {
        return "__ERROR__:$message"
    }


    @JvmStatic
    fun saveImageToPhotosAlbum(imageUri: String, callbackId: Long) {
        saveMediaToGalleryWithCallback(imageUri, "image/jpeg", true, callbackId)
    }

    @JvmStatic
    fun saveVideoToPhotosAlbum(videoUri: String, callbackId: Long) {
        saveMediaToGalleryWithCallback(videoUri, "video/mp4", false, callbackId)
    }

    private fun saveMediaToGalleryWithCallback(
        uriString: String,
        mimeType: String,
        isImage: Boolean,
        callbackId: Long
    ) {
        val context = LxApp.applicationContext()
        if (context == null) {
            com.lingxia.lxapp.NativeApi.onCallback(callbackId, false, "1000")
            return
        }

        Thread {
            val errorCode = try {
                saveMediaToGallery(context, uriString, mimeType, isImage)
            } catch (sec: SecurityException) {
                "3004"
            } catch (e: Exception) {
                Log.e(TAG, "Error saving media to gallery: ${e.message}", e)
                "1000"
            }

            if (errorCode == null) {
                com.lingxia.lxapp.NativeApi.onCallback(callbackId, true, "{}")
            } else {
                com.lingxia.lxapp.NativeApi.onCallback(callbackId, false, errorCode)
            }
        }.start()
    }

    // Returns null on success; otherwise returns error code string.
    private fun saveMediaToGallery(
        context: android.content.Context,
        uriString: String,
        mimeType: String,
        isImage: Boolean
    ): String? {
        // Handle both file URIs (file://) and regular paths
        val sourceFile = if (uriString.startsWith("file://")) {
            File(android.net.Uri.parse(uriString).path ?: uriString)
        } else {
            File(uriString)
        }

        if (!sourceFile.exists()) {
            Log.e(TAG, "Source file does not exist: $uriString")
            return "1001"
        }

        val contentResolver = context.contentResolver
        val contentValues = ContentValues()

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            // Use MediaStore for Android 10+ (no permission required).
            contentValues.put(MediaStore.MediaColumns.DISPLAY_NAME, sourceFile.name)
            contentValues.put(MediaStore.MediaColumns.MIME_TYPE, mimeType)
            contentValues.put(
                MediaStore.MediaColumns.RELATIVE_PATH,
                if (isImage) Environment.DIRECTORY_PICTURES else Environment.DIRECTORY_MOVIES
            )
            contentValues.put(MediaStore.Video.Media.DATE_ADDED, System.currentTimeMillis() / 1000)
            contentValues.put(MediaStore.Video.Media.DATE_MODIFIED, System.currentTimeMillis() / 1000)
        } else {
            // For older Android versions, use MediaStore; permission may still be required.
            contentValues.put(MediaStore.MediaColumns.DISPLAY_NAME, sourceFile.name)
            contentValues.put(MediaStore.MediaColumns.MIME_TYPE, mimeType)
        }

        val collection = if (isImage) MediaStore.Images.Media.EXTERNAL_CONTENT_URI
        else MediaStore.Video.Media.EXTERNAL_CONTENT_URI

        val uri = contentResolver.insert(collection, contentValues) ?: return "1001"
        return try {
            contentResolver.openOutputStream(uri).use { outputStream ->
                if (outputStream == null) {
                    contentResolver.delete(uri, null, null)
                    return "1001"
                }
                val ok = copyFile(sourceFile, outputStream)
                if (!ok) {
                    contentResolver.delete(uri, null, null)
                    return "1001"
                }
            }
            null
        } catch (io: IOException) {
            Log.e(TAG, "Failed to copy file to MediaStore: ${io.message}")
            contentResolver.delete(uri, null, null)
            "1001"
        }
    }

    private fun copyFile(sourceFile: File, outputStream: OutputStream): Boolean {
        return try {
            sourceFile.inputStream().use { inputStream ->
                inputStream.copyTo(outputStream)
            }
            true
        } catch (e: IOException) {
            Log.e(TAG, "Error copying file: ${e.message}", e)
            false
        }
    }

    /**
     * Choose media from album (initial implementation).
     * @param maxCount Maximum number of items to select
     * @param mode 0 = images, 1 = videos, 2 = mix
     * @param sources Source selector: 0 = album, 1 = camera, 2 = both
     * @param maxDurationSeconds Max duration for video capture (ignored here)
     * @param cameraFacing 0 = front, 1 = back (ignored here)
     * @param callbackId Callback identifier to deliver result
     */
    @JvmStatic
    fun chooseMedia(
        maxCount: Int,
        mode: Int,
        sources: Int,
        maxDurationSeconds: Int,
        cameraFacing: Int,
        callbackId: Long
    ) {
        val activity = LxApp.getCurrentActivity()
        if (activity == null) {
            Log.w(TAG, "chooseMedia: current activity is null")
            com.lingxia.lxapp.NativeApi.onCallback(callbackId, false, "1000")
            return
        }

        val normalizedSources = if (sources in 0..2) sources else 2
        val allowAlbum = normalizedSources == 0 || normalizedSources == 2
        val allowCamera = normalizedSources == 1 || normalizedSources == 2

        val appCompat = activity as? AppCompatActivity
        if (appCompat == null) {
            com.lingxia.lxapp.NativeApi.onCallback(callbackId, false, "1000")
            return
        }

        appCompat.runOnUiThread {
            val modeStr = when (mode) { 1 -> "videos"; 2 -> "mix"; else -> "images" }
            if (allowCamera && !allowAlbum) {
                // Camera-only: honor image/video; Mix is album-only, default to image
                val captureMode = when (mode) {
                    1 -> "video"
                    else -> "image" // 0: images, 2: mix -> image
                }
                MediaCaptureFragment.start(
                    appCompat,
                    captureMode,
                    maxDurationSeconds,
                    callbackId,
                    cameraFacing
                )
            } else if (allowAlbum) {
                MediaPickerFragment.start(
                    appCompat,
                    maxCount.coerceAtLeast(1),
                    callbackId,
                    modeStr,
                    allowCamera,
                    maxDurationSeconds,
                    cameraFacing
                )
            } else {
                com.lingxia.lxapp.NativeApi.onCallback(callbackId, false, "1002")
            }
        }
    }

    @JvmStatic
    fun scanCode(scanTypes: IntArray, onlyFromCamera: Boolean, callbackId: Long) {
        val activity = LxApp.getCurrentActivity()
        if (activity == null) {
            NativeApi.onCallback(callbackId, false, "1000")
            return
        }
        val appCompat = activity as? AppCompatActivity
        if (appCompat == null) {
            NativeApi.onCallback(callbackId, false, "1000")
            return
        }

        appCompat.runOnUiThread {
            try {
                val normalizedTypes = if (scanTypes.isNotEmpty()) scanTypes else intArrayOf()
                ScanCodeFragment.start(appCompat, normalizedTypes, onlyFromCamera, callbackId)
            } catch (e: Exception) {
                Log.e(TAG, "scanCode failed", e)
                NativeApi.onCallback(callbackId, false, "1001")
            }
        }
    }

}
