package com.lingxia.lxapp.APIs

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Path
import android.graphics.RectF
import android.graphics.drawable.GradientDrawable
import android.os.Build
import android.util.DisplayMetrics
import android.util.Log
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import com.lingxia.lxapp.LxApp
import com.lingxia.lxapp.LxAppActivity
import com.lingxia.lxapp.NativeApi
import kotlin.math.max
import kotlin.math.roundToInt

internal enum class PopupPosition(val value: Int) {
    CENTER(0),
    BOTTOM(1);

    companion object {
        fun fromInt(value: Int): PopupPosition = when (value) {
            BOTTOM.value -> BOTTOM
            else -> CENTER
        }
    }
}

internal object LxAppPopup {
    private const val TAG = "LingXia.LxAppPopup"

    private var overlayView: FrameLayout? = null
    private var popupWebView: com.lingxia.lxapp.WebView? = null
    private var popupAppId: String? = null

    @JvmStatic
    fun showPopup(appId: String, path: String, widthRatio: Double, heightRatio: Double, position: Int) {
        val activity = LxApp.getCurrentActivity()
        if (activity == null) {
            Log.w(TAG, "showPopup: current activity is null")
            return
        }
        if (activity.getAppId() != appId) {
            Log.w(TAG, "showPopup: activity appId=${activity.getAppId()} does not match requested appId=$appId")
            return
        }
        activity.runOnUiThread {
            showPopup(activity, appId, path, widthRatio, heightRatio, PopupPosition.fromInt(position))
        }
    }

    @JvmStatic
    fun hidePopup(appId: String) {
        val activity = LxApp.getCurrentActivity()
        if (activity == null) {
            Log.w(TAG, "hidePopup: current activity is null")
            return
        }
        activity.runOnUiThread {
            hidePopup(activity, appId)
        }
    }

    fun showPopup(
        activity: LxAppActivity,
        appId: String,
        path: String,
        widthRatio: Double,
        heightRatio: Double,
        position: PopupPosition
    ) {
        Log.d(
            TAG,
            "showPopup start appId=$appId path=$path widthRatio=$widthRatio heightRatio=$heightRatio position=$position"
        )

        hidePopup(activity, appId)

        val rootView = activity.findViewById<ViewGroup>(android.R.id.content)
        if (rootView == null) {
            Log.w(TAG, "showPopup: root view not available")
            return
        }

        val webView = NativeApi.findWebView(appId, path)
        if (webView == null) {
            Log.w(TAG, "showPopup: WebView not found for path=$path")
            return
        }

        val metrics = activity.resources.displayMetrics
        val widthFraction = sanitizeFraction(widthRatio)
        val heightFraction = if (heightRatio.isNaN()) {
            defaultHeightFraction(position, metrics)
        } else {
            sanitizeFraction(heightRatio)
        }

        val resolvedWidth = resolveDimension(widthFraction, metrics.widthPixels, metrics.density)
        val resolvedHeight = resolveDimension(heightFraction, metrics.heightPixels, metrics.density)

        Log.d(
            TAG,
            "showPopup resolved size width=${resolvedWidth.size}px height=${resolvedHeight.size}px"
        )

        val overlay = FrameLayout(activity).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
            isClickable = true
            isFocusable = true
        }

        val maskView = View(activity).apply {
            layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
            setBackgroundColor(Color.parseColor("#80000000"))
            isClickable = true
        }
        overlay.addView(maskView)

        val container = FrameLayout(activity).apply {
            val baseHeight = resolvedHeight.size
            val params = FrameLayout.LayoutParams(resolvedWidth.size, baseHeight)
            params.gravity = when (position) {
                PopupPosition.BOTTOM -> Gravity.BOTTOM or Gravity.CENTER_HORIZONTAL
                PopupPosition.CENTER -> Gravity.CENTER
            }

            val margin = (16 * metrics.density).roundToInt()
            val horizontalMargin = if (resolvedWidth.isFull) 0 else margin
            val topMargin = if (position == PopupPosition.BOTTOM) 0 else margin
            val bottomInset = getBottomInset(rootView)
            val bottomMargin = if (position == PopupPosition.BOTTOM) 0 else margin
            params.setMargins(horizontalMargin, topMargin, horizontalMargin, bottomMargin)
            layoutParams = params

            if (position == PopupPosition.BOTTOM && !resolvedHeight.isFull && bottomInset > 0) {
                val adjustedHeight = (baseHeight + bottomInset).coerceAtMost(metrics.heightPixels)
                layoutParams.height = adjustedHeight
                setPadding(paddingLeft, paddingTop, paddingRight, bottomInset)
            }

            clipToPadding = false
            isClickable = false
            isFocusable = false
        }

        val popupSurface = RoundedContainer(activity).apply {
            val contentHeight = if (resolvedHeight.isFull) {
                FrameLayout.LayoutParams.MATCH_PARENT
            } else {
                resolvedHeight.size
            }
            val lp = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                contentHeight
            )
            lp.gravity = when (position) {
                PopupPosition.BOTTOM -> Gravity.TOP or Gravity.CENTER_HORIZONTAL
                PopupPosition.CENTER -> Gravity.CENTER
            }
            layoutParams = lp

            val radiusPx = 16f * metrics.density
            val radii = when (position) {
                PopupPosition.BOTTOM -> floatArrayOf(
                    radiusPx,
                    radiusPx,
                    radiusPx,
                    radiusPx,
                    0f,
                    0f,
                    0f,
                    0f
                )
                PopupPosition.CENTER -> FloatArray(8) { radiusPx }
            }

