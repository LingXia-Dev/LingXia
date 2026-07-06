package com.lingxia.lxapp.NativeComponents

import android.graphics.SurfaceTexture
import android.app.ActivityManager
import android.content.Context
import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioTrack
import android.media.MediaCodec
import android.media.MediaFormat
import android.os.Build
import android.os.Handler
import android.os.HandlerThread
import android.os.Looper
import android.os.SystemClock
import android.util.Base64
import android.util.Log
import com.lingxia.app.LxLog
import android.view.Surface
import android.view.TextureView
import android.view.View
import org.json.JSONArray
import org.json.JSONObject
import java.nio.ByteBuffer
import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.atomic.AtomicInteger

internal class StreamDecoderSession(
    private val componentId: String,
    private val textureView: TextureView,
    private val eventEmitter: (String, Map<String, Any?>) -> Unit
) : TextureView.SurfaceTextureListener {
    private val logTag = "LingXia.StreamDecoder"
    private val decodeThread = HandlerThread("StreamDecoder-$componentId").apply { start() }
    private val handler = Handler(decodeThread.looper)
    private val audioThread = HandlerThread("StreamDecoderAudio-$componentId").apply { start() }
    private val audioHandler = Handler(audioThread.looper)
    private val mainHandler = Handler(Looper.getMainLooper())
    private val queueProfile = buildQueueProfile(textureView.context)

    private val videoQueue = ArrayBlockingQueue<VideoFrame>(queueProfile.maxVideoQueue)
    private val audioQueue = ArrayBlockingQueue<AudioFrame>(queueProfile.maxAudioQueue)

    private var videoConfig: VideoConfig? = null
    @Volatile private var lastVideoConfigJson: String? = null
    @Volatile private var audioConfig: AudioConfig? = null
    @Volatile private var lastAudioConfigJson: String? = null

    private var videoDecoder: MediaCodec? = null
    @Volatile private var audioDecoder: MediaCodec? = null
    private var videoSurface: Surface? = null
    @Volatile private var audioTrack: AudioTrack? = null
    @Volatile private var audioIsPcm = false
    private var surfaceTexture: SurfaceTexture? = null
    private var firstVideoOutputSeen = false
    private var videoDrainScheduled = false
    private val videoDrainRunnable = Runnable { drainVideo() }
    private val videoBufferInfo = MediaCodec.BufferInfo()

    private var videoBasePtsUs: Long? = null
    private var videoBaseNanoTime: Long? = null
    private var videoLastPtsUs: Long? = null
    @Volatile private var playbackPositionMs: Long = 0
    @Volatile private var needKeyframe = true
    @Volatile private var pendingVideoReconfigure = false

    private var audioDrainScheduled = false
    private val audioDrainRunnable = Runnable { drainAudio() }
    private val audioBufferInfo = MediaCodec.BufferInfo()
    private var audioScratch = ByteArray(0)

    private var pcmPending: ByteArray? = null
    private var pcmPendingOffset: Int = 0
    private var streamVolume = 1.0f
    private var streamMuted = false
    @Volatile private var pendingAudioReconfigure = false

    @Volatile private var pendingPlay = false
    @Volatile private var surfaceReady = false
    private val surfaceGeneration = AtomicInteger(0)
    @Volatile private var paused = false
    @Volatile private var lastPauseAtMs: Long = 0
    private var playNotified = false
    private var metadataNotified = false
    private var previousListener: TextureView.SurfaceTextureListener? = null
    private val detachReleaseRunnable = Runnable {
        if (surfaceReady || textureView.isAttachedToWindow) return@Runnable
        releaseVideoDecoder()
        videoSurface?.release()
        videoSurface = null
    }
    @Volatile private var rebindRequestPosted = false
    @Volatile private var lastRebindRequestAtMs: Long = 0
    private val rebindRunnable = Runnable {
        rebindRequestPosted = false
        lastRebindRequestAtMs = SystemClock.uptimeMillis()
        attachTextureView()
    }

    @Volatile private var lastRectSyncAtMs: Long = 0

    private fun requestRectSync(reason: String) {
        val now = SystemClock.uptimeMillis()
        if (now - lastRectSyncAtMs < 200) return
        lastRectSyncAtMs = now
        mainHandler.post {
            try {
                ComponentRouter.requestRectSync(componentId)
            } catch (e: Exception) {
                Log.w(logTag, "[$componentId] requestRectSync failed ($reason): ${e.message}")
            }
        }
    }
    private val attachStateListener = object : View.OnAttachStateChangeListener {
        override fun onViewAttachedToWindow(v: View) {
            try {
                handler.removeCallbacks(detachReleaseRunnable)
            } catch (_: Exception) {
            }
            attachTextureView()
        }

        override fun onViewDetachedFromWindow(v: View) {
            surfaceReady = false
            surfaceGeneration.incrementAndGet()
            try {
                handler.removeCallbacks(detachReleaseRunnable)
            } catch (_: Exception) {
            }
            handler.postDelayed(detachReleaseRunnable, 900L)
        }
    }

    init {
        attachTextureView()
        mainHandler.post { textureView.addOnAttachStateChangeListener(attachStateListener) }
    }

    fun usesTextureView(view: TextureView): Boolean = textureView === view

    fun isTextureViewAttached(): Boolean = textureView.isAttachedToWindow

    fun playbackPositionSeconds(): Double {
        return playbackPositionMs.toDouble() / 1000.0
    }

    fun applyDesiredAudioState(volume: Float, muted: Boolean) {
        streamVolume = volume.coerceIn(0f, 1f)
        streamMuted = muted
        applyStreamVolume()
    }

    fun configureVideo(configJson: String): Boolean {
        if (configJson == lastVideoConfigJson) return true
        return try {
            videoConfig = VideoConfig.fromJson(configJson)
            lastVideoConfigJson = configJson
            needKeyframe = true
            playNotified = false
            pendingVideoReconfigure = true
            handler.post {
                updateSurfaceBufferSize()
                ensureVideoDecoder()
            }
            true
        } catch (e: Exception) {
            emitError("configure video failed: ${e.message}")
            false
        }
    }

    fun configureAudio(configJson: String): Boolean {
        if (configJson == lastAudioConfigJson) return true
        return try {
            val config = AudioConfig.fromJson(configJson)
            audioConfig = config
            lastAudioConfigJson = configJson
            pendingAudioReconfigure = true
            audioHandler.post { ensureAudioDecoder() }
            true
        } catch (e: Exception) {
            emitError("configure audio failed: ${e.message}")
            false
        }
    }

    fun pushVideo(data: ByteArray, dtsMs: Int, ptsMs: Int, keyframe: Boolean): Boolean {
        if (paused) {
            return true
        }
        if (!surfaceReady) {
            if (pendingPlay) {
                requestSurfaceRebind()
            }
            needKeyframe = true
            return true
        }
        if (needKeyframe && !keyframe) {
            return true
        }
        if (needKeyframe && keyframe) {
            needKeyframe = false
        }

        val frame = VideoFrame(data, dtsMs, ptsMs, keyframe)
        if (!videoQueue.offer(frame)) {
            dropVideoBacklogAndOffer(frame)
        }
        handler.post {
            if (!surfaceReady) return@post
            ensureVideoDecoder()
            scheduleVideoDrain(0L)
        }
        return true
    }

    fun pushAudio(data: ByteArray, dtsMs: Int, ptsMs: Int): Boolean {
        if (paused) {
            return true
        }
        // If audio pipeline isn't ready yet (or was torn down by the system), avoid queuing
        // indefinitely; just request (re)initialization and drop frames until we're ready.
        val decoder = audioDecoder
        val track = audioTrack
        val ready = if (audioIsPcm) {
            // PCM path writes directly to AudioTrack.
            track != null
        } else {
            // AAC path must accept frames as soon as MediaCodec exists; AudioTrack is created lazily
            // when we observe decoder output format / first output buffer in drainAudio().
            decoder != null
        }
        if (!ready) {
            audioHandler.post {
                pendingAudioReconfigure = true
                ensureAudioDecoder()
                scheduleAudioDrain()
            }
            return true
        }
        val frame = AudioFrame(data, dtsMs, ptsMs)
        if (!audioQueue.offer(frame)) {
            val dropped = audioQueue.poll()
            val acceptedAfterDrop = audioQueue.offer(frame)
            if (!acceptedAfterDrop) {
                Log.w(
                    logTag,
                    "[$componentId] audio queue full, dropping frame ptsMs=$ptsMs queue=${audioQueue.size}"
                )
                return true
            }
            Log.w(
                logTag,
                "[$componentId] audio backlog dropped, remaining=${audioQueue.size} (droppedPtsMs=${dropped?.ptsMs}) paused=$paused audioIsPcm=$audioIsPcm pendingAudioReconfigure=$pendingAudioReconfigure decoder=${audioDecoder != null} trackState=${audioTrack?.playState}"
            )
        }
        audioHandler.post {
            ensureAudioDecoder()
            scheduleAudioDrain()
        }
        return true
    }

    fun handleCommand(name: String, paramsJson: String?): Boolean {
        when (name) {
            "play" -> {
                val wasPaused = paused
                val pauseDurationMs = if (wasPaused && lastPauseAtMs > 0L) {
                    SystemClock.elapsedRealtime() - lastPauseAtMs
                } else {
                    0L
                }
                val longPause = pauseDurationMs >= RESUME_RECONFIGURE_THRESHOLD_MS
                paused = false
                pendingPlay = true
                // Always re-emit play after the first rendered frame so UI can clear "loading".
                playNotified = false
                if (wasPaused && longPause) {
                    // Recreate decoder on resume to avoid stale surface/state after long pauses.
                    Log.i(
                        logTag,
                        "[$componentId] play: resuming after long pause (${pauseDurationMs}ms), forcing video reconfigure + keyframe gate"
                    )
                    pendingVideoReconfigure = true
                    needKeyframe = true
                    videoBasePtsUs = null
                    videoBaseNanoTime = null
                    videoLastPtsUs = null
                    firstVideoOutputSeen = false
                    // Audio path can also get stuck after a long pause (AudioTrack/codec state).
                    // Force a soft rebuild so we don't end up in "no audio" with an accumulating queue.
                    pendingAudioReconfigure = true
                    pcmPending = null
                    pcmPendingOffset = 0
                }
                handler.post {
                    if (!surfaceReady) {
                        Log.w(logTag, "[$componentId] play: surface not ready, requesting rebind")
                        requestSurfaceRebind()
                        return@post
                    }
                    ensureVideoDecoder()
                    scheduleVideoDrain(0L)
                }
                audioHandler.post {
                    if (wasPaused && longPause) {
                        releaseAudioTrack()
                    }
                    audioTrack?.play()
                    scheduleAudioDrain()
                }
                // In stream mode, stay in "waiting" until the first decoded video frame is actually
                // rendered (drainVideo -> playNotified).
                eventEmitter("waiting", mapOf("reason" to "buffering"))
                return true
            }
            "pause" -> {
                paused = true
                pendingPlay = false
                lastPauseAtMs = SystemClock.elapsedRealtime()
                handler.post {
                    videoQueue.clear()
                }
                audioHandler.post {
                    audioQueue.clear()
                    pcmPending = null
                    pcmPendingOffset = 0
                    audioTrack?.pause()
                    audioTrack?.flush()
                }
                val obj = try {
                    paramsJson?.let { JSONObject(it) }
                } catch (_: Exception) {
                    null
                }
                val emitEvent = obj?.optBoolean("emitEvent", true) ?: true
                val reason = obj?.optString("reason", "user").takeIf { !it.isNullOrBlank() } ?: "user"
                val currentTime =
                    if (obj != null && obj.has("currentTime") && !obj.isNull("currentTime")) {
                        obj.optDouble("currentTime").takeIf { it.isFinite() && it >= 0.0 }
                    } else {
                        null
                    }

                val detail = mutableMapOf<String, Any?>("reason" to reason)
                if (currentTime != null) {
                    detail["currentTime"] = currentTime
                }
                if (emitEvent) {
                    eventEmitter("pause", detail)
                }
                return true
            }
            "stop" -> {
                stop()
                eventEmitter("stop", mapOf("reason" to "user"))
                return true
            }
            "resetStream" -> {
                val hard = try {
                    paramsJson?.let { JSONObject(it).optBoolean("hard", false) } ?: false
                } catch (_: Exception) {
                    false
                }
                val emitWaiting = try {
                    paramsJson?.let { JSONObject(it).optBoolean("emitWaiting", true) } ?: true
                } catch (_: Exception) {
                    true
                }
                reset(hard, emitWaiting)
                return true
            }
            "rebindSurface" -> {
                requestSurfaceRebind()
                return true
            }
            "setVolume" -> {
                val volume = paramsJson?.let { parseVolume(it) } ?: return false
                streamVolume = volume
                applyStreamVolume()
                eventEmitter("volumechange", mapOf("volume" to streamVolume))
                return true
            }
            "setMuted" -> {
                val muted = paramsJson?.let { parseMuted(it) } ?: return false
                streamMuted = muted
                applyStreamVolume()
                eventEmitter("volumechange", mapOf("muted" to muted, "volume" to streamVolume))
                return true
            }
        }
        return false
    }

    private fun requestSurfaceRebind() {
        if (rebindRequestPosted) return
        val now = SystemClock.uptimeMillis()
        val elapsed = now - lastRebindRequestAtMs
        val delayMs = (REBIND_MIN_INTERVAL_MS - elapsed).coerceAtLeast(0L)
        rebindRequestPosted = true
        mainHandler.postDelayed(rebindRunnable, delayMs)
    }

    fun rebindSurface() {
        requestSurfaceRebind()
    }

    fun stop() {
        paused = true
        pendingPlay = false
        surfaceGeneration.incrementAndGet()
        rebindRequestPosted = false
        mainHandler.removeCallbacks(rebindRunnable)

        handler.post {
            releaseVideoDecoder()
            videoQueue.clear()
        }
        audioHandler.post {
            try {
                audioHandler.removeCallbacks(audioDrainRunnable)
            } catch (_: Exception) {
            }
            audioDrainScheduled = false
            releaseAudioDecoder()
            releaseAudioTrack()
            audioQueue.clear()
        }
    }

    fun release() {
        stop()
        surfaceReady = false
        rebindRequestPosted = false
        mainHandler.removeCallbacks(rebindRunnable)
        handler.post {
            try {
                handler.removeCallbacks(detachReleaseRunnable)
            } catch (_: Exception) {
            }
            videoSurface?.release()
            videoSurface = null
        }
        mainHandler.post { restoreTextureListener() }
        mainHandler.post { textureView.removeOnAttachStateChangeListener(attachStateListener) }
        decodeThread.quitSafely()
        audioThread.quitSafely()
    }

    fun reset(hard: Boolean, emitWaiting: Boolean = true) {
        pendingPlay = false
        playNotified = false
        metadataNotified = false
        needKeyframe = true
        playbackPositionMs = 0
        if (emitWaiting) {
            eventEmitter("waiting", mapOf("reason" to "seeking"))
        }
        pendingVideoReconfigure = hard
        pendingAudioReconfigure = hard
        surfaceGeneration.incrementAndGet()
        requestRectSync("reset")

        handler.post {
            videoQueue.clear()
            videoBasePtsUs = null
            videoBaseNanoTime = null
            videoLastPtsUs = null
            playbackPositionMs = 0
            firstVideoOutputSeen = false
            if (hard) {
                releaseVideoDecoder()
            } else {
                try {
                    videoDecoder?.flush()
                } catch (e: Exception) {
                    Log.w(logTag, "[$componentId] reset: failed to flush video decoder", e)
                }
            }
        }
        audioHandler.post {
            audioQueue.clear()
            pcmPending = null
            pcmPendingOffset = 0
            if (hard) {
                releaseAudioDecoder()
                releaseAudioTrack()
                audioIsPcm = false
            } else {
                try {
                    audioDecoder?.flush()
                    audioTrack?.pause()
                    audioTrack?.flush()
                } catch (e: Exception) {
                    Log.w(logTag, "[$componentId] reset: failed to flush audio", e)
                }
            }
        }
        if (hard) {
            videoConfig = null
            audioConfig = null
            lastVideoConfigJson = null
            lastAudioConfigJson = null
        }
    }

    private fun attachTextureView() {
        mainHandler.post {
            textureView.visibility = View.VISIBLE
            textureView.alpha = 1.0f
            if (textureView.surfaceTextureListener != this) {
                previousListener = textureView.surfaceTextureListener
                textureView.surfaceTextureListener = this
            }
            if (textureView.isAvailable) {
                val st = textureView.surfaceTexture
                if (st != null) {
                    setSurface(st)
                }
            }
        }
    }

    private fun restoreTextureListener() {
        if (textureView.surfaceTextureListener == this) {
            textureView.surfaceTextureListener = previousListener
        }
    }

    private fun setSurface(surfaceTexture: SurfaceTexture?) {
        if (surfaceTexture == null) return
        this.surfaceTexture = surfaceTexture
        surfaceReady = true
        surfaceGeneration.incrementAndGet()
        handler.post {
            try {
                handler.removeCallbacks(detachReleaseRunnable)
            } catch (_: Exception) {
            }

            val oldSurface = videoSurface
            val newSurface = Surface(surfaceTexture)
            videoSurface = newSurface
            updateSurfaceBufferSize()
            val decoder = videoDecoder
            if (decoder != null && Build.VERSION.SDK_INT >= 23) {
                try {
                    decoder.setOutputSurface(newSurface)
                    oldSurface?.release()
                    if (pendingPlay && !paused) {
                        scheduleVideoDrain(0L)
                    }
                    return@post
                } catch (e: Exception) {
                    Log.w(logTag, "[$componentId] setOutputSurface failed: ${e.message}")
                    releaseVideoDecoder()
                }
            } else {
                releaseVideoDecoder()
            }

            oldSurface?.release()
            ensureVideoDecoder()
            if (pendingPlay && !paused) {
                scheduleVideoDrain(0L)
            }
        }
    }

    private fun updateSurfaceBufferSize() {
        val st = surfaceTexture ?: return
        val (w, h) = videoConfig?.let { cfg ->
            val ww = cfg.width?.toInt()?.takeIf { it > 0 }
            val hh = cfg.height?.toInt()?.takeIf { it > 0 }
            if (ww != null && hh != null) ww to hh else null
        } ?: run {
            val ww = textureView.width.takeIf { it > 0 }
            val hh = textureView.height.takeIf { it > 0 }
            if (ww != null && hh != null) ww to hh else return
        }
        try {
            st.setDefaultBufferSize(w, h)
        } catch (e: Exception) {
            Log.w(logTag, "[$componentId] setDefaultBufferSize failed: ${e.message}")
        }
    }

    private fun ensureVideoDecoder() {
        if (videoDecoder != null) {
            if (!pendingVideoReconfigure) return
            releaseVideoDecoder()
        }
        if (!surfaceReady) return
        val config = videoConfig ?: return
        val surface = videoSurface ?: return
        if (!surface.isValid) {
            Log.w(logTag, "[$componentId] video surface not ready")
            return
        }

        try {
            val format = buildVideoFormat(config)
            val decoder = MediaCodec.createDecoderByType(format.getString(MediaFormat.KEY_MIME) ?: return)
            decoder.configure(format, surface, null, 0)
            decoder.start()
            videoDecoder = decoder
            pendingVideoReconfigure = false
            requestRectSync("video_decoder_started")

            scheduleVideoDrain()

            if (!metadataNotified) {
                metadataNotified = true
                val width = config.width ?: 0
                val height = config.height ?: 0
                eventEmitter(
                    "loadedmetadata",
                    mapOf("width" to width, "height" to height, "duration" to 0)
                )
            }
        } catch (e: Exception) {
            emitError("video decoder init failed: ${e.message}")
        }
    }

    private fun ensureAudioDecoder() {
        if (audioDecoder != null) {
            if (!pendingAudioReconfigure) return
            releaseAudioDecoder()
        }
        if (audioIsPcm) {
            // Don't early-return if AudioTrack was torn down (can happen after pause/resume or
            // audio focus changes). Recreate based on the latest config.
            if (!pendingAudioReconfigure && audioTrack != null) return
            releaseAudioTrack()
            audioIsPcm = false
        }
        val config = audioConfig ?: return

        if (config.codec == CODEC_PCM_S16LE) {
            audioIsPcm = true
            ensurePcmAudioTrack(config)
            pendingAudioReconfigure = false
            drainAudio()
            return
        }
        if (config.codec != CODEC_AAC) {
            emitError("unsupported audio codec: ${config.codec}")
            return
        }

        try {
            val sampleRate = config.sampleRate ?: 44100
            val channels = config.channels ?: 2
            val format = MediaFormat.createAudioFormat(MIME_AAC, sampleRate, channels)
            format.setByteBuffer("csd-0", ByteBuffer.wrap(config.audioSpecificConfig))
            if (config.aacIsAdts) {
                format.setInteger(MediaFormat.KEY_IS_ADTS, 1)
            }
            val decoder = MediaCodec.createDecoderByType(MIME_AAC)
            decoder.configure(format, null, null, 0)
            decoder.start()
            audioDecoder = decoder
            pendingAudioReconfigure = false

            drainAudio()
        } catch (e: Exception) {
            emitError("audio decoder init failed: ${e.message}")
        }
    }

    private fun buildVideoFormat(config: VideoConfig): MediaFormat {
        val mime = if (config.codec == "h265") MIME_HEVC else MIME_AVC
        val width = config.width?.coerceAtLeast(1) ?: 1
        val height = config.height?.coerceAtLeast(1) ?: 1
        val format = MediaFormat.createVideoFormat(mime, width, height)

        if (mime == MIME_HEVC) {
            val csd0 = buildHevcCsd(config)
            if (csd0.isNotEmpty()) {
                format.setByteBuffer("csd-0", ByteBuffer.wrap(csd0))
            }
        } else {
            val sps = prefixCsd(config.sps)
            val pps = prefixCsd(config.pps)
            if (sps.isNotEmpty()) {
                format.setByteBuffer("csd-0", ByteBuffer.wrap(sps))
            }
            if (pps.isNotEmpty()) {
                format.setByteBuffer("csd-1", ByteBuffer.wrap(pps))
            }
        }

        return format
    }

    private fun buildHevcCsd(config: VideoConfig): ByteArray {
        val chunks = ArrayList<ByteArray>()
        if (config.vps.isNotEmpty()) chunks.add(prefixCsd(config.vps))
        if (config.sps.isNotEmpty()) chunks.add(prefixCsd(config.sps))
        if (config.pps.isNotEmpty()) chunks.add(prefixCsd(config.pps))
        if (chunks.isEmpty()) return ByteArray(0)
        val total = chunks.sumOf { it.size }
        val combined = ByteArray(total)
        var offset = 0
        for (chunk in chunks) {
            System.arraycopy(chunk, 0, combined, offset, chunk.size)
            offset += chunk.size
        }
        return combined
    }

    private fun prefixCsd(data: ByteArray): ByteArray {
        if (data.isEmpty()) return data
        val prefix = byteArrayOf(0, 0, 0, 1)
        return prefix + data
    }

    private fun dropVideoBacklogAndOffer(incoming: VideoFrame) {
        var dropped = 0
        while (videoQueue.remainingCapacity() == 0) {
            val removed = videoQueue.poll() ?: break
            dropped++
            while (videoQueue.isNotEmpty() && videoQueue.peek()?.keyframe == false) {
                videoQueue.poll()
                dropped++
            }
            if (removed.keyframe) break
        }

        if (!videoQueue.offer(incoming)) {
            Log.w(
                logTag,
                "[$componentId] video queue full, dropping frame ptsMs=${incoming.ptsMs} queue=${videoQueue.size}"
            )
        } else if (dropped > 0) {
            Log.w(logTag, "[$componentId] video backlog dropped: $dropped, remaining=${videoQueue.size}")
            if (pendingPlay && !paused) {
                // If we're actively trying to play but the video pipeline can't keep up (often due
                // to a stale/invalid surface after transitions), force a surface rebind and decoder
                // recreate so video can recover instead of staying "audio-only".
                Log.w(logTag, "[$componentId] video backlog while playing; requesting surface rebind + decoder recreate")
                needKeyframe = true
                playNotified = false
                pendingVideoReconfigure = true
                requestSurfaceRebind()
            }
        }
    }

    private fun drainVideo() {
        videoDrainScheduled = false
        if (paused || !surfaceReady) return
        val decoder = videoDecoder ?: return
        val drainGeneration = surfaceGeneration.get()
        val surface = videoSurface
        if (surface == null || !surface.isValid) {
            Log.w(logTag, "[$componentId] video surface not ready, pausing drain")
            surfaceReady = false
            releaseVideoDecoder()
            requestSurfaceRebind()
            return
        }

        try {
            while (!videoQueue.isEmpty()) {
                if (paused || !surfaceReady || surfaceGeneration.get() != drainGeneration) return
                val frame = videoQueue.peek() ?: break
                val ptsUs = frame.ptsMs.toLong() * 1000L
                val nowNs = System.nanoTime()
                val basePts = videoBasePtsUs
                val baseNs = videoBaseNanoTime
                if (basePts == null || baseNs == null) {
                    videoBasePtsUs = ptsUs
                    videoBaseNanoTime = nowNs + VIDEO_START_BUFFER_NS
                }
                val effectiveBasePts = videoBasePtsUs ?: ptsUs
                val effectiveBaseNs = videoBaseNanoTime ?: nowNs
                val targetNs = effectiveBaseNs + (ptsUs - effectiveBasePts) * 1000L
                val leadNs = targetNs - nowNs
                if (leadNs > VIDEO_INPUT_MAX_LEAD_NS) {
                    scheduleVideoDrain((leadNs / 1_000_000L).coerceIn(5L, 50L))
                    return
                }
                if (nowNs - targetNs > VIDEO_INPUT_MAX_LAG_NS) {
                    videoBasePtsUs = ptsUs
                    videoBaseNanoTime = nowNs + VIDEO_START_BUFFER_NS
                    Log.w(logTag, "[$componentId] video lag reset, rebuffering")
                }

                val inputIndex = decoder.dequeueInputBuffer(0)
                if (inputIndex < 0) break
                val buffer = decoder.getInputBuffer(inputIndex) ?: break
                val queued = videoQueue.poll() ?: break
                buffer.clear()
                if (queued.data.size > buffer.remaining()) {
                    Log.w(logTag, "[$componentId] video frame too large: ${queued.data.size}")
                    continue
                }
                val size = queued.data.size
                buffer.put(queued.data, 0, size)
                decoder.queueInputBuffer(inputIndex, 0, size, ptsUs, 0)
            }

            val info = videoBufferInfo
            var outputIndex = decoder.dequeueOutputBuffer(info, 10_000)
            var renderedAny = false
            var nextDelayMs: Long? = null
            while (outputIndex >= 0) {
                if (paused || !surfaceReady || surfaceGeneration.get() != drainGeneration) return
                val ptsUs = info.presentationTimeUs
                val nowNs = System.nanoTime()
                val basePts = videoBasePtsUs
                val baseNs = videoBaseNanoTime
                val lastPts = videoLastPtsUs
                val isInitialBase = basePts == null || baseNs == null || lastPts == null
                val isDiscontinuity = !isInitialBase && (
                    ptsUs < basePts || kotlin.math.abs(ptsUs - lastPts) > VIDEO_PTS_RESET_THRESHOLD_US
                )
                if (isInitialBase || isDiscontinuity) {
                    videoBasePtsUs = ptsUs
                    videoBaseNanoTime = nowNs + VIDEO_START_BUFFER_NS
                    if (isDiscontinuity) {
                        Log.w(logTag, "[$componentId] video pts reset, rebuffering")
                    }
                }
                videoLastPtsUs = ptsUs
                val effectiveBasePts = videoBasePtsUs ?: ptsUs
                val effectiveBaseNs = videoBaseNanoTime ?: nowNs
                // Report relative position (media time) to avoid wall-clock drift and double-offsetting.
                playbackPositionMs = (ptsUs - effectiveBasePts).coerceAtLeast(0L) / 1000L

                if (Build.VERSION.SDK_INT >= 21) {
                    val targetNs = effectiveBaseNs + (ptsUs - effectiveBasePts) * 1000L
                    val leadNs = targetNs - nowNs
                    if (leadNs > VIDEO_OUTPUT_MAX_LEAD_NS) {
                        if (!surface.isValid) {
                            Log.w(logTag, "[$componentId] video surface invalid, pausing drain")
                            surfaceReady = false
                            releaseVideoDecoder()
                            requestSurfaceRebind()
                            return
                        }
                        try {
                            decoder.releaseOutputBuffer(outputIndex, targetNs)
                        } catch (e: Throwable) {
                            LxLog.e(logTag, "[$componentId] video releaseOutputBuffer error: ${e.message}", e)
                            releaseVideoDecoder()
                            return
                        }
                        renderedAny = true
                        val delayMs = (leadNs / 1_000_000L).coerceIn(5L, 50L)
                        nextDelayMs = nextDelayMs?.let { minOf(it, delayMs) } ?: delayMs
                        break
                    }
                    try {
                        if (!surface.isValid) {
                            Log.w(logTag, "[$componentId] video surface invalid, pausing drain")
                            surfaceReady = false
                            releaseVideoDecoder()
                            requestSurfaceRebind()
                            return
                        }
                        decoder.releaseOutputBuffer(outputIndex, maxOf(nowNs, targetNs))
                    } catch (e: Throwable) {
                        LxLog.e(logTag, "[$componentId] video releaseOutputBuffer error: ${e.message}", e)
                        releaseVideoDecoder()
                        return
                    }
                } else {
                    try {
                        if (!surface.isValid) {
                            Log.w(logTag, "[$componentId] video surface invalid, pausing drain")
                            surfaceReady = false
                            releaseVideoDecoder()
                            requestSurfaceRebind()
                            return
                        }
                        decoder.releaseOutputBuffer(outputIndex, true)
                    } catch (e: Throwable) {
                        LxLog.e(logTag, "[$componentId] video releaseOutputBuffer error: ${e.message}", e)
                        releaseVideoDecoder()
                        return
                    }
                }
                renderedAny = true
                if (!firstVideoOutputSeen) {
                    firstVideoOutputSeen = true
                    requestRectSync("first_video_output")
                }
                if (!playNotified) {
                    playNotified = true
                    eventEmitter("playing", emptyMap())
                }
                outputIndex = decoder.dequeueOutputBuffer(info, 0)
            }

            if (!paused && surfaceReady && surfaceGeneration.get() == drainGeneration &&
                (renderedAny || videoQueue.isNotEmpty())) {
                scheduleVideoDrain(nextDelayMs ?: 5L)
            }
        } catch (e: Throwable) {
            LxLog.e(logTag, "[$componentId] video decode error: ${e.message}", e)
            releaseVideoDecoder()
        }
    }

    private fun scheduleVideoDrain(delayMs: Long = 5L) {
        if (videoDrainScheduled) return
        videoDrainScheduled = true
        handler.postDelayed(videoDrainRunnable, delayMs)
    }

    private fun drainAudio() {
        // Always clear scheduled flag first. If we return early (e.g. paused),
        // future callers must be able to schedule a new drain tick.
        audioDrainScheduled = false
        if (paused) return

        if (audioIsPcm) {
            val config = audioConfig ?: return
            ensurePcmAudioTrack(config)
            val track = audioTrack ?: return

            val maxBytesPerTick = 32768
            var remainingBudget = maxBytesPerTick

            fun writeSome(buf: ByteArray, off: Int, len: Int): Int {
                return if (Build.VERSION.SDK_INT >= 23) {
                    track.write(buf, off, len, AudioTrack.WRITE_NON_BLOCKING)
                } else {
                    track.write(buf, off, len)
                }
            }

            while (remainingBudget > 0) {
                val pending = pcmPending
                if (pending != null) {
                    val left = pending.size - pcmPendingOffset
                    if (left <= 0) {
                        pcmPending = null
                        pcmPendingOffset = 0
                        continue
                    }
                    val toWrite = minOf(left, remainingBudget)
                    val written = writeSome(pending, pcmPendingOffset, toWrite)
                    if (written > 0) {
                        pcmPendingOffset += written
                        remainingBudget -= written
                        continue
                    }
                    break
                }

                if (audioQueue.isEmpty()) break
                val frame = audioQueue.poll() ?: break
                if (frame.data.isEmpty()) continue
                pcmPending = frame.data
                pcmPendingOffset = 0
            }

            if (pcmPending != null || audioQueue.isNotEmpty()) {
                scheduleAudioDrain()
            }
            return
        }

        val decoder = audioDecoder ?: return
        try {
            while (!audioQueue.isEmpty()) {
                val inputIndex = decoder.dequeueInputBuffer(0)
                if (inputIndex < 0) break
                val buffer = decoder.getInputBuffer(inputIndex) ?: break
                val frame = audioQueue.poll() ?: break
                buffer.clear()
                if (frame.data.size > buffer.remaining()) {
                    Log.w(logTag, "[$componentId] audio frame too large: ${frame.data.size}")
                    continue
                }
                val size = frame.data.size
                buffer.put(frame.data, 0, size)
                val ptsUs = frame.ptsMs.toLong() * 1000L
                decoder.queueInputBuffer(inputIndex, 0, size, ptsUs, 0)
            }

            val info = audioBufferInfo
            var outputIndex = decoder.dequeueOutputBuffer(info, 0)
            while (outputIndex >= 0) {
                if (info.size > 0) {
                    val outBuffer = decoder.getOutputBuffer(outputIndex)
                    if (outBuffer != null) {
                        ensureAudioTrack(decoder.outputFormat)
                        outBuffer.position(info.offset)
                        outBuffer.limit(info.offset + info.size)
                        val track = audioTrack
                        if (track != null) {
                            if (!paused && track.playState != AudioTrack.PLAYSTATE_PLAYING) {
                                try {
                                    track.play()
                                } catch (_: Throwable) {
                                }
                            }
                            if (Build.VERSION.SDK_INT >= 23) {
                                // Avoid blocking the audio thread; blocking writes can hang after pause/resume
                                // on some devices, leading to an ever-growing input queue and "no audio".
                                track.write(outBuffer, info.size, AudioTrack.WRITE_NON_BLOCKING)
                            } else {
                                if (audioScratch.size < info.size) {
                                    audioScratch = ByteArray(info.size)
                                }
                                outBuffer.get(audioScratch, 0, info.size)
                                track.write(audioScratch, 0, info.size)
                            }
                        }
                    }
                }
                decoder.releaseOutputBuffer(outputIndex, false)
                outputIndex = decoder.dequeueOutputBuffer(info, 0)
            }

            if (outputIndex == MediaCodec.INFO_OUTPUT_FORMAT_CHANGED) {
                ensureAudioTrack(decoder.outputFormat)
            }
        } catch (e: Exception) {
            LxLog.e(logTag, "[$componentId] audio decode error: ${e.message}", e)
            releaseAudioDecoder()
        }
    }

    private fun scheduleAudioDrain() {
        if (audioDrainScheduled) return
        audioDrainScheduled = true
        audioHandler.postDelayed(audioDrainRunnable, 5L)
    }

    private fun ensurePcmAudioTrack(config: AudioConfig) {
        if (audioTrack != null) return
        val sampleRate = config.sampleRate ?: 44100
        val channels = config.channels ?: 1
        val channelMask = if (channels == 1) {
            AudioFormat.CHANNEL_OUT_MONO
        } else {
            AudioFormat.CHANNEL_OUT_STEREO
        }
        val encoding = AudioFormat.ENCODING_PCM_16BIT
        val minBuffer = AudioTrack.getMinBufferSize(sampleRate, channelMask, encoding)
        if (minBuffer <= 0) {
            emitError("pcm AudioTrack minBuffer invalid: $minBuffer (sr=$sampleRate ch=$channels)")
            return
        }
        val bufferSize = minBuffer
        try {
            audioTrack = AudioTrack.Builder()
                .setAudioAttributes(
                    AudioAttributes.Builder()
                        .setUsage(AudioAttributes.USAGE_MEDIA)
                        .setContentType(AudioAttributes.CONTENT_TYPE_MOVIE)
                        .build()
                )
                .setAudioFormat(
                    AudioFormat.Builder()
                        .setSampleRate(sampleRate)
                        .setChannelMask(channelMask)
                        .setEncoding(encoding)
                        .build()
                )
                .setBufferSizeInBytes(bufferSize)
                .setTransferMode(AudioTrack.MODE_STREAM)
                .also {
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                        it.setPerformanceMode(AudioTrack.PERFORMANCE_MODE_LOW_LATENCY)
                    }
                }
                .build()
            applyStreamVolume()
            if (!paused) {
                audioTrack?.play()
            }
        } catch (t: Throwable) {
            emitError("pcm AudioTrack create failed: ${t.message}")
            releaseAudioTrack()
            pendingAudioReconfigure = true
        }
    }

    private fun ensureAudioTrack(format: MediaFormat) {
        if (audioTrack != null) return
        val sampleRate = format.getInteger(MediaFormat.KEY_SAMPLE_RATE)
        val channels = format.getInteger(MediaFormat.KEY_CHANNEL_COUNT)
        val channelMask = if (channels == 1) {
            AudioFormat.CHANNEL_OUT_MONO
        } else {
            AudioFormat.CHANNEL_OUT_STEREO
        }
        val encoding = if (format.containsKey(MediaFormat.KEY_PCM_ENCODING)) {
            format.getInteger(MediaFormat.KEY_PCM_ENCODING)
        } else {
            AudioFormat.ENCODING_PCM_16BIT
        }
        val minBuffer = AudioTrack.getMinBufferSize(sampleRate, channelMask, encoding)
        if (minBuffer <= 0) {
            emitError("aac AudioTrack minBuffer invalid: $minBuffer (sr=$sampleRate ch=$channels enc=$encoding)")
            return
        }
        val bufferSize = (minBuffer * 2).coerceAtLeast(minBuffer)
        try {
            audioTrack = AudioTrack.Builder()
                .setAudioAttributes(
                    AudioAttributes.Builder()
                        .setUsage(AudioAttributes.USAGE_MEDIA)
                        .setContentType(AudioAttributes.CONTENT_TYPE_MOVIE)
                        .build()
                )
                .setAudioFormat(
                    AudioFormat.Builder()
                        .setSampleRate(sampleRate)
                        .setChannelMask(channelMask)
                        .setEncoding(encoding)
                        .build()
                )
                .setBufferSizeInBytes(bufferSize)
                .setTransferMode(AudioTrack.MODE_STREAM)
                .build()
            applyStreamVolume()
            if (!paused) {
                audioTrack?.play()
            }
        } catch (t: Throwable) {
            emitError("aac AudioTrack create failed: ${t.message}")
            releaseAudioTrack()
            pendingAudioReconfigure = true
        }
    }

    private fun releaseVideoDecoder() {
        try {
            handler.removeCallbacks(videoDrainRunnable)
        } catch (_: Exception) {
        }
        try {
            videoDecoder?.stop()
        } catch (_: Exception) {
        }
        try {
            videoDecoder?.release()
        } catch (_: Exception) {
        }
        videoDecoder = null
        videoQueue.clear()
        needKeyframe = true
        firstVideoOutputSeen = false
        videoDrainScheduled = false
        videoBasePtsUs = null
        videoBaseNanoTime = null
        videoLastPtsUs = null
    }

    private fun releaseAudioDecoder() {
        try {
            audioHandler.removeCallbacks(audioDrainRunnable)
        } catch (_: Exception) {
        }
        audioDrainScheduled = false
        try {
            audioDecoder?.stop()
        } catch (_: Exception) {
        }
        try {
            audioDecoder?.release()
        } catch (_: Exception) {
        }
        audioDecoder = null
        audioIsPcm = false
    }

    private fun releaseAudioTrack() {
        try {
            audioTrack?.stop()
        } catch (_: Exception) {
        }
        try {
            audioTrack?.release()
        } catch (_: Exception) {
        }
        audioTrack = null
    }

    private fun applyStreamVolume() {
        val effective = if (streamMuted) 0.0f else streamVolume
        audioHandler.post {
            if (Build.VERSION.SDK_INT >= 21) {
                audioTrack?.setVolume(effective)
            } else {
                @Suppress("DEPRECATION")
                audioTrack?.setStereoVolume(effective, effective)
            }
        }
    }

    private fun parseVolume(paramsJson: String): Float? {
        return try {
            val obj = JSONObject(paramsJson)
            val value = obj.optDouble("volume", Double.NaN)
            if (value.isNaN()) null else value.toFloat().coerceIn(0f, 1f)
        } catch (_: Exception) {
            null
        }
    }

    private fun parseMuted(paramsJson: String): Boolean? {
        return try {
            val obj = JSONObject(paramsJson)
            if (obj.has("muted") && !obj.isNull("muted")) obj.optBoolean("muted") else null
        } catch (_: Exception) {
            null
        }
    }

    private fun emitError(message: String) {
        Log.w(logTag, "[$componentId] $message")
        eventEmitter("error", mapOf("code" to "stream_decoder", "message" to message))
    }

    override fun onSurfaceTextureAvailable(surface: SurfaceTexture, width: Int, height: Int) {
        previousListener?.onSurfaceTextureAvailable(surface, width, height)
        setSurface(surface)
    }

    override fun onSurfaceTextureSizeChanged(surface: SurfaceTexture, width: Int, height: Int) {
        previousListener?.onSurfaceTextureSizeChanged(surface, width, height)
    }

    override fun onSurfaceTextureDestroyed(surface: SurfaceTexture): Boolean {
        previousListener?.onSurfaceTextureDestroyed(surface)
        surfaceReady = false
        surfaceGeneration.incrementAndGet()
        handler.post {
            releaseVideoDecoder()
            videoQueue.clear()
            videoSurface?.release()
            videoSurface = null
            surfaceTexture = null
        }
        return true
    }

    override fun onSurfaceTextureUpdated(surface: SurfaceTexture) {
        previousListener?.onSurfaceTextureUpdated(surface)
        if (!paused && surfaceReady && !playNotified) {
            playNotified = true
            eventEmitter("playing", emptyMap())
        }
    }

    private data class VideoConfig(
        val codec: String,
        val format: String,
        val sps: ByteArray,
        val pps: ByteArray,
        val vps: ByteArray,
        val nalLengthSize: Int?,
        val width: Int?,
        val height: Int?
    ) {
        companion object {
            fun fromJson(json: String): VideoConfig {
                val obj = JSONObject(json)
                return VideoConfig(
                    codec = obj.optString("codec", "h264"),
                    format = obj.optString("format", "annexb"),
                    sps = obj.optByteArray("sps"),
                    pps = obj.optByteArray("pps"),
                    vps = obj.optByteArray("vps"),
                    nalLengthSize = obj.optIntOrNull("nalLengthSize"),
                    width = obj.optIntOrNull("width"),
                    height = obj.optIntOrNull("height")
                )
            }
        }
    }

    private data class AudioConfig(
        val codec: String,
        val audioSpecificConfig: ByteArray,
        val sampleRate: Int?,
        val channels: Int?,
        val aacIsAdts: Boolean
    ) {
        companion object {
            fun fromJson(json: String): AudioConfig {
                val obj = JSONObject(json)
                return AudioConfig(
                    codec = obj.optString("codec", "aac"),
                    audioSpecificConfig = obj.optByteArray("audioSpecificConfig"),
                    sampleRate = obj.optIntOrNull("sampleRate"),
                    channels = obj.optIntOrNull("channels"),
                    aacIsAdts = obj.optBoolean("aacIsAdts", false)
                )
            }
        }
    }

    private data class VideoFrame(
        val data: ByteArray,
        val dtsMs: Int,
        val ptsMs: Int,
        val keyframe: Boolean
    )

    private data class AudioFrame(
        val data: ByteArray,
        val dtsMs: Int,
        val ptsMs: Int
    )

    private data class QueueProfile(
        val maxVideoQueue: Int,
        val maxAudioQueue: Int,
    )

    private fun buildQueueProfile(context: Context): QueueProfile {
        val am = context.getSystemService(Context.ACTIVITY_SERVICE) as? ActivityManager
        val isLowRam = am?.isLowRamDevice == true
        val memoryClass = am?.memoryClass ?: 256
        val lowTier = isLowRam || memoryClass <= 256
        return if (lowTier) {
            QueueProfile(
                maxVideoQueue = MAX_VIDEO_QUEUE_LOW_RAM,
                maxAudioQueue = MAX_AUDIO_QUEUE_LOW_RAM,
            )
        } else {
            QueueProfile(
                maxVideoQueue = MAX_VIDEO_QUEUE_NORMAL,
                maxAudioQueue = MAX_AUDIO_QUEUE_NORMAL,
            )
        }
    }

    private companion object {
        private const val RESUME_RECONFIGURE_THRESHOLD_MS = 10_000L
        private const val REBIND_MIN_INTERVAL_MS = 120L
        private const val VIDEO_PTS_RESET_THRESHOLD_US = 5_000_000L
        private const val VIDEO_START_BUFFER_NS = 500_000_000L
        private const val VIDEO_OUTPUT_MAX_LEAD_NS = 300_000_000L
        private const val VIDEO_INPUT_MAX_LEAD_NS = 2_000_000_000L
        private const val VIDEO_INPUT_MAX_LAG_NS = 2_000_000_000L
        private const val MAX_VIDEO_QUEUE_LOW_RAM = 48
        private const val MAX_VIDEO_QUEUE_NORMAL = 96
        private const val MAX_AUDIO_QUEUE_LOW_RAM = 72
        private const val MAX_AUDIO_QUEUE_NORMAL = 120
        private const val MIME_AVC = "video/avc"
        private const val MIME_HEVC = "video/hevc"
        private const val MIME_AAC = "audio/mp4a-latm"
        private const val CODEC_AAC = "aac"
        private const val CODEC_PCM_S16LE = "pcm_s16le"
    }
}

private fun JSONObject.optByteArray(key: String): ByteArray {
    val value = opt(key)
    return when (value) {
        is JSONArray -> value.toByteArray()
        is String -> {
            if (value.isEmpty()) {
                ByteArray(0)
            } else {
                try {
                    Base64.decode(value, Base64.DEFAULT)
                } catch (_: Exception) {
                    ByteArray(0)
                }
            }
        }
        else -> ByteArray(0)
    }
}

private fun JSONArray.toByteArray(): ByteArray {
    val data = ByteArray(length())
    for (i in 0 until length()) {
        data[i] = (optInt(i) and 0xFF).toByte()
    }
    return data
}

private fun JSONObject.optIntOrNull(key: String): Int? {
    return if (has(key) && !isNull(key)) optInt(key) else null
}
