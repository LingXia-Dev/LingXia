package com.lingxia.lxapp.APIs

import android.animation.Animator
import android.animation.AnimatorListenerAdapter
import android.app.Activity
import android.content.Context
import android.graphics.*
import android.graphics.drawable.Drawable
import android.graphics.drawable.GradientDrawable
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.animation.AccelerateDecelerateInterpolator
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.TextView
import java.io.File
import com.lingxia.app.Lingxia
import com.lingxia.lxapp.LxApp

/**
 * Toast icon types
 */
internal enum class ToastIcon {
    Success,
    Error,
    Loading,
    None;

    companion object {
        fun fromInt(value: Int) = values().firstOrNull { it.ordinal == value } ?: None
    }
}

/**
 * Toast position types
 */
internal enum class ToastPosition {
    Top,
    Center,
    Bottom;

    companion object {
        fun fromInt(value: Int) = values().firstOrNull { it.ordinal == value } ?: Center
    }
}

/**
 * Toast configuration
 */
internal data class ToastConfig(
    val title: String,
    val icon: ToastIcon = ToastIcon.None,
    val image: String? = null,
    val duration: Double = 1.5, // Duration in seconds
    val mask: Boolean = false,
    val position: ToastPosition = ToastPosition.Center
)

/**
 * Toast-related errors
 */
sealed class ToastError : Exception() {
    object NoWindow : ToastError() {
        override val message: String = "No window available to display toast"
    }
    object InvalidImage : ToastError() {
        override val message: String = "Invalid image path provided"
    }
}

/**
 * LingXia Toast implementation
 */
internal object LxAppToast {
    private const val TAG = "LxAppToast"

    private var currentToastView: View? = null
    private var currentMaskView: View? = null
    private var hideHandler: Handler? = null
    private var hideRunnable: Runnable? = null

    @JvmStatic
    fun showToast(
        title: String,
        icon: Int,
        image: String?,
        duration: Double,
        mask: Boolean,
        position: Int
    ) {
        val activity = LxApp.getCurrentActivity()
        if (activity == null) {
            Log.w(TAG, "showToast: current activity is null")
            return
        }
        activity.runOnUiThread {
            showToast(
                context = activity,
                title = title,
                icon = ToastIcon.fromInt(icon),
                image = image,
                duration = duration,
                mask = mask,
                position = ToastPosition.fromInt(position)
            )
        }
    }

    @JvmStatic
    fun hideToast() {
        LxApp.getCurrentActivity()?.runOnUiThread {
            hideToastInternal()
        } ?: hideToastInternal()
    }

    /**
     * Show toast with specified configuration
     */
    fun showToast(
        context: Context,
        title: String,
        icon: ToastIcon = ToastIcon.None,
        image: String? = null,
        duration: Double = 1.5,
        mask: Boolean = false,
        position: ToastPosition = ToastPosition.Center
    ) {
        hideToastInternal()

        val config = ToastConfig(title, icon, image, duration, mask, position)
        showToastInternal(context, config)
    }

    /**
     * Hide current toast immediately
     */
    private fun hideToastInternal() {
        // Cancel auto-hide timer
        hideRunnable?.let { runnable ->
            hideHandler?.removeCallbacks(runnable)
        }
        hideHandler = null
        hideRunnable = null

        // Hide toast immediately without animation to prevent conflicts
        currentToastView?.let { toastView ->
            // Cancel any ongoing animations
            toastView.animate().cancel()
            removeToastFromParent(toastView)
            currentToastView = null
        }

        // Hide mask immediately
        currentMaskView?.let { maskView ->
            removeToastFromParent(maskView)
            currentMaskView = null
        }
    }

