package com.lingxia.lxapp.chrome

import android.content.Context
import android.view.Gravity
import android.view.View
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.widget.ImageButton
import android.widget.ImageView
import android.widget.LinearLayout

/**
 * Capsule button component with menu and close buttons.
 * Used in the top-right corner of LxApp pages.
 */
internal class CapsuleButton(
    context: Context,
    backgroundColor: Int,
    foregroundColor: Int,
    dividerColor: Int,
    interactionColor: Int
) : LinearLayout(context) {

    private val menuButton: ImageButton
    private val closeButton: ImageButton
    private val divider: View

    init {
        orientation = HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL
        tag = "capsule_button"
        elevation = 1000f

        val density = resources.displayMetrics.density

        setPadding(
            (LxAppTheme.Metrics.CAPSULE_PADDING_HORIZONTAL_DP * density).toInt(),
            0,
            (LxAppTheme.Metrics.CAPSULE_PADDING_HORIZONTAL_DP * density).toInt(),
            0
        )

        menuButton = createButton()
        divider = createDivider()

        closeButton = createButton()

        addView(menuButton)
        addView(divider)
        addView(closeButton)
        applyStyle(backgroundColor, foregroundColor, dividerColor, interactionColor)
    }

    private fun createButton(): ImageButton {
        val density = resources.displayMetrics.density
        val buttonWidth = (LxAppTheme.Metrics.CAPSULE_BUTTON_WIDTH_DP * density).toInt()

        return ImageButton(context).apply {
            layoutParams = LayoutParams(buttonWidth, MATCH_PARENT)
            scaleType = ImageView.ScaleType.CENTER_INSIDE
        }
    }

    private fun createDivider(): View {
        val density = resources.displayMetrics.density
        val dividerWidth = (LxAppTheme.Metrics.CAPSULE_DIVIDER_WIDTH_DP * density).toInt().coerceAtLeast(1)
        val dividerHeight = (LxAppTheme.Metrics.CAPSULE_DIVIDER_HEIGHT_DP * density).toInt()
        val horizontalMargin = (LxAppTheme.Metrics.CAPSULE_PADDING_HORIZONTAL_DP * density).toInt()

        return View(context).apply {
            layoutParams = LayoutParams(dividerWidth, dividerHeight).apply {
                gravity = Gravity.CENTER_VERTICAL
                marginStart = horizontalMargin
                marginEnd = horizontalMargin
            }
        }
    }

    fun applyStyle(
        backgroundColor: Int,
        foregroundColor: Int,
        dividerColor: Int,
        interactionColor: Int
    ) {
        LxNavBarUtils.applyCapsuleBackground(this, backgroundColor, dividerColor)
        LxNavBarUtils.configureCapsuleMenuButton(menuButton, foregroundColor)
        LxNavBarUtils.configureCapsuleCloseButton(closeButton, foregroundColor)
        LxNavBarUtils.applyCapsuleInteraction(menuButton, interactionColor)
        LxNavBarUtils.applyCapsuleInteraction(closeButton, interactionColor)
        divider.background = LxNavBarUtils.createCapsuleDivider(dividerColor)
    }

    fun setOnMenuClickListener(listener: OnClickListener) {
        menuButton.setOnClickListener(listener)
    }

    fun setOnCloseClickListener(listener: OnClickListener) {
        closeButton.setOnClickListener(listener)
    }
}
