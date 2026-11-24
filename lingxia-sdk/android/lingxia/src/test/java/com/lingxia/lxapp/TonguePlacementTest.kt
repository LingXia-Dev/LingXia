package com.lingxia.lxapp

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class TonguePlacementTest {
    private fun place(blocked: List<Pair<Float, Float>>, start: Float = 50f, end: Float = 80f) =
        TonguePlacement.place(400f, blocked, start, end, 88f, 8f)

    @Test fun clearEdgeCentresFlush() = assertEquals(156f, place(emptyList()))

    @Test fun blockedMiddleSlidesUpWhenOnlyAboveHasRoom() =
        assertEquals(84f, place(listOf(180f to 220f), end = 110f))

    @Test fun blockedMiddleSlidesDownWhenOnlyBelowHasRoom() =
        assertEquals(228f, place(listOf(180f to 220f), start = 120f))

    @Test fun noClearStretchFallsBackToInset() = assertNull(place(listOf(140f to 260f)))

    @Test fun wholeEdgeBlockedFallsBackToInset() = assertNull(place(listOf(0f to 400f)))

    // The caller passes only this edge's blocks; a cutout on the far edge adds none.
    @Test fun cutoutOnTheOtherEdgeLeavesThisOneClear() = assertEquals(156f, place(emptyList()))

    @Test fun prefersTheSideNearerTheMiddle() = assertEquals(104f, place(listOf(200f to 230f), end = 30f))
}
