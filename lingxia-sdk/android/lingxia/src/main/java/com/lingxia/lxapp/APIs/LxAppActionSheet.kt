package com.lingxia.lxapp.APIs

import android.app.Activity
import android.content.Context
import android.graphics.Color
import android.util.Log
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.*
import androidx.core.view.ViewCompat
import com.lingxia.lxapp.LxApp
import android.graphics.drawable.GradientDrawable
import com.lingxia.lxapp.NativeApi
import org.json.JSONObject

/**
 * LingXia ActionSheet implementation for Android
 */
internal object LxAppActionSheet {
    private const val TAG = "LingXia.LxAppActionSheet"

    private var currentActionSheetView: View? = null
    private var currentMaskView: View? = null

    @JvmStatic
    fun showActionSheet(options: Array<String>, cancelText: String, itemColor: String, callbackId: Long) {
        val activity = LxApp.getCurrentActivity()
        if (activity == null) {
            Log.e(TAG, "showActionSheet: current activity is null")
            sendActionSheetResult(callbackId, -1)
            return
        }
        activity.runOnUiThread {
            showActionSheet(activity, options.toList(), cancelText, itemColor, callbackId)
        }
    }

    @JvmStatic
    fun hideActionSheet() {
        LxApp.getCurrentActivity()?.runOnUiThread {
            hideActionSheetInternal()
        } ?: hideActionSheetInternal()
    }

    /**
     * Show action sheet with options and callback
     */
    fun showActionSheet(context: Context, options: List<String>, cancelText: String, itemColor: String, callbackId: Long) {
        val activity = context as? Activity ?: run {
            Log.e(TAG, "showActionSheet: context is not an Activity")
            return
        }
        val rootView = activity.findViewById<ViewGroup>(android.R.id.content) ?: run {
            Log.e(TAG, "showActionSheet: rootView is null")
            return
        }

        // Hide any existing action sheet first
        hideActionSheetInternal()

        // Create mask
        currentMaskView = createMaskView(activity) {
            // Cancel on mask click
            sendActionSheetResult(callbackId, -1)
            hideActionSheetInternal()
        }
        rootView.addView(currentMaskView)

        // Create action sheet view
        currentActionSheetView = createActionSheetView(activity, options, cancelText, itemColor, callbackId)
        rootView.addView(currentActionSheetView)
    }

    private fun createMaskView(context: Context, onCancel: () -> Unit): View {
        return View(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
            setBackgroundColor(Color.parseColor("#80000000"))
            isClickable = true
            setOnClickListener { onCancel() }
        }
    }

    private fun createActionSheetView(
        context: Context,
        options: List<String>,
        cancelText: String,
        itemColor: String,
        callbackId: Long
    ): View {
        val container = FrameLayout(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
        }

        val actionSheetContent = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL

            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                gravity = Gravity.BOTTOM
            }

            background = createActionSheetBackground(context)
        }

        // Add option buttons
        options.forEachIndexed { index, option ->
            val optionButton = createOptionButton(context, option, itemColor) {
                sendActionSheetResult(callbackId, index)
                hideActionSheetInternal()
            }
            actionSheetContent.addView(optionButton)

            // Add separator (except for last item)
            if (index < options.size - 1) {
                val separator = createSeparator(context)
                actionSheetContent.addView(separator)
            }
        }

        // Add thicker separator before cancel button
        val thickSeparator = View(context).apply {
            setBackgroundColor(Color.parseColor("#E0E0E0"))
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                (8 * context.resources.displayMetrics.density).toInt()
            )
        }
        actionSheetContent.addView(thickSeparator)

        // Add cancel button
        val cancelButton = createCancelButton(context, cancelText) {
            sendActionSheetResult(callbackId, -1)
            hideActionSheetInternal()
        }
        actionSheetContent.addView(cancelButton)

        container.addView(actionSheetContent)

        // Use Activity-provided bottom inset (encapsulated helper)
        com.lingxia.lxapp.util.ActivityInsets.applyBottomMargin(container, actionSheetContent, 0)
        return container
    }

    private fun createOptionButton(context: Context, text: String, itemColor: String, onClick: () -> Unit): TextView {
        return TextView(context).apply {
            this.text = text
            textSize = 18f
            setTextColor(Color.parseColor(itemColor))
            gravity = Gravity.CENTER
            isClickable = true
            setOnClickListener { onClick() }

            val paddingPx = (20 * context.resources.displayMetrics.density).toInt()
            setPadding(paddingPx, paddingPx, paddingPx, paddingPx)

            // Set minimum height
            val minHeightPx = (56 * context.resources.displayMetrics.density).toInt()
            minHeight = minHeightPx

            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            )
        }
    }

    private fun createCancelButton(context: Context, text: String, onClick: () -> Unit): TextView {
        return TextView(context).apply {
            this.text = text
            textSize = 18f
            setTextColor(Color.parseColor("#666666"))
            gravity = Gravity.CENTER
            isClickable = true
            setOnClickListener { onClick() }

            val paddingPx = (20 * context.resources.displayMetrics.density).toInt()
            setPadding(paddingPx, paddingPx, paddingPx, paddingPx)

            // Set minimum height
            val minHeightPx = (56 * context.resources.displayMetrics.density).toInt()
            minHeight = minHeightPx

            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            )
        }
    }

    private fun createSeparator(context: Context): View {
        return View(context).apply {
            setBackgroundColor(Color.parseColor("#E0E0E0"))
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                1
            )
        }
    }

    private fun createActionSheetBackground(context: Context): GradientDrawable {
        val density = context.resources.displayMetrics.density
        val radius = 16f * density
        return GradientDrawable().apply {
            shape = GradientDrawable.RECTANGLE
            setColor(Color.WHITE)
            cornerRadii = floatArrayOf(radius, radius, radius, radius, 0f, 0f, 0f, 0f)
        }
    }

    private fun sendActionSheetResult(callbackId: Long, tapIndex: Int) {
        val result = JSONObject().apply {
            put("tapIndex", tapIndex)
        }
        NativeApi.onCallback(callbackId, true, result.toString())
    }

    private fun hideActionSheetInternal() {
        currentActionSheetView?.let { actionSheetView ->
            removeActionSheetFromParent(actionSheetView)
            currentActionSheetView = null
        }

        currentMaskView?.let { maskView ->
            removeActionSheetFromParent(maskView)
            currentMaskView = null
        }
    }

    private fun removeActionSheetFromParent(view: View) {
        (view.parent as? ViewGroup)?.removeView(view)
    }
}
