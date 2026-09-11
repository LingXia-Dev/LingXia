package com.lingxia.lxapp.APIs.media.player

import android.os.Handler
import android.os.Looper
import android.view.View
import com.lingxia.app.media.UrlPlayerEngine
import com.lingxia.app.media.UrlPlayerEngineEvent
import com.lingxia.app.media.UrlPlayerErrorCode
import com.lingxia.app.media.UrlPlayerOutput
import com.lingxia.app.media.UrlPlayerSource
import com.lingxia.app.media.UrlPlayerWaitingReason

internal class HostUrlEngineAdapter(
    private val host: UrlPlayerEngine,
    private val output: UrlPlayerOutput,
    private val expectedToken: SurfaceToken,
) : PlayerEngine {
    private val mainHandler = Handler(Looper.getMainLooper())
    private var coreListener: EngineListener? = null
    private var loopEnabled = false
    private var boundView: View? = null
    private var released = false

    override var capabilities: PlayerCapabilities = PlayerCapabilities(
        supportsRate = true,
        supportsQualities = false,
    )
        private set

    init {
        host.setListener { event -> dispatchHostEvent(event) }
    }

    override fun setListener(listener: EngineListener?) {
        coreListener = listener
    }

    override fun setSource(source: PlayerSource) {
        val url = source as? PlayerSource.Url ?: return
        safe {
            host.setSource(
                UrlPlayerSource(
                    url = url.url,
                    headers = url.headers,
                )
            )
        }
    }

    override fun attachSurface(token: SurfaceToken) {
        if (released) return
        val view = viewOf(output)
        if (boundView === view) return
        boundView = view
        safe { host.attachOutput(output) }
    }

    override fun detachSurface(token: SurfaceToken) {
        if (released) return
        if (boundView == null) return
        if (token.ownerKey != expectedToken.ownerKey) return
        boundView = null
        safe { host.detachOutput() }
    }

    override fun play() {
        safe { host.play() }
    }

    override fun pause() {
        safe { host.pause() }
    }

    override fun stop() {
        safe { host.stop() }
    }

    override fun seek(positionMs: Long) {
        safe { host.seek(positionMs) }
    }

    override fun setDurationMs(durationMs: Long?) {
        // Feed-only. URL duration comes from Prepared / TimeUpdate.
    }

    override fun setVolume(volume: Float) {
        safe { host.setVolume(volume) }
    }

    override fun setMuted(muted: Boolean) {
        safe { host.setMuted(muted) }
    }

    override fun setRate(rate: Float) {
        safe { host.setRate(rate) }
    }

    override fun setLoopEnabled(loopEnabled: Boolean) {
        this.loopEnabled = loopEnabled
        safe { host.setLoopEnabled(loopEnabled) }
    }

    override fun getCurrentTimeMs(): Long = if (released) 0L else safeValue { host.getCurrentTimeMs() } ?: 0L

    override fun getDurationMs(): Long? = if (released) null else safeValue { host.getDurationMs() }

    override fun isPlaying(): Boolean = if (released) false else safeValue { host.isPlaying() } ?: false

    override fun release() {
        if (released) return
        released = true
        coreListener = null
        val stillBound = boundView != null
        boundView = null
        try {
            if (stillBound) host.detachOutput()
            host.setListener(null)
            host.release()
        } catch (_: Throwable) {
        }
    }

    private fun dispatchHostEvent(event: UrlPlayerEngineEvent) {
        if (released) return
        if (event is UrlPlayerEngineEvent.Ended && loopEnabled) return
        val mapped = mapEvent(event) ?: return
        emit(mapped)
    }

    private fun mapEvent(event: UrlPlayerEngineEvent): EngineEvent? = when (event) {
        is UrlPlayerEngineEvent.Prepared -> EngineEvent.Prepared(
            durationMs = event.durationMs,
            videoSize = event.videoSize?.let {
                VideoSize(
                    width = it.width,
                    height = it.height,
                    rotationDegrees = it.rotationDegrees,
                )
            },
        )
        is UrlPlayerEngineEvent.BufferingChanged -> EngineEvent.BufferingChanged(
            isBuffering = event.isBuffering,
            reason = event.reason.toInternal(),
        )
        is UrlPlayerEngineEvent.PlayingChanged -> EngineEvent.PlayingChanged(event.isPlaying)
        is UrlPlayerEngineEvent.TimeUpdate -> EngineEvent.TimeUpdate(
            currentTimeMs = event.currentTimeMs,
            durationMs = event.durationMs,
        )
        is UrlPlayerEngineEvent.SeekCompleted -> EngineEvent.SeekCompleted(event.currentTimeMs)
        UrlPlayerEngineEvent.FirstFrameRendered -> EngineEvent.FirstFrameRendered
        UrlPlayerEngineEvent.Ended -> EngineEvent.Ended
        is UrlPlayerEngineEvent.Error -> EngineEvent.Error(
            EngineError(
                code = event.error.code.toInternal(),
                message = event.error.message,
                nativeCode = event.error.nativeCode,
                httpStatus = event.error.httpStatus,
                retryable = event.error.retryable,
                backend = BackendKind.URL,
            )
        )
    }

    private fun emit(event: EngineEvent) {
        val listener = coreListener ?: return
        if (Looper.myLooper() == Looper.getMainLooper()) {
            listener.onEngineEvent(event)
        } else {
            mainHandler.post {
                if (!released) listener.onEngineEvent(event)
            }
        }
    }

    private fun emitInternalError(t: Throwable) {
        emit(
            EngineEvent.Error(
                EngineError(
                    code = ErrorCode.INTERNAL,
                    message = t.message ?: "Host URL engine error",
                    nativeCode = t.javaClass.simpleName,
                    backend = BackendKind.URL,
                )
            )
        )
    }

    private inline fun safe(block: () -> Unit) {
        if (released) return
        try {
            block()
        } catch (t: Throwable) {
            emitInternalError(t)
        }
    }

    private inline fun <T> safeValue(block: () -> T): T? {
        if (released) return null
        return try {
            block()
        } catch (t: Throwable) {
            emitInternalError(t)
            null
        }
    }

    companion object {
        fun viewOf(output: UrlPlayerOutput): View = when (output) {
            is UrlPlayerOutput.Texture -> output.textureView
            is UrlPlayerOutput.Surface -> output.surfaceView
        }
    }
}

