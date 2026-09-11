package com.lingxia.lxapp.APIs.media.player

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class PlayerCoreTest {
    private fun coreWith(fake: FakeUrlEngine): Pair<PlayerCore, MutableList<PlayerEvent>> {
        val events = mutableListOf<PlayerEvent>()
        var created = 0
        val core = PlayerCore(
            createUrlEngine = {
                created += 1
                fake.createCount = created
                fake
            },
            createFeedEngine = { error("feed not used") },
            emit = { events += it },
        )
        return core to events
    }

    @Test
    fun firstUrlSetSourceCreatesEngineOnceAndDoesNotPlay() {
        val fake = FakeUrlEngine()
        val (core, _) = coreWith(fake)
        core.setSource(PlayerSource.Url("https://example.com/a.mp4"))
        assertEquals(1, fake.createCount)
        assertEquals(listOf("setSource:https://example.com/a.mp4"), fake.calls.filter {
            it.startsWith("setSource") || it == "play" || it == "stop"
        })
    }

    @Test
    fun sameBackendDifferentUrlStopsWithoutRecreateOrPlay() {
        val fake = FakeUrlEngine()
        val (core, _) = coreWith(fake)
        core.setSource(PlayerSource.Url("https://example.com/a.mp4"))
        core.setSource(PlayerSource.Url("https://example.com/b.mp4"))
        assertEquals(1, fake.createCount)
        assertEquals(
            listOf("setSource:https://example.com/a.mp4", "stop", "setSource:https://example.com/b.mp4"),
            fake.calls.filter { it.startsWith("setSource") || it == "play" || it == "stop" },
        )
    }

    @Test
    fun playPauseSeekAreForwarded() {
        val fake = FakeUrlEngine()
        val (core, _) = coreWith(fake)
        core.setSource(PlayerSource.Url("https://example.com/a.mp4"))
        core.play()
        core.pause()
        core.seek(1500)
        assertTrue(fake.calls.contains("play"))
        assertTrue(fake.calls.contains("pause"))
        assertTrue(fake.calls.contains("seek:1500"))
    }

    @Test
    fun firstFrameRenderedIsEmitted() {
        val fake = FakeUrlEngine()
        val (core, events) = coreWith(fake)
        core.setSource(PlayerSource.Url("https://example.com/a.mp4"))
        fake.emit(EngineEvent.FirstFrameRendered)
        assertTrue(events.any { it is PlayerEvent.FirstFrameRendered })
    }

    @Test
    fun playAfterEndedSeeksToZero() {
        val fake = FakeUrlEngine()
        val (core, _) = coreWith(fake)
        core.setSource(PlayerSource.Url("https://example.com/a.mp4"))
        fake.emit(EngineEvent.Ended)
        core.play()
        assertTrue(fake.calls.contains("seek:0"))
        assertTrue(fake.calls.contains("play"))
    }

    @Test
    fun teardownDetachesThenReleases() {
        val fake = FakeUrlEngine()
        val (core, _) = coreWith(fake)
        val token = SurfaceToken("url", 0, "owner")
        core.setSurfaceToken(token)
        core.setSource(PlayerSource.Url("https://example.com/a.mp4"))
        core.release()
        val detachAt = fake.calls.indexOfFirst { it.startsWith("detachSurface") }
        val releaseAt = fake.calls.indexOf("release")
        assertTrue(detachAt >= 0)
        assertTrue(releaseAt > detachAt)
    }

    @Test
    fun setSurfaceTokenSameInstanceIsNoOp() {
        val fake = FakeUrlEngine()
        val (core, _) = coreWith(fake)
        val token = SurfaceToken("url", 0, "owner")
        core.setSource(PlayerSource.Url("https://example.com/a.mp4"))
        core.setSurfaceToken(token)
        val attachCount = fake.calls.count { it.startsWith("attachSurface") }
        core.setSurfaceToken(token)
        assertEquals(attachCount, fake.calls.count { it.startsWith("attachSurface") })
        assertEquals(0, fake.calls.count { it.startsWith("detachSurface") })
    }

    @Test
    fun feedToUrlAttachesOnceOnNewUrlEngine() {
        val url = FakeUrlEngine()
        val feed = FakeUrlEngine()
        val events = mutableListOf<PlayerEvent>()
        val core = PlayerCore(
            createUrlEngine = { url },
            createFeedEngine = { feed },
            emit = { events += it },
        )
        val feedToken = SurfaceToken("feed", 1, "owner")
        val urlToken = SurfaceToken("url", 0, "owner")
        core.setSurfaceToken(feedToken)
        core.setSource(PlayerSource.Feed("session"))
        core.setSurfaceToken(urlToken)
        core.setSource(PlayerSource.Url("https://example.com/a.mp4"))
        assertEquals(1, url.calls.count { it.startsWith("attachSurface") })
        assertEquals(0, url.calls.count { it.startsWith("detachSurface") })
        assertTrue(feed.calls.contains("release"))
        assertTrue(feed.calls.any { it.startsWith("detachSurface") })
    }

    @Test
    fun playlistItemChangeReattachesSameToken() {
        val fake = FakeUrlEngine()
        val (core, _) = coreWith(fake)
        val token = SurfaceToken("url", 0, "owner")
        core.setSurfaceToken(token)
        core.setSource(PlayerSource.Url("https://example.com/a.mp4"))
        core.setSource(PlayerSource.Url("https://example.com/b.mp4"))
        assertEquals(1, fake.createCount)
        assertEquals(2, fake.calls.count { it.startsWith("attachSurface") })
    }
}

internal class FakeUrlEngine : PlayerEngine {
    var createCount: Int = 0
    val calls = mutableListOf<String>()
    private var listener: EngineListener? = null

    override var capabilities: PlayerCapabilities = PlayerCapabilities(supportsRate = true)
        private set

    override fun setListener(listener: EngineListener?) {
        this.listener = listener
        calls += "setListener:${listener != null}"
    }

    override fun setSource(source: PlayerSource) {
        val url = (source as? PlayerSource.Url)?.url ?: return
        calls += "setSource:$url"
    }

    override fun attachSurface(token: SurfaceToken) {
        calls += "attachSurface:${token.id}:${token.generation}"
    }

    override fun detachSurface(token: SurfaceToken) {
        calls += "detachSurface:${token.id}:${token.generation}"
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

    override fun setDurationMs(durationMs: Long?) {
        calls += "setDurationMs:$durationMs"
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
        calls += "release"
    }

    fun emit(event: EngineEvent) {
        listener?.onEngineEvent(event)
    }
}
