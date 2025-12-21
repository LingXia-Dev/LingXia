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
        const val FROSTED_GLASS_ALPHA = 50
        const val MARGIN_START_DP = 12
        const val ANIMATION_DURATION_MS = 300L
        const val HOME_STROKE_WIDTH_FACTOR = 2.8f
        const val BACK_ICON_PADDING_DP = 6
    }

    fun configureBackButton(imageView: ImageView, paddingDp: Int = Constants.BACK_ICON_PADDING_DP) {
        val context = imageView.context
        val density = context.resources.displayMetrics.density
        val paddingPx = (paddingDp * density).toInt()

        imageView.apply {
            setImageResource(R.drawable.icon_back)
            scaleType = ImageView.ScaleType.CENTER_INSIDE
            setPadding(paddingPx, paddingPx, paddingPx, paddingPx)
        }
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
     * Home Button Drawable with frosted glass effect
     */
    class HomeButtonDrawable(
        private val resources: Resources,
        private var currentFrontColor: Int = Color.BLACK
    ) : Drawable() {

        private val strokePaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            style = Paint.Style.STROKE
            strokeJoin = Paint.Join.ROUND
            strokeCap = Paint.Cap.ROUND
        }

        override fun draw(canvas: Canvas) {
            val width = bounds.width()
            val height = bounds.height()
            val centerX = width / 2f
            val centerY = height / 2f

            // Draw perfect circle background (frosted glass effect)
            val backgroundPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
                color = Color.argb(Constants.FROSTED_GLASS_ALPHA, 255, 255, 255) // Semi-transparent white
                style = Paint.Style.FILL
            }

            val radius = (minOf(width, height) / 2f) - (2f * resources.displayMetrics.density)
            canvas.drawCircle(centerX, centerY, radius, backgroundPaint)

            // Update paint colors for the icon
            strokePaint.color = currentFrontColor
            strokePaint.strokeWidth = Constants.HOME_STROKE_WIDTH_FACTOR * resources.displayMetrics.density
            strokePaint.strokeCap = Paint.Cap.ROUND
            strokePaint.strokeJoin = Paint.Join.ROUND

            // Draw refined home icon with better proportions
            val iconSize = minOf(width, height) * 0.5f
            val houseWidth = iconSize * 0.7f
            val houseHeight = iconSize * 0.45f
            val roofHeight = iconSize * 0.32f

            // Calculate positions for better centering
            val totalHeight = houseHeight + roofHeight
            val iconTop = centerY - totalHeight / 2f
            val iconBottom = centerY + totalHeight / 2f

            val left = centerX - houseWidth / 2f
            val right = centerX + houseWidth / 2f
            val bottom = iconBottom
            val houseTop = iconBottom - houseHeight
            val roofPeak = iconTop

            // Draw house outline
            val housePath = android.graphics.Path().apply {
                moveTo(left, bottom)
                lineTo(left, houseTop)
                lineTo(centerX, roofPeak)
                lineTo(right, houseTop)
                lineTo(right, bottom)
                close()
            }
            canvas.drawPath(housePath, strokePaint)

            // Draw door with better proportions
            val doorWidth = houseWidth * 0.18f
            val doorHeight = houseHeight * 0.55f
            val doorLeft = centerX - doorWidth / 2f
            val doorRight = centerX + doorWidth / 2f
            val doorTop = bottom - doorHeight

            // Draw door with rounded top
            val doorPath = android.graphics.Path().apply {
                val cornerRadius = doorWidth * 0.1f
                moveTo(doorLeft, bottom)
                lineTo(doorLeft, doorTop + cornerRadius)
                quadTo(doorLeft, doorTop, doorLeft + cornerRadius, doorTop)
                lineTo(doorRight - cornerRadius, doorTop)
                quadTo(doorRight, doorTop, doorRight, doorTop + cornerRadius)
                lineTo(doorRight, bottom)
                close()
            }
            canvas.drawPath(doorPath, strokePaint)
        }

        override fun setAlpha(alpha: Int) {
            strokePaint.alpha = alpha
            invalidateSelf()
        }

        override fun setColorFilter(colorFilter: android.graphics.ColorFilter?) {
            strokePaint.colorFilter = colorFilter
            invalidateSelf()
        }

        @Deprecated("Deprecated in Java")
        override fun getOpacity(): Int = android.graphics.PixelFormat.TRANSLUCENT

        fun updateColor(color: Int) {
            currentFrontColor = color
            strokePaint.color = color
            invalidateSelf()
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
    fun createHomeButton(resources: Resources, color: Int = Color.BLACK) =
        HomeButtonDrawable(resources, color)

    fun createMoreDots(color: Int = Color.BLACK) =
        MoreDotsDrawable(color)

    fun createCloseButton(resources: Resources, color: Int = Color.BLACK) =
        CloseButtonDrawable(resources, color)
}
