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
        return insets?.getInsets(WindowInsetsCompat.Type.systemBars())?.bottom ?: 0
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
            val b = insets.getInsets(WindowInsetsCompat.Type.systemBars()).bottom
            setMargin(b)
            insets
        }
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
            val b = insets.getInsets(WindowInsetsCompat.Type.systemBars()).bottom
            setPadding(b)
            insets
        }
    }
}

