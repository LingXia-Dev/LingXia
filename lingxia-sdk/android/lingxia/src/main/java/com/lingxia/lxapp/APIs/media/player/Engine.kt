package com.lingxia.lxapp.APIs.media.player

sealed interface EngineEvent {
    data class Prepared(
        val durationMs: Long?,
        val videoSize: VideoSize?
    ) : EngineEvent

    data class CapabilitiesChanged(
        val capabilities: PlayerCapabilities
    ) : EngineEvent

    data class BufferingChanged(
        val isBuffering: Boolean,
        val reason: WaitingReason
    ) : EngineEvent

    data class PlayingChanged(
        val isPlaying: Boolean
    ) : EngineEvent

    data class TimeUpdate(
        val currentTimeMs: Long,
        val durationMs: Long?
    ) : EngineEvent

    data class SeekCompleted(
        val currentTimeMs: Long
    ) : EngineEvent

    data object FirstFrameRendered : EngineEvent

    data object Ended : EngineEvent

    data class Error(
        val error: EngineError
    ) : EngineEvent
}

fun interface EngineListener {
    fun onEngineEvent(event: EngineEvent)
}

interface PlayerEngine {
    val capabilities: PlayerCapabilities

    fun setListener(listener: EngineListener?)

    fun setSource(source: PlayerSource)
    fun attachSurface(token: SurfaceToken)
    fun detachSurface(token: SurfaceToken)

    fun play()
    fun pause()
    fun stop()
    fun seek(positionMs: Long)

    fun setDurationMs(durationMs: Long?)

    fun setVolume(volume: Float)
    fun setMuted(muted: Boolean)
    fun setRate(rate: Float)

    fun getCurrentTimeMs(): Long
    fun getDurationMs(): Long?
    fun isPlaying(): Boolean

    fun release()
}
