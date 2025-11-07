package com.lingxia.lxapp.APIs

import android.content.ContentResolver
import android.content.ContentValues
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Matrix
import android.media.ExifInterface
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import android.util.Log
import com.lingxia.lxapp.LxApp
import com.lingxia.lxapp.NativeApi
import androidx.appcompat.app.AppCompatActivity
import com.lingxia.lxapp.APIs.media.MediaCaptureFragment
import com.lingxia.lxapp.APIs.media.MediaPickerFragment
import com.lingxia.lxapp.APIs.media.MediaPreviewFragment
import com.lingxia.lxapp.APIs.media.PreviewMediaPayload
import com.lingxia.lxapp.APIs.media.ScanCodeFragment
import org.json.JSONObject
import java.io.File
import java.io.IOException
import java.io.OutputStream
import kotlin.math.max

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

            if (isJpeg) {
                val parsed = android.net.Uri.parse(uri)
                val bitmap = decodeBitmapForCopy(contentResolver, parsed)
                if (bitmap == null) {
                    Log.w(TAG, "decodeBitmapForCopy failed, falling back to byte copy for $uri")
                    return streamCopy(contentResolver, parsed, outFile)
                }
                val orientedBitmap = correctOrientation(bitmap, uri)
                try {
                    outFile.outputStream().use { outputStream ->
                        orientedBitmap.compress(Bitmap.CompressFormat.JPEG, 80, outputStream)
                    }
                } finally {
                    if (orientedBitmap !== bitmap) {
                        orientedBitmap.recycle()
                    }
                    bitmap.recycle()
                }
                true
            } else {
                streamCopy(contentResolver, android.net.Uri.parse(uri), outFile)
            }
        } catch (oom: OutOfMemoryError) {
            Log.e(TAG, "copyAlbumMediaToFile OOM for $uri, falling back to stream", oom)
            val ctx = LxApp.getApplicationContext()
            return streamCopy(ctx.contentResolver, android.net.Uri.parse(uri), File(destPath))
        } catch (e: Exception) {
            Log.e(TAG, "copyAlbumMediaToFile failed: ${e.message}", e)
            false
        }
    }

    private fun decodeBitmapForCopy(resolver: ContentResolver, uri: android.net.Uri): Bitmap? {
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        resolver.openInputStream(uri)?.use { BitmapFactory.decodeStream(it, null, bounds) }
        val width = bounds.outWidth
        val height = bounds.outHeight
        if (width <= 0 || height <= 0) {
            return null
        }
        val options = BitmapFactory.Options().apply {
            inPreferredConfig = Bitmap.Config.ARGB_8888
            inSampleSize = calculateSampleSize(max(width, height), 4096)
        }
        return try {
            resolver.openInputStream(uri)?.use { BitmapFactory.decodeStream(it, null, options) }
        } catch (oom: OutOfMemoryError) {
            Log.e(TAG, "decodeBitmapForCopy OOM for $uri", oom)
            null
        }
    }

    private fun calculateSampleSize(maxDimension: Int, targetMax: Int): Int {
        if (maxDimension <= 0) return 1
        var sample = 1
        var current = maxDimension
        while (current > targetMax) {
            sample *= 2
            current /= 2
        }
        return sample.coerceAtLeast(1)
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
     * Correct image orientation based on EXIF data
     */
    private fun correctOrientation(bitmap: Bitmap, sourceUri: String): Bitmap {
        return try {
            val ctx = LxApp.getApplicationContext()
            val exif = if (sourceUri.startsWith("content://")) {
                ctx.contentResolver.openInputStream(android.net.Uri.parse(sourceUri))?.use { inputStream ->
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
                        ExifInterface(inputStream)
                    } else {
                        null
                    }
                }
            } else {
                val path = if (sourceUri.startsWith("file://")) {
                    android.net.Uri.parse(sourceUri).path ?: sourceUri
                } else {
                    sourceUri
                }
                ExifInterface(path)
            }

            val orientation = exif?.getAttributeInt(
                ExifInterface.TAG_ORIENTATION,
                ExifInterface.ORIENTATION_NORMAL
            ) ?: ExifInterface.ORIENTATION_NORMAL

            when (orientation) {
                ExifInterface.ORIENTATION_ROTATE_90 -> rotateBitmap(bitmap, 90f)
                ExifInterface.ORIENTATION_ROTATE_180 -> rotateBitmap(bitmap, 180f)
                ExifInterface.ORIENTATION_ROTATE_270 -> rotateBitmap(bitmap, 270f)
                ExifInterface.ORIENTATION_FLIP_HORIZONTAL -> flipBitmap(bitmap, horizontal = true, vertical = false)
                ExifInterface.ORIENTATION_FLIP_VERTICAL -> flipBitmap(bitmap, horizontal = false, vertical = true)
                else -> bitmap
            }
        } catch (e: Exception) {
            Log.w(TAG, "Failed to correct orientation: ${e.message}")
            bitmap
        }
    }

    /**
     * Rotate a bitmap by the specified degrees
     */
    private fun rotateBitmap(bitmap: Bitmap, degrees: Float): Bitmap {
        val matrix = Matrix().apply { postRotate(degrees) }
        return Bitmap.createBitmap(bitmap, 0, 0, bitmap.width, bitmap.height, matrix, true)
    }

    /**
     * Flip a bitmap horizontally or vertically
     */
    private fun flipBitmap(bitmap: Bitmap, horizontal: Boolean, vertical: Boolean): Bitmap {
        val matrix = Matrix().apply {
            postScale(
                if (horizontal) -1f else 1f,
                if (vertical) -1f else 1f
            )
        }
        return Bitmap.createBitmap(bitmap, 0, 0, bitmap.width, bitmap.height, matrix, true)
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
            val payload = org.json.JSONObject().apply { put("error", "No current activity available") }
            com.lingxia.lxapp.NativeApi.onCallback(callbackId, false, payload.toString())
            return
        }

        val normalizedSources = if (sources in 0..2) sources else 2
        val allowAlbum = normalizedSources == 0 || normalizedSources == 2
        val allowCamera = normalizedSources == 1 || normalizedSources == 2

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

    @JvmStatic
    fun scanCode(scanTypes: IntArray, onlyFromCamera: Boolean, callbackId: Long) {
        val activity = LxApp.getCurrentActivity()
        if (activity == null) {
            val payload = JSONObject().apply { put("error", "No current activity available") }
            NativeApi.onCallback(callbackId, false, payload.toString())
            return
        }
        val appCompat = activity as? AppCompatActivity
        if (appCompat == null) {
            val payload = JSONObject().apply { put("error", "Activity is not AppCompatActivity") }
            NativeApi.onCallback(callbackId, false, payload.toString())
            return
        }

        appCompat.runOnUiThread {
            try {
                val normalizedTypes = if (scanTypes.isNotEmpty()) scanTypes else intArrayOf()
                ScanCodeFragment.start(appCompat, normalizedTypes, onlyFromCamera, callbackId)
            } catch (e: Exception) {
                Log.e(TAG, "scanCode failed", e)
                NativeApi.onCallback(
                    callbackId,
                    false,
                    e.message ?: "Failed to start scan"
                )
            }
        }
    }

}
