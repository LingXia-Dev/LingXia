package com.lingxia.lxapp.util

import android.view.View
import android.view.ViewGroup
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

/**
 * Small helpers to read and apply system window insets consistently.
 * Use these to avoid repeating WindowInsets boilerplate in overlays.
 */
object WindowInsetsUtils {
    @JvmStatic
    fun getBottomInset(container: View): Int {
        val insets = ViewCompat.getRootWindowInsets(container)
        if (insets == null) return navBarHeightFallback(container)
        // Prefer navigationBars ignoring visibility to account for 3-button navbar area
        val nav = insets.getInsets(WindowInsetsCompat.Type.navigationBars()).bottom
        val navIgnoring = insets.getInsetsIgnoringVisibility(WindowInsetsCompat.Type.navigationBars()).bottom
        val systemBars = insets.getInsets(WindowInsetsCompat.Type.systemBars()).bottom
        return maxOf(nav, navIgnoring, systemBars, navBarHeightFallback(container))
    }

    /**
     * Applies the system bottom inset as additional bottomMargin for [target].
     * [baseBottomMarginPx] lets you keep existing spacing (e.g., 64dp bar height).
     */
    @JvmStatic
    fun applyBottomMargin(container: View, target: View, baseBottomMarginPx: Int = 0) {
        fun setMargin(bottomInset: Int) {
            val lp = target.layoutParams
            if (lp is ViewGroup.MarginLayoutParams) {
                val desired = baseBottomMarginPx + bottomInset
                if (lp.bottomMargin != desired) {
                    lp.bottomMargin = desired
                    target.layoutParams = lp
                }
            }
        }

        setMargin(getBottomInset(container))
        ViewCompat.setOnApplyWindowInsetsListener(container) { _, insets ->
            val nav = insets.getInsets(WindowInsetsCompat.Type.navigationBars()).bottom
            val navIgnoring = insets.getInsetsIgnoringVisibility(WindowInsetsCompat.Type.navigationBars()).bottom
            val systemBars = insets.getInsets(WindowInsetsCompat.Type.systemBars()).bottom
            val b = maxOf(nav, navIgnoring, systemBars)
            setMargin(b)
            insets
        }
        ViewCompat.requestApplyInsets(container)
    }

    /**
     * Applies the system bottom inset as additional paddingBottom for [target].
     * [basePaddingBottomPx] lets you keep existing padding.
     */
    @JvmStatic
    fun applyBottomPadding(container: View, target: View, basePaddingBottomPx: Int = 0) {
        fun setPadding(bottomInset: Int) {
            val desired = basePaddingBottomPx + bottomInset
            if (target.paddingBottom != desired) {
                target.setPadding(target.paddingLeft, target.paddingTop, target.paddingRight, desired)
            }
        }

        setPadding(getBottomInset(container))
        ViewCompat.setOnApplyWindowInsetsListener(container) { _, insets ->
            val nav = insets.getInsets(WindowInsetsCompat.Type.navigationBars()).bottom
            val navIgnoring = insets.getInsetsIgnoringVisibility(WindowInsetsCompat.Type.navigationBars()).bottom
            val systemBars = insets.getInsets(WindowInsetsCompat.Type.systemBars()).bottom
            val b = maxOf(nav, navIgnoring, systemBars)
            setPadding(b)
            insets
        }
        ViewCompat.requestApplyInsets(container)
    }

    private fun navBarHeightFallback(view: View): Int {
        val resId = view.resources.getIdentifier("navigation_bar_height", "dimen", "android")
        return if (resId > 0) view.resources.getDimensionPixelSize(resId) else 0
    }
}
