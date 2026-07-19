package com.lingxia.lxapp.APIs

import android.content.ContentResolver
import android.content.ContentValues
import android.graphics.Color
import android.graphics.Bitmap
import android.media.MediaMetadataRetriever
import android.media.ThumbnailUtils
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.os.Handler
import android.os.Looper
import android.provider.MediaStore
import android.view.Gravity
import android.view.TextureView
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import androidx.annotation.OptIn
import androidx.appcompat.app.AppCompatActivity
import androidx.media3.common.Effect
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.VideoSize
import androidx.media3.common.util.UnstableApi
import androidx.media3.effect.Presentation
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.transformer.Composition
import androidx.media3.transformer.DefaultEncoderFactory
import androidx.media3.transformer.EditedMediaItem
import androidx.media3.transformer.Effects
import androidx.media3.transformer.ExportException
import androidx.media3.transformer.ExportResult
import androidx.media3.transformer.ProgressHolder
import androidx.media3.transformer.TransformationRequest
import androidx.media3.transformer.Transformer
import androidx.media3.transformer.VideoEncoderSettings
import com.lingxia.lxapp.APIs.media.ImageOps
import com.lingxia.lxapp.APIs.media.MediaCaptureFragment
import com.lingxia.lxapp.APIs.media.MediaPickerFragment
import com.lingxia.lxapp.APIs.media.MediaPreviewFragment
import com.lingxia.lxapp.APIs.media.PreviewMediaPayload
import com.lingxia.lxapp.APIs.media.ScanCodeFragment
import com.lingxia.app.Lingxia
import com.lingxia.lxapp.LxApp
import com.lingxia.app.LxLog
import com.lingxia.app.NativeApi
import org.json.JSONObject
import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import java.io.IOException
import java.io.OutputStream
import java.util.concurrent.CountDownLatch
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.Executors
import java.util.concurrent.Semaphore
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicReference
import java.util.Locale
import kotlin.math.roundToInt

internal object LxAppMedia {
    private const val TAG = "LingXia.LxAppMedia"
    private const val PLAYER_THUMBNAIL_TIMEOUT_MS = 3_500L
    private const val PLAYER_THUMBNAIL_WAIT_GRACE_MS = 1_000L
    private const val PLAYER_THUMBNAIL_CAPTURE_DELAY_MS = 80L
    private const val PLAYER_THUMBNAIL_DEFAULT_WIDTH = 640
    private const val PLAYER_THUMBNAIL_DEFAULT_HEIGHT = 360
    private val mainHandler = Handler(Looper.getMainLooper())
    private val playerThumbnailSemaphore = Semaphore(1, true)

    @JvmStatic
    fun previewMedia(
        items: Array<PreviewMediaPayload>,
        startIndex: Int,
        advance: String,
        showIndexIndicator: Boolean,
        callbackId: Long,
        presentedCallbackId: Long,
        changeCallbackId: Long
    ) {
        val activity = LxApp.getCurrentActivity()
        if (activity == null) {
            LxLog.w(TAG, "previewMedia: current activity is null")
            if (callbackId > 0L) {
                NativeApi.onCallback(callbackId, false, "1000")
            }
            return
        }
        if (items.isEmpty()) {
            LxLog.w(TAG, "previewMedia: invalid media payload")
            if (callbackId > 0L) {
                NativeApi.onCallback(callbackId, false, "1000")
            }
            return
        }
        val appCompat = activity as? AppCompatActivity
        if (appCompat == null) {
            LxLog.w(TAG, "previewMedia: activity is not AppCompatActivity")
            if (callbackId > 0L) {
                NativeApi.onCallback(callbackId, false, "1000")
            }
            return
        }
        appCompat.runOnUiThread {
            MediaPreviewFragment.show(
                activity = appCompat,
                payloads = items,
                startIndex = startIndex,
                advance = advance,
                showIndexIndicator = showIndexIndicator,
                callbackId = callbackId,
                presentedCallbackId = presentedCallbackId,
                changeCallbackId = changeCallbackId
            )
        }
    }

    @JvmStatic
    fun closePreview(callbackId: Long) {
        val activity = LxApp.getCurrentActivity() as? AppCompatActivity ?: return
        activity.runOnUiThread {
            MediaPreviewFragment.close(activity, callbackId)
        }
    }

