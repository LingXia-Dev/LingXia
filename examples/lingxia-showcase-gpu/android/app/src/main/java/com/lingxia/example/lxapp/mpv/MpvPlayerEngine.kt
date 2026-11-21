package com.lingxia.example.lxapp.mpv

import android.content.Context
import android.graphics.SurfaceTexture
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.Surface
import android.view.SurfaceHolder
import android.view.TextureView
import com.lingxia.app.media.UrlPlayerEngine
import com.lingxia.app.media.UrlPlayerEngineEvent
import com.lingxia.app.media.UrlPlayerEngineListener
import com.lingxia.app.media.UrlPlayerError
import com.lingxia.app.media.UrlPlayerErrorCode
import com.lingxia.app.media.UrlPlayerOutput
import com.lingxia.app.media.UrlPlayerSource
import com.lingxia.app.media.UrlPlayerVideoSize
import com.lingxia.app.media.UrlPlayerWaitingReason
import dev.jdtech.mpv.MPVLib

/**
 * Host URL engine backed by libmpv. Construction is cheap; the mpv context is
 * created on first [setSource] / [attachOutput], not in the factory.
 */
class MpvPlayerEngine(
    private val context: Context,
) : UrlPlayerEngine, MPVLib.EventObserver {
    private val mainHandler = Handler(Looper.getMainLooper())
    private var listener: UrlPlayerEngineListener? = null
    private var mpv: MPVLib? = null
    private var released = false

    private var pendingUrl: String? = null
    private var attachedOutput: UrlPlayerOutput? = null
    private var boundSurface: Surface? = null
    private var ownsBoundSurface = false
    private var textureListener: TextureView.SurfaceTextureListener? = null
    private var holderCallback: SurfaceHolder.Callback? = null

    private var volume = 1f
    private var muted = false
    private var rate = 1f
    private var loopEnabled = false
    private var playWhenReady = false
    private var stopping = false
    private var fileLoaded = false
    private var firstFrameEmitted = false
    private var seeking = false
    private var pausedForCache = false
    private var pauseFlag = true
    private var eofReached = false
    private var durationMs: Long? = null
    private var videoSize: UrlPlayerVideoSize? = null
    private var timePoll: Runnable? = null

    override fun setListener(listener: UrlPlayerEngineListener?) {
        this.listener = listener
    }

    override fun setSource(source: UrlPlayerSource) {
        if (released) return
        pendingUrl = source.url
        fileLoaded = false
        firstFrameEmitted = false
        eofReached = false
        durationMs = null
        videoSize = null
        seeking = false
        playWhenReady = false
        pauseFlag = true
        if (!ensureCreated()) return
        loadPending()
    }

    override fun attachOutput(output: UrlPlayerOutput) {
        if (released) return
        if (attachedOutput === output && boundSurface != null) return
        detachOutput()
        attachedOutput = output
        if (!ensureCreated()) return
        when (output) {
            is UrlPlayerOutput.Texture -> bindTextureView(output.textureView)
            is UrlPlayerOutput.Surface -> bindSurfaceView(output.surfaceView)
        }
        loadPending()
        applyPlayState()
    }

    override fun detachOutput() {
        unregisterSurfaceCallbacks()
        unbindSurface()
        attachedOutput = null
    }

    override fun play() {
        if (released) return
        playWhenReady = true
        eofReached = false
        if (!ensureCreated()) return
        loadPending()
        applyPlayState()
    }

    override fun pause() {
        playWhenReady = false
        mpv?.setPropertyBoolean("pause", true)
        pauseFlag = true
        stopPolling()
        emit(UrlPlayerEngineEvent.PlayingChanged(false))
    }

    override fun stop() {
        if (released) return
        playWhenReady = false
        stopping = true
        pendingUrl = null
        fileLoaded = false
        firstFrameEmitted = false
        eofReached = false
        stopPolling()
        mpv?.command(arrayOf("stop"))
        pauseFlag = true
        stopping = false
        emit(UrlPlayerEngineEvent.PlayingChanged(false))
    }

    override fun seek(positionMs: Long) {
        if (released) return
        seeking = true
        emit(
            UrlPlayerEngineEvent.BufferingChanged(
                isBuffering = true,
                reason = UrlPlayerWaitingReason.SEEKING,
            )
        )
        val seconds = (positionMs.coerceAtLeast(0L) / 1000.0).toString()
        mpv?.command(arrayOf("seek", seconds, "absolute"))
    }

    override fun setVolume(volume: Float) {
        this.volume = volume.coerceIn(0f, 1f)
        applyAudio()
    }

    override fun setMuted(muted: Boolean) {
        this.muted = muted
        applyAudio()
    }

    override fun setRate(rate: Float) {
        this.rate = rate.coerceAtLeast(0.25f)
        mpv?.setPropertyDouble("speed", this.rate.toDouble())
    }

    override fun setLoopEnabled(loopEnabled: Boolean) {
        this.loopEnabled = loopEnabled
        mpv?.setPropertyString("loop-file", if (loopEnabled) "inf" else "no")
    }

    override fun getCurrentTimeMs(): Long {
        val pos = mpv?.getPropertyDouble("time-pos") ?: return 0L
        if (pos.isNaN() || pos < 0) return 0L
        return (pos * 1000.0).toLong()
    }

    override fun getDurationMs(): Long? = durationMs

    override fun isPlaying(): Boolean = playWhenReady && !pauseFlag && !pausedForCache && !eofReached

    override fun release() {
        if (released) return
        released = true
        listener = null
        stopPolling()
        detachOutput()
        val instance = mpv
        mpv = null
        if (instance != null) {
            instance.removeObserver(this)
            instance.destroy()
        }
    }

    override fun eventProperty(property: String) {}

    override fun eventProperty(property: String, value: Long) {
        onMain {
            when (property) {
                "dwidth" -> updateVideoSize(width = value.toInt())
                "dheight" -> updateVideoSize(height = value.toInt())
            }
        }
    }

    override fun eventProperty(property: String, value: Double) {
        onMain {
            when (property) {
                "duration" -> {
                    if (value.isFinite() && value > 0) {
                        durationMs = (value * 1000.0).toLong()
                    }
                }
                "video-params/rotate" -> updateVideoSize(rotation = value.toInt())
            }
        }
    }

    override fun eventProperty(property: String, value: Boolean) {
        onMain {
            when (property) {
                "pause" -> {
                    pauseFlag = value
                    emit(UrlPlayerEngineEvent.PlayingChanged(isPlaying()))
                    if (isPlaying()) startPolling() else stopPolling()
                }
                "paused-for-cache" -> {
                    pausedForCache = value
                    emit(
                        UrlPlayerEngineEvent.BufferingChanged(
                            isBuffering = value,
                            reason = UrlPlayerWaitingReason.BUFFERING,
                        )
                    )
                    emit(UrlPlayerEngineEvent.PlayingChanged(isPlaying()))
                    if (isPlaying()) startPolling() else stopPolling()
                }
                "eof-reached" -> {
                    eofReached = value
                    if (value && !loopEnabled) {
                        playWhenReady = false
                        pauseFlag = true
                        stopPolling()
                        emit(UrlPlayerEngineEvent.Ended)
                    }
                }
                "seeking" -> {
                    if (seeking && !value) {
                        seeking = false
                        emit(UrlPlayerEngineEvent.SeekCompleted(getCurrentTimeMs()))
                        emit(
                            UrlPlayerEngineEvent.BufferingChanged(
                                isBuffering = false,
                                reason = UrlPlayerWaitingReason.SEEKING,
                            )
                        )
                    }
                    seeking = value
                }
            }
        }
    }

    override fun eventProperty(property: String, value: String) {}

    override fun event(eventId: Int) {
        onMain {
            when (eventId) {
                MPVLib.MpvEvent.MPV_EVENT_FILE_LOADED -> {
                    fileLoaded = true
                    eofReached = false
                    refreshMetadata()
                    emit(
                        UrlPlayerEngineEvent.Prepared(
                            durationMs = durationMs,
                            videoSize = videoSize,
                        )
                    )
                    emit(
                        UrlPlayerEngineEvent.BufferingChanged(
                            isBuffering = false,
                            reason = UrlPlayerWaitingReason.INITIAL,
                        )
                    )
                }
                MPVLib.MpvEvent.MPV_EVENT_VIDEO_RECONFIG -> {
                    refreshMetadata()
                    if (fileLoaded) {
                        emit(
                            UrlPlayerEngineEvent.Prepared(
                                durationMs = durationMs,
                                videoSize = videoSize,
                            )
                        )
                    }
                }
                MPVLib.MpvEvent.MPV_EVENT_PLAYBACK_RESTART -> {
                    if (fileLoaded && boundSurface != null && !firstFrameEmitted) {
                        firstFrameEmitted = true
                        emit(UrlPlayerEngineEvent.FirstFrameRendered)
                    }
                    if (seeking) {
                        seeking = false
                        emit(UrlPlayerEngineEvent.SeekCompleted(getCurrentTimeMs()))
                        emit(
                            UrlPlayerEngineEvent.BufferingChanged(
                                isBuffering = false,
                                reason = UrlPlayerWaitingReason.SEEKING,
                            )
                        )
                    }
                }
                MPVLib.MpvEvent.MPV_EVENT_END_FILE -> {
                    if (stopping || loopEnabled) return@onMain
                    if (eofReached) return@onMain
                    if (fileLoaded) {
                        eofReached = true
                        playWhenReady = false
                        pauseFlag = true
                        stopPolling()
                        emit(UrlPlayerEngineEvent.Ended)
                    }
                }
            }
        }
    }

    private fun ensureCreated(): Boolean {
        if (released) return false
        if (mpv != null) return true
        if (!MpvNative.available) {
            emitError(UrlPlayerErrorCode.INTERNAL, "libmpv is not loaded")
            return false
        }
        val created = try {
            MPVLib.create(context.applicationContext)
        } catch (t: Throwable) {
            Log.e(TAG, "MPVLib.create failed", t)
            emitError(UrlPlayerErrorCode.INTERNAL, t.message ?: "mpv create failed")
            return false
        }
        if (created == null) {
            emitError(UrlPlayerErrorCode.INTERNAL, "mpv create returned null")
            return false
        }
        applyOptions(created)
        created.init()
        created.addObserver(this)
        observe(created)
        created.setPropertyBoolean("pause", true)
        applyAudio(created)
        created.setPropertyDouble("speed", rate.toDouble())
        created.setPropertyString("loop-file", if (loopEnabled) "inf" else "no")
        mpv = created
        Log.i(TAG, "mpv context ready")
        return true
    }

    private fun applyOptions(mpv: MPVLib) {
        val cacheDir = context.cacheDir.resolve("mpv").apply { mkdirs() }
        mpv.setOptionString("profile", "fast")
        mpv.setOptionString("vo", "gpu")
        mpv.setOptionString("gpu-context", "android")
        mpv.setOptionString("opengl-es", "yes")
        mpv.setOptionString("hwdec", "mediacodec,mediacodec-copy")
        mpv.setOptionString("hwdec-codecs", "h264,hevc,mpeg4,mpeg2video,vp8,vp9,av1")
        mpv.setOptionString("ao", "audiotrack,opensles")
        mpv.setOptionString("idle", "yes")
        mpv.setOptionString("force-window", "no")
        mpv.setOptionString("keep-open", "yes")
        mpv.setOptionString("keep-open-pause", "yes")
        mpv.setOptionString("osd-level", "0")
        mpv.setOptionString("pause", "yes")
        mpv.setOptionString("gpu-shader-cache-dir", cacheDir.absolutePath)
        mpv.setOptionString("icc-cache-dir", cacheDir.absolutePath)
        mpv.setOptionString("demuxer-max-bytes", "${64 * 1024 * 1024}")
        mpv.setOptionString("demuxer-max-back-bytes", "${64 * 1024 * 1024}")
        // Showcase HTTPS samples; hosts that care about TLS should ship a CA bundle.
        mpv.setOptionString("tls-verify", "no")
    }

    private fun observe(mpv: MPVLib) {
        mpv.observeProperty("pause", MPVLib.MpvFormat.MPV_FORMAT_FLAG)
        mpv.observeProperty("paused-for-cache", MPVLib.MpvFormat.MPV_FORMAT_FLAG)
        mpv.observeProperty("eof-reached", MPVLib.MpvFormat.MPV_FORMAT_FLAG)
        mpv.observeProperty("seeking", MPVLib.MpvFormat.MPV_FORMAT_FLAG)
        mpv.observeProperty("duration", MPVLib.MpvFormat.MPV_FORMAT_DOUBLE)
        mpv.observeProperty("dwidth", MPVLib.MpvFormat.MPV_FORMAT_INT64)
        mpv.observeProperty("dheight", MPVLib.MpvFormat.MPV_FORMAT_INT64)
        mpv.observeProperty("video-params/rotate", MPVLib.MpvFormat.MPV_FORMAT_DOUBLE)
    }

    private fun loadPending() {
        val url = pendingUrl ?: return
        val instance = mpv ?: return
        instance.setPropertyBoolean("pause", true)
        pauseFlag = true
        fileLoaded = false
        firstFrameEmitted = false
        eofReached = false
        emit(
            UrlPlayerEngineEvent.BufferingChanged(
                isBuffering = true,
                reason = UrlPlayerWaitingReason.INITIAL,
            )
        )
        instance.command(arrayOf("loadfile", url, "replace"))
        applyPlayState()
    }

    private fun applyPlayState() {
        val instance = mpv ?: return
        instance.setPropertyBoolean("pause", !playWhenReady)
        pauseFlag = !playWhenReady
        if (isPlaying()) startPolling() else stopPolling()
        emit(UrlPlayerEngineEvent.PlayingChanged(isPlaying()))
    }

    private fun applyAudio(target: MPVLib? = mpv) {
        val instance = target ?: return
        instance.setPropertyDouble("volume", (volume * 100.0))
        instance.setPropertyBoolean("mute", muted)
    }

    private fun bindTextureView(view: TextureView) {
        val listener = object : TextureView.SurfaceTextureListener {
            override fun onSurfaceTextureAvailable(surface: SurfaceTexture, width: Int, height: Int) {
                attachAndroidSurface(Surface(surface), width, height, owns = true)
            }

            override fun onSurfaceTextureSizeChanged(surface: SurfaceTexture, width: Int, height: Int) {
                mpv?.setPropertyString("android-surface-size", "${width}x$height")
            }

            override fun onSurfaceTextureDestroyed(surface: SurfaceTexture): Boolean {
                unbindSurface()
                return true
            }

            override fun onSurfaceTextureUpdated(surface: SurfaceTexture) {}
        }
        textureListener = listener
        view.surfaceTextureListener = listener
        val existing = view.surfaceTexture
        if (existing != null) {
            val w = view.width.coerceAtLeast(1)
            val h = view.height.coerceAtLeast(1)
            attachAndroidSurface(Surface(existing), w, h, owns = true)
        }
    }

    private fun bindSurfaceView(view: android.view.SurfaceView) {
        val callback = object : SurfaceHolder.Callback {
            override fun surfaceCreated(holder: SurfaceHolder) {
                val w = view.width.coerceAtLeast(1)
                val h = view.height.coerceAtLeast(1)
                attachAndroidSurface(holder.surface, w, h, owns = false)
            }

            override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
                mpv?.setPropertyString("android-surface-size", "${width}x$height")
            }

            override fun surfaceDestroyed(holder: SurfaceHolder) {
                unbindSurface()
            }
        }
        holderCallback = callback
        view.holder.addCallback(callback)
        val surface = view.holder.surface
        if (surface != null && surface.isValid) {
            val w = view.width.coerceAtLeast(1)
            val h = view.height.coerceAtLeast(1)
            attachAndroidSurface(surface, w, h, owns = false)
        }
    }

    private fun attachAndroidSurface(surface: Surface, width: Int, height: Int, owns: Boolean) {
        if (released || !surface.isValid) {
            if (owns) surface.release()
            return
        }
        unbindSurface()
        val instance = mpv ?: run {
            if (owns) surface.release()
            return
        }
        boundSurface = surface
        ownsBoundSurface = owns
        instance.attachSurface(surface)
        instance.setPropertyString("android-surface-size", "${width}x$height")
        instance.setPropertyString("force-window", "yes")
        instance.setPropertyString("vo", "gpu")
        Log.i(TAG, "surface attached ${width}x$height owns=$owns")
    }

    private fun unbindSurface() {
        val instance = mpv
        if (instance != null && boundSurface != null) {
            instance.setPropertyString("vo", "null")
            instance.setPropertyString("force-window", "no")
            instance.detachSurface()
        }
        if (ownsBoundSurface) {
            boundSurface?.release()
        }
        boundSurface = null
        ownsBoundSurface = false
    }

    private fun unregisterSurfaceCallbacks() {
        when (val output = attachedOutput) {
            is UrlPlayerOutput.Texture -> {
                if (output.textureView.surfaceTextureListener === textureListener) {
                    output.textureView.surfaceTextureListener = null
                }
            }
            is UrlPlayerOutput.Surface -> {
                holderCallback?.let { output.surfaceView.holder.removeCallback(it) }
            }
            null -> Unit
        }
        textureListener = null
        holderCallback = null
    }

    private fun refreshMetadata() {
        val instance = mpv ?: return
        instance.getPropertyDouble("duration")?.let { value ->
            if (value.isFinite() && value > 0) durationMs = (value * 1000.0).toLong()
        }
        val width = instance.getPropertyInt("dwidth") ?: 0
        val height = instance.getPropertyInt("dheight") ?: 0
        val rotation = instance.getPropertyDouble("video-params/rotate")?.toInt() ?: 0
        if (width > 0 && height > 0) {
            videoSize = UrlPlayerVideoSize(width, height, rotation)
        }
    }

    private fun updateVideoSize(width: Int? = null, height: Int? = null, rotation: Int? = null) {
        val current = videoSize
        val nextWidth = width ?: current?.width ?: 0
        val nextHeight = height ?: current?.height ?: 0
        val nextRotation = rotation ?: current?.rotationDegrees ?: 0
        if (nextWidth <= 0 || nextHeight <= 0) return
        videoSize = UrlPlayerVideoSize(nextWidth, nextHeight, nextRotation)
        if (fileLoaded) {
            emit(UrlPlayerEngineEvent.Prepared(durationMs = durationMs, videoSize = videoSize))
        }
    }

    private fun startPolling() {
        if (timePoll != null) return
        val task = object : Runnable {
            override fun run() {
                if (released || !isPlaying()) {
                    timePoll = null
                    return
                }
                emit(
                    UrlPlayerEngineEvent.TimeUpdate(
                        currentTimeMs = getCurrentTimeMs(),
                        durationMs = durationMs,
                    )
                )
                mainHandler.postDelayed(this, 100L)
            }
        }
        timePoll = task
        mainHandler.post(task)
    }

    private fun stopPolling() {
        timePoll?.let { mainHandler.removeCallbacks(it) }
        timePoll = null
    }

    private fun emitError(code: UrlPlayerErrorCode, message: String) {
        emit(
            UrlPlayerEngineEvent.Error(
                UrlPlayerError(code = code, message = message)
            )
        )
    }

    private fun emit(event: UrlPlayerEngineEvent) {
        if (released) return
        val target = listener ?: return
        if (Looper.myLooper() == Looper.getMainLooper()) {
            target.onEngineEvent(event)
        } else {
            mainHandler.post {
                if (!released) target.onEngineEvent(event)
            }
        }
    }

    private fun onMain(block: () -> Unit) {
        if (Looper.myLooper() == Looper.getMainLooper()) {
            if (!released) block()
        } else {
            mainHandler.post { if (!released) block() }
        }
    }

    companion object {
        private const val TAG = "LingXia.MpvEngine"
    }
}