    private fun showToastInternal(context: Context, config: ToastConfig) {
        val activity = context as? Activity ?: return
        val rootView = activity.findViewById<ViewGroup>(android.R.id.content) ?: return

        // Create mask if needed
        if (config.mask) {
            currentMaskView = createMaskView(activity)
            rootView.addView(currentMaskView)
        }

        // Create toast view
        currentToastView = createToastView(activity, config)
        rootView.addView(currentToastView)

        // Animate in
        currentToastView?.let { toastView ->
            animateIn(toastView)
        }

        // Auto-hide after duration
        if (config.duration > 0) {
            hideHandler = Handler(Looper.getMainLooper())
            hideRunnable = Runnable { hideToastInternal() }
            hideHandler?.postDelayed(hideRunnable!!, (config.duration * 1000).toLong())
        }
    }

    private fun createMaskView(context: Context): View {
        return View(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
            setBackgroundColor(Color.parseColor("#80000000")) // Semi-transparent black
            isClickable = true // Prevent touch through
        }
    }

    private fun createToastView(context: Context, config: ToastConfig): View {
        val container = FrameLayout(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
        }

        val toastContent = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER
            setPadding(48, 32, 48, 32)

            // Background
            background = GradientDrawable().apply {
                setColor(Color.parseColor("#E6000000")) // Semi-transparent black
                cornerRadius = 16f
            }

            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.WRAP_CONTENT,
                FrameLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                gravity = when (config.position) {
                    ToastPosition.Top -> Gravity.CENTER_HORIZONTAL or Gravity.TOP
                    ToastPosition.Center -> Gravity.CENTER
                    ToastPosition.Bottom -> Gravity.CENTER_HORIZONTAL or Gravity.BOTTOM
                }

                // Add margins based on position
                when (config.position) {
                    ToastPosition.Top -> topMargin = 200
                    ToastPosition.Bottom -> bottomMargin = 200
                    ToastPosition.Center -> {}
                }
            }
        }

        // Add icon or image
        if (config.image != null) {
            val imageView = createImageView(context, config.image)
            if (imageView != null) {
                toastContent.addView(imageView)
            }
        } else if (config.icon != ToastIcon.None) {
            val iconView = createIconView(context, config.icon)
            toastContent.addView(iconView)
        }

        // Add title
        val titleView = TextView(context).apply {
            text = config.title
            textSize = 16f
            setTextColor(Color.WHITE)
            gravity = Gravity.CENTER
            maxLines = if (config.icon == ToastIcon.None && config.image == null) 2 else 1

            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                if (config.icon != ToastIcon.None || config.image != null) {
                    topMargin = 24
                }
            }
        }
        toastContent.addView(titleView)

        container.addView(toastContent)
        return container
    }

    private fun createIconView(context: Context, icon: ToastIcon): View {
        return when (icon) {
            ToastIcon.Loading -> {
                ProgressBar(context).apply {
                    layoutParams = LinearLayout.LayoutParams(64, 64)
                    indeterminateDrawable?.setTint(Color.WHITE)
                }
            }
            else -> {
                ImageView(context).apply {
                    layoutParams = LinearLayout.LayoutParams(64, 64)
                    scaleType = ImageView.ScaleType.CENTER_INSIDE
                    setImageDrawable(createIconDrawable(icon))
                }
            }
        }
    }

    private fun createIconDrawable(icon: ToastIcon): Drawable {
        return when (icon) {
            ToastIcon.Success -> CheckmarkDrawable()
            ToastIcon.Error -> CrossDrawable()
            else -> object : Drawable() {
                override fun draw(canvas: Canvas) {}
                override fun setAlpha(alpha: Int) {}
                override fun setColorFilter(colorFilter: ColorFilter?) {}
                override fun getOpacity(): Int = PixelFormat.TRANSPARENT
            }
        }
    }

    private fun createImageView(context: Context, imagePath: String): ImageView? {
        return try {
            // Only support absolute paths
            if (!File(imagePath).isAbsolute) {
                Log.w(TAG, "Image path must be absolute: $imagePath")
                return null
            }

            val file = File(imagePath)
            if (!file.exists() || !file.isFile) {
                Log.w(TAG, "Image file does not exist: $imagePath")
                return null
            }

            val bitmap = BitmapFactory.decodeFile(imagePath)
            if (bitmap == null) {
                Log.w(TAG, "Failed to decode image: $imagePath")
                return null
            }

            ImageView(context).apply {
                layoutParams = LinearLayout.LayoutParams(64, 64)
                scaleType = ImageView.ScaleType.CENTER_INSIDE
                setImageBitmap(bitmap)
            }
        } catch (e: Exception) {
            Log.e(TAG, "Error loading image: $imagePath", e)
            null
        }
    }

    private fun animateIn(view: View) {
        view.alpha = 0f
        view.scaleX = 0.8f
        view.scaleY = 0.8f

        view.animate()
            .alpha(1f)
            .scaleX(1f)
            .scaleY(1f)
            .setDuration(200)
            .setInterpolator(AccelerateDecelerateInterpolator())
            .start()
    }

    private fun animateOut(view: View, onComplete: () -> Unit) {
        view.animate()
            .alpha(0f)
            .scaleX(0.8f)
            .scaleY(0.8f)
            .setDuration(150)
            .setInterpolator(AccelerateDecelerateInterpolator())
            .setListener(object : AnimatorListenerAdapter() {
                override fun onAnimationEnd(animation: Animator) {
                    onComplete()
                }
            })
            .start()
    }

    private fun removeToastFromParent(view: View) {
        (view.parent as? ViewGroup)?.removeView(view)
    }
}

