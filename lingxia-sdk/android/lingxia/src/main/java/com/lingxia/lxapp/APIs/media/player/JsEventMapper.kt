package com.lingxia.lxapp.APIs.media.player

internal object JsEventMapper {
    fun toPayload(event: PlayerEvent): Map<String, Any> {
        return mapOf(
            "event" to event.name,
            "detail" to toDetail(event),
        )
    }

    private fun toDetail(event: PlayerEvent): Map<String, Any> {
        return when (event) {
            PlayerEvent.PlayRequest,
            PlayerEvent.Play,
            PlayerEvent.FirstFrameRendered,
            is PlayerEvent.Playing -> emptyMap()
            is PlayerEvent.Pause -> buildMap {
                put("reason", "user")
                event.currentTimeMs?.let {
                    put("currentTime", it.toDouble() / 1000.0)
                }
            }
            is PlayerEvent.Ended,
            is PlayerEvent.FullscreenChange -> emptyMap()
            is PlayerEvent.Waiting -> mapOf("reason" to event.reason.value)
            is PlayerEvent.Seeking -> mapOf("time" to (event.targetTimeMs.toDouble() / 1000.0))
            is PlayerEvent.Seeked -> mapOf(
                "time" to (event.currentTimeMs.toDouble() / 1000.0),
                "currentTime" to (event.currentTimeMs.toDouble() / 1000.0),
            )
            is PlayerEvent.TimeUpdate -> mapOf(
                "currentTime" to (event.currentTimeMs.toDouble() / 1000.0),
                "duration" to ((event.durationMs ?: 0L).toDouble() / 1000.0)
            )
            is PlayerEvent.LoadedMetadata -> mapOf(
                "width" to event.width,
                "height" to event.height,
                "rotation" to event.rotation,
                "duration" to ((event.durationMs ?: 0L).toDouble() / 1000.0)
            )
            is PlayerEvent.Error -> buildMap {
                put("code", event.code.value)
                put("message", event.message)
                event.nativeCode?.let { put("nativeCode", it) }
                event.httpStatus?.let { put("httpStatus", it) }
                event.retryable?.let { put("retryable", it) }
                event.backend?.let { put("backend", it.value) }
            }
            is PlayerEvent.RateChange -> mapOf("rate" to event.rate)
            is PlayerEvent.VolumeChange -> mapOf(
                "volume" to event.volume,
                "muted" to event.muted
            )
            is PlayerEvent.Stop -> mapOf("reason" to event.reason.value)
        }
    }
}
