package com.lingxia.lxapp.APIs.media

import android.content.ContentResolver
import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Matrix
import android.media.ExifInterface
import android.net.Uri
import android.os.Build
import androidx.annotation.WorkerThread
import com.lingxia.app.LxLog
import java.io.File
import kotlin.math.max
import kotlin.math.roundToInt

internal object ImageOps {
    private const val TAG = "LingXia.ImageOps"

    data class Info(
        val width: Int,
        val height: Int,
        val mimeType: String?
    )

    @WorkerThread
    fun transcodeToJpeg(
        resolver: ContentResolver,
        uri: Uri,
        dest: File,
        quality: Int = 80,
        maxDimension: Int = 4096,
        targetWidth: Int? = null,
        targetHeight: Int? = null
    ): Boolean {
        val decodeHint = listOfNotNull(
            targetWidth?.takeIf { it > 0 },
            targetHeight?.takeIf { it > 0 }
        ).maxOrNull() ?: maxDimension

        val bitmap = decodeBitmap(resolver, uri, decodeHint) ?: return false
        val oriented = correctOrientation(bitmap, uri, resolver)
        val resized = resizeBitmapIfNeeded(oriented, targetWidth, targetHeight)
        val clampedQuality = quality.coerceIn(0, 100)

        return try {
            dest.parentFile?.let { if (!it.exists()) it.mkdirs() }
            dest.outputStream().use { output ->
                resized.compress(Bitmap.CompressFormat.JPEG, clampedQuality, output)
            }
            true
        } finally {
            if (resized !== oriented) {
                resized.recycle()
            }
            if (oriented !== bitmap) {
                oriented.recycle()
            }
            bitmap.recycle()
        }
    }

    private fun resizeBitmapIfNeeded(
        bitmap: Bitmap,
        targetWidth: Int?,
        targetHeight: Int?
    ): Bitmap {
        val width = targetWidth?.takeIf { it > 0 }
        val height = targetHeight?.takeIf { it > 0 }

        if (width == null && height == null) {
            return bitmap
        }

        val (finalWidth, finalHeight) = when {
            width != null && height != null -> Pair(width, height)
            width != null -> {
                if (bitmap.width == 0) return bitmap
                val ratio = bitmap.height.toFloat() / bitmap.width.toFloat()
                val computedHeight = max(1, (width * ratio).roundToInt())
                Pair(width, computedHeight)
            }
            height != null -> {
                if (bitmap.height == 0) return bitmap
                val ratio = bitmap.width.toFloat() / bitmap.height.toFloat()
                val computedWidth = max(1, (height * ratio).roundToInt())
                Pair(computedWidth, height)
            }
            else -> Pair(bitmap.width, bitmap.height)
        }

        if (finalWidth <= 0 || finalHeight <= 0) {
            return bitmap
        }

        if (bitmap.width == finalWidth && bitmap.height == finalHeight) {
            return bitmap
        }

        return Bitmap.createScaledBitmap(bitmap, finalWidth, finalHeight, true)
    }

    @WorkerThread
    fun readInfo(context: Context, uri: Uri): Info? {
        return try {
            val resolver = context.contentResolver
            val opts = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            resolver.openInputStream(uri)?.use { BitmapFactory.decodeStream(it, null, opts) }
            if (opts.outWidth <= 0 || opts.outHeight <= 0) {
                return null
            }
            val mime = resolver.getType(uri)
            Info(opts.outWidth, opts.outHeight, mime)
        } catch (e: Exception) {
            LxLog.e(TAG, "readInfo failed: ${e.message}", e)
            null
        }
    }

    private fun decodeBitmap(
        resolver: ContentResolver,
        uri: Uri,
        maxDimension: Int
    ): Bitmap? {
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        resolver.openInputStream(uri)?.use { BitmapFactory.decodeStream(it, null, bounds) }
        if (bounds.outWidth <= 0 || bounds.outHeight <= 0) {
            return null
        }
        val boundedDimension = max(1, maxDimension)
        val sample = calculateSampleSize(max(bounds.outWidth, bounds.outHeight), boundedDimension)
        val opts = BitmapFactory.Options().apply {
            inPreferredConfig = Bitmap.Config.ARGB_8888
            inSampleSize = sample
        }
        return try {
            resolver.openInputStream(uri)?.use { BitmapFactory.decodeStream(it, null, opts) }
        } catch (oom: OutOfMemoryError) {
            LxLog.e(TAG, "decodeBitmap OOM for $uri", oom)
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

    private fun correctOrientation(
        bitmap: Bitmap,
        sourceUri: Uri,
        resolver: ContentResolver
    ): Bitmap {
        val orientation = readExifOrientation(resolver, sourceUri)
        return when (orientation) {
            ExifInterface.ORIENTATION_ROTATE_90 -> rotate(bitmap, 90f)
            ExifInterface.ORIENTATION_ROTATE_180 -> rotate(bitmap, 180f)
            ExifInterface.ORIENTATION_ROTATE_270 -> rotate(bitmap, 270f)
            ExifInterface.ORIENTATION_FLIP_HORIZONTAL -> flip(bitmap, horizontal = true)
            ExifInterface.ORIENTATION_FLIP_VERTICAL -> flip(bitmap, vertical = true)
            else -> bitmap
        }
    }

    private fun readExifOrientation(resolver: ContentResolver, uri: Uri): Int {
        return try {
            resolver.openInputStream(uri)?.use { inputStream ->
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
                    ExifInterface(inputStream).getAttributeInt(
                        ExifInterface.TAG_ORIENTATION,
                        ExifInterface.ORIENTATION_NORMAL
                    )
                } else {
                    ExifInterface(uri.path ?: "").getAttributeInt(
                        ExifInterface.TAG_ORIENTATION,
                        ExifInterface.ORIENTATION_NORMAL
                    )
                }
            } ?: ExifInterface.ORIENTATION_NORMAL
        } catch (_: Exception) {
            ExifInterface.ORIENTATION_NORMAL
        }
    }

    private fun readExifOrientation(context: Context, uri: Uri): Int {
        return readExifOrientation(context.contentResolver, uri)
    }

    private fun rotate(bitmap: Bitmap, degrees: Float): Bitmap {
        val matrix = Matrix().apply { postRotate(degrees) }
        return Bitmap.createBitmap(bitmap, 0, 0, bitmap.width, bitmap.height, matrix, true)
    }

    private fun flip(bitmap: Bitmap, horizontal: Boolean = false, vertical: Boolean = false): Bitmap {
        val matrix = Matrix().apply {
            postScale(if (horizontal) -1f else 1f, if (vertical) -1f else 1f)
        }
        return Bitmap.createBitmap(bitmap, 0, 0, bitmap.width, bitmap.height, matrix, true)
    }
}
