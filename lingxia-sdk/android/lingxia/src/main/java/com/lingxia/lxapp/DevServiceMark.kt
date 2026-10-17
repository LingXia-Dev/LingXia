package com.lingxia.lxapp

import android.app.Activity
import android.graphics.Color
import android.graphics.PixelFormat
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.util.Log
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.view.WindowManager
import android.widget.FrameLayout
import android.widget.TextView
import com.lingxia.app.LxLog
import com.lingxia.app.NativeApi
import java.util.WeakHashMap

/**
 * Touch-through dev mark. Auth sheets and guest lxapps are views in the
 * current activity, so a chip inside the page paints underneath the next one
 * added. This is its own sub-window, above those, and it does not take
 * touches. A new activity gets its own; the previous one hides with its parent.
 */
internal object DevServiceMark {
    private const val TAG = "LingXia.DevMark"
    private val shown = WeakHashMap<Activity, View>()

    fun attach(activity: Activity) {
        if (activity.isFinishing || activity.isDestroyed) return
        if (!NativeApi.ensureLoaded()) return
        val label = NativeApi.devMark(true)
        if (label.isEmpty()) return
        shown[activity]?.let { existing ->
            if (existing.isAttachedToWindow) return
            shown.remove(activity)
        }
        val decor = activity.window?.decorView ?: return
        if (!decor.isAttachedToWindow) {
            decor.addOnAttachStateChangeListener(object : View.OnAttachStateChangeListener {
                override fun onViewAttachedToWindow(v: View) {
                    v.removeOnAttachStateChangeListener(this)
                    attach(activity)
                }

                override fun onViewDetachedFromWindow(v: View) = Unit
            })
            return
        }
        val mark = chip(activity, label)
        val params = WindowManager.LayoutParams(
            dp(activity, 24),
            dp(activity, mark.heightDp),
            WindowManager.LayoutParams.TYPE_APPLICATION_SUB_PANEL,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
                WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE or
                WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN,
            PixelFormat.TRANSLUCENT,
        ).apply {
            token = decor.windowToken
            gravity = Gravity.LEFT or Gravity.CENTER_VERTICAL
            x = 0
            y = 0
        }
        val wm = activity.getSystemService(Activity.WINDOW_SERVICE) as WindowManager
        try {
            wm.addView(mark.view, params)
        } catch (error: RuntimeException) {
            LxLog.e(TAG, "open failed: $error")
            return
        }
        shown[activity] = mark.view
        Log.i(TAG, "dev service mark shown")
    }

    fun detach(activity: Activity) {
        val chip = shown.remove(activity) ?: return
        val wm = activity.getSystemService(Activity.WINDOW_SERVICE) as? WindowManager ?: return
        try {
            wm.removeView(chip)
        } catch (_: RuntimeException) {
        }
    }

    private class MarkChip(val view: View, val heightDp: Int)

    private fun chip(activity: Activity, label: String): MarkChip {
        val word = label.length <= 3
        val textWidthDp = if (word) 36 else label.length * 8 + 4
        val tabHeightDp = if (word) 60 else textWidthDp + 28
        val tab = FrameLayout(activity).apply {
            background = GradientDrawable().apply {
                setColor(Color.parseColor("#C62828"))
                val radius = dp(activity, 12).toFloat()
                cornerRadii = floatArrayOf(0f, 0f, radius, radius, radius, radius, 0f, 0f)
            }
            isClickable = false
            isFocusable = false
            importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
        }
        tab.addView(View(activity).apply {
            background = GradientDrawable().apply {
                shape = GradientDrawable.OVAL
                setColor(Color.WHITE)
            }
            isClickable = false
        }, FrameLayout.LayoutParams(dp(activity, 6), dp(activity, 6), Gravity.TOP or Gravity.CENTER_HORIZONTAL).apply {
            topMargin = dp(activity, 8)
        })
        tab.addView(TextView(activity).apply {
            text = label
            gravity = Gravity.CENTER
            rotation = -90f
            translationY = dp(activity, if (word) 7 else 8).toFloat()
            setTextColor(Color.WHITE)
            setTextSize(TypedValue.COMPLEX_UNIT_DIP, 11f)
            typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
            includeFontPadding = false
            isClickable = false
            importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
        }, FrameLayout.LayoutParams(dp(activity, textWidthDp), dp(activity, 16), Gravity.CENTER))
        return MarkChip(tab, tabHeightDp)
    }

    private fun dp(activity: Activity, value: Int): Int {
        val density = activity.resources.displayMetrics.density
        return (value * density).toInt()
    }
}
