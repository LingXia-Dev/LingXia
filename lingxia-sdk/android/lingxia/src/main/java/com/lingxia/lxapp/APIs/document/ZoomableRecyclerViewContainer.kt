package com.lingxia.lxapp.APIs.document

import android.animation.ValueAnimator
import android.content.Context
import android.graphics.Canvas
import android.util.AttributeSet
import android.view.GestureDetector
import android.view.MotionEvent
import android.view.ScaleGestureDetector
import android.widget.FrameLayout
import kotlin.math.abs

/**
 * A container that wraps a RecyclerView and provides pinch-to-zoom functionality
 * for the entire content, similar to iOS QLPreviewController behavior.
 *
 * Uses Canvas-based scaling to avoid rendering artifacts.
 */
class ZoomableRecyclerViewContainer @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null
) : FrameLayout(context, attrs) {

    private val scaleDetector = ScaleGestureDetector(context, ScaleListener())
    private val gestureDetector = GestureDetector(context, GestureListener())

    private var currentScale = 1f
    private var translateX = 0f
    private var translateY = 0f

    private var lastTouchX = 0f
    private var lastTouchY = 0f
    private var isDragging = false
    private var isScaling = false

    private val minScale = 1f
    private val maxScale = 4f

    init {
        setWillNotDraw(false)
    }

    override fun dispatchDraw(canvas: Canvas) {
        if (currentScale == 1f && translateX == 0f && translateY == 0f) {
            super.dispatchDraw(canvas)
            return
        }

        canvas.save()

        // Apply transform: translate then scale from center
        val centerX = width / 2f
        val centerY = height / 2f

        canvas.translate(centerX + translateX, centerY + translateY)
        canvas.scale(currentScale, currentScale)
        canvas.translate(-centerX, -centerY)

        super.dispatchDraw(canvas)
        canvas.restore()
    }

    override fun dispatchTouchEvent(ev: MotionEvent): Boolean {
        // Transform touch coordinates when zoomed
        val transformedEvent = if (isZoomed()) {
            transformTouchEvent(ev)
        } else {
            ev
        }

        // Always feed gesture detectors with original event
        scaleDetector.onTouchEvent(ev)

        val multiTouch = ev.pointerCount > 1
        val zoomed = isZoomed()

        when (ev.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                lastTouchX = ev.x
                lastTouchY = ev.y
                isDragging = false
                isScaling = false
            }
            MotionEvent.ACTION_POINTER_DOWN -> {
                isScaling = true
                isDragging = false
            }
            MotionEvent.ACTION_MOVE -> {
                if (multiTouch || isScaling) {
                    return true
                }

                if (zoomed && !isScaling) {
                    val dx = ev.x - lastTouchX
                    val dy = ev.y - lastTouchY

                    if (!isDragging && (abs(dx) > 10 || abs(dy) > 10)) {
                        isDragging = true
                    }

                    if (isDragging) {
                        translateX += dx
                        translateY += dy
                        constrainTranslation()
                        invalidate()
                        lastTouchX = ev.x
                        lastTouchY = ev.y
                        return true
                    }
                }
            }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                if (isScaling) {
                    isScaling = false
                    if (currentScale < minScale) {
                        animateScale(currentScale, minScale)
                    }
                }
                isDragging = false

                if (!multiTouch) {
                    gestureDetector.onTouchEvent(ev)
                }
            }
        }

        if (multiTouch || isScaling) {
            return true
        }

        if (zoomed && isDragging) {
            return true
        }

        // Pass transformed event to children when zoomed
        return if (zoomed && transformedEvent !== ev) {
            super.dispatchTouchEvent(transformedEvent)
        } else {
            super.dispatchTouchEvent(ev)
        }
    }

    private fun transformTouchEvent(ev: MotionEvent): MotionEvent {
        val centerX = width / 2f
        val centerY = height / 2f

        // Inverse transform: touch point to content coordinates
        val transformedX = (ev.x - centerX - translateX) / currentScale + centerX
        val transformedY = (ev.y - centerY - translateY) / currentScale + centerY

        return MotionEvent.obtain(
            ev.downTime,
            ev.eventTime,
            ev.action,
            transformedX,
            transformedY,
            ev.metaState
        )
    }

    override fun onInterceptTouchEvent(ev: MotionEvent): Boolean {
        val multiTouch = ev.pointerCount > 1
        val zoomed = isZoomed()

        when (ev.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                lastTouchX = ev.x
                lastTouchY = ev.y
            }
            MotionEvent.ACTION_MOVE -> {
                if (zoomed) {
                    val dx = abs(ev.x - lastTouchX)
                    val dy = abs(ev.y - lastTouchY)
                    if (dx > 10 || dy > 10) {
                        return true
                    }
                }
            }
        }

        return multiTouch || isScaling
    }

    private fun constrainTranslation() {
        if (currentScale <= 1f) {
            translateX = 0f
            translateY = 0f
            return
        }

        val scaledWidth = width * currentScale
        val scaledHeight = height * currentScale

        val maxTranslateX = (scaledWidth - width) / 2f
        val maxTranslateY = (scaledHeight - height) / 2f

        translateX = translateX.coerceIn(-maxTranslateX, maxTranslateX)
        translateY = translateY.coerceIn(-maxTranslateY, maxTranslateY)
    }

    private fun isZoomed(): Boolean = currentScale > 1.05f

    private inner class ScaleListener : ScaleGestureDetector.SimpleOnScaleGestureListener() {
        private var lastFocusX = 0f
        private var lastFocusY = 0f

        override fun onScaleBegin(detector: ScaleGestureDetector): Boolean {
            isScaling = true
            lastFocusX = detector.focusX
            lastFocusY = detector.focusY
            return true
        }

        override fun onScale(detector: ScaleGestureDetector): Boolean {
            val prevScale = currentScale
            val scaleFactor = detector.scaleFactor
            val newScale = (currentScale * scaleFactor).coerceIn(minScale * 0.8f, maxScale)

            val centerX = width / 2f
            val centerY = height / 2f

            // Keep focus point stable during scale
            val focusDeltaX = detector.focusX - lastFocusX
            val focusDeltaY = detector.focusY - lastFocusY

            val scaleChange = newScale / prevScale
            val focusOffsetX = detector.focusX - centerX - translateX
            val focusOffsetY = detector.focusY - centerY - translateY

            translateX += focusOffsetX * (1 - scaleChange) + focusDeltaX
            translateY += focusOffsetY * (1 - scaleChange) + focusDeltaY

            currentScale = newScale
            constrainTranslation()
            invalidate()

            lastFocusX = detector.focusX
            lastFocusY = detector.focusY
            return true
        }

        override fun onScaleEnd(detector: ScaleGestureDetector) {
            isScaling = false
            if (currentScale < minScale) {
                animateScale(currentScale, minScale)
            } else if (currentScale > maxScale) {
                animateScale(currentScale, maxScale)
            }
        }
    }

    private inner class GestureListener : GestureDetector.SimpleOnGestureListener() {
        override fun onDoubleTap(e: MotionEvent): Boolean {
            val targetScale = if (currentScale > 1.1f) minScale else 2f

            if (targetScale > minScale) {
                val centerX = width / 2f
                val centerY = height / 2f
                val zoomRatio = targetScale / currentScale
                translateX = (centerX - e.x) * (zoomRatio - 1)
                translateY = (centerY - e.y) * (zoomRatio - 1)
            }

            animateScale(currentScale, targetScale)
            return true
        }
    }

    private fun animateScale(from: Float, to: Float) {
        if (from == to) return

        val startTranslateX = translateX
        val startTranslateY = translateY

        ValueAnimator.ofFloat(from, to).apply {
            duration = 200
            addUpdateListener { animator ->
                currentScale = animator.animatedValue as Float

                if (to <= minScale) {
                    val progress = animator.animatedFraction
                    translateX = startTranslateX * (1 - progress)
                    translateY = startTranslateY * (1 - progress)
                } else {
                    constrainTranslation()
                }

                invalidate()
            }
            start()
        }
    }

    fun resetZoom() {
        animateScale(currentScale, minScale)
    }
}
