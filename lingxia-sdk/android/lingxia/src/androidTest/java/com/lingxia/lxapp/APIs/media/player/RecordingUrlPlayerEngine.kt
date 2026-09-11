package com.lingxia.lxapp.APIs.media.player

import com.lingxia.app.media.UrlPlayerEngine
import com.lingxia.app.media.UrlPlayerEngineEvent
import com.lingxia.app.media.UrlPlayerEngineListener
import com.lingxia.app.media.UrlPlayerOutput
import com.lingxia.app.media.UrlPlayerSource

internal open class RecordingUrlPlayerEngine : UrlPlayerEngine {
    val calls = mutableListOf<String>()
    var listener: UrlPlayerEngineListener? = null
        private set
    var attachOutputCount = 0
        private set
    var detachOutputCount = 0
        private set
    var releaseCount = 0
        private set
    var lastSource: UrlPlayerSource? = null
        private set

    override fun setListener(listener: UrlPlayerEngineListener?) {
        this.listener = listener
        calls += "setListener:${listener != null}"
    }

    override fun setSource(source: UrlPlayerSource) {
        lastSource = source
        calls += "setSource:${source.url}"
    }

    override fun attachOutput(output: UrlPlayerOutput) {
        attachOutputCount += 1
        calls += "attachOutput"
    }

    override fun detachOutput() {
        detachOutputCount += 1
        calls += "detachOutput"
    }

    override fun play() {
        calls += "play"
    }

    override fun pause() {
        calls += "pause"
    }

    override fun stop() {
        calls += "stop"
    }

    override fun seek(positionMs: Long) {
        calls += "seek:$positionMs"
    }

    override fun setVolume(volume: Float) {
        calls += "setVolume:$volume"
    }

    override fun setMuted(muted: Boolean) {
        calls += "setMuted:$muted"
    }

    override fun setRate(rate: Float) {
        calls += "setRate:$rate"
    }

    override fun setLoopEnabled(loopEnabled: Boolean) {
        calls += "setLoopEnabled:$loopEnabled"
    }

    override fun getCurrentTimeMs(): Long = 0

    override fun getDurationMs(): Long? = null

    override fun isPlaying(): Boolean = false

    override fun release() {
        releaseCount += 1
        calls += "release"
    }

    fun emit(event: UrlPlayerEngineEvent) {
        listener?.onEngineEvent(event)
    }
}
