package com.lingxia.lxapp.chrome

import kotlin.math.roundToInt

/**
 * Bottom inset math shared by [com.lingxia.lxapp.LxAppActivity] and overlays.
 *
 * A 3-button navigation bar is a reserved strip (`systemBars.bottom`). Gesture
 * navigation reports that strip as 0 and instead carves a tap-eating zone
 * (`mandatorySystemGestures`). The TabBar must grow or lift into that zone —
 * returning 0 (the old "keep content flush" path) parked the strip under the
 * home indicator, and `FLAG_LAYOUT_NO_LIMITS` then clipped it.
 *
 * Matches Harmony `tabBarBottomPadding` / `tabBarExtraBottomHeight`: the
 * gesture zone only eats taps in its lower part, so a small overlap keeps the
 * bar visually snug to the screen bottom.
 */
internal object TabBarInsets {
    const val GESTURE_ZONE_OVERLAP_DP = 6
    /** Cap for the systemGestures fallback so a swipe-edge inset cannot grow the strip by half a screen. */
    const val MAX_GESTURE_EXTRA_DP = 48

    /**
     * How far above the physical bottom chrome and sheets should sit.
     *
     * Prefer the visible navigation bar when it actually occupies space;
     * otherwise the mandatory gesture zone. `systemGestures` is a last resort
     * — some OEMs leave `mandatorySystemGestures` at 0.
     */
    fun contentBottomPx(
        navVisible: Boolean,
        navBottom: Int,
        systemBarsBottom: Int,
        mandatoryGesturesBottom: Int,
        systemGesturesBottom: Int = 0,
    ): Int {
        val visibleNav = maxOf(navBottom, systemBarsBottom)
        if (navVisible && visibleNav > 0) {
            return visibleNav
        }
        if (mandatoryGesturesBottom > 0) {
            return mandatoryGesturesBottom
        }
        return systemGesturesBottom.coerceAtLeast(0)
    }

    /**
     * Extra pixels a horizontal TabBar grows (opaque) or lifts (transparent).
     *
     * Only opaque bars have the visible navigation strip reserved by root
     * padding. Transparent bars must lift by its full height, without the
     * gesture overlap or cap.
     */
    fun extraBottomPx(
        contentInsetPx: Int,
        navHasBottomInset: Boolean,
        density: Float,
        transparent: Boolean,
    ): Int {
        if (contentInsetPx <= 0) {
            return 0
        }
        if (navHasBottomInset) {
            return if (transparent) contentInsetPx else 0
        }
        val overlap = (GESTURE_ZONE_OVERLAP_DP * density).roundToInt()
        val extra = (contentInsetPx - overlap).coerceAtLeast(0)
        val cap = (MAX_GESTURE_EXTRA_DP * density).roundToInt()
        return extra.coerceAtMost(cap)
    }
}