            if (!resolvedHeight.isFull) {
                setCornerRadii(radii)
                background = GradientDrawable().apply {
                    shape = GradientDrawable.RECTANGLE
                    setColor(Color.WHITE)
                    when (position) {
                        PopupPosition.BOTTOM -> cornerRadii = radii
                        PopupPosition.CENTER -> cornerRadius = radiusPx
                    }
                }
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.LOLLIPOP) {
                    elevation = 24f * metrics.density
                } else {
                    @Suppress("DEPRECATION")
                    setLayerType(View.LAYER_TYPE_SOFTWARE, null)
                }
            } else {
                setCornerRadii(FloatArray(8))
                setBackgroundColor(Color.WHITE)
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.LOLLIPOP) {
                    elevation = 0f
                } else {
                    @Suppress("DEPRECATION")
                    setLayerType(View.LAYER_TYPE_SOFTWARE, null)
                }
            }
            isClickable = true
            isFocusable = true
        }

        (webView.parent as? ViewGroup)?.removeView(webView)
        webView.layoutParams = FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.MATCH_PARENT
        )
        webView.setBackgroundColor(Color.TRANSPARENT)
        webView.visibility = View.VISIBLE
        webView.resume()

        popupSurface.addView(webView)
        container.addView(popupSurface)
        overlay.addView(container)
        rootView.addView(overlay)

        Log.d(TAG, "showPopup overlay attached to root")

        NativeApi.onPageShow(appId, path)

        overlayView = overlay
        popupWebView = webView
        popupAppId = appId
    }

    fun hidePopup(activity: LxAppActivity, appId: String) {
        Log.d(TAG, "hidePopup start appId=$appId current=$popupAppId")

        if (popupWebView != null && popupAppId != null && popupAppId != appId) {
            Log.w(
                TAG,
                "hidePopup called with non-matching appId=$appId current=$popupAppId"
            )
        }

        popupWebView?.let { view ->
            Log.d(TAG, "hidePopup removing WebView from container")
            view.pause()
            (view.parent as? ViewGroup)?.removeView(view)
            view.visibility = View.GONE
        }
        popupWebView = null
        popupAppId = null

        overlayView?.let { overlay ->
            Log.d(TAG, "hidePopup removing overlay")
            (overlay.parent as? ViewGroup)?.removeView(overlay)
            overlay.removeAllViews()
        }
        overlayView = null
    }

    private fun resolveDimension(fraction: Double, totalPx: Int, density: Float): ResolvedDimension {
        if (fraction <= 0.0) {
            return ResolvedDimension(FrameLayout.LayoutParams.WRAP_CONTENT, false)
        }

        val rawSize = (fraction * totalPx).roundToInt()

        val isFull = fraction >= 0.999 || rawSize >= totalPx
        if (isFull) {
            Log.d(TAG, "resolveDimension -> MATCH_PARENT for fraction=$fraction")
            return ResolvedDimension(ViewGroup.LayoutParams.MATCH_PARENT, true)
        }

        val minPx = (120 * density).roundToInt()
        val clamped = rawSize.coerceIn(minPx, totalPx)
        Log.d(TAG, "resolveDimension fraction=$fraction totalPx=$totalPx density=$density -> $clamped")
        return ResolvedDimension(clamped, false)
    }

    private fun getBottomInset(root: View): Int {
        val insets = ViewCompat.getRootWindowInsets(root)
        return insets?.getInsets(WindowInsetsCompat.Type.systemBars())?.bottom ?: 0
    }

    private fun sanitizeFraction(value: Double): Double {
        return when {
            value.isNaN() -> 1.0
            value <= 0.0 -> 0.0
            value >= 1.0 -> 1.0
            else -> value
        }
    }

    private fun defaultHeightFraction(position: PopupPosition, metrics: DisplayMetrics): Double {
        val minDp = minOf(metrics.widthPixels, metrics.heightPixels) / metrics.density
        val maxDp = max(
            metrics.widthPixels / metrics.density,
            metrics.heightPixels / metrics.density
        )
        val isTablet = minDp >= 600f
        return when (position) {
            PopupPosition.BOTTOM -> if (isTablet) 0.45 else 0.55
            PopupPosition.CENTER -> {
                if (isTablet) {
                    0.5
                } else {
                    when {
                        maxDp >= 900f -> 0.55
                        maxDp >= 780f -> 0.58
                        else -> 0.6
                    }
                }
            }
        }
    }

    private data class ResolvedDimension(val size: Int, val isFull: Boolean)
}

private class RoundedContainer(context: Context) : FrameLayout(context) {
    private val clipPath = Path()
    private val rect = RectF()
    private var radii: FloatArray = FloatArray(8)
    private var clipValid = false

    fun setCornerRadii(newRadii: FloatArray) {
        radii = newRadii.copyOf()
        updateClipPath(width, height)
        invalidate()
    }

    override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
        super.onSizeChanged(w, h, oldw, oldh)
        updateClipPath(w, h)
    }

    override fun draw(canvas: Canvas) {
        if (!clipValid) {
            super.draw(canvas)
            return
        }
        val save = canvas.save()
        canvas.clipPath(clipPath)
        super.draw(canvas)
        canvas.restoreToCount(save)
    }

    private fun updateClipPath(w: Int, h: Int) {
        clipPath.reset()
        if (w <= 0 || h <= 0) {
            clipValid = false
            return
        }
        rect.set(0f, 0f, w.toFloat(), h.toFloat())
        val hasRadius = radii.any { it > 0f }
        if (hasRadius) {
            clipPath.addRoundRect(rect, radii, Path.Direction.CW)
        } else {
            clipPath.addRect(rect, Path.Direction.CW)
        }
        clipValid = true
    }
}
