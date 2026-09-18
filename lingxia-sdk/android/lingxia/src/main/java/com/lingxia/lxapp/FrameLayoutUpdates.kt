package com.lingxia.lxapp

import android.view.View
import android.widget.FrameLayout

/** Insets can be dispatched during traversal; identical params must not dirty the tree again. */
internal fun View.updateFrameLayoutParamsIfChanged(next: FrameLayout.LayoutParams): Boolean {
    val current = layoutParams as? FrameLayout.LayoutParams
    if (current != null &&
        current.width == next.width && current.height == next.height &&
        current.gravity == next.gravity &&
        current.leftMargin == next.leftMargin && current.topMargin == next.topMargin &&
        current.rightMargin == next.rightMargin && current.bottomMargin == next.bottomMargin &&
        current.isMarginRelative == next.isMarginRelative &&
        (!current.isMarginRelative ||
            (current.marginStart == next.marginStart && current.marginEnd == next.marginEnd))
    ) {
        return false
    }
    // setLayoutParams already requests layout. Do not request a second pass.
    layoutParams = next
    return true
}
