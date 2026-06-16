package com.lingxia.lxapp.chrome

import com.lingxia.lxapp.R

import android.content.Context
import android.graphics.Color
import android.graphics.drawable.Drawable
import android.graphics.drawable.GradientDrawable
import android.view.View
import android.widget.ImageButton
import android.widget.ImageView
import androidx.appcompat.widget.Toolbar
import androidx.core.content.ContextCompat

internal object LxNavBarUtils {

    object Constants {
        const val BUTTON_SIZE_DP = 32
        const val ANIMATION_DURATION_MS = 300L
        const val BACK_ICON_PADDING_DP = 6
    }

    object CapsuleConstants {
        const val CORNER_RADIUS_DP = 16f
        const val STROKE_WIDTH_DP = 0.5f
        const val STROKE_COLOR = 0xFFDDDDDD.toInt()
        const val BACKGROUND_COLOR = Color.WHITE
    }

    private fun dpToPx(context: Context, dp: Float): Float =
        dp * context.resources.displayMetrics.density

    private fun configureNavButton(
        imageView: ImageView,
        iconResId: Int,
        paddingDp: Int
    ) {
        val context = imageView.context
        val paddingPx = dpToPx(context, paddingDp.toFloat()).toInt()

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
        val paddingPx = dpToPx(context, paddingDp.toFloat()).toInt()
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

    fun applyCapsuleBackground(view: View) {
        val context = view.context
        val cornerRadiusPx = dpToPx(context, CapsuleConstants.CORNER_RADIUS_DP)
        val strokeWidthPx = dpToPx(context, CapsuleConstants.STROKE_WIDTH_DP)
            .toInt()
            .coerceAtLeast(1)
        view.background = GradientDrawable().apply {
            shape = GradientDrawable.RECTANGLE
            setColor(CapsuleConstants.BACKGROUND_COLOR)
            cornerRadius = cornerRadiusPx
            setStroke(strokeWidthPx, CapsuleConstants.STROKE_COLOR)
        }
    }

    fun configureCapsuleMenuButton(imageView: ImageView, color: Int = Color.BLACK) {
        configureCapsuleButton(
            imageView,
            iconResId = R.drawable.icon_capsule_menu,
            color = color,
            scaleType = ImageView.ScaleType.CENTER_INSIDE
        )
    }

    fun configureCapsuleCloseButton(imageView: ImageView, color: Int = Color.BLACK) {
        configureCapsuleButton(
            imageView,
            iconResId = R.drawable.icon_capsule_close,
            color = color,
            scaleType = ImageView.ScaleType.FIT_CENTER,
            clearPadding = true
        )
    }

    private fun configureCapsuleButton(
        imageView: ImageView,
        iconResId: Int,
        color: Int,
        scaleType: ImageView.ScaleType,
        clearPadding: Boolean = false
    ) {
        imageView.setImageResource(iconResId)
        imageView.setColorFilter(color)
        imageView.scaleType = scaleType
        if (clearPadding) {
            imageView.setPadding(0, 0, 0, 0)
        }
    }

    fun createCapsuleDivider(): Drawable =
        GradientDrawable().apply {
            shape = GradientDrawable.RECTANGLE
            setColor(CapsuleConstants.STROKE_COLOR)
        }
}
