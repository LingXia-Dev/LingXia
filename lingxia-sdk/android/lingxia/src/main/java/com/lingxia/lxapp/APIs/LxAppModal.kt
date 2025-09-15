package com.lingxia.lxapp.APIs

import android.app.Activity
import android.content.Context
import android.graphics.Color
import android.graphics.drawable.GradientDrawable

import android.util.Log
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.*
import androidx.core.view.setPadding
import com.lingxia.lxapp.NativeApi
import org.json.JSONObject

/**
 * Modal configuration data class
 */
data class ModalConfig(
    val title: String = "Alert",
    val content: String = "",
    val showCancel: Boolean = true,
    val cancelText: String = "Cancel",
    val confirmText: String = "OK",
    val confirmColor: String? = null
)

/**
 * Modal result data class
 */
data class ModalResult(
    val confirm: Boolean,
    val cancel: Boolean
)

/**
 * LingXia Modal implementation for Android
 */
internal object LxAppModal {
    private const val TAG = "LingXia.LxAppModal"

    private var currentModalView: View? = null
    private var currentMaskView: View? = null

    /**
     * Show modal with options map and callback
     */
    fun showModal(context: Context, options: Map<String, Any?>, callbackId: Long) {
        val config = ModalConfig(
            title = options["title"] as? String ?: "",
            content = options["content"] as? String ?: "",
            showCancel = options["showCancel"] as? Boolean ?: true,
            cancelText = options["cancelText"] as? String ?: "Cancel",
            confirmText = options["confirmText"] as? String ?: "OK",
            confirmColor = options["confirmColor"] as? String
        )

        showModalInternal(context, config, callbackId)
    }

    private fun showModalInternal(context: Context, config: ModalConfig, callbackId: Long) {
        val activity = context as? Activity ?: return
        val rootView = activity.findViewById<ViewGroup>(android.R.id.content) ?: return

        // Hide any existing modal first
        hideModalInternal()

        // Create mask
        currentMaskView = createMaskView(activity, config.showCancel)
        rootView.addView(currentMaskView)

        // Create modal view
        currentModalView = createModalView(activity, config, callbackId)
        rootView.addView(currentModalView)
    }

    private fun createMaskView(context: Context, allowCancel: Boolean): View {
        return View(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
            setBackgroundColor(Color.parseColor("#80000000")) // Semi-transparent black
            isClickable = true

            if (allowCancel) {
                setOnClickListener {
                    if (allowCancel) {
                        Log.i(TAG, "Modal cancelled by mask click")
                        // TODO: Add callback for mask click cancel
                        hideModalInternal()
                    }
                }
            }
        }
    }

    private fun createModalView(context: Context, config: ModalConfig, callbackId: Long): View {
        val container = FrameLayout(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
        }

        val modalContent = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            val paddingPx = (24 * context.resources.displayMetrics.density).toInt()
            setPadding(paddingPx)

            // Background with shadow effect
            background = GradientDrawable().apply {
                setColor(Color.WHITE)
                cornerRadius = 12f * context.resources.displayMetrics.density
            }
            elevation = 20f * context.resources.displayMetrics.density

            layoutParams = FrameLayout.LayoutParams(
                (280 * context.resources.displayMetrics.density).toInt(),
                FrameLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                gravity = Gravity.CENTER
                // Add margins to prevent modal touching screen edges
                val marginPx = (24 * context.resources.displayMetrics.density).toInt()
                setMargins(marginPx, marginPx, marginPx, marginPx)
            }
        }

        // Add title
        if (config.title.isNotEmpty()) {
            val titleView = TextView(context).apply {
                text = config.title
                textSize = 18f
                setTextColor(Color.BLACK)
                gravity = Gravity.CENTER
                maxLines = 2
                typeface = android.graphics.Typeface.DEFAULT_BOLD
                val bottomMarginPx = (20 * context.resources.displayMetrics.density).toInt()
                setPadding(0, 0, 0, bottomMarginPx)
            }
            modalContent.addView(titleView)
        }

