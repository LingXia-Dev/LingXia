package com.lingxia.lxapp.APIs.media.player

import android.view.Gravity
import android.view.ViewGroup
import com.lingxia.lxapp.APIs.media.LxMediaObjectFit
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

/**
 * Two-layer URL display transform.
 *
 * Inner [urlOutputView] layoutParams own 0° objectFit (COVER/CONTAIN/FILL) at every
 * content rotation. The wrapper only applies today's 90/270 formula
 * (`rotation` + `rotatedScale/baseScale`).
 */
internal data class UrlInnerLayout(
    val width: Int,
    val height: Int,
    val gravity: Int = Gravity.CENTER,
)

internal object DisplayTransform {
    /**
     * Wrapper scale for a content rotation. Non-90° returns 1,1.
     * FILL×90 swaps the container ratio. Unknown size at 90° COVER/CONTAIN
     * returns NaN so the caller can skip rather than apply a stale fallback.
     */
    fun wrapperRotationScales(
        degrees: Int,
        objectFit: LxMediaObjectFit,
        containerW: Float,
        containerH: Float,
        sourceW: Double,
        sourceH: Double,
    ): Pair<Float, Float> {
        val rotate90 = degrees == 90 || degrees == 270
        if (!rotate90) {
            return 1f to 1f
        }

        if (objectFit == LxMediaObjectFit.FILL) {
            val ratioX = containerW / containerH
            val ratioY = containerH / containerW
            return ratioX to ratioY
        }

        if (sourceW <= 0.0 || sourceH <= 0.0) {
            return Float.NaN to Float.NaN
        }

        val baseScale = fitScale(objectFit, sourceW, sourceH, containerW.toDouble(), containerH.toDouble())
        val rotatedScale = fitScale(objectFit, sourceH, sourceW, containerW.toDouble(), containerH.toDouble())
        if (baseScale <= 0.0 || rotatedScale <= 0.0) {
            return 1f to 1f
        }

        val uniform = (rotatedScale / baseScale).toFloat()
        return uniform to uniform
    }

    /**
     * Inner surface layout. Returns null when COVER/CONTAIN must not change
     * layout (size unknown — same skip as today's NaN wrapper path).
     */
    fun innerLayout(
        objectFit: LxMediaObjectFit,
        containerW: Int,
        containerH: Int,
        sourceW: Double,
        sourceH: Double,
    ): UrlInnerLayout? {
        if (containerW <= 0 || containerH <= 0) return null
        when (objectFit) {
            LxMediaObjectFit.FILL -> {
                return UrlInnerLayout(
                    width = ViewGroup.LayoutParams.MATCH_PARENT,
                    height = ViewGroup.LayoutParams.MATCH_PARENT,
                )
            }
            LxMediaObjectFit.COVER -> {
                if (sourceW <= 0.0 || sourceH <= 0.0) return null
                val scale = max(containerW / sourceW, containerH / sourceH)
                return UrlInnerLayout(
                    width = (sourceW * scale).roundToInt().coerceAtLeast(1),
                    height = (sourceH * scale).roundToInt().coerceAtLeast(1),
                )
            }
            LxMediaObjectFit.CONTAIN, LxMediaObjectFit.FIT -> {
                if (sourceW <= 0.0 || sourceH <= 0.0) return null
                val scale = min(containerW / sourceW, containerH / sourceH)
                return UrlInnerLayout(
                    width = (sourceW * scale).roundToInt().coerceAtLeast(1),
                    height = (sourceH * scale).roundToInt().coerceAtLeast(1),
                )
            }
        }
    }

    fun fitScale(
        objectFit: LxMediaObjectFit,
        sourceW: Double,
        sourceH: Double,
        containerW: Double,
        containerH: Double,
    ): Double {
        if (sourceW <= 0.0 || sourceH <= 0.0 || containerW <= 0.0 || containerH <= 0.0) {
            return 0.0
        }
        val scaleX = containerW / sourceW
        val scaleY = containerH / sourceH
        return when (objectFit) {
            LxMediaObjectFit.COVER -> max(scaleX, scaleY)
            LxMediaObjectFit.CONTAIN, LxMediaObjectFit.FIT -> min(scaleX, scaleY)
            LxMediaObjectFit.FILL -> min(scaleX, scaleY)
        }
    }
}