    /** Retrieve basic metadata for an image URI (width/height/mime). */
    @JvmStatic
    fun getImageInfo(uri: String): String {
        val ctx = Lingxia.applicationContext() ?: return JSONObject().apply {
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
            val ctx = Lingxia.getApplicationContext()
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
                    LxLog.w(TAG, "transcodeToJpeg failed, streaming fallback for $uri")
                    streamCopy(contentResolver, parsed, outFile)
                }
            } else {
                streamCopy(contentResolver, parsed, outFile)
            }
        } catch (oom: OutOfMemoryError) {
            LxLog.e(TAG, "copyAlbumMediaToFile OOM for $uri, falling back to stream", oom)
            val ctx = Lingxia.getApplicationContext()
            streamCopy(ctx.contentResolver, android.net.Uri.parse(uri), File(destPath))
        } catch (e: Exception) {
            LxLog.e(TAG, "copyAlbumMediaToFile failed: ${e.message}", e)
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
            LxLog.e(TAG, "streamCopy failed: ${e.message}", e)
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
            val ctx = Lingxia.getApplicationContext()
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
                    LxLog.e(TAG, "compressImage: failed to create parent for $outputPath")
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
            LxLog.e(TAG, "compressImage OOM for $uri", oom)
            errorResult("Out of memory during compression")
        } catch (e: Exception) {
            LxLog.e(TAG, "compressImage failed: ${e.message}", e)
            errorResult(e.message ?: "compressImage failed")
        }
    }

    @JvmStatic
    fun getVideoInfo(uri: String): String {
        val sourceFile = resolveLocalFile(uri) ?: return JSONObject().apply {
            put("success", false)
            put("error", "Only local file paths are supported")
        }.toString()
        if (!sourceFile.exists()) {
            return JSONObject().apply {
                put("success", false)
                put("error", "Source file does not exist")
            }.toString()
        }

        val retriever = MediaMetadataRetriever()
        return try {
            retriever.setDataSource(sourceFile.absolutePath)

            val width = retriever.extractMetadata(MediaMetadataRetriever.METADATA_KEY_VIDEO_WIDTH)
                ?.toIntOrNull() ?: 0
            val height =
                retriever.extractMetadata(MediaMetadataRetriever.METADATA_KEY_VIDEO_HEIGHT)
                    ?.toIntOrNull() ?: 0
            val durationMs =
                retriever.extractMetadata(MediaMetadataRetriever.METADATA_KEY_DURATION)
                    ?.toLongOrNull() ?: 0L
            val rotation =
                retriever.extractMetadata(MediaMetadataRetriever.METADATA_KEY_VIDEO_ROTATION)
                    ?.toIntOrNull()
            val bitrate = retriever.extractMetadata(MediaMetadataRetriever.METADATA_KEY_BITRATE)
                ?.toLongOrNull()
            val fps = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
                retriever.extractMetadata(MediaMetadataRetriever.METADATA_KEY_CAPTURE_FRAMERATE)
                    ?.toDoubleOrNull()
            } else {
                null
            }
            val mimeType = retriever.extractMetadata(MediaMetadataRetriever.METADATA_KEY_MIMETYPE)
                ?: inferVideoMimeType(sourceFile)

            JSONObject().apply {
                put("success", true)
                put("width", width)
                put("height", height)
                put("durationMs", durationMs)
                if (rotation != null) put("rotation", rotation)
                if (bitrate != null) put("bitrate", bitrate)
                if (fps != null) put("fps", fps)
                put("mimeType", mimeType)
            }.toString()
        } catch (e: Exception) {
            JSONObject().apply {
                put("success", false)
                put("error", e.message ?: "getVideoInfo failed")
            }.toString()
        } finally {
            try {
                retriever.release()
            } catch (_: Exception) {
            }
        }
    }

    @JvmStatic
    fun extractVideoThumbnail(
        uri: String,
        outputPath: String,
        quality: Int,
        targetWidth: Int,
        targetHeight: Int,
        timeMs: Long
    ): String {
        val sourceFile = resolveLocalFile(uri) ?: return JSONObject().apply {
            put("success", false)
            put("error", "Only local file paths are supported")
        }.toString()
        if (!sourceFile.exists()) {
            return JSONObject().apply {
                put("success", false)
                put("error", "Source file does not exist")
            }.toString()
        }
        if (outputPath.isBlank()) {
            return JSONObject().apply {
                put("success", false)
                put("error", "outputPath is empty")
            }.toString()
        }

        val outputFile = File(outputPath)
        outputFile.parentFile?.let { parent ->
            if (!parent.exists() && !parent.mkdirs()) {
                return JSONObject().apply {
                    put("success", false)
                    put("error", "Failed to create output directory")
                }.toString()
            }
        }

        var bitmap: Bitmap? = null
        var decodeError: Throwable? = null
        val retriever = MediaMetadataRetriever()
        var inputStream: FileInputStream? = null
        try {
            try {
                retriever.setDataSource(sourceFile.absolutePath)
            } catch (_: Exception) {
                try {
                    val context = Lingxia.getApplicationContext()
                    retriever.setDataSource(context, Uri.fromFile(sourceFile))
                } catch (_: Exception) {
                    inputStream = FileInputStream(sourceFile)
                    retriever.setDataSource(inputStream.fd)
                }
            }
            val frameTimeUs = if (timeMs >= 0) timeMs * 1000L else 0L
            bitmap = retriever.getFrameAtTime(
                frameTimeUs,
                MediaMetadataRetriever.OPTION_CLOSEST
            )
            if (bitmap == null) {
                bitmap = retriever.getFrameAtTime(
                    frameTimeUs,
                    MediaMetadataRetriever.OPTION_CLOSEST_SYNC
                )
            }
        } catch (e: Exception) {
            decodeError = e
            LxLog.w(TAG, "MediaMetadataRetriever thumbnail failed: ${e.message}")
        } finally {
            try {
                inputStream?.close()
            } catch (_: Exception) {
            }
            try {
                retriever.release()
            } catch (_: Exception) {
            }
        }

        if (bitmap == null) {
            try {
                bitmap = ThumbnailUtils.createVideoThumbnail(
                    sourceFile.absolutePath,
                    MediaStore.Images.Thumbnails.MINI_KIND
                )
            } catch (oom: OutOfMemoryError) {
                outputFile.delete()
                return JSONObject().apply {
                    put("success", false)
                    put("error", "Out of memory during thumbnail generation")
                }.toString()
            } catch (e: Exception) {
                if (decodeError == null) decodeError = e
                LxLog.w(TAG, "ThumbnailUtils thumbnail failed: ${e.message}")
            } catch (t: Throwable) {
                if (decodeError == null) decodeError = t
                LxLog.w(TAG, "ThumbnailUtils thumbnail failed: ${t.message}")
            }
        }

        if (bitmap == null) {
            try {
                bitmap = extractVideoThumbnailViaPlayer(
                    sourceFile = sourceFile,
                    targetWidth = targetWidth,
                    targetHeight = targetHeight,
                    timeMs = timeMs
                )
            } catch (oom: OutOfMemoryError) {
                outputFile.delete()
                return JSONObject().apply {
                    put("success", false)
                    put("error", "Out of memory during player thumbnail generation")
                }.toString()
            } catch (t: Throwable) {
                if (decodeError == null) decodeError = t
                LxLog.w(TAG, "ExoPlayer thumbnail fallback failed: ${t.message}")
            }
        }

        if (bitmap == null) {
            val detail = decodeError?.message?.takeIf { it.isNotBlank() }
            return JSONObject().apply {
                put("success", false)
                put("error", if (detail != null) "Failed to decode video frame: $detail" else "Failed to decode video frame")
            }.toString()
        }

        return try {
            val decodedBitmap = bitmap
            if (
                decodedBitmap.config == null
                || (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O && decodedBitmap.config == Bitmap.Config.HARDWARE)
            ) {
                val softwareBitmap = decodedBitmap.copy(Bitmap.Config.ARGB_8888, false)
                if (softwareBitmap != null) {
                    bitmap = softwareBitmap
                }
            }

            val maxWidth = targetWidth.takeIf { it > 0 }
            val maxHeight = targetHeight.takeIf { it > 0 }
            if (maxWidth != null || maxHeight != null) {
                val (resizedWidth, resizedHeight) = calculateTargetSize(
                    bitmap.width,
                    bitmap.height,
                    maxWidth,
                    maxHeight
                )
                if (resizedWidth != bitmap.width || resizedHeight != bitmap.height) {
                    bitmap = Bitmap.createScaledBitmap(bitmap, resizedWidth, resizedHeight, true)
                }
            }

            val normalizedQuality = quality.coerceIn(0, 100)
            FileOutputStream(outputFile).use { output ->
                if (!bitmap.compress(Bitmap.CompressFormat.JPEG, normalizedQuality, output)) {
                    outputFile.delete()
                    return JSONObject().apply {
                        put("success", false)
                        put("error", "Failed to encode JPEG")
                    }.toString()
                }
            }

            JSONObject().apply {
                put("success", true)
                put("path", outputFile.absolutePath)
                put("width", bitmap.width)
                put("height", bitmap.height)
                put("mimeType", "image/jpeg")
            }.toString()
        } catch (oom: OutOfMemoryError) {
            outputFile.delete()
            JSONObject().apply {
                put("success", false)
                put("error", "Out of memory during thumbnail generation")
            }.toString()
        } catch (e: Exception) {
            outputFile.delete()
            LxLog.w(TAG, "extractVideoThumbnail encode failed: ${e.message}")
            JSONObject().apply {
                put("success", false)
                put("error", e.message ?: "extractVideoThumbnail failed")
            }.toString()
        } catch (t: Throwable) {
            outputFile.delete()
            LxLog.w(TAG, "extractVideoThumbnail encode failed: ${t.message}")
            JSONObject().apply {
                put("success", false)
                put("error", t.message ?: "extractVideoThumbnail failed")
            }.toString()
        }
    }

    private fun extractVideoThumbnailViaPlayer(
        sourceFile: File,
        targetWidth: Int,
        targetHeight: Int,
        timeMs: Long
    ): Bitmap? {
        if (Looper.myLooper() == Looper.getMainLooper()) {
            LxLog.w(TAG, "ExoPlayer thumbnail fallback skipped on main thread")
            return null
        }
        if (!playerThumbnailSemaphore.tryAcquire()) {
            LxLog.w(TAG, "ExoPlayer thumbnail fallback skipped because another extraction is running")
            return null
        }

        val activity = Lingxia.getLastResumedActivity()
        if (activity == null) {
            playerThumbnailSemaphore.release()
            LxLog.w(TAG, "ExoPlayer thumbnail fallback skipped: no foreground activity")
            return null
        }

        val done = AtomicBoolean(false)
        val latch = CountDownLatch(1)
        val bitmapRef = AtomicReference<Bitmap?>()
        val playerRef = AtomicReference<ExoPlayer?>()
        val containerRef = AtomicReference<FrameLayout?>()
        val timeoutRef = AtomicReference<Runnable?>()
        val videoWidthRef = AtomicInteger(0)
        val videoHeightRef = AtomicInteger(0)

        fun cleanupOnMain() {
            val player = playerRef.getAndSet(null)
            if (player != null) {
                try {
                    player.stop()
                } catch (_: Throwable) {
                }
                try {
                    player.release()
                } catch (_: Throwable) {
                }
            }
            val container = containerRef.getAndSet(null)
            if (container != null) {
                try {
                    (container.parent as? ViewGroup)?.removeView(container)
                } catch (_: Throwable) {
                }
            }
        }

        fun finish(bitmap: Bitmap?, message: String?) {
            if (!done.compareAndSet(false, true)) {
                bitmap?.recycle()
                return
            }
            timeoutRef.getAndSet(null)?.let { mainHandler.removeCallbacks(it) }
            if (message != null) {
                LxLog.w(TAG, "ExoPlayer thumbnail fallback failed: $message")
            }
            bitmapRef.set(bitmap)
            mainHandler.post { cleanupOnMain() }
            latch.countDown()
        }

        fun captureFrame(textureView: TextureView) {
            textureView.postDelayed({
                if (done.get()) return@postDelayed
                val (captureWidth, captureHeight) = resolvePlayerThumbnailCaptureSize(
                    targetWidth = targetWidth,
                    targetHeight = targetHeight,
                    videoWidth = videoWidthRef.get(),
                    videoHeight = videoHeightRef.get()
                )
                try {
                    val bitmap = textureView.getBitmap(captureWidth, captureHeight)
                    if (bitmap == null || bitmap.width <= 0 || bitmap.height <= 0) {
                        finish(bitmap, "TextureView returned empty bitmap")
                    } else {
                        finish(bitmap, null)
                    }
                } catch (oom: OutOfMemoryError) {
                    finish(null, "out of memory while reading TextureView bitmap")
                } catch (t: Throwable) {
                    finish(null, t.message ?: t.toString())
                }
            }, PLAYER_THUMBNAIL_CAPTURE_DELAY_MS)
        }

        mainHandler.post {
            if (done.get()) return@post
            try {
                val currentActivity = Lingxia.getLastResumedActivity() ?: activity
                val (initialWidth, initialHeight) = resolvePlayerThumbnailCaptureSize(
                    targetWidth = targetWidth,
                    targetHeight = targetHeight,
                    videoWidth = 0,
                    videoHeight = 0
                )
                val container = FrameLayout(currentActivity).apply {
                    alpha = 0.01f
                    visibility = View.VISIBLE
                    isClickable = false
                    isFocusable = false
                    setBackgroundColor(Color.TRANSPARENT)
                }
                val textureView = TextureView(currentActivity).apply {
                    setOpaque(false)
                    layoutParams = FrameLayout.LayoutParams(
                        FrameLayout.LayoutParams.MATCH_PARENT,
                        FrameLayout.LayoutParams.MATCH_PARENT
                    )
                }
                container.addView(textureView)
                containerRef.set(container)

                val layoutParams = FrameLayout.LayoutParams(initialWidth, initialHeight).apply {
                    gravity = Gravity.START or Gravity.TOP
                }
                currentActivity.addContentView(container, layoutParams)

                fun updateContainerSize(width: Int, height: Int) {
                    val params = (container.layoutParams as? FrameLayout.LayoutParams)
                        ?: FrameLayout.LayoutParams(width, height)
                    params.width = width
                    params.height = height
                    params.gravity = Gravity.START or Gravity.TOP
                    container.layoutParams = params
                    container.requestLayout()
                }

                val player = ExoPlayer.Builder(currentActivity).build()
                playerRef.set(player)
                player.addListener(object : Player.Listener {
                    override fun onVideoSizeChanged(videoSize: VideoSize) {
                        videoWidthRef.set(videoSize.width)
                        videoHeightRef.set(videoSize.height)
                        val (width, height) = resolvePlayerThumbnailCaptureSize(
                            targetWidth = targetWidth,
                            targetHeight = targetHeight,
                            videoWidth = videoSize.width,
                            videoHeight = videoSize.height
                        )
                        updateContainerSize(width, height)
                    }

                    override fun onRenderedFirstFrame() {
                        captureFrame(textureView)
                    }

                    override fun onPlayerError(error: PlaybackException) {
                        finish(null, "${error.errorCodeName}: ${error.message}")
                    }

                    override fun onPlaybackStateChanged(playbackState: Int) {
                        if (playbackState == Player.STATE_ENDED && !done.get()) {
                            finish(null, "playback ended before first frame")
                        }
                    }
                })

                player.volume = 0f
                player.repeatMode = Player.REPEAT_MODE_OFF
                player.setVideoTextureView(textureView)
                player.setMediaItem(MediaItem.fromUri(Uri.fromFile(sourceFile)))
                if (timeMs > 0L) {
                    player.seekTo(timeMs)
                }
                player.prepare()
                player.playWhenReady = true

                val timeoutRunnable = Runnable {
                    finish(null, "timeout after ${PLAYER_THUMBNAIL_TIMEOUT_MS}ms")
                }
                timeoutRef.set(timeoutRunnable)
                mainHandler.postDelayed(timeoutRunnable, PLAYER_THUMBNAIL_TIMEOUT_MS)
            } catch (t: Throwable) {
                finish(null, t.message ?: t.toString())
            }
        }

        return try {
            val completed = latch.await(
                PLAYER_THUMBNAIL_TIMEOUT_MS + PLAYER_THUMBNAIL_WAIT_GRACE_MS,
                TimeUnit.MILLISECONDS
            )
            if (!completed && done.compareAndSet(false, true)) {
                timeoutRef.getAndSet(null)?.let { mainHandler.removeCallbacks(it) }
                LxLog.w(TAG, "ExoPlayer thumbnail fallback failed: wait timed out")
                mainHandler.post { cleanupOnMain() }
            }
            bitmapRef.get()
        } catch (e: InterruptedException) {
            Thread.currentThread().interrupt()
            if (done.compareAndSet(false, true)) {
                timeoutRef.getAndSet(null)?.let { mainHandler.removeCallbacks(it) }
                mainHandler.post { cleanupOnMain() }
            }
            null
        } finally {
            playerThumbnailSemaphore.release()
        }
    }

    private fun resolvePlayerThumbnailCaptureSize(
        targetWidth: Int,
        targetHeight: Int,
        videoWidth: Int,
        videoHeight: Int
    ): Pair<Int, Int> {
        val baseWidth = when {
            videoWidth > 0 -> videoWidth
            targetWidth > 0 -> targetWidth
            targetHeight > 0 -> (targetHeight * 16) / 9
            else -> PLAYER_THUMBNAIL_DEFAULT_WIDTH
        }.coerceAtLeast(1)
        val baseHeight = when {
            videoHeight > 0 -> videoHeight
            targetHeight > 0 -> targetHeight
            targetWidth > 0 -> (targetWidth * 9) / 16
            else -> PLAYER_THUMBNAIL_DEFAULT_HEIGHT
        }.coerceAtLeast(1)

        val maxWidth = targetWidth.takeIf { it > 0 } ?: PLAYER_THUMBNAIL_DEFAULT_WIDTH
        val maxHeight = targetHeight.takeIf { it > 0 }
        val (width, height) = calculateTargetSize(baseWidth, baseHeight, maxWidth, maxHeight)
        return Pair(
            width.takeIf { it > 0 } ?: PLAYER_THUMBNAIL_DEFAULT_WIDTH,
            height.takeIf { it > 0 } ?: PLAYER_THUMBNAIL_DEFAULT_HEIGHT
        )
    }

    // Running transcodes keyed by completion callback id so cancelCompressVideo
    // can stop them. A job removed from this map must not fire its completion.
    private val activeCompressJobs = ConcurrentHashMap<Long, Transformer>()
    private val compressExecutor = Executors.newSingleThreadExecutor { runnable ->
        Thread(runnable, "lingxia-compress-video")
    }

    private fun compressVideoFail(callbackId: Long, message: String): Boolean {
        val payload = JSONObject().apply {
            put("success", false)
            put("error", message)
        }
        NativeApi.onCallback(callbackId, true, payload.toString())
        return true
    }

    @OptIn(UnstableApi::class)
    @JvmStatic
    fun cancelCompressVideo(callbackId: Long): Boolean {
        val transformer = activeCompressJobs.remove(callbackId) ?: return false
        mainHandler.post {
            try {
                transformer.cancel()
            } catch (_: Exception) {
            }
        }
        return true
    }

    @OptIn(UnstableApi::class)
    @JvmStatic
    fun compressVideo(
        uri: String,
        outputPath: String,
        quality: String,
        bitrateKbps: Int,
        fps: Int,
        resolution: Float,
        progressCallbackId: Long,
        callbackId: Long
    ): Boolean {
        val context = Lingxia.applicationContext()
            ?: return compressVideoFail(callbackId, "Application context unavailable")
        val sourceFile = resolveLocalFile(uri)
            ?: return compressVideoFail(callbackId, "Only local file paths are supported")
        if (!sourceFile.exists()) {
            return compressVideoFail(callbackId, "Source file does not exist")
        }
        if (outputPath.isBlank()) {
            return compressVideoFail(callbackId, "outputPath is empty")
        }

        val outputFile = File(outputPath)
        val samePath = try {
            sourceFile.canonicalFile == outputFile.canonicalFile
        } catch (_: IOException) {
            sourceFile.absolutePath == outputFile.absolutePath
        }
        if (samePath) {
            return compressVideoFail(callbackId, "outputPath must differ from source file")
        }
        outputFile.parentFile?.let { parent ->
            if (!parent.exists() && !parent.mkdirs()) {
                return compressVideoFail(callbackId, "Failed to create output directory")
            }
        }
        if (outputFile.exists()) {
            outputFile.delete()
        }

        val sourceInfo = readSourceVideoMetadata(sourceFile)
        val normalizedQuality = quality.trim().lowercase(Locale.ROOT)
        val targetBitrate = selectTargetVideoBitrate(
            quality = normalizedQuality,
            bitrateKbps = bitrateKbps,
            sourceBitrate = sourceInfo?.bitrate
        )
        val targetFps = selectTargetFrameRate(fps)
        val targetResolutionRatio = selectTargetResolutionRatio(
            quality = normalizedQuality,
            resolutionRatio = resolution
        )

        val mediaItem = MediaItem.fromUri(Uri.fromFile(sourceFile))
        val requestBuilder = TransformationRequest.Builder()
            .setVideoMimeType(MimeTypes.VIDEO_H264)
            .setAudioMimeType(MimeTypes.AUDIO_AAC)
        val videoEffects = buildVideoEffects(sourceInfo, targetResolutionRatio)
        val editedMediaItemBuilder = EditedMediaItem.Builder(mediaItem)
        if (targetFps != null) {
            editedMediaItemBuilder.setFrameRate(targetFps)
        }
        if (videoEffects.isNotEmpty()) {
            editedMediaItemBuilder.setEffects(Effects(emptyList(), videoEffects))
        }
        val editedMediaItem = editedMediaItemBuilder.build()
        val request = requestBuilder.build()

        mainHandler.post {
            try {
                val transformerBuilder = Transformer.Builder(context)
                    .setTransformationRequest(request)
                if (targetBitrate != null) {
                    val videoEncoderSettings = VideoEncoderSettings.Builder()
                        .setBitrate(targetBitrate)
                        .build()
                    val encoderFactory = DefaultEncoderFactory.Builder(context)
                        .setRequestedVideoEncoderSettings(videoEncoderSettings)
                        .build()
                    transformerBuilder.setEncoderFactory(encoderFactory)
                }
                val transformer = transformerBuilder
                    .addListener(object : Transformer.Listener {
                        override fun onCompleted(composition: Composition, exportResult: ExportResult) {
                            // remove() returning null means the job was cancelled.
                            if (activeCompressJobs.remove(callbackId) == null) {
                                outputFile.delete()
                                return
                            }
                            compressExecutor.execute {
                                finishCompressVideo(sourceFile, outputFile, progressCallbackId, callbackId)
                            }
                        }

                        override fun onError(
                            composition: Composition,
                            exportResult: ExportResult,
                            exportException: ExportException
                        ) {
                            outputFile.delete()
                            if (activeCompressJobs.remove(callbackId) == null) {
                                return
                            }
                            compressVideoFail(callbackId, exportException.message ?: "compressVideo failed")
                        }
                    })
                    .build()
                activeCompressJobs[callbackId] = transformer
                transformer.start(editedMediaItem, outputFile.absolutePath)
                if (progressCallbackId != 0L) {
                    pollCompressProgress(transformer, progressCallbackId, callbackId)
                }
            } catch (e: Exception) {
                activeCompressJobs.remove(callbackId)
                outputFile.delete()
                compressVideoFail(callbackId, e.message ?: "compressVideo failed")
            }
        }
        return true
    }

    @OptIn(UnstableApi::class)
    private fun pollCompressProgress(transformer: Transformer, progressCallbackId: Long, callbackId: Long) {
        val holder = ProgressHolder()
        val poller = object : Runnable {
            override fun run() {
                if (!activeCompressJobs.containsKey(callbackId)) {
                    return
                }
                val state = transformer.getProgress(holder)
                if (state == Transformer.PROGRESS_STATE_AVAILABLE) {
                    val pct = holder.progress.coerceIn(0, 100)
                    NativeApi.onCallback(progressCallbackId, true, "{\"progress\":$pct}")
                }
                mainHandler.postDelayed(this, 250)
            }
        }
        mainHandler.postDelayed(poller, 250)
    }

    private fun finishCompressVideo(
        sourceFile: File,
        outputFile: File,
        progressCallbackId: Long,
        callbackId: Long
    ) {
        try {
            if (sourceFile.length() > 0 && outputFile.length() >= sourceFile.length()) {
                if (!replaceOutputWithSource(sourceFile, outputFile)) {
                    outputFile.delete()
                    compressVideoFail(callbackId, "Failed to fallback to source video")
                    return
                }
            }

            val infoObj = JSONObject(getVideoInfo(outputFile.absolutePath))
            if (!infoObj.optBoolean("success", false)) {
                outputFile.delete()
                compressVideoFail(
                    callbackId,
                    infoObj.optString("error", "Failed to read compressed video info")
                )
                return
            }
            val mimeType = infoObj.optString("mimeType", "").ifBlank {
                inferVideoMimeType(outputFile)
            }

            // Final 100% so progress bars land cleanly before the result.
            if (progressCallbackId != 0L) {
                NativeApi.onCallback(progressCallbackId, true, "{\"progress\":100}")
            }
            val payload = JSONObject().apply {
                put("success", true)
                put("path", outputFile.absolutePath)
                put("width", infoObj.optInt("width", 0))
                put("height", infoObj.optInt("height", 0))
                put("durationMs", infoObj.optLong("durationMs", 0L))
                put("size", outputFile.length())
                put("mimeType", mimeType)
            }
            NativeApi.onCallback(callbackId, true, payload.toString())
        } catch (e: Exception) {
            outputFile.delete()
            compressVideoFail(callbackId, e.message ?: "compressVideo failed")
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

    private fun inferVideoMimeType(file: File): String {
        return when (file.extension.lowercase()) {
            "mp4", "m4v" -> "video/mp4"
            "mov" -> "video/quicktime"
            "webm" -> "video/webm"
            "mkv" -> "video/x-matroska"
            "avi" -> "video/x-msvideo"
            "3gp", "3gpp" -> "video/3gpp"
            else -> ""
        }
    }

    private data class SourceVideoMetadata(
        val width: Int,
        val height: Int,
        val bitrate: Int?
    )

    private fun readSourceVideoMetadata(file: File): SourceVideoMetadata? {
        val retriever = MediaMetadataRetriever()
        return try {
            retriever.setDataSource(file.absolutePath)
            val width = retriever.extractMetadata(MediaMetadataRetriever.METADATA_KEY_VIDEO_WIDTH)
                ?.toIntOrNull() ?: 0
            val height = retriever.extractMetadata(MediaMetadataRetriever.METADATA_KEY_VIDEO_HEIGHT)
                ?.toIntOrNull() ?: 0
            if (width <= 0 || height <= 0) {
                null
            } else {
                val bitrate = retriever.extractMetadata(MediaMetadataRetriever.METADATA_KEY_BITRATE)
                    ?.toIntOrNull()
                SourceVideoMetadata(width = width, height = height, bitrate = bitrate)
            }
        } catch (e: Exception) {
            LxLog.w(TAG, "Failed to read source video metadata: ${e.message}")
            null
        } finally {
            try {
                retriever.release()
            } catch (_: Exception) {
            }
        }
    }

    private fun selectTargetVideoBitrate(
        quality: String,
        bitrateKbps: Int,
        sourceBitrate: Int?
    ): Int? {
        if (bitrateKbps > 0) {
            return (bitrateKbps.toLong() * 1000L).coerceIn(300_000L, 20_000_000L).toInt()
        }
        if (quality.isEmpty()) {
            return null
        }

        val qualityRatio = when (quality) {
            "low" -> 0.35
            "high" -> 0.80
            else -> 0.55
        }
        val fallbackBitrate = when (quality) {
            "low" -> 900_000
            "high" -> 2_400_000
            else -> 1_500_000
        }
        val estimatedBitrate = if (sourceBitrate != null && sourceBitrate > 0) {
            (sourceBitrate * qualityRatio).roundToInt()
        } else {
            fallbackBitrate
        }
        return estimatedBitrate.coerceIn(300_000, 20_000_000)
    }

    private fun selectTargetFrameRate(fps: Int): Int? {
        if (fps <= 0) {
            return null
        }
        return fps.coerceIn(10, 60)
    }

    private fun selectTargetResolutionRatio(
        quality: String,
        resolutionRatio: Float
    ): Float? {
        if (resolutionRatio > 0f && resolutionRatio < 1f) {
            return resolutionRatio.coerceIn(0.10f, 0.99f)
        }
        return when (quality) {
            "low" -> 0.60f
            "medium" -> 0.80f
            else -> null
        }
    }

    @OptIn(UnstableApi::class)
    private fun buildVideoEffects(
        sourceInfo: SourceVideoMetadata?,
        resolutionRatio: Float?
    ): List<Effect> {
        if (sourceInfo == null || resolutionRatio == null) {
            return emptyList()
        }
        val targetWidth = toEven((sourceInfo.width * resolutionRatio).roundToInt())
        val targetHeight = toEven((sourceInfo.height * resolutionRatio).roundToInt())
        if (targetWidth <= 0 || targetHeight <= 0) {
            return emptyList()
        }
        if (targetWidth >= sourceInfo.width && targetHeight >= sourceInfo.height) {
            return emptyList()
        }
        val presentation = Presentation.createForWidthAndHeight(
            targetWidth,
            targetHeight,
            Presentation.LAYOUT_SCALE_TO_FIT
        )
        return listOf(presentation)
    }

    private fun toEven(value: Int): Int {
        val clamped = value.coerceAtLeast(2)
        return if (clamped % 2 == 0) clamped else clamped - 1
    }

    private fun replaceOutputWithSource(sourceFile: File, outputFile: File): Boolean {
        return try {
            if (outputFile.exists() && !outputFile.delete()) {
                return false
            }
            sourceFile.inputStream().use { input ->
                outputFile.outputStream().use { output ->
                    input.copyTo(output)
                }
            }
            true
        } catch (e: IOException) {
            LxLog.e(TAG, "Failed to fallback to source video: ${e.message}", e)
            false
        }
    }

    private fun calculateTargetSize(
        originalWidth: Int,
        originalHeight: Int,
        maxWidth: Int?,
        maxHeight: Int?
    ): Pair<Int, Int> {
        if (originalWidth <= 0 || originalHeight <= 0) {
            return Pair(0, 0)
        }

        return when {
            maxWidth != null && maxHeight != null -> {
                val widthRatio = maxWidth.toDouble() / originalWidth.toDouble()
                val heightRatio = maxHeight.toDouble() / originalHeight.toDouble()
                val ratio = minOf(widthRatio, heightRatio)
                if (ratio < 1.0) {
                    Pair(
                        (originalWidth * ratio).toInt().coerceAtLeast(1),
                        (originalHeight * ratio).toInt().coerceAtLeast(1)
                    )
                } else {
                    Pair(originalWidth, originalHeight)
                }
            }
            maxWidth != null -> {
                if (maxWidth < originalWidth) {
                    val ratio = maxWidth.toDouble() / originalWidth.toDouble()
                    Pair(
                        maxWidth,
                        (originalHeight * ratio).toInt().coerceAtLeast(1)
                    )
                } else {
                    Pair(originalWidth, originalHeight)
                }
            }
            maxHeight != null -> {
                if (maxHeight < originalHeight) {
                    val ratio = maxHeight.toDouble() / originalHeight.toDouble()
                    Pair(
                        (originalWidth * ratio).toInt().coerceAtLeast(1),
                        maxHeight
                    )
                } else {
                    Pair(originalWidth, originalHeight)
                }
            }
            else -> Pair(originalWidth, originalHeight)
        }
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
        val context = Lingxia.applicationContext()
        if (context == null) {
            com.lingxia.app.NativeApi.onCallback(callbackId, false, "1000")
            return
        }

        Thread {
            val errorCode = try {
                saveMediaToGallery(context, uriString, mimeType, isImage)
            } catch (sec: SecurityException) {
                "3004"
            } catch (e: Exception) {
                LxLog.e(TAG, "Error saving media to gallery: ${e.message}", e)
                "1000"
            }

            if (errorCode == null) {
                com.lingxia.app.NativeApi.onCallback(callbackId, true, "{}")
            } else {
                com.lingxia.app.NativeApi.onCallback(callbackId, false, errorCode)
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
            LxLog.e(TAG, "Source file does not exist: $uriString")
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
            LxLog.e(TAG, "Failed to copy file to MediaStore: ${io.message}")
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
            LxLog.e(TAG, "Error copying file: ${e.message}", e)
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
            LxLog.w(TAG, "chooseMedia: current activity is null")
            com.lingxia.app.NativeApi.onCallback(callbackId, false, "1000")
            return
        }

        val normalizedSources = if (sources in 0..2) sources else 2
        val allowAlbum = normalizedSources == 0 || normalizedSources == 2
        val allowCamera = normalizedSources == 1 || normalizedSources == 2

        val appCompat = activity as? AppCompatActivity
        if (appCompat == null) {
            com.lingxia.app.NativeApi.onCallback(callbackId, false, "1000")
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
                com.lingxia.app.NativeApi.onCallback(callbackId, false, "1002")
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
                LxLog.e(TAG, "scanCode failed", e)
                NativeApi.onCallback(callbackId, false, "1001")
            }
        }
    }

}
