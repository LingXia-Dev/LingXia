package com.lingxia.lxapp.APIs.media.player

internal sealed class PlayerEvent(val name: String) {

    data object PlayRequest : PlayerEvent("playrequest")

    data object Play : PlayerEvent("play")

    data class Playing(val currentTimeMs: Long? = null) : PlayerEvent("playing")

    data object FirstFrameRendered : PlayerEvent("firstframerendered")

    data class Pause(val currentTimeMs: Long? = null) : PlayerEvent("pause")

    data class Waiting(
        val reason: WaitingReason,
        val currentTimeMs: Long? = null
    ) : PlayerEvent("waiting")

    data class Seeking(
        val targetTimeMs: Long,
        val currentTimeMs: Long? = null
    ) : PlayerEvent("seeking")

    data class Seeked(val currentTimeMs: Long) : PlayerEvent("seeked")

    data class TimeUpdate(
        val currentTimeMs: Long,
        val durationMs: Long?
    ) : PlayerEvent("timeupdate")

    data class LoadedMetadata(
        val durationMs: Long?,
        val width: Int,
        val height: Int,
        val rotation: Int = 0,
    ) : PlayerEvent("loadedmetadata")

    data class Ended(val currentTimeMs: Long? = null) : PlayerEvent("ended")

    data class Error(
        val code: ErrorCode,
        val message: String,
        val nativeCode: String? = null,
        val httpStatus: Int? = null,
        val retryable: Boolean? = null,
        val backend: BackendKind? = null
    ) : PlayerEvent("error")

    data class RateChange(val rate: Float) : PlayerEvent("ratechange")

    data class VolumeChange(val volume: Float, val muted: Boolean) : PlayerEvent("volumechange")

    data class Stop(val reason: StopReason) : PlayerEvent("stop")

    data class FullscreenChange(val fullscreen: Boolean) : PlayerEvent("fullscreenchange")
}
