package com.lingxia.lxapp.APIs.media

import android.animation.ValueAnimator
import android.content.Context
import android.graphics.Matrix
import android.graphics.RectF
import android.graphics.drawable.Drawable
import android.util.AttributeSet
import android.view.GestureDetector
import android.view.MotionEvent
import android.view.ScaleGestureDetector
import android.view.ViewConfiguration
import android.view.View
import kotlin.math.abs
import kotlin.math.max
import kotlin.math.min

class ZoomImageView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null
) : androidx.appcompat.widget.AppCompatImageView(context, attrs), View.OnTouchListener {

    private val baseMatrix = Matrix()
    private val suppMatrix = Matrix()
    private val displayMatrix = Matrix()
    private val displayRect = RectF()
    private val matrixValues = FloatArray(9)

    private val scaleDetector = ScaleGestureDetector(context, ScaleListener())
    private val gestureDetector = GestureDetector(context, GestureListener())

    private var lastTouchX = 0f
    private var lastTouchY = 0f
    private var downTouchX = 0f
    private var downTouchY = 0f
    private var isDragging = false
    private var isScaling = false

    private var minScale = 1f
    private var maxScale = 4f
    private var fitMode = LxMediaObjectFit.CONTAIN
    private var previewRotation = 0

    private var dismissListener: (() -> Unit)? = null
    private var scaleStateListener: ((Boolean) -> Unit)? = null
    private var tapToDismissEnabled = true
    private val touchSlop = ViewConfiguration.get(context).scaledTouchSlop.toFloat()
    private val tapDismissMaxDurationMs = 280L
    private var downEventTime = 0L
    private var gestureExceededSlop = false

    init {
        scaleType = ScaleType.MATRIX
        imageMatrix = Matrix()
        setOnTouchListener(this)
    }

    fun setDismissListener(listener: (() -> Unit)?) {
        dismissListener = listener
    }

    fun setTapToDismissEnabled(enabled: Boolean) {
        tapToDismissEnabled = enabled
    }

    fun setOnScaleStateListener(listener: ((Boolean) -> Unit)?) {
        scaleStateListener = listener
        listener?.invoke(isZoomed())
    }

    fun setPreviewObjectFit(value: LxMediaObjectFit?) {
        val next = value ?: LxMediaObjectFit.CONTAIN
        if (fitMode == next) return
        fitMode = next
        configureMatrix()
    }

    fun setPreviewRotationDegrees(value: Int?) {
        val next = when (value) {
            0, 90, 180, 270 -> value
            else -> 0
        }
        if (previewRotation == next) return
        previewRotation = next
        configureMatrix()
    }

    override fun setImageDrawable(drawable: Drawable?) {
        super.setImageDrawable(drawable)
        configureMatrix()
    }

    override fun setImageBitmap(bm: android.graphics.Bitmap?) {
        super.setImageBitmap(bm)
        configureMatrix()
    }

    override fun setImageResource(resId: Int) {
        super.setImageResource(resId)
        configureMatrix()
    }

    override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
        super.onSizeChanged(w, h, oldw, oldh)
        configureMatrix()
    }

    private fun configureMatrix() {
        val d = drawable ?: return
        val viewWidth = width.toFloat()
        val viewHeight = height.toFloat()
        if (viewWidth <= 0f || viewHeight <= 0f) return

        baseMatrix.reset()
        suppMatrix.reset()

        val drawableWidth = d.intrinsicWidth.toFloat()
        val drawableHeight = d.intrinsicHeight.toFloat()
        if (drawableWidth <= 0f || drawableHeight <= 0f) {
            imageMatrix = baseMatrix
            return
        }

        val rotatedWidth = if (previewRotation == 90 || previewRotation == 270) drawableHeight else drawableWidth
        val rotatedHeight = if (previewRotation == 90 || previewRotation == 270) drawableWidth else drawableHeight
        val scaleXRatio = viewWidth / rotatedWidth
        val scaleYRatio = viewHeight / rotatedHeight
        val (baseScaleX, baseScaleY) = when (fitMode) {
            LxMediaObjectFit.COVER -> {
                val scale = max(scaleXRatio, scaleYRatio)
                scale to scale
            }
            LxMediaObjectFit.FILL -> {
                scaleXRatio to scaleYRatio
            }
            LxMediaObjectFit.CONTAIN, LxMediaObjectFit.FIT -> {
                val scale = min(scaleXRatio, scaleYRatio)
                scale to scale
            }
        }

        baseMatrix.postTranslate(-drawableWidth / 2f, -drawableHeight / 2f)
        if (previewRotation != 0) {
            baseMatrix.postRotate(previewRotation.toFloat())
        }
        baseMatrix.postScale(baseScaleX, baseScaleY)
        baseMatrix.postTranslate(viewWidth / 2f, viewHeight / 2f)

        minScale = min(baseScaleX, baseScaleY).coerceAtLeast(0.01f)
        maxScale = max(minScale * 4f, minScale + 0.01f)

        applyMatrix()
        notifyScaleState()
    }

    override fun onTouch(v: View?, event: MotionEvent): Boolean {
        if (drawable == null) return false

        // Feed gesture detectors first
        scaleDetector.onTouchEvent(event)
        if (!isScaling) {
            gestureDetector.onTouchEvent(event)
        }

        // Strengthen disallow intercept logic for better pinch zooming in RecyclerView
        val multiTouch = event.pointerCount > 1
        val shouldDisallow = multiTouch || isScaling || isZoomed()

        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                lastTouchX = event.x
                lastTouchY = event.y
                downTouchX = event.x
                downTouchY = event.y
                downEventTime = event.eventTime
                gestureExceededSlop = false
                isDragging = true
                parent?.requestDisallowInterceptTouchEvent(shouldDisallow)
            }
            MotionEvent.ACTION_POINTER_DOWN -> {
                isDragging = false
                gestureExceededSlop = true
                parent?.requestDisallowInterceptTouchEvent(true)
            }
            MotionEvent.ACTION_MOVE -> {
                parent?.requestDisallowInterceptTouchEvent(shouldDisallow)
                if (isDragging && event.pointerCount == 1) {
                    val dx = event.x - lastTouchX
                    val dy = event.y - lastTouchY
                    if (!gestureExceededSlop && (abs(event.x - downTouchX) > touchSlop || abs(event.y - downTouchY) > touchSlop)) {
                        gestureExceededSlop = true
                    }
                    if (abs(dx) > 1f || abs(dy) > 1f) {
                        suppMatrix.postTranslate(dx, dy)
                        constrain()
                        applyMatrix()
                        lastTouchX = event.x
                        lastTouchY = event.y
                    }
                }
            }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                if (event.actionMasked == MotionEvent.ACTION_CANCEL) {
                    gestureExceededSlop = true
                }
                isDragging = false
                parent?.requestDisallowInterceptTouchEvent(false)
                if (getCurrentScale() < minScale) {
                    animateScale(getCurrentScale(), minScale, width / 2f, height / 2f)
                }
            }
        }

        return true
    }

    private fun applyMatrix() {
        displayMatrix.set(baseMatrix)
        displayMatrix.postConcat(suppMatrix)
        imageMatrix = displayMatrix
        invalidate()
        notifyScaleState()
    }

    private fun getCurrentScale(): Float {
        suppMatrix.getValues(matrixValues)
        val suppScale = matrixValues[Matrix.MSCALE_X]
        return minScale * suppScale
    }

    private fun constrain() {
        val rect = getDisplayRect() ?: return
        var deltaX = 0f
        var deltaY = 0f
        val viewWidth = width.toFloat()
        val viewHeight = height.toFloat()

        if (rect.width() <= viewWidth) {
            deltaX = viewWidth / 2f - rect.centerX()
        } else {
            if (rect.left > 0) deltaX = -rect.left
            else if (rect.right < viewWidth) deltaX = viewWidth - rect.right
        }

        if (rect.height() <= viewHeight) {
            deltaY = viewHeight / 2f - rect.centerY()
        } else {
            if (rect.top > 0) deltaY = -rect.top
            else if (rect.bottom < viewHeight) deltaY = viewHeight - rect.bottom
        }

        suppMatrix.postTranslate(deltaX, deltaY)
    }

    private fun getDisplayRect(): RectF? {
        val d = drawable ?: return null
        displayRect.set(0f, 0f, d.intrinsicWidth.toFloat(), d.intrinsicHeight.toFloat())
        displayMatrix.set(baseMatrix)
        displayMatrix.postConcat(suppMatrix)
        displayMatrix.mapRect(displayRect)
        return displayRect
    }

    private inner class ScaleListener : ScaleGestureDetector.SimpleOnScaleGestureListener() {
        override fun onScaleBegin(detector: ScaleGestureDetector): Boolean {
            isScaling = true
            parent?.requestDisallowInterceptTouchEvent(true)
            return true
        }

        override fun onScaleEnd(detector: ScaleGestureDetector) {
            isScaling = false
            if (getCurrentScale() < minScale) {
                animateScale(getCurrentScale(), minScale, width / 2f, height / 2f)
            } else if (getCurrentScale() > maxScale) {
                animateScale(getCurrentScale(), maxScale, width / 2f, height / 2f)
            }
        }

        override fun onScale(detector: ScaleGestureDetector): Boolean {
            val currentScale = getCurrentScale()
            val targetScale = (currentScale * detector.scaleFactor).coerceIn(minScale, maxScale)
            val scaleFactor = targetScale / currentScale
            suppMatrix.postScale(scaleFactor, scaleFactor, detector.focusX, detector.focusY)
            constrain()
            applyMatrix()
            return true
        }
    }

    private inner class GestureListener : GestureDetector.SimpleOnGestureListener() {
        override fun onDoubleTap(e: MotionEvent): Boolean {
            parent?.requestDisallowInterceptTouchEvent(true)
            val currentScale = getCurrentScale()
            val target = if (currentScale > minScale + 0.05f) minScale else min(maxScale, minScale * 2f)
            animateScale(currentScale, target, e.x, e.y)
            return true
        }

        override fun onSingleTapConfirmed(e: MotionEvent): Boolean {
            val deltaX = abs(e.x - downTouchX)
            val deltaY = abs(e.y - downTouchY)
            val tapDurationMs = e.eventTime - downEventTime
            if (!tapToDismissEnabled
                || gestureExceededSlop
                || deltaX > touchSlop
                || deltaY > touchSlop
                || tapDurationMs > tapDismissMaxDurationMs
            ) {
                return false
            }
            dismissListener?.invoke()
            return true
        }
    }

    private fun animateScale(from: Float, to: Float, pivotX: Float, pivotY: Float) {
        if (from == to) return
        val animator = ValueAnimator.ofFloat(from, to)
        animator.duration = 200
        animator.addUpdateListener { valueAnimator ->
            val current = valueAnimator.animatedValue as Float
            val scaleFactor = (current / getCurrentScale()).coerceIn(0.5f, 2f)
            suppMatrix.postScale(scaleFactor, scaleFactor, pivotX, pivotY)
            constrain()
            applyMatrix()
        }
        animator.start()
    }

    private fun isZoomed(): Boolean = getCurrentScale() > minScale + 0.05f

    private fun notifyScaleState() {
        scaleStateListener?.invoke(isZoomed())
    }
}
