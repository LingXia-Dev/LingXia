package com.lingxia.lxapp

import com.lingxia.lxapp.chrome.TabBarState

/**
 * Where a kept GPU page container sits while hidden: as it will be shown, under its own
 * chrome, so its environment (and so its last frame) stays its own while other pages
 * change the shared parent's margins.
 */
internal object KeptPageGeometry {
    /** Page container parent margins for one page's chrome. */
    data class Margins(val top: Int, val bottom: Int, val left: Int, val right: Int)

    /** What a page shows around it; the pixels come from the current [Bars]. */
    data class Chrome(val navigationBar: Boolean, val tabBar: Boolean, val fullscreen: Boolean)

    /**
     * Bar sizes in the current window. They change with configuration (a three-button
     * navigation bar moves to the side in landscape), so kept pages are framed from these,
     * never from margins recorded under another configuration.
     */
    data class Bars(
        val navigationBar: Int,
        val tabBarHeight: Int,
        val tabBarWidth: Int,
        val tabBarTransparent: Boolean,
        val position: TabBarState.Position?,
        val systemBottom: Int,
    )

    /** Size, and the offset from where the parent's current margins place a child. */
    data class Frame(val width: Int, val height: Int, val translationX: Float, val translationY: Float)

    fun margins(chrome: Chrome, bars: Bars, ime: Int = 0): Margins {
        var bottom = 0
        var left = 0
        var right = 0
        if (chrome.tabBar && !bars.tabBarTransparent) when (bars.position) {
            TabBarState.Position.BOTTOM -> bottom = bars.tabBarHeight
            TabBarState.Position.LEFT -> left = bars.tabBarWidth
            TabBarState.Position.RIGHT -> right = bars.tabBarWidth
            null -> {}
        }
        // Pages without the tab bar stay above the gesture / navigation zone.
        if (!chrome.tabBar && !chrome.fullscreen) bottom = maxOf(bottom, bars.systemBottom)
        return Margins(
            top = if (chrome.navigationBar) bars.navigationBar else 0,
            bottom = maxOf(bottom, ime),
            left = left,
            right = right,
        )
    }

    /** Chrome overlap a page reports as insets: left, right, bottom. */
    data class Overlap(val left: Int, val right: Int, val bottom: Int)

    /** Edges in root coordinates. */
    data class Box(val left: Int, val top: Int, val right: Int, val bottom: Int)

    /**
     * How far the page's own tab bar overlaps the page when shown, as the shown page
     * measures it on screen. Margins already keep an opaque bar off the page.
     */
    fun overlap(page: Box, bar: Box, chrome: Chrome, position: TabBarState.Position?): Overlap {
        if (!chrome.tabBar) return Overlap(0, 0, 0)
        if (minOf(page.right, bar.right) <= maxOf(page.left, bar.left) ||
            minOf(page.bottom, bar.bottom) <= maxOf(page.top, bar.top)) return Overlap(0, 0, 0)
        return when (position) {
            TabBarState.Position.BOTTOM -> Overlap(0, 0, page.bottom - bar.top)
            TabBarState.Position.LEFT -> Overlap(bar.right - page.left, 0, 0)
            TabBarState.Position.RIGHT -> Overlap(0, page.right - bar.left, 0)
            null -> Overlap(0, 0, 0)
        }
    }

    /** [parentWidth] × [parentHeight] is the area before margins; [own] the page's margins when shown. */
    fun frame(parentWidth: Int, parentHeight: Int, own: Margins, current: Margins) = Frame(
        width = parentWidth - own.left - own.right,
        height = parentHeight - own.top - own.bottom,
        translationX = (own.left - current.left).toFloat(),
        translationY = (own.top - current.top).toFloat(),
    )
}
