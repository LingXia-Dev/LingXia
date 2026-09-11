package com.lingxia.lxapp.APIs.media.player

import android.view.TextureView
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.lingxia.app.media.UrlPlayerEngineEvent
import com.lingxia.app.media.UrlPlayerOutput
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class HostUrlEngineAdapterTest {
    private fun adapter(host: RecordingUrlPlayerEngine): HostUrlEngineAdapter {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        val output = UrlPlayerOutput.Texture(TextureView(context))
        val token = SurfaceToken("url", 0, "owner")
        return HostUrlEngineAdapter(host, output, token)
    }

    @Test
    fun loopOnDropsEnded() {
        val host = RecordingUrlPlayerEngine()
        val adapter = adapter(host)
        val events = mutableListOf<EngineEvent>()
        adapter.setListener { events += it }
        adapter.setLoopEnabled(true)
        host.emit(UrlPlayerEngineEvent.Ended)
        assertTrue(events.none { it is EngineEvent.Ended })
    }

    @Test
    fun playlistSetSourceDoesNotReattachOutput() {
        val host = RecordingUrlPlayerEngine()
        val adapter = adapter(host)
        val token = SurfaceToken("url", 0, "owner")
        adapter.attachSurface(token)
        adapter.setSource(PlayerSource.Url("https://example.com/a.mp4"))
        adapter.setSource(PlayerSource.Url("https://example.com/b.mp4"))
        adapter.attachSurface(token)
        assertEquals(1, host.attachOutputCount)
        assertEquals(0, host.detachOutputCount)
    }

    @Test
    fun newGenerationTokenDoesNotReattach() {
        val host = RecordingUrlPlayerEngine()
        val adapter = adapter(host)
        adapter.attachSurface(SurfaceToken("url", 0, "owner"))
        adapter.attachSurface(SurfaceToken("url", 1, "owner"))
        assertEquals(1, host.attachOutputCount)
    }

    @Test
    fun setDurationMsIsNoOp() {
        val host = RecordingUrlPlayerEngine()
        val adapter = adapter(host)
        adapter.setDurationMs(1200)
        assertTrue(host.calls.none { it.startsWith("setDuration") })
    }

    @Test
    fun methodThrowBecomesInternalError() {
        val host = object : RecordingUrlPlayerEngine() {
            override fun play() {
                throw IllegalStateException("boom")
            }
        }
        val adapter = adapter(host)
        val events = mutableListOf<EngineEvent>()
        adapter.setListener { events += it }
        adapter.play()
        val error = events.filterIsInstance<EngineEvent.Error>().single()
        assertEquals(ErrorCode.INTERNAL, error.error.code)
    }
}