/**
 * Custom drawable for checkmark icon (Success)
 */
private class CheckmarkDrawable : Drawable() {
    private val paint = Paint().apply {
        isAntiAlias = true
        color = Color.WHITE
        strokeWidth = 8f
        style = Paint.Style.STROKE
        strokeCap = Paint.Cap.ROUND
        strokeJoin = Paint.Join.ROUND
    }

    override fun draw(canvas: Canvas) {
        val bounds = bounds
        val centerX = bounds.centerX().toFloat()
        val centerY = bounds.centerY().toFloat()
        val size = minOf(bounds.width(), bounds.height()) * 0.35f

        // Draw a more prominent checkmark path
        val path = Path().apply {
            // Start point (left side of checkmark)
            moveTo(centerX - size * 0.8f, centerY - size * 0.1f)
            // Middle point (bottom of checkmark)
            lineTo(centerX - size * 0.2f, centerY + size * 0.5f)
            // End point (right side of checkmark)
            lineTo(centerX + size * 0.9f, centerY - size * 0.6f)
        }
        canvas.drawPath(path, paint)
    }

    override fun setAlpha(alpha: Int) {
        paint.alpha = alpha
    }

    override fun setColorFilter(colorFilter: ColorFilter?) {
        paint.colorFilter = colorFilter
    }

    @Deprecated("Deprecated in Java")
    override fun getOpacity(): Int = PixelFormat.TRANSLUCENT
}

/**
 * Custom drawable for cross icon (Error)
 */
private class CrossDrawable : Drawable() {
    private val paint = Paint().apply {
        isAntiAlias = true
        color = Color.parseColor("#F44336")
        strokeWidth = 8f
        style = Paint.Style.STROKE
        strokeCap = Paint.Cap.ROUND
    }

    override fun draw(canvas: Canvas) {
        val bounds = bounds
        val centerX = bounds.centerX().toFloat()
        val centerY = bounds.centerY().toFloat()
        val size = minOf(bounds.width(), bounds.height()) * 0.3f  // Slightly larger

        // Draw X with better proportions
        canvas.drawLine(
            centerX - size, centerY - size,
            centerX + size, centerY + size,
            paint
        )
        canvas.drawLine(
            centerX + size, centerY - size,
            centerX - size, centerY + size,
            paint
        )
    }

    override fun setAlpha(alpha: Int) {
        paint.alpha = alpha
    }

    override fun setColorFilter(colorFilter: ColorFilter?) {
        paint.colorFilter = colorFilter
    }

    @Deprecated("Deprecated in Java")
    override fun getOpacity(): Int = PixelFormat.TRANSLUCENT
}
