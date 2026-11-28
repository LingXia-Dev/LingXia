package com.lingxia.lxapp

import android.app.Activity
import android.graphics.Bitmap
import android.graphics.Rect
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.view.PixelCopy
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.ImageView
import com.lingxia.app.LxLog
import com.lingxia.view.GpuView

/**
 * Native bars repaint the moment the scheme changes while a GPU page needs a frame, so the
 * page column holds a picture of itself until every GPU page on it shows the new scheme
 * (300 ms at most), then fades over 200 ms; under reduced motion it goes at once.
 */
internal object SchemeFade {
    private const val TAG = "LingXia.Appearance"
    private const val TIMEOUT_MS = 300L
    private const val DURATION_MS = 200L
    private val main = Handler(Looper.getMainLooper())

    /** Run [apply] (the scheme change) under a picture of the page column. */
    fun begin(activity: Activity, apply: () -> Unit) {
        val column = activity.findViewById<ViewGroup>(android.R.id.content)
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O || column == null ||
            column.width == 0 || column.height == 0 || gpuPages(column).isEmpty()) {
            apply()
            return
        }
        val origin = IntArray(2)
        column.getLocationInWindow(origin)
        val area = Rect(origin[0], origin[1], origin[0] + column.width, origin[1] + column.height)
        val picture = Bitmap.createBitmap(column.width, column.height, Bitmap.Config.ARGB_8888)
        try {
            // The window copy includes the GPU pages' surfaces, which a view draw leaves out.
            PixelCopy.request(activity.window, area, picture, { result ->
                if (result != PixelCopy.SUCCESS || activity.isFinishing) {
                    picture.recycle()
                    apply()
                    return@request
                }
                show(activity, column, picture, apply)
            }, main)
        } catch (error: IllegalArgumentException) {
            picture.recycle()
            apply()
        }
    }

    private fun show(activity: Activity, column: ViewGroup, picture: Bitmap, apply: () -> Unit) {
        val overlay = ImageView(activity).apply {
            setImageBitmap(picture)
            scaleType = ImageView.ScaleType.FIT_XY
            importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS
            isClickable = false
            isFocusable = false
        }
        column.addView(overlay, FrameLayout.LayoutParams(column.width, column.height))
        val started = System.nanoTime()
        var ended = false
        val end = { timedOut: Boolean ->
            if (!ended) {
                ended = true
                val ms = (System.nanoTime() - started) / 1_000_000
                LxLog.d(TAG, "scheme fade: ${if (timedOut) "timed out" else "page shown"} after $ms ms")
                fadeOut(column, overlay, picture)
            }
        }
        apply()
        val pages = gpuPages(column)
        var waiting = pages.size
        LxLog.d(TAG, "scheme fade: began over $waiting GPU pages")
        main.postDelayed({ end(true) }, TIMEOUT_MS)
        for (page in pages) {
            page.coverUntilShown {
                waiting -= 1
                if (waiting == 0) end(false)
            }
        }
    }

    private fun fadeOut(column: ViewGroup, overlay: ImageView, picture: Bitmap) {
        val remove = {
            column.removeView(overlay)
            picture.recycle()
        }
        if (reducedMotion(column)) {
            remove()
            return
        }
        overlay.animate().alpha(0f).setDuration(DURATION_MS).withEndAction(remove).start()
    }

    private fun reducedMotion(view: View): Boolean =
        Settings.Global.getFloat(view.context.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) == 0f

    private fun gpuPages(root: View): List<GpuView> {
        val found = ArrayList<GpuView>()
        fun walk(view: View) {
            if (view is GpuView) {
                if (view.isShown) found.add(view)
                return
            }
            if (view is ViewGroup) for (i in 0 until view.childCount) walk(view.getChildAt(i))
        }
        walk(root)
        return found
    }
}
