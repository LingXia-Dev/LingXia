package com.lingxia.lxapp.chrome

import org.junit.Assert.assertEquals
import org.junit.Test

class TabBarInsetsTest {

    @Test
    fun transparentThreeButtonNavLiftsByTheFullStrip() {
        // Transparent and translucent bars have no root bottom padding.
        for (inset in listOf(140, 200)) {
            assertEquals(
                inset,
                TabBarInsets.extraBottomPx(inset, navHasBottomInset = true, density = 3f, transparent = true),
            )
        }
    }

    @Test
    fun transparentGestureNavKeepsTheOverlapAndCap() {
        assertEquals(102, TabBarInsets.extraBottomPx(120, navHasBottomInset = false, density = 3f, transparent = true))
        assertEquals(144, TabBarInsets.extraBottomPx(400, navHasBottomInset = false, density = 3f, transparent = true))
    }

    @Test
    fun threeButtonNavUsesTheVisibleStrip() {
        assertEquals(
            140,
            TabBarInsets.contentBottomPx(
                navVisible = true,
                navBottom = 140,
                systemBarsBottom = 140,
                mandatoryGesturesBottom = 0,
            ),
        )
        assertEquals(0, TabBarInsets.extraBottomPx(140, navHasBottomInset = true, density = 3f, transparent = false))
    }

    @Test
    fun gestureNavUsesMandatoryZoneNotZero() {
        // Huawei TFY-AN00: nav bar frame is 0-height, mandatory gestures are 120px.
        assertEquals(
            120,
            TabBarInsets.contentBottomPx(
                navVisible = false,
                navBottom = 0,
                systemBarsBottom = 0,
                mandatoryGesturesBottom = 120,
                systemGesturesBottom = 120,
            ),
        )
        assertEquals(102, TabBarInsets.extraBottomPx(120, navHasBottomInset = false, density = 3f, transparent = false))
    }

    @Test
    fun gestureNavFallsBackToSystemGesturesWhenMandatoryIsMissing() {
        assertEquals(
            80,
            TabBarInsets.contentBottomPx(
                navVisible = false,
                navBottom = 0,
                systemBarsBottom = 0,
                mandatoryGesturesBottom = 0,
                systemGesturesBottom = 80,
            ),
        )
    }

    @Test
    fun extraCapsAHugeGestureFallback() {
        assertEquals(144, TabBarInsets.extraBottomPx(400, navHasBottomInset = false, density = 3f, transparent = false))
    }

    @Test
    fun extraIsZeroWhenTheInsetIsOnlyTheOverlap() {
        assertEquals(0, TabBarInsets.extraBottomPx(18, navHasBottomInset = false, density = 3f, transparent = false))
    }

    @Test
    fun visibleNavWinsOverAStaleGestureZone() {
        assertEquals(
            96,
            TabBarInsets.contentBottomPx(
                navVisible = true,
                navBottom = 96,
                systemBarsBottom = 96,
                mandatoryGesturesBottom = 120,
            ),
        )
    }
}
