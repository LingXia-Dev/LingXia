package com.lingxia.lxapp.APIs.media.player

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class UrlPlaybackGenerationTest {
    @Test
    fun newPlaybackRejectsEventsFromPreviousSource() {
        val generation = UrlPlaybackGeneration()
        val previous = generation.begin()

        val current = generation.begin()

        assertFalse(generation.accepts(previous))
        assertTrue(generation.accepts(current))
    }

    @Test
    fun stoppingPlaybackRejectsQueuedEvents() {
        val generation = UrlPlaybackGeneration()
        val mediaId = generation.begin()

        generation.invalidate()

        assertFalse(generation.accepts(mediaId))
    }
}
