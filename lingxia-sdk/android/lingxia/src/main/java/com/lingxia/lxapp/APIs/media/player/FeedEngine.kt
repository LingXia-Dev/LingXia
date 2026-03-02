package com.lingxia.lxapp.APIs.media.player

import android.os.Handler
import android.os.Looper
import com.lingxia.lxapp.NativeComponents.ComponentRouter

internal class FeedEngine(
    private val componentId: String,
) : PlayerEngine {
    private val mainHandler = Handler(Looper.getMainLooper())
    private var listener: EngineListener? = null

    @Volatile
    override var capabilities: PlayerCapabilities = PlayerCapabilities(
        supportsRate = false,
        supportsQualities = false,
    )
        private set

    private var baseOffsetMs: Long = 0
    private var durationMs: Long? = null
    private var playing = false
    private var lastAttachedToken: SurfaceToken? = null
    private var timePoll: Runnable? = null
    private var lastTimeMs: Long = 0
    private var lastVideoSize: VideoSize? = null
    private var endStallCount: Int = 0
    private val endToleranceMs: Long = 250
    private var hasDecoderClock: Boolean = false

    override fun setListener(listener: EngineListener?) {
        this.listener = listener
    }

    override fun setSource(source: PlayerSource) {
        baseOffsetMs = 0
        lastTimeMs = 0
        playing = false
        endStallCount = 0
        hasDecoderClock = false
        updateCapabilities()
        listener?.onEngineEvent(
            EngineEvent.Prepared(
                durationMs = durationMs,
                videoSize = lastVideoSize
            )
        )
    }

    override fun attachSurface(token: SurfaceToken) {
        lastAttachedToken = token
    }

    override fun detachSurface(token: SurfaceToken) {
        val current = lastAttachedToken ?: return
        if (current.ownerKey != token.ownerKey || current.generation != token.generation) return
        lastAttachedToken = null
    }

    override fun play() {
        playing = true
        endStallCount = 0
        startPolling()
        ComponentRouter.dispatchStreamDecoderCommand(componentId, "play", "{}")
    }

    override fun pause() {
        playing = false
        stopPolling()
        endStallCount = 0
        ComponentRouter.dispatchStreamDecoderCommand(componentId, "pause", """{"reason":"user"}""")
    }

    override fun stop() {
        playing = false
        baseOffsetMs = 0
        lastTimeMs = 0
        stopPolling()
        endStallCount = 0
        hasDecoderClock = false
        ComponentRouter.dispatchStreamDecoderCommand(componentId, "stop", "{}")
    }

    override fun seek(positionMs: Long) {
        val wasPlaying = playing
        baseOffsetMs = positionMs.coerceAtLeast(0)
        lastTimeMs = baseOffsetMs
        endStallCount = 0
        if (wasPlaying) {
            listener?.onEngineEvent(
                EngineEvent.BufferingChanged(
                    isBuffering = true,
                    reason = WaitingReason.SEEKING
                )
            )
        }
        ComponentRouter.dispatchStreamDecoderCommand(
            componentId,
            "resetStream",
            """{"hard":false,"emitWaiting":false}"""
        )
        if (wasPlaying) {
            startPolling()
        } else {
            stopPolling()
            listener?.onEngineEvent(
                EngineEvent.TimeUpdate(
                    currentTimeMs = lastTimeMs,
                    durationMs = durationMs
                )
            )
            listener?.onEngineEvent(EngineEvent.SeekCompleted(lastTimeMs))
            listener?.onEngineEvent(
                EngineEvent.BufferingChanged(
                    isBuffering = false,
                    reason = WaitingReason.SEEKING
                )
            )
        }
    }

    override fun setDurationMs(durationMs: Long?) {
        this.durationMs = durationMs
        endStallCount = 0
        updateCapabilities()
        listener?.onEngineEvent(
            EngineEvent.Prepared(
                durationMs = durationMs,
                videoSize = lastVideoSize
            )
        )
    }

    override fun setVolume(volume: Float) {
        ComponentRouter.dispatchStreamDecoderCommand(componentId, "setVolume", """{"volume":${volume.coerceIn(0f, 1f)}}""")
    }

    override fun setMuted(muted: Boolean) {
        ComponentRouter.dispatchStreamDecoderCommand(componentId, "setMuted", """{"muted":$muted}""")
    }

    override fun setRate(rate: Float) {
        // Not supported by decoder pipeline today.
    }

    override fun getCurrentTimeMs(): Long = lastTimeMs

    override fun getDurationMs(): Long? = durationMs

    override fun isPlaying(): Boolean = playing

    override fun release() {
        listener = null
        stopPolling()
        endStallCount = 0
        ComponentRouter.stopStreamDecoder(componentId)
    }

    fun handleStreamDecoderEvent(event: String, detail: Map<String, Any?>) {
        when (event) {
            "loadedmetadata" -> {
                val w = (detail["width"] as? Number)?.toInt() ?: 0
                val h = (detail["height"] as? Number)?.toInt() ?: 0
                val rotation = ((detail["rotation"] as? Number)?.toInt()
                    ?: (detail["rotate"] as? Number)?.toInt()
                    ?: 0)
                lastVideoSize = if (w > 0 && h > 0) VideoSize(w, h, rotation) else null
                listener?.onEngineEvent(
                    EngineEvent.Prepared(
                        durationMs = durationMs,
                        videoSize = lastVideoSize
                    )
                )
            }
            "waiting" -> {
                val reason = mapWaitingReason(detail["reason"] as? String)
                val d = durationMs
                val atEnd =
                    d != null && d > 0 && lastTimeMs >= (d - endToleranceMs).coerceAtLeast(0)
                if (playing && reason == WaitingReason.DECODER && atEnd) {
                    playing = false
                    stopPolling()
                    endStallCount = 0
                    listener?.onEngineEvent(EngineEvent.Ended)
                    return
                }
                listener?.onEngineEvent(EngineEvent.BufferingChanged(isBuffering = true, reason = reason))
            }
            "playing" -> {
                playing = true
                listener?.onEngineEvent(EngineEvent.PlayingChanged(true))
                listener?.onEngineEvent(EngineEvent.FirstFrameRendered)
                listener?.onEngineEvent(
                    EngineEvent.BufferingChanged(
                        isBuffering = false,
                        reason = WaitingReason.BUFFERING
                    )
                )
                startPolling()
            }
            "pause" -> {
                playing = false
                listener?.onEngineEvent(EngineEvent.PlayingChanged(false))
                stopPolling()
                endStallCount = 0
            }
            "stop" -> {
                playing = false
                listener?.onEngineEvent(EngineEvent.PlayingChanged(false))
                stopPolling()
                endStallCount = 0
            }
            "ended" -> {
                playing = false
                stopPolling()
                endStallCount = 0
                listener?.onEngineEvent(EngineEvent.Ended)
            }
            "error" -> {
                val code = (detail["code"] as? String).orEmpty()
                val message = (detail["message"] as? String) ?: "Decoder error"
                listener?.onEngineEvent(
                    EngineEvent.Error(
                        EngineError(
                            code = mapErrorCode(code),
                            message = message,
                            nativeCode = code,
                            backend = BackendKind.FEED
                        )
                    )
                )
            }
        }
    }

    private fun startPolling() {
        if (timePoll != null) return
        val task = object : Runnable {
            override fun run() {
                if (!playing) {
                    stopPolling()
                    return
                }
                val relSeconds = ComponentRouter.streamPlaybackPositionSeconds(componentId)
                if (relSeconds == null) {
                    if (hasDecoderClock) {
                        playing = false
                        stopPolling()
                        endStallCount = 0
                        listener?.onEngineEvent(EngineEvent.Ended)
                        return
                    }
                    listener?.onEngineEvent(EngineEvent.BufferingChanged(isBuffering = true, reason = WaitingReason.DECODER))
                    mainHandler.postDelayed(this, 50L)
                    return
                }
                hasDecoderClock = true
                val relMs = (relSeconds * 1000.0).toLong().coerceAtLeast(0)
                val rawMs = baseOffsetMs + relMs
                val duration = durationMs
                val currentMs = if (duration != null && duration > 0) rawMs.coerceAtMost(duration) else rawMs
                val prevMs = lastTimeMs
                lastTimeMs = currentMs
                listener?.onEngineEvent(
                    EngineEvent.TimeUpdate(
                        currentTimeMs = currentMs,
                        durationMs = durationMs
                    )
                )

                val atEnd =
                    duration != null && duration > 0 && currentMs >= (duration - endToleranceMs).coerceAtLeast(0)
                if (atEnd) {
                    val stalled = kotlin.math.abs(currentMs - prevMs) <= 1
                    endStallCount = if (stalled) endStallCount + 1 else 0
                    if (endStallCount >= 4) {
                        playing = false
                        stopPolling()
                        listener?.onEngineEvent(EngineEvent.Ended)
                        return
                    }
                } else {
                    endStallCount = 0
                }

                mainHandler.postDelayed(this, 50L)
            }
        }
        timePoll = task
        mainHandler.post(task)
    }

    private fun stopPolling() {
        timePoll?.let { mainHandler.removeCallbacks(it) }
        timePoll = null
    }

    private fun updateCapabilities() {
        val hasDuration = durationMs != null && durationMs!! > 0
        capabilities = capabilities.copy(
            isLive = false,
            hasDuration = hasDuration,
            canSeek = hasDuration,
        )
        listener?.onEngineEvent(EngineEvent.CapabilitiesChanged(capabilities))
    }

    private fun mapWaitingReason(raw: String?): WaitingReason {
        return when (raw) {
            "initial" -> WaitingReason.INITIAL
            "buffering" -> WaitingReason.BUFFERING
            "seeking" -> WaitingReason.SEEKING
            "surface_rebind" -> WaitingReason.SURFACE_REBIND
            "quality_switch" -> WaitingReason.QUALITY_SWITCH
            "decoder" -> WaitingReason.DECODER
            else -> WaitingReason.DECODER
        }
    }

    private fun mapErrorCode(raw: String): ErrorCode {
        return when (raw) {
            "network" -> ErrorCode.NETWORK
            "timeout" -> ErrorCode.TIMEOUT
            "decode", "stream_decoder" -> ErrorCode.DECODE
            "unsupported" -> ErrorCode.UNSUPPORTED
            "drm" -> ErrorCode.DRM
            "surface" -> ErrorCode.SURFACE
            "aborted" -> ErrorCode.ABORTED
            else -> ErrorCode.UNKNOWN
        }
    }
}
