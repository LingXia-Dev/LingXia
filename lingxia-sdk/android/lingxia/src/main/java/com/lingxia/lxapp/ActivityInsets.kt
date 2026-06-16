package com.lingxia.lxapp

import android.view.View
import android.view.ViewGroup
import androidx.core.view.ViewCompat
import com.lingxia.app.Lingxia
import com.lingxia.lxapp.LxApp

/**
 * Helper object for querying activity-provided insets and applying them to transient UI.
 *
 * All overlays (pickers, sheets, media controls) should rely on the activity's notion of
 * "content inset" so they remain flush with gesture navigation while staying above the
 * legacy navigation bar when necessary.
 */
internal object ActivityInsets {

    /**
     * Current bottom inset exposed by [LxAppActivity.getContentBottomInset].
     * Defaults to zero when there is no active activity or no inset available.
     */
    @JvmStatic
    fun contentBottomInset(): Int {
        return LxApp.getCurrentActivity()?.getContentBottomInset() ?: 0
    }

    /**
     * Apply bottom margin adjustments to a transient view so it sits above system navigation.
     *
     * @param root The container that will receive window inset callbacks.
     * @param target The view whose bottom margin (or padding) should be adjusted.
     * @param extra Additional pixels to add on top of the activity-provided inset.
     */
    @JvmStatic
    fun applyBottomMargin(root: ViewGroup, target: View, extra: Int) {
        fun applyInset() {
            val bottom = (contentBottomInset() + extra).coerceAtLeast(0)
            val lp = target.layoutParams
            if (lp is ViewGroup.MarginLayoutParams) {
                if (lp.bottomMargin != bottom) {
                    lp.bottomMargin = bottom
                    target.layoutParams = lp
                }
            } else {
                if (target.paddingBottom != bottom) {
                    target.setPadding(
                        target.paddingLeft,
                        target.paddingTop,
                        target.paddingRight,
                        bottom
                    )
                }
            }
        }

        // Apply immediately after the current layout pass.
        root.post { applyInset() }

        // Re-apply whenever window insets change for this container.
        ViewCompat.setOnApplyWindowInsetsListener(root) { view, insets ->
            applyInset()
            insets
        }

        if (root.isAttachedToWindow) {
            ViewCompat.requestApplyInsets(root)
        } else {
            root.addOnAttachStateChangeListener(object : View.OnAttachStateChangeListener {
                override fun onViewAttachedToWindow(v: View) {
                    v.removeOnAttachStateChangeListener(this)
                    ViewCompat.requestApplyInsets(v)
                }

                override fun onViewDetachedFromWindow(v: View) = Unit
            })
        }
    }
}