        // Add content with better spacing
        if (config.content.isNotEmpty()) {
            val contentView = TextView(context).apply {
                text = config.content
                textSize = 16f
                setTextColor(Color.parseColor("#666666"))
                gravity = Gravity.CENTER
                maxLines = 4
                setLineSpacing(6f * context.resources.displayMetrics.density, 1f)
                val bottomMarginPx = (24 * context.resources.displayMetrics.density).toInt()
                setPadding(0, 0, 0, bottomMarginPx)
            }
            modalContent.addView(contentView)
        }

        // Add buttons
        val buttonsContainer = createButtonsContainer(context, config, callbackId)
        modalContent.addView(buttonsContainer)

        container.addView(modalContent)
        return container
    }

    private fun createButtonsContainer(
        context: Context,
        config: ModalConfig,
        callbackId: Long
    ): LinearLayout {
        return LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER

            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            )

            if (config.showCancel) {
                // Two buttons layout
                val cancelButton = createButton(
                    context = context,
                    text = config.cancelText,
                    isPrimary = false,
                    onClick = {
                        // Call callback with cancel result
                        val result = JSONObject().apply {
                            put("confirm", false)
                            put("cancel", true)
                        }
                        NativeApi.onCallback(callbackId, true, result.toString())
                        hideModalInternal()
                    }
                )
                addView(cancelButton)

                // Add spacing between buttons
                val spacerWidthPx = (12 * context.resources.displayMetrics.density).toInt()
                val spacer = View(context).apply {
                    layoutParams = LinearLayout.LayoutParams(spacerWidthPx, 0)
                }
                addView(spacer)

                val confirmButton = createButton(
                    context = context,
                    text = config.confirmText,
                    isPrimary = true,
                    color = config.confirmColor,
                    onClick = {
                        // Call callback with confirm result
                        val result = JSONObject().apply {
                            put("confirm", true)
                            put("cancel", false)
                        }
                        NativeApi.onCallback(callbackId, true, result.toString())
                        hideModalInternal()
                    }
                )
                addView(confirmButton)
            } else {
                // Single button layout - ensure button has proper width and height
                val confirmButton = createButton(
                    context = context,
                    text = config.confirmText,
                    isPrimary = true,
                    color = config.confirmColor,
                    onClick = {
                        // Call callback with confirm result
                        val result = JSONObject().apply {
                            put("confirm", true)
                            put("cancel", false)
                        }
                        NativeApi.onCallback(callbackId, true, result.toString())
                        hideModalInternal()
                    }
                )
                confirmButton.layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT,
                    (44 * context.resources.displayMetrics.density).toInt()
                )
                addView(confirmButton)
            }
        }
    }

    private fun createButton(
        context: Context,
        text: String,
        isPrimary: Boolean,
        color: String? = null,
        onClick: () -> Unit
    ): Button {
        return Button(context).apply {
            this.text = text
            textSize = 16f

            // Remove default padding and set minimum height
            minHeight = 0
            minimumHeight = (44 * context.resources.displayMetrics.density).toInt()
            val buttonPaddingPx = (16 * context.resources.displayMetrics.density).toInt()
            setPadding(buttonPaddingPx, 0, buttonPaddingPx, 0)

            if (isPrimary) {
                val buttonColor = color?.let {
                    try { Color.parseColor(it) } catch (e: Exception) { Color.parseColor("#007AFF") }
                } ?: Color.parseColor("#007AFF")

                setTextColor(Color.WHITE)
                background = GradientDrawable().apply {
                    setColor(buttonColor)
                    cornerRadius = 8f * context.resources.displayMetrics.density
                }
            } else {
                setTextColor(Color.parseColor("#666666"))
                background = GradientDrawable().apply {
                    setColor(Color.parseColor("#F5F5F5"))
                    cornerRadius = 8f * context.resources.displayMetrics.density
                }
            }

            layoutParams = LinearLayout.LayoutParams(
                0,
                (44 * context.resources.displayMetrics.density).toInt()
            ).apply {
                weight = 1f
            }

            setOnClickListener { onClick() }
        }
    }

    private fun hideModalInternal() {
        currentModalView?.let { modalView ->
            removeModalFromParent(modalView)
            currentModalView = null
        }

        currentMaskView?.let { maskView ->
            removeModalFromParent(maskView)
            currentMaskView = null
        }
    }

    private fun removeModalFromParent(view: View) {
        (view.parent as? ViewGroup)?.removeView(view)
    }
}
