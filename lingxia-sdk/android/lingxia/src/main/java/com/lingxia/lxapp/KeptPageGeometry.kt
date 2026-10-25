package com.lingxia.lxapp

/**
 * Where a kept GPU page container sits while hidden: as it will be shown, under its own
 * chrome, so its environment (and so its last frame) stays its own while other pages
 * change the shared parent's margins.
 */
internal object KeptPageGeometry {
    /** Page container parent margins for one page's chrome. */
    data class Margins(val top: Int, val bottom: Int, val left: Int, val right: Int)

    /** Size, and the offset from where the parent's current margins place a child. */
    data class Frame(val width: Int, val height: Int, val translationX: Float, val translationY: Float)

    /** [parentWidth] × [parentHeight] is the area before margins; [own] the page's margins when shown. */
    fun frame(parentWidth: Int, parentHeight: Int, own: Margins, current: Margins) = Frame(
        width = parentWidth - own.left - own.right,
        height = parentHeight - own.top - own.bottom,
        translationX = (own.left - current.left).toFloat(),
        translationY = (own.top - current.top).toFloat(),
    )
}
