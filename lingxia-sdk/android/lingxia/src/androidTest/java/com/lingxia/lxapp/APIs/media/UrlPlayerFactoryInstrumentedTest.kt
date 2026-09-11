package com.lingxia.lxapp.APIs.media

import android.os.Build
import android.view.SurfaceView
import android.view.TextureView
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.lingxia.app.Lingxia
import com.lingxia.app.media.UrlPlayerEngine
import com.lingxia.app.media.UrlPlayerEngineFactory
import com.lingxia.app.media.UrlPlayerEngineRequest
import com.lingxia.app.media.UrlPlayerOutputKind
import com.lingxia.app.media.UrlPlayerSurfaceKind
import com.lingxia.lxapp.APIs.media.player.RecordingUrlPlayerEngine
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class UrlPlayerFactoryInstrumentedTest {
    private val context = ApplicationProvider.getApplicationContext<android.content.Context>()

    @After
    fun tearDown() {
        runOnMain { Lingxia.setUrlPlayerEngineFactory(null) }
    }

    @Test
    fun noFactoryUsesTextureView() {
        runOnMain { Lingxia.setUrlPlayerEngineFactory(null) }
        lateinit var player: LxMediaPlayer
        runOnMain {
            player = LxMediaPlayer(context, eventSink = {})
        }
        try {
            assertTrue(player.urlOutputViewForTest() is TextureView)
            assertTrue(player.urlOutputHonorsAlpha())
        } finally {
            runOnMain { player.detach() }
        }
    }

    @Test
    fun inlineFeedOnlyDoesNotCreateHostEngine() {
        val factory = CountingFactory { RecordingUrlPlayerEngine() }
        runOnMain { Lingxia.setUrlPlayerEngineFactory(factory) }
        lateinit var player: LxMediaPlayer
        runOnMain {
            player = LxMediaPlayer(
                context,
                eventSink = {},
                componentId = "feed-only",
                urlSurfaceKind = UrlPlayerSurfaceKind.INLINE,
            )
            player.acquireStreamTextureView()
            player.detach()
        }
        assertEquals(0, factory.createCount)
    }

    @Test
    fun previewSurfaceViewNullCreateFallsBackToTextureView() {
        val factory = object : UrlPlayerEngineFactory {
            override fun preferredOutput(kind: UrlPlayerSurfaceKind) =
                UrlPlayerOutputKind.SURFACE_VIEW

            override fun create(request: UrlPlayerEngineRequest): UrlPlayerEngine? = null
        }
        runOnMain { Lingxia.setUrlPlayerEngineFactory(factory) }
        lateinit var player: LxMediaPlayer
        runOnMain {
            player = LxMediaPlayer(
                context,
                eventSink = {},
                urlSurfaceKind = UrlPlayerSurfaceKind.PREVIEW,
            )
        }
        try {
            assertTrue(player.urlOutputViewForTest() is TextureView)
        } finally {
            runOnMain { player.detach() }
        }
    }

    @Test
    fun previewSurfaceViewEagerPendingIsReleasedWithoutDetach() {
        assumeTrue(Build.VERSION.SDK_INT >= 24)
        val engine = RecordingUrlPlayerEngine()
        val factory = object : UrlPlayerEngineFactory {
            override fun preferredOutput(kind: UrlPlayerSurfaceKind) =
                UrlPlayerOutputKind.SURFACE_VIEW

            override fun create(request: UrlPlayerEngineRequest): UrlPlayerEngine = engine
        }
        runOnMain { Lingxia.setUrlPlayerEngineFactory(factory) }
        lateinit var player: LxMediaPlayer
        runOnMain {
            player = LxMediaPlayer(
                context,
                eventSink = {},
                urlSurfaceKind = UrlPlayerSurfaceKind.PREVIEW,
            )
            assertTrue(player.urlOutputViewForTest() is SurfaceView)
            player.detach()
        }
        assertEquals(1, engine.releaseCount)
        assertEquals(0, engine.detachOutputCount)
    }

    @Test
    fun snapshotIgnoresLaterFactoryChange() {
        val first = CountingFactory { RecordingUrlPlayerEngine() }
        val second = CountingFactory { RecordingUrlPlayerEngine() }
        runOnMain { Lingxia.setUrlPlayerEngineFactory(first) }
        lateinit var player: LxMediaPlayer
        runOnMain {
            player = LxMediaPlayer(
                context,
                eventSink = {},
                urlSurfaceKind = UrlPlayerSurfaceKind.INLINE,
            )
            Lingxia.setUrlPlayerEngineFactory(second)
            player.update(
                LxMediaPlayerConfig(src = "https://example.com/a.mp4")
            )
            player.detach()
        }
        assertEquals(1, first.createCount)
        assertEquals(0, second.createCount)
    }

    @Test
    fun previewSurfaceViewHasDefaultZOrder() {
        assumeTrue(Build.VERSION.SDK_INT >= 24)
        val factory = object : UrlPlayerEngineFactory {
            override fun preferredOutput(kind: UrlPlayerSurfaceKind) =
                UrlPlayerOutputKind.SURFACE_VIEW

            override fun create(request: UrlPlayerEngineRequest): UrlPlayerEngine =
                RecordingUrlPlayerEngine()
        }
        runOnMain { Lingxia.setUrlPlayerEngineFactory(factory) }
        lateinit var player: LxMediaPlayer
        runOnMain {
            player = LxMediaPlayer(
                context,
                eventSink = {},
                urlSurfaceKind = UrlPlayerSurfaceKind.PREVIEW,
            )
        }
        try {
            assertTrue(player.urlOutputViewForTest() is SurfaceView)
            assertTrue(!player.urlOutputHonorsAlpha())
        } finally {
            runOnMain { player.detach() }
        }
    }

    private fun runOnMain(block: () -> Unit) {
        InstrumentationRegistry.getInstrumentation().runOnMainSync(block)
    }

    private class CountingFactory(
        private val engine: () -> UrlPlayerEngine?,
    ) : UrlPlayerEngineFactory {
        var createCount = 0
            private set

        override fun create(request: UrlPlayerEngineRequest): UrlPlayerEngine? {
            createCount += 1
            return engine()
        }
    }
}
