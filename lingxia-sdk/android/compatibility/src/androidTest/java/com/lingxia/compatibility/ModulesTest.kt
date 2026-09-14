package com.lingxia.compatibility

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.lingxia.app.media.modules.MediaModules
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

@RunWith(AndroidJUnit4::class)
class ModulesTest {
    @Test fun optionalModulesMatchPackagedClasses() {
        val full = BuildConfig.FLAVOR == "full"
        assertEquals(full, MediaModules.hasCamera)
        assertEquals(full, MediaModules.hasScanner)
        if (!full) {
            assertFalse(runCatching { Class.forName("androidx.camera.core.Camera") }.isSuccess)
            assertFalse(runCatching { Class.forName("com.google.mlkit.vision.barcode.BarcodeScanning") }.isSuccess)
        }
    }

    @Test fun playbackWorksWithoutInitializingLingxiaOrCamera() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val context = instrumentation.targetContext
        val file = File(context.cacheDir, "module-playback.wav")
        val samples = 8000
        val wav = ByteBuffer.allocate(44 + samples * 2).order(ByteOrder.LITTLE_ENDIAN)
        wav.put("RIFF".toByteArray()).putInt(36 + samples * 2).put("WAVEfmt ".toByteArray())
        wav.putInt(16).putShort(1).putShort(1).putInt(8000).putInt(16000).putShort(2).putShort(16)
        wav.put("data".toByteArray()).putInt(samples * 2)
        repeat(samples) { wav.putShort(0) }
        file.writeBytes(wav.array())
        val active = AtomicReference<PlaybackProbe>()
        try {
            instrumentation.runOnMainSync {
                active.set(PlaybackProbe(context, file.toURI().toString()))
            }
            val probe = active.get()
            assertTrue("Playback did not complete", probe.ended.await(15, TimeUnit.SECONDS))
            assertNull(probe.failure.get())
        } finally {
            active.get()?.let { probe -> instrumentation.runOnMainSync { probe.close() } }
            file.delete()
        }
    }
}
