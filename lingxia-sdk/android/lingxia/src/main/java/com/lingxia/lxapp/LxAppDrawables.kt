package com.lingxia.lxapp

import android.content.res.Resources
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.drawable.Drawable
import android.widget.ImageButton
import android.widget.ImageView
import androidx.appcompat.widget.Toolbar
import androidx.core.content.ContextCompat

object LxAppDrawables {

    object Constants {
        const val BUTTON_SIZE_DP = 32
        const val ANIMATION_DURATION_MS = 300L
        const val BACK_ICON_PADDING_DP = 6
    }

    private fun configureNavButton(
        imageView: ImageView,
        iconResId: Int,
        paddingDp: Int
    ) {
        val context = imageView.context
        val density = context.resources.displayMetrics.density
        val paddingPx = (paddingDp * density).toInt()

        imageView.apply {
            setImageResource(iconResId)
            scaleType = ImageView.ScaleType.CENTER_INSIDE
            setPadding(paddingPx, paddingPx, paddingPx, paddingPx)
        }
    }

    fun configureBackButton(imageView: ImageView, paddingDp: Int = Constants.BACK_ICON_PADDING_DP) {
        configureNavButton(imageView, R.drawable.icon_back, paddingDp)
    }

    fun configureHomeButton(imageView: ImageView, paddingDp: Int = Constants.BACK_ICON_PADDING_DP) {
        configureNavButton(imageView, R.drawable.icon_home, paddingDp)
    }

    fun configureToolbarBackButton(toolbar: Toolbar, paddingDp: Int = 12) {
        val context = toolbar.context
        val density = context.resources.displayMetrics.density
        val paddingPx = (paddingDp * density).toInt()
        ContextCompat.getDrawable(context, R.drawable.icon_back)?.mutate()?.let {
            toolbar.navigationIcon = it
        }

        for (i in 0 until toolbar.childCount) {
            val child = toolbar.getChildAt(i)
            if (child is ImageButton) {
                child.apply {
                    scaleType = ImageView.ScaleType.CENTER_INSIDE
                    setPadding(paddingPx, paddingPx, paddingPx, paddingPx)
                }
                break
            }
        }
    }


    /**
     * More Dots Drawable for capsule button
     */
    class MoreDotsDrawable(
        private var currentColor: Int = Color.BLACK
    ) : Drawable() {
        private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = currentColor
            style = Paint.Style.FILL
        }

        override fun draw(canvas: Canvas) {
            val centerY = bounds.height() / 2f
            val centerX = bounds.width() / 2f

            // Center dot is larger, side dots are smaller
            val centerDotRadius = bounds.height() / 5.5f  // Larger center dot
            val sideDotRadius = bounds.height() / 10f   // Smaller side dots
            val spacing = centerDotRadius * 2.5f        // Adjusted spacing

            // Draw side dots
            canvas.drawCircle(centerX - spacing, centerY, sideDotRadius, paint)
            canvas.drawCircle(centerX + spacing, centerY, sideDotRadius, paint)

            // Draw center dot (larger)
            canvas.drawCircle(centerX, centerY, centerDotRadius, paint)
        }

        override fun setAlpha(alpha: Int) {
            paint.alpha = alpha
        }

        override fun setColorFilter(colorFilter: android.graphics.ColorFilter?) {
            paint.colorFilter = colorFilter
        }

        @Deprecated("Deprecated in Java")
        override fun getOpacity(): Int = android.graphics.PixelFormat.TRANSLUCENT

        fun updateColor(color: Int) {
            currentColor = color
            paint.color = color
            invalidateSelf()
        }
    }

    /**
     * Close Button Drawable for capsule button
     */
    class CloseButtonDrawable(
        private val resources: Resources,
        private var currentColor: Int = Color.BLACK
    ) : Drawable() {
        private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = currentColor
            style = Paint.Style.STROKE
            strokeWidth = 3f * resources.displayMetrics.density
        }

        private val dotPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = currentColor
            style = Paint.Style.FILL
        }

        override fun draw(canvas: Canvas) {
            val centerX = bounds.width() / 2f
            val centerY = bounds.height() / 2f
            val radius = bounds.width() / 2.2f

            // Draw circle with thicker stroke
            paint.style = Paint.Style.STROKE
            canvas.drawCircle(centerX, centerY, radius, paint)

            // Draw smaller center dot
            paint.style = Paint.Style.FILL
            canvas.drawCircle(centerX, centerY, radius / 3f, dotPaint)
        }

        override fun setAlpha(alpha: Int) {
            paint.alpha = alpha
            dotPaint.alpha = alpha
        }

        override fun setColorFilter(colorFilter: android.graphics.ColorFilter?) {
            paint.colorFilter = colorFilter
            dotPaint.colorFilter = colorFilter
        }

        @Deprecated("Deprecated in Java")
        override fun getOpacity(): Int = android.graphics.PixelFormat.TRANSLUCENT

        fun updateColor(color: Int) {
            currentColor = color
            paint.color = color
            dotPaint.color = color
            invalidateSelf()
        }
    }

    /**
     * Factory methods for creating drawable instances
     */
    fun createMoreDots(color: Int = Color.BLACK) =
        MoreDotsDrawable(color)

    fun createCloseButton(resources: Resources, color: Int = Color.BLACK) =
        CloseButtonDrawable(resources, color)
}
