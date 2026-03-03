package com.lingxia.lxapp.APIs.media.player

import android.app.ActivityManager
import android.content.Context
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.util.Log
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.VideoSize as M3VideoSize
import androidx.media3.exoplayer.DefaultLoadControl
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.ui.PlayerView

internal class UrlEngine(
    context: Context,
    private val playerView: PlayerView,
) : PlayerEngine {
    private val tag = "LingXia.UrlEngine"
    private val memoryProfile = buildMemoryProfile(context)
    internal val exoPlayer: ExoPlayer = ExoPlayer.Builder(context)
        .setLoadControl(buildLoadControl(memoryProfile))
        .build()
    private val mainHandler = Handler(Looper.getMainLooper())
    private var listener: EngineListener? = null
    private var muted = false
    private var volume = 1.0f
    private var timePoll: Runnable? = null
    private var currentSourceUri: Uri? = null
    private var autoRecoverAttemptedForCurrentSource = false

    @Volatile
    override var capabilities: PlayerCapabilities = PlayerCapabilities(
        supportsRate = true,
        supportsQualities = true,
    )
        private set

    init {
        playerView.player = exoPlayer
        exoPlayer.addListener(
            object : Player.Listener {
                override fun onPlaybackStateChanged(playbackState: Int) {
                    when (playbackState) {
                        Player.STATE_BUFFERING -> {
                            listener?.onEngineEvent(
                                EngineEvent.BufferingChanged(
                                    isBuffering = true,
                                    reason = WaitingReason.BUFFERING
                                )
                            )
                        }
                        Player.STATE_READY -> {
                            listener?.onEngineEvent(
                                EngineEvent.Prepared(
                                    durationMs = durationMsOrNull(),
                                    videoSize = videoSizeOrNull()
                                )
                            )
                            updateCapabilities()
                            listener?.onEngineEvent(
                                EngineEvent.BufferingChanged(
                                    isBuffering = false,
                                    reason = WaitingReason.BUFFERING
                                )
                            )
                        }
                        Player.STATE_ENDED -> {
                            stopPolling()
                            listener?.onEngineEvent(EngineEvent.Ended)
                        }
                    }
                }

                override fun onIsPlayingChanged(isPlaying: Boolean) {
                    if (isPlaying) startPolling() else stopPolling()
                    listener?.onEngineEvent(EngineEvent.PlayingChanged(isPlaying))
                }

                override fun onPositionDiscontinuity(
                    oldPosition: Player.PositionInfo,
                    newPosition: Player.PositionInfo,
                    reason: Int
                ) {
                    if (reason == Player.DISCONTINUITY_REASON_SEEK) {
                        listener?.onEngineEvent(EngineEvent.SeekCompleted(exoPlayer.currentPosition))
                    }
                }

                override fun onRenderedFirstFrame() {
                    listener?.onEngineEvent(EngineEvent.FirstFrameRendered)
                }

                override fun onVideoSizeChanged(videoSize: M3VideoSize) {
                    listener?.onEngineEvent(
                        EngineEvent.Prepared(
                            durationMs = durationMsOrNull(),
                            videoSize = videoSizeOrNull()
                        )
                    )
                }

                override fun onPlayerError(error: PlaybackException) {
                    val mappedCode = mapErrorCode(error)
                    val retryable = isRetryableError(error)
                    if (retryable && !autoRecoverAttemptedForCurrentSource && recoverFromError("onPlayerError", error)) {
                        autoRecoverAttemptedForCurrentSource = true
                        listener?.onEngineEvent(
                            EngineEvent.BufferingChanged(
                                isBuffering = true,
                                reason = WaitingReason.DECODER
                            )
                        )
                        return
                    }
                    listener?.onEngineEvent(
                        EngineEvent.Error(
                            EngineError(
                                code = mappedCode,
                                message = error.message ?: "Playback error",
                                nativeCode = error.errorCodeName,
                                retryable = retryable,
                                backend = BackendKind.URL
                            )
                        )
                    )
                }
            }
        )
    }

    override fun setListener(listener: EngineListener?) {
        this.listener = listener
    }

    override fun setSource(source: PlayerSource) {
        val url = (source as? PlayerSource.Url)?.url ?: return
        currentSourceUri = Uri.parse(url)
        autoRecoverAttemptedForCurrentSource = false
        // Drop previous queue/buffered samples early before binding the new source.
        exoPlayer.stop()
        exoPlayer.clearMediaItems()
        exoPlayer.setMediaItem(androidx.media3.common.MediaItem.fromUri(currentSourceUri!!))
        exoPlayer.prepare()
        updateCapabilities()
    }

    override fun attachSurface(token: SurfaceToken) {
        // PlayerView owns the surface lifecycle for URL playback.
    }

    override fun detachSurface(token: SurfaceToken) {
        // PlayerView owns the surface lifecycle for URL playback.
    }

    override fun play() {
        val playerError = exoPlayer.playerError
        if (
            playerError != null &&
            isRetryableError(playerError) &&
            !autoRecoverAttemptedForCurrentSource &&
            recoverFromError("play", playerError)
        ) {
            autoRecoverAttemptedForCurrentSource = true
        }
        exoPlayer.playWhenReady = true
        exoPlayer.play()
    }

    override fun pause() {
        exoPlayer.pause()
        stopPolling()
    }

    override fun stop() {
        exoPlayer.playWhenReady = false
        exoPlayer.pause()
        exoPlayer.seekTo(0)
        stopPolling()
    }

    override fun seek(positionMs: Long) {
        exoPlayer.seekTo(positionMs)
    }

    override fun setDurationMs(durationMs: Long?) {
        // URL engine duration is driven by the media pipeline.
    }

    override fun setVolume(volume: Float) {
        this.volume = volume.coerceIn(0f, 1f)
        applyAudioState()
    }

    override fun setMuted(muted: Boolean) {
        this.muted = muted
        applyAudioState()
    }

    override fun setRate(rate: Float) {
        exoPlayer.setPlaybackSpeed(rate)
    }

    override fun getCurrentTimeMs(): Long = exoPlayer.currentPosition

    override fun getDurationMs(): Long? = durationMsOrNull()

    override fun isPlaying(): Boolean = exoPlayer.isPlaying

    override fun release() {
        listener = null
        stopPolling()
        playerView.player = null
        exoPlayer.stop()
        exoPlayer.clearMediaItems()
        exoPlayer.release()
    }

    fun setLoopEnabled(loopEnabled: Boolean) {
        exoPlayer.repeatMode = if (loopEnabled) Player.REPEAT_MODE_ONE else Player.REPEAT_MODE_OFF
    }

    private fun applyAudioState() {
        exoPlayer.volume = if (muted) 0f else volume
    }

    private fun durationMsOrNull(): Long? {
        val d = exoPlayer.duration
        return if (d <= 0) null else d
    }

    private fun videoSizeOrNull(): VideoSize? {
        val s = exoPlayer.videoSize
        return if (s.width > 0 && s.height > 0) {
            VideoSize(
                width = s.width,
                height = s.height,
                rotationDegrees = s.unappliedRotationDegrees
            )
        } else {
            null
        }
    }

    private fun updateCapabilities() {
        val isLive = exoPlayer.isCurrentMediaItemLive
        val durationMs = durationMsOrNull()
        val hasDuration = durationMs != null && durationMs > 0
        val canSeek = !isLive && hasDuration
        capabilities = capabilities.copy(
            isLive = isLive,
            hasDuration = hasDuration,
            canSeek = canSeek,
        )
        listener?.onEngineEvent(EngineEvent.CapabilitiesChanged(capabilities))
    }

    private fun startPolling() {
        if (timePoll != null) return
        val task = object : Runnable {
            override fun run() {
                listener?.onEngineEvent(
                    EngineEvent.TimeUpdate(
                        currentTimeMs = exoPlayer.currentPosition,
                        durationMs = durationMsOrNull(),
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

    private fun mapErrorCode(error: PlaybackException): ErrorCode {
        return when (error.errorCode) {
            PlaybackException.ERROR_CODE_IO_BAD_HTTP_STATUS,
            PlaybackException.ERROR_CODE_IO_INVALID_HTTP_CONTENT_TYPE,
            PlaybackException.ERROR_CODE_IO_NETWORK_CONNECTION_FAILED,
            PlaybackException.ERROR_CODE_IO_NETWORK_CONNECTION_TIMEOUT,
            PlaybackException.ERROR_CODE_IO_UNSPECIFIED -> ErrorCode.NETWORK
            PlaybackException.ERROR_CODE_DECODING_FAILED,
            PlaybackException.ERROR_CODE_DECODER_INIT_FAILED,
            PlaybackException.ERROR_CODE_DECODER_QUERY_FAILED -> ErrorCode.DECODE
            PlaybackException.ERROR_CODE_DRM_CONTENT_ERROR,
            PlaybackException.ERROR_CODE_DRM_LICENSE_ACQUISITION_FAILED,
            PlaybackException.ERROR_CODE_DRM_DISALLOWED_OPERATION,
            PlaybackException.ERROR_CODE_DRM_PROVISIONING_FAILED -> ErrorCode.DRM
            PlaybackException.ERROR_CODE_PARSING_CONTAINER_MALFORMED,
            PlaybackException.ERROR_CODE_PARSING_MANIFEST_MALFORMED -> ErrorCode.UNSUPPORTED
            else -> ErrorCode.UNKNOWN
        }
    }

    private fun isRetryableError(error: PlaybackException): Boolean {
        return when (error.errorCode) {
            PlaybackException.ERROR_CODE_DECODING_FAILED,
            PlaybackException.ERROR_CODE_DECODER_INIT_FAILED,
            PlaybackException.ERROR_CODE_DECODER_QUERY_FAILED,
            PlaybackException.ERROR_CODE_UNSPECIFIED -> true
            else -> false
        }
    }

    private fun recoverFromError(trigger: String, error: PlaybackException?): Boolean {
        val source = currentSourceUri ?: return false
        return try {
            Log.w(
                tag,
                "recoverFromError trigger=$trigger code=${error?.errorCodeName ?: "unknown"} source=$source"
            )
            exoPlayer.stop()
            exoPlayer.clearMediaItems()
            exoPlayer.setMediaItem(androidx.media3.common.MediaItem.fromUri(source))
            exoPlayer.prepare()
            true
        } catch (t: Throwable) {
            Log.e(tag, "recoverFromError failed: ${t.message}", t)
            false
        }
    }

    private data class MemoryProfile(
        val minBufferMs: Int,
        val maxBufferMs: Int,
        val bufferForPlaybackMs: Int,
        val bufferForPlaybackAfterRebufferMs: Int,
        val targetBufferBytes: Int,
    )

    private fun buildMemoryProfile(context: Context): MemoryProfile {
        val am = context.getSystemService(Context.ACTIVITY_SERVICE) as? ActivityManager
        val lowRam = am?.isLowRamDevice == true
        val memoryClass = am?.memoryClass ?: 256

        return when {
            lowRam || memoryClass <= 256 -> MemoryProfile(
                minBufferMs = 3000,
                maxBufferMs = 10000,
                bufferForPlaybackMs = 500,
                bufferForPlaybackAfterRebufferMs = 1500,
                targetBufferBytes = 8 * 1024 * 1024,
            )
            memoryClass <= 384 -> MemoryProfile(
                minBufferMs = 5000,
                maxBufferMs = 15000,
                bufferForPlaybackMs = 700,
                bufferForPlaybackAfterRebufferMs = 2000,
                targetBufferBytes = 12 * 1024 * 1024,
            )
            else -> MemoryProfile(
                minBufferMs = 7000,
                maxBufferMs = 20000,
                bufferForPlaybackMs = 800,
                bufferForPlaybackAfterRebufferMs = 2500,
                targetBufferBytes = 16 * 1024 * 1024,
            )
        }
    }

    private fun buildLoadControl(profile: MemoryProfile): DefaultLoadControl {
        return DefaultLoadControl.Builder()
            .setBufferDurationsMs(
                profile.minBufferMs,
                profile.maxBufferMs,
                profile.bufferForPlaybackMs,
                profile.bufferForPlaybackAfterRebufferMs
            )
            .setTargetBufferBytes(profile.targetBufferBytes)
            .setPrioritizeTimeOverSizeThresholds(true)
            .setBackBuffer(0, false)
            .build()
    }
}
