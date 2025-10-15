package com.lingxia.lxapp.APIs

import android.content.ContentValues
import android.content.Context
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import android.util.Log
import com.lingxia.lxapp.LxApp
import androidx.appcompat.app.AppCompatActivity
import com.lingxia.lxapp.media.MediaCaptureFragment
import com.lingxia.lxapp.media.MediaPickerFragment
import com.lingxia.lxapp.media.MediaPreviewFragment
import com.lingxia.lxapp.media.PreviewMediaPayload
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
     * Copy a content URI to destination path using the application ContentResolver
     */
    @JvmStatic
    fun copyUriToPath(uri: String, destPath: String): Boolean {
        return try {
            val ctx = LxApp.getApplicationContext()
            val cr = ctx.contentResolver
            val outFile = java.io.File(destPath)
            outFile.parentFile?.let { if (!it.exists()) it.mkdirs() }
            cr.openInputStream(android.net.Uri.parse(uri))?.use { input ->
                outFile.outputStream().use { output -> input.copyTo(output) }
            } ?: return false
            true
        } catch (e: Exception) {
            Log.e(TAG, "copyUriToPath failed: ${e.message}")
            false
        }
    }

    @JvmStatic
    fun saveImageToPhotosAlbum(imageUri: String): Boolean {
        return saveMediaToGallery(imageUri, "image/jpeg", true)
    }

    @JvmStatic
    fun saveVideoToPhotosAlbum(videoUri: String): Boolean {
        return saveMediaToGallery(videoUri, "video/mp4", false)
    }

    private fun saveMediaToGallery(uriString: String, mimeType: String, isImage: Boolean): Boolean {
        val context = LxApp.applicationContext() ?: return false

        return try {
            // Handle both file URIs (file://) and regular paths
            val sourceFile = if (uriString.startsWith("file://")) {
                File(android.net.Uri.parse(uriString).path ?: uriString)
            } else {
                File(uriString)
            }

            if (!sourceFile.exists()) {
                Log.e(TAG, "Source file does not exist: $uriString")
                return false
            }

            val contentResolver = context.contentResolver
            val contentValues = ContentValues()

            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                // Use MediaStore for Android 10+ (no permission required)
                contentValues.put(MediaStore.MediaColumns.DISPLAY_NAME, sourceFile.name)
                contentValues.put(MediaStore.MediaColumns.MIME_TYPE, mimeType)
                contentValues.put(
                    MediaStore.MediaColumns.RELATIVE_PATH,
                    if (isImage) Environment.DIRECTORY_PICTURES else Environment.DIRECTORY_MOVIES
                )
                contentValues.put(MediaStore.Video.Media.DATE_ADDED, System.currentTimeMillis() / 1000)
                contentValues.put(MediaStore.Video.Media.DATE_MODIFIED, System.currentTimeMillis() / 1000)

                val collection = if (isImage) MediaStore.Images.Media.EXTERNAL_CONTENT_URI
                else MediaStore.Video.Media.EXTERNAL_CONTENT_URI

                val uri = contentResolver.insert(collection, contentValues)
                uri?.let { contentUri ->
                    try {
                        contentResolver.openOutputStream(contentUri).use { outputStream ->
                            if (outputStream != null) {
                                copyFile(sourceFile, outputStream)
                            }
                        }
                        true
                    } catch (e: IOException) {
                        Log.e(TAG, "Failed to copy file to MediaStore: ${e.message}")
                        contentResolver.delete(contentUri, null, null) // Clean up on failure
                        false
                    }
                } ?: false
            } else {
                // For older Android versions, use MediaStore to avoid needing WRITE_EXTERNAL_STORAGE
                // This still requires WRITE_EXTERNAL_STORAGE permission, but we'll try the best approach
                // First try to use MediaStore
                contentValues.put(MediaStore.MediaColumns.DISPLAY_NAME, sourceFile.name)
                contentValues.put(MediaStore.MediaColumns.MIME_TYPE, mimeType)

                val collection = if (isImage) MediaStore.Images.Media.EXTERNAL_CONTENT_URI
                else MediaStore.Video.Media.EXTERNAL_CONTENT_URI

                val uri = contentResolver.insert(collection, contentValues)
                uri?.let { contentUri ->
                    try {
                        contentResolver.openOutputStream(contentUri).use { outputStream ->
                            if (outputStream != null) {
                                copyFile(sourceFile, outputStream)
                            }
                        }
                        true
                    } catch (e: Exception) {
                        Log.e(TAG, "Failed to save using MediaStore, attempting alternative method: ${e.message}")
                        // On older Android versions without permission, we cannot save to public directories
                        false
                    }
                } ?: false
            }
        } catch (e: Exception) {
            Log.e(TAG, "Error saving media to gallery: ${e.message}", e)
            false
        }
    }

    private fun copyFile(sourceFile: File, outputStream: OutputStream): Boolean {
        return try {
            sourceFile.inputStream().use { inputStream ->
                outputStream.use { output ->
                    inputStream.copyTo(output)
                }
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
     * @param sources Int array of sources: 0 = album, 1 = camera
     * @param allowOriginal Allow original size (images)
     * @param allowCompressed Allow compressed (images)
     * @param maxDurationSeconds Max duration for video capture (ignored here)
     * @param cameraFacing 0 = front, 1 = back (ignored here)
     * @param callbackId Callback identifier to deliver result
     */
    @JvmStatic
    fun chooseMedia(
        maxCount: Int,
        mode: Int,
        sources: IntArray?,
        allowOriginal: Boolean,
        allowCompressed: Boolean,
        maxDurationSeconds: Int,
        cameraFacing: Int,
        callbackId: Long
    ) {
        val activity = LxApp.getCurrentActivity()
        if (activity == null) {
            Log.w(TAG, "chooseMedia: current activity is null")
            val payload = org.json.JSONObject().apply { put("error", "No current activity available") }
            com.lingxia.lxapp.NativeApi.onCallback(callbackId, false, payload.toString())
            return
        }

        val allowAlbum = sources == null || sources.isEmpty() || sources.any { it == 0 }
        val allowCamera = sources == null || sources.isEmpty() || sources.any { it == 1 }

        val allowMultiple = maxCount.coerceAtLeast(1) > 1

        val appCompat = activity as? AppCompatActivity
        if (appCompat == null) {
            val payload = org.json.JSONObject().apply { put("error", "Activity is not AppCompatActivity") }
            com.lingxia.lxapp.NativeApi.onCallback(callbackId, false, payload.toString())
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
                val payload = org.json.JSONObject().apply { put("error", "No valid source (album/camera)") }
                com.lingxia.lxapp.NativeApi.onCallback(callbackId, false, payload.toString())
            }
        }
    }

}
