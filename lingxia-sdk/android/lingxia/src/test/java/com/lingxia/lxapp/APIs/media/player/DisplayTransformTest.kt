package com.lingxia.lxapp.APIs.media.player

import android.view.ViewGroup
import com.lingxia.lxapp.APIs.media.LxMediaObjectFit
import kotlin.math.roundToInt
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class DisplayTransformTest {
    @Test
    fun nonNinetyRotationReturnsIdentityScale() {
        val scale = DisplayTransform.wrapperRotationScales(
            degrees = 0,
            objectFit = LxMediaObjectFit.COVER,
            containerW = 400f,
            containerH = 800f,
            sourceW = 1920.0,
            sourceH = 1080.0,
        )
        assertEquals(1f, scale.first, 0f)
        assertEquals(1f, scale.second, 0f)

        val scale180 = DisplayTransform.wrapperRotationScales(
            degrees = 180,
            objectFit = LxMediaObjectFit.CONTAIN,
            containerW = 400f,
            containerH = 800f,
            sourceW = 1920.0,
            sourceH = 1080.0,
        )
        assertEquals(1f, scale180.first, 0f)
        assertEquals(1f, scale180.second, 0f)
    }

    @Test
    fun fillNinetySwapsContainerRatio() {
        val scale = DisplayTransform.wrapperRotationScales(
            degrees = 90,
            objectFit = LxMediaObjectFit.FILL,
            containerW = 400f,
            containerH = 800f,
            sourceW = 0.0,
            sourceH = 0.0,
        )
        assertEquals(400f / 800f, scale.first, 0.0001f)
        assertEquals(800f / 400f, scale.second, 0.0001f)
    }

    @Test
    fun coverNinetyWithUnknownSizeReturnsNaN() {
        val scale = DisplayTransform.wrapperRotationScales(
            degrees = 90,
            objectFit = LxMediaObjectFit.COVER,
            containerW = 400f,
            containerH = 800f,
            sourceW = 0.0,
            sourceH = 0.0,
        )
        assertTrue(scale.first.isNaN())
        assertTrue(scale.second.isNaN())
    }

    @Test
    fun coverNinetyWrapperScaleMatchesRotatedOverBase() {
        val containerW = 400.0
        val containerH = 800.0
        val sourceW = 1920.0
        val sourceH = 1080.0
        val base = DisplayTransform.fitScale(
            LxMediaObjectFit.COVER, sourceW, sourceH, containerW, containerH,
        )
        val rotated = DisplayTransform.fitScale(
            LxMediaObjectFit.COVER, sourceH, sourceW, containerW, containerH,
        )
        val expected = (rotated / base).toFloat()
        val scale = DisplayTransform.wrapperRotationScales(
            degrees = 90,
            objectFit = LxMediaObjectFit.COVER,
            containerW = containerW.toFloat(),
            containerH = containerH.toFloat(),
            sourceW = sourceW,
            sourceH = sourceH,
        )
        assertEquals(expected, scale.first, 0.0001f)
        assertEquals(expected, scale.second, 0.0001f)
    }

    @Test
    fun fillInnerLayoutIsMatchParentEvenWhenSizeUnknown() {
        val layout = DisplayTransform.innerLayout(
            objectFit = LxMediaObjectFit.FILL,
            containerW = 400,
            containerH = 800,
            sourceW = 0.0,
            sourceH = 0.0,
        )
        assertEquals(ViewGroup.LayoutParams.MATCH_PARENT, layout!!.width)
        assertEquals(ViewGroup.LayoutParams.MATCH_PARENT, layout.height)
    }

    @Test
    fun coverInnerLayoutSkipsWhenSizeUnknown() {
        assertNull(
            DisplayTransform.innerLayout(
                objectFit = LxMediaObjectFit.COVER,
                containerW = 400,
                containerH = 800,
                sourceW = 0.0,
                sourceH = 0.0,
            )
        )
    }

    @Test
    fun coverNinetyInnerSizeUsesMaxScale() {
        val containerW = 400
        val containerH = 800
        val sourceW = 1920.0
        val sourceH = 1080.0
        val scale = kotlin.math.max(containerW / sourceW, containerH / sourceH)
        val layout = DisplayTransform.innerLayout(
            objectFit = LxMediaObjectFit.COVER,
            containerW = containerW,
            containerH = containerH,
            sourceW = sourceW,
            sourceH = sourceH,
        )!!
        assertEquals((sourceW * scale).roundToInt().coerceAtLeast(1), layout.width)
        assertEquals((sourceH * scale).roundToInt().coerceAtLeast(1), layout.height)
    }
}
