package com.lingxia.lxapp.APIs.media

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class MediaPreviewFrameRevealTrackerTest {
    @Test
    fun revealedFrameCanBeRestoredAfterHostWasHidden() {
        val tracker = MediaPreviewFrameRevealTracker()
        tracker.markRendered(GENERATION)

        assertTrue(tracker.beginReveal(GENERATION, PAGER_POSITION, currentlyVisible = false))
        assertTrue(tracker.commitReveal(GENERATION, PAGER_POSITION))
        assertFalse(tracker.beginReveal(GENERATION, PAGER_POSITION, currentlyVisible = true))
        assertTrue(tracker.beginReveal(GENERATION, PAGER_POSITION, currentlyVisible = false))
    }

    @Test
    fun abortedRevealCanRetryWhenPagerReturnsIdle() {
        val tracker = MediaPreviewFrameRevealTracker()
        tracker.markRendered(GENERATION)

        assertTrue(tracker.beginReveal(GENERATION, PAGER_POSITION, currentlyVisible = false))
        tracker.cancelReveal(GENERATION, PAGER_POSITION)

        assertTrue(tracker.beginReveal(GENERATION, PAGER_POSITION, currentlyVisible = false))
        assertTrue(tracker.commitReveal(GENERATION, PAGER_POSITION))
    }

    @Test
    fun invalidatedPendingRevealCannotCommitForPreviousActivation() {
        val tracker = MediaPreviewFrameRevealTracker()
        tracker.markRendered(GENERATION)
        assertTrue(tracker.beginReveal(GENERATION, PAGER_POSITION, currentlyVisible = false))

        tracker.invalidatePendingReveal()

        assertFalse(tracker.commitReveal(GENERATION, PAGER_POSITION))
        assertFalse(tracker.hasRendered(GENERATION + 1L))
        assertFalse(
            tracker.beginReveal(
                GENERATION + 1L,
                PAGER_POSITION + 1,
                currentlyVisible = false,
            )
        )
    }

    private companion object {
        const val GENERATION = 7L
        const val PAGER_POSITION = 3
    }
}
