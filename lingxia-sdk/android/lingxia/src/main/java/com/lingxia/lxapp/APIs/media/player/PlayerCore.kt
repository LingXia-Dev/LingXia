package com.lingxia.lxapp.APIs.media.player

import android.os.SystemClock
import kotlin.math.abs

internal class PlayerCore(
    private val createUrlEngine: () -> PlayerEngine,
    private val createFeedEngine: () -> PlayerEngine,
    private val emit: (PlayerEvent) -> Unit,
) : EngineListener {
    private var engine: PlayerEngine? = null
    private var source: PlayerSource? = null
    private var backend: BackendKind? = null
    private var surfaceToken: SurfaceToken? = null

    private var playIntent = false
    private var hasEmittedPlay = false
    private var enginePlaying = false
    private var hasEmittedPlayingSinceLastWaiting = false
    private var hasFirstFrame = false
    private var lastKnownTimeMs: Long? = null
    private var lastKnownDurationMs: Long? = null
    private var volume: Float = 1.0f
    private var muted: Boolean = false
    private var rate: Float = 1.0f

    private var lastTimeUpdateEmitAtMs: Long = 0
    private var lastTimeUpdateValueMs: Long? = null
    private var pendingSeekTargetMs: Long? = null

    private var capabilities: PlayerCapabilities = PlayerCapabilities()

    private var lastWaitingReason: WaitingReason? = null
    private var isWaiting: Boolean = false
    private var hasEnded: Boolean = false

    fun getBackend(): BackendKind? = backend

    fun getCapabilitiesSnapshot(): PlayerCapabilities = capabilities

    fun getLastKnownTimeMs(): Long? = lastKnownTimeMs

    fun getLastKnownDurationMs(): Long? = lastKnownDurationMs

    fun setSurfaceToken(token: SurfaceToken?) {
        val old = surfaceToken
        surfaceToken = token
        val currentEngine = engine ?: return
        if (old != null) {
            currentEngine.detachSurface(old)
        }
        if (token != null) {
            currentEngine.attachSurface(token)
        }
    }

    fun setSource(next: PlayerSource?) {
        val nextBackend = next?.let(::backendOf)
        val prevBackend = backend
        val hadSource = source != null
        val isSourceChanging = source != next

        if (hadSource && isSourceChanging) {
            playIntent = false
            hasEnded = false
            isWaiting = false
            lastWaitingReason = null
            emit(PlayerEvent.Stop(StopReason.SOURCE_CHANGE))
        }

        if (nextBackend != prevBackend) {
            teardownEngine()
            engine = when (nextBackend) {
                BackendKind.URL -> createUrlEngine()
                BackendKind.FEED -> createFeedEngine()
                null -> null
            }?.also { it.setListener(this) }
            backend = nextBackend
        } else if (hadSource && isSourceChanging) {
            engine?.stop()
        }

        source = next
        playIntent = false
        hasEnded = false
        enginePlaying = false
        hasEmittedPlay = false
        hasEmittedPlayingSinceLastWaiting = false
        hasFirstFrame = false
        lastKnownTimeMs = null
        pendingSeekTargetMs = null
        lastTimeUpdateEmitAtMs = 0
        lastTimeUpdateValueMs = null
        isWaiting = false
        lastWaitingReason = null

        val currentEngine = engine ?: return
        currentEngine.setVolume(volume)
        currentEngine.setMuted(muted)
        currentEngine.setRate(rate)
        val surface = surfaceToken
        if (surface != null) {
            currentEngine.attachSurface(surface)
        }
        if (next != null) {
            currentEngine.setSource(next)
            capabilities = currentEngine.capabilities
            emitCapabilitiesIfAny()
        }
    }

    fun play() {
        playIntent = true
        val currentEngine = engine
        val currentSource = source
        if (currentEngine == null || currentSource == null) {
            if (backend == BackendKind.FEED || backend == null) {
                emit(PlayerEvent.PlayRequest)
                emit(PlayerEvent.Waiting(WaitingReason.INITIAL))
            }
            return
        }
        if (hasEnded) {
            hasEnded = false
            seekInternal(0, resume = false)
        }
        maybeEmitPlay()
        if (backend == BackendKind.FEED) {
            emit(PlayerEvent.PlayRequest)
            emit(PlayerEvent.Waiting(WaitingReason.INITIAL))
            isWaiting = true
            lastWaitingReason = WaitingReason.INITIAL
            hasEmittedPlayingSinceLastWaiting = false
        }
        currentEngine.play()
    }

    fun pause() {
        playIntent = false
        hasEmittedPlay = false
        enginePlaying = false
        hasEmittedPlayingSinceLastWaiting = false
        hasEnded = false
        engine?.pause()
        emit(PlayerEvent.Pause(lastKnownTimeMs))
    }

    fun stop(reason: StopReason) {
        playIntent = false
        pendingSeekTargetMs = null
        hasEmittedPlay = false
        enginePlaying = false
        hasEmittedPlayingSinceLastWaiting = false
        hasFirstFrame = false
        hasEnded = false
        isWaiting = false
        lastWaitingReason = null
        engine?.stop()
        emit(PlayerEvent.Stop(reason))
    }

    fun seek(targetTimeMs: Long) {
        val shouldResumeAfterSeek = playIntent || hasEnded
        if (hasEnded) {
            playIntent = true
            if (backend == BackendKind.FEED) {
                emit(PlayerEvent.PlayRequest)
                emit(PlayerEvent.Waiting(WaitingReason.INITIAL, currentTimeMs = lastKnownTimeMs))
                isWaiting = true
                lastWaitingReason = WaitingReason.INITIAL
                hasEmittedPlayingSinceLastWaiting = false
            }
        }
        hasEnded = false
        hasEmittedPlayingSinceLastWaiting = false
        pendingSeekTargetMs = targetTimeMs
        emit(PlayerEvent.Seeking(targetTimeMs = targetTimeMs, currentTimeMs = lastKnownTimeMs))
        seekInternal(targetTimeMs, resume = shouldResumeAfterSeek)
    }

    fun setDurationMs(durationMs: Long?) {
        lastKnownDurationMs = durationMs
        engine?.setDurationMs(durationMs)
    }

    fun setVolume(volume: Float) {
        this.volume = volume.coerceIn(0f, 1f)
        engine?.setVolume(this.volume)
        emit(PlayerEvent.VolumeChange(volume = this.volume, muted = muted))
    }

    fun setMuted(muted: Boolean) {
        this.muted = muted
        engine?.setMuted(muted)
        emit(PlayerEvent.VolumeChange(volume = volume, muted = muted))
    }

    fun setRate(rate: Float) {
        val next = rate
        if (abs(this.rate - next) < 0.0001f) return
        this.rate = next
        engine?.setRate(next)
        emit(PlayerEvent.RateChange(next))
    }

    fun release() {
        teardownEngine()
    }

    override fun onEngineEvent(event: EngineEvent) {
        when (event) {
            is EngineEvent.CapabilitiesChanged -> {
                capabilities = event.capabilities
                emitCapabilitiesIfAny()
            }
            is EngineEvent.Prepared -> {
                lastKnownDurationMs = event.durationMs ?: lastKnownDurationMs
                val size = event.videoSize
                emit(
                    PlayerEvent.LoadedMetadata(
                        durationMs = lastKnownDurationMs,
                        width = size?.width ?: 0,
                        height = size?.height ?: 0,
                        rotation = size?.rotationDegrees ?: 0,
                    )
                )
            }
            is EngineEvent.BufferingChanged -> {
                if (event.isBuffering) {
                    if (hasEnded && !playIntent) return
                    if (!isWaiting || lastWaitingReason != event.reason) {
                        isWaiting = true
                        lastWaitingReason = event.reason
                        hasEmittedPlayingSinceLastWaiting = false
                        emit(PlayerEvent.Waiting(event.reason, currentTimeMs = lastKnownTimeMs))
                    }
                } else {
                    val wasWaiting = isWaiting
                    isWaiting = false
                    lastWaitingReason = null
                    if (wasWaiting) {
                        maybeEmitPlaying()
                    }
                }
            }
            is EngineEvent.FirstFrameRendered -> {
                hasFirstFrame = true
                maybeEmitPlaying()
            }
            is EngineEvent.PlayingChanged -> {
                enginePlaying = event.isPlaying
                if (event.isPlaying) {
                    playIntent = true
                    isWaiting = false
                    lastWaitingReason = null
                    maybeEmitPlay()
                    maybeEmitPlaying()
                } else {
                    hasEmittedPlayingSinceLastWaiting = false
                }
            }
            is EngineEvent.TimeUpdate -> {
                lastKnownTimeMs = event.currentTimeMs
                lastKnownDurationMs = event.durationMs ?: lastKnownDurationMs
                maybeEmitTimeUpdate(event.currentTimeMs, lastKnownDurationMs)
                maybeEmitSeekedIfNeeded(event.currentTimeMs)
                maybeEmitPlaying()
            }
            is EngineEvent.SeekCompleted -> {
                lastKnownTimeMs = event.currentTimeMs
                maybeEmitSeekedIfNeeded(event.currentTimeMs)
            }
            is EngineEvent.Ended -> {
                playIntent = false
                enginePlaying = false
                hasEmittedPlay = false
                hasEmittedPlayingSinceLastWaiting = false
                hasEnded = true
                isWaiting = false
                lastWaitingReason = null
                emit(PlayerEvent.Ended(lastKnownTimeMs))
            }
            is EngineEvent.Error -> {
                val e = event.error
                emit(
                    PlayerEvent.Error(
                        code = e.code,
                        message = e.message,
                        nativeCode = e.nativeCode,
                        httpStatus = e.httpStatus,
                        retryable = e.retryable,
                        backend = e.backend,
                    )
                )
            }
        }
    }

    private fun emitCapabilitiesIfAny() {
        // Not a JS event by itself today; kept as snapshot for policy decisions.
    }

    private fun maybeEmitPlaying() {
        if (!playIntent) return
        if (!enginePlaying) return
        if (hasEmittedPlayingSinceLastWaiting) return
        val time = lastKnownTimeMs
        if (hasFirstFrame || (time != null && time > 0)) {
            hasEmittedPlayingSinceLastWaiting = true
            isWaiting = false
            lastWaitingReason = null
            emit(PlayerEvent.Playing(time))
        }
    }

    private fun maybeEmitPlay() {
        if (hasEmittedPlay) return
        hasEmittedPlay = true
        emit(PlayerEvent.Play)
    }

    private fun maybeEmitSeekedIfNeeded(currentTimeMs: Long) {
        val target = pendingSeekTargetMs ?: return
        val closeEnough = abs(currentTimeMs - target) <= 50
        if (closeEnough) {
            pendingSeekTargetMs = null
            emit(PlayerEvent.Seeked(currentTimeMs))
        }
    }

    private fun maybeEmitTimeUpdate(currentTimeMs: Long, durationMs: Long?) {
        val now = SystemClock.uptimeMillis()
        val lastSentAt = lastTimeUpdateEmitAtMs
        val lastSentValue = lastTimeUpdateValueMs

        val valueChanged = lastSentValue == null || abs(currentTimeMs - lastSentValue) >= 30
        val timeOk = now - lastSentAt >= 200

        if (valueChanged && timeOk) {
            lastTimeUpdateEmitAtMs = now
            lastTimeUpdateValueMs = currentTimeMs
            emit(PlayerEvent.TimeUpdate(currentTimeMs = currentTimeMs, durationMs = durationMs))
        }
    }

    private fun backendOf(source: PlayerSource): BackendKind = when (source) {
        is PlayerSource.Url -> BackendKind.URL
        is PlayerSource.Feed -> BackendKind.FEED
    }

    private fun seekInternal(targetTimeMs: Long, resume: Boolean) {
        val currentEngine = engine
        currentEngine?.seek(targetTimeMs)
        if (resume) {
            currentEngine?.play()
        }
    }

    private fun teardownEngine() {
        engine?.setListener(null)
        engine?.release()
        engine = null
    }
}