internal fun UrlPlayerErrorCode.toInternal(): ErrorCode = when (this) {
    UrlPlayerErrorCode.ABORTED -> ErrorCode.ABORTED
    UrlPlayerErrorCode.NETWORK -> ErrorCode.NETWORK
    UrlPlayerErrorCode.TIMEOUT -> ErrorCode.TIMEOUT
    UrlPlayerErrorCode.DECODE -> ErrorCode.DECODE
    UrlPlayerErrorCode.UNSUPPORTED -> ErrorCode.UNSUPPORTED
    UrlPlayerErrorCode.DRM -> ErrorCode.DRM
    UrlPlayerErrorCode.SURFACE -> ErrorCode.SURFACE
    UrlPlayerErrorCode.INTERNAL -> ErrorCode.INTERNAL
    UrlPlayerErrorCode.UNKNOWN -> ErrorCode.UNKNOWN
}

internal fun UrlPlayerWaitingReason.toInternal(): WaitingReason = when (this) {
    UrlPlayerWaitingReason.INITIAL -> WaitingReason.INITIAL
    UrlPlayerWaitingReason.BUFFERING -> WaitingReason.BUFFERING
    UrlPlayerWaitingReason.SEEKING -> WaitingReason.SEEKING
    UrlPlayerWaitingReason.SURFACE_REBIND -> WaitingReason.SURFACE_REBIND
    UrlPlayerWaitingReason.QUALITY_SWITCH -> WaitingReason.QUALITY_SWITCH
    UrlPlayerWaitingReason.DECODER -> WaitingReason.DECODER
}
