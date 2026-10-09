package com.lingxia.lxapp.APIs

import com.lingxia.lxapp.LxAppDismissal

import android.os.Handler
import android.os.Looper
import android.app.Activity
import android.content.Context
import android.graphics.Color
import android.graphics.drawable.GradientDrawable

import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.*
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback
import androidx.core.view.setPadding
import com.lingxia.app.LxLog
import com.lingxia.app.NativeApi
import org.json.JSONObject
import com.lingxia.app.Lingxia
import com.lingxia.lxapp.LxApp
import com.lingxia.lxapp.chrome.OverlayPalette

/**
 * Modal configuration data class
 */
internal data class ModalConfig(
    val title: String = "Alert",
    val content: String = "",
    val showCancel: Boolean = true,
    val cancelText: String? = null,
    val confirmText: String? = null,
    val confirmColor: String? = null
)

/**
 * Modal result data class
 */
internal data class ModalResult(
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
    private var currentCallbackId: Long? = null
    private var backCallback: OnBackPressedCallback? = null

    @JvmStatic
    fun showModal(
        title: String,
        content: String,
        showCancel: Boolean,
        cancelText: String?,
        cancelColor: String?,
        confirmText: String?,
        confirmColor: String?,
        callbackId: Long
    ) {
        val config = ModalConfig(
            title = title,
            content = content,
            showCancel = showCancel,
            cancelText = cancelText,
            confirmText = confirmText,
            confirmColor = confirmColor?.takeIf { it.isNotBlank() }
        )

        LxApp.withCurrentActivity { activity ->
            if (activity == null) {
                LxLog.e(TAG, "showModal: no activity to present on")
                val result = JSONObject().apply {
                    put("confirm", false)
                    put("cancel", true)
                    put("error", "No active activity")
                }
                NativeApi.onCallback(callbackId, false, result.toString())
                return@withCurrentActivity
            }
            showModalInternal(activity, config, callbackId)
        }
    }

    @JvmStatic
    fun hideModal() {
        LxApp.getCurrentActivity()?.runOnUiThread {
            hideModalInternal()
        } ?: hideModalInternal()
    }

    /**
     * Show modal with options map and callback
     */
    fun showModal(context: Context, options: Map<String, Any?>, callbackId: Long) {
        val config = ModalConfig(
            title = options["title"] as? String ?: "",
            content = options["content"] as? String ?: "",
            showCancel = options["showCancel"] as? Boolean ?: true,
            cancelText = options["cancelText"] as? String,
            confirmText = options["confirmText"] as? String,
            confirmColor = options["confirmColor"] as? String
        )

        showModalInternal(context, config, callbackId)
    }

    private fun showModalInternal(context: Context, config: ModalConfig, callbackId: Long) {
        val activity = context as? Activity ?: return
        val rootView = activity.findViewById<ViewGroup>(android.R.id.content) ?: return

        // Hide any existing modal first
        hideModalInternal()

        currentCallbackId = callbackId
        val palette = OverlayPalette.of(activity)

        // Create mask
        currentMaskView = createMaskView(activity, palette)
        rootView.addView(currentMaskView)

        // Create modal view
        currentModalView = createModalView(activity, config, callbackId, palette)
        rootView.addView(currentModalView)
        currentModalView?.addOnAttachStateChangeListener(object : View.OnAttachStateChangeListener {
            override fun onViewAttachedToWindow(view: View) = Unit
            override fun onViewDetachedFromWindow(view: View) {
                // Let the parent finish its detach traversal before removing children.
                Handler(Looper.getMainLooper()).post { completeModal(callbackId, false) }
            }
        })
        (activity as? ComponentActivity)?.let { owner ->
            backCallback = object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() {
                    if (config.showCancel) completeModal(callbackId, false)
                }
            }.also { owner.onBackPressedDispatcher.addCallback(owner, it) }
        }
    }

    private fun createMaskView(context: Context, palette: OverlayPalette): View {
        return View(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
            setBackgroundColor(palette.scrim)
            isClickable = true
        }
    }

    private fun createModalView(
        context: Context,
        config: ModalConfig,
        callbackId: Long,
        palette: OverlayPalette
    ): View {
        val container = FrameLayout(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
            // Prevent clicks from passing through to views behind the modal
            isClickable = true
            isFocusable = true
            if (config.showCancel) setOnClickListener { completeModal(callbackId, false) }
        }

        val modalContent = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            isClickable = true
            val paddingPx = (24 * context.resources.displayMetrics.density).toInt()
            setPadding(paddingPx)

            // Background with shadow effect
            background = GradientDrawable().apply {
                setColor(palette.surface)
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
                setTextColor(palette.title)
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
                setTextColor(palette.body)
                gravity = Gravity.CENTER
                maxLines = 4
                setLineSpacing(6f * context.resources.displayMetrics.density, 1f)
                val bottomMarginPx = (24 * context.resources.displayMetrics.density).toInt()
                setPadding(0, 0, 0, bottomMarginPx)
            }
            modalContent.addView(contentView)
        }

        // Add buttons
        val buttonsContainer = createButtonsContainer(context, config, callbackId, palette)
        modalContent.addView(buttonsContainer)

        container.addView(modalContent)
        return container
    }

    private fun createButtonsContainer(
        context: Context,
        config: ModalConfig,
        callbackId: Long,
        palette: OverlayPalette
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
                    palette = palette,
                    text = config.cancelText ?: "",
                    isPrimary = false,
                    onClick = {
                        completeModal(callbackId, false)
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
                    palette = palette,
                    text = config.confirmText ?: "",
                    isPrimary = true,
                    color = config.confirmColor,
                    onClick = {
                        completeModal(callbackId, true)
                    }
                )
                addView(confirmButton)
            } else {
                // Single button layout - ensure button has proper width and height
                val confirmButton = createButton(
                    context = context,
                    palette = palette,
                    text = config.confirmText ?: "",
                    isPrimary = true,
                    color = config.confirmColor,
                    onClick = {
                        completeModal(callbackId, true)
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
        palette: OverlayPalette,
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
                setTextColor(palette.secondaryText)
                background = GradientDrawable().apply {
                    setColor(palette.secondaryFill)
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

    private fun completeModal(callbackId: Long, confirmed: Boolean) {
        if (currentCallbackId != callbackId) return
        currentCallbackId = null
        hideModalInternal()
        val result = if (confirmed) JSONObject().apply {
            put("confirm", true)
            put("cancel", false)
        }.toString() else LxAppDismissal.USER_DISMISSED
        NativeApi.onCallback(callbackId, confirmed, result)
    }

    private fun hideModalInternal() {
        // Replacement and activity teardown must settle the old request exactly once.
        val pending = currentCallbackId
        currentCallbackId = null
        backCallback?.remove()
        backCallback = null
        currentModalView?.let { modalView ->
            removeModalFromParent(modalView)
            currentModalView = null
        }

        currentMaskView?.let { maskView ->
            removeModalFromParent(maskView)
            currentMaskView = null
        }
        if (pending != null) NativeApi.onCallback(pending, false, LxAppDismissal.USER_DISMISSED)
    }

    private fun removeModalFromParent(view: View) {
        (view.parent as? ViewGroup)?.removeView(view)
    }
}