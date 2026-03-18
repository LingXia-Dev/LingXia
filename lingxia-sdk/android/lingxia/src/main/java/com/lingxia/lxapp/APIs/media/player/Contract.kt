package com.lingxia.lxapp.APIs.media.player

internal enum class BackendKind(val value: String) {
    URL("url"),
    FEED("feed")
}

internal data class VideoSize(
    val width: Int,
    val height: Int,
    val rotationDegrees: Int = 0,
)

internal enum class StopReason(val value: String) {
    USER("user"),
    UNMOUNT("unmount"),
    SOURCE_CHANGE("source_change")
}

internal enum class WaitingReason(val value: String) {
    INITIAL("initial"),
    BUFFERING("buffering"),
    SEEKING("seeking"),
    SURFACE_REBIND("surface_rebind"),
    QUALITY_SWITCH("quality_switch"),
    DECODER("decoder")
}

internal enum class ErrorCode(val value: String) {
    ABORTED("aborted"),
    NETWORK("network"),
    TIMEOUT("timeout"),
    DECODE("decode"),
    UNSUPPORTED("unsupported"),
    DRM("drm"),
    SURFACE("surface"),
    INTERNAL("internal"),
    UNKNOWN("unknown")
}

internal data class EngineError(
    val code: ErrorCode,
    val message: String,
    val nativeCode: String? = null,
    val httpStatus: Int? = null,
    val retryable: Boolean? = null,
    val backend: BackendKind? = null
)

internal data class PlayerCapabilities(
    val isLive: Boolean? = null,
    val canSeek: Boolean? = null,
    val hasDuration: Boolean? = null,
    val supportsRate: Boolean = false,
    val supportsQualities: Boolean = false
)

sealed class PlayerSource {
    data class Url(
        val url: String,
        val headers: Map<String, String> = emptyMap()
    ) : PlayerSource()

    data class Feed(
        val sessionId: String
    ) : PlayerSource()
}
