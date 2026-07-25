package com.lingxia.lxapp.APIs.media

internal class MediaPreviewFrameRevealTracker {
    private data class Target(val generation: Long, val pagerPosition: Int)

    private var renderedGeneration: Long = NO_GENERATION
    private var revealedGeneration: Long = NO_GENERATION
    private var pendingTarget: Target? = null

    fun markRendered(generation: Long) {
        renderedGeneration = generation
    }

    fun hasRendered(generation: Long): Boolean = renderedGeneration == generation

    fun wasRevealed(generation: Long): Boolean = revealedGeneration == generation

    fun beginReveal(
        generation: Long,
        pagerPosition: Int,
        currentlyVisible: Boolean,
    ): Boolean {
        if (!hasRendered(generation)) return false
        val target = Target(generation, pagerPosition)
        if (pendingTarget == target) return false
        if (pendingTarget != null) return false
        if (wasRevealed(generation) && currentlyVisible) return false
        pendingTarget = target
        return true
    }

    fun isPending(generation: Long, pagerPosition: Int): Boolean =
        pendingTarget == Target(generation, pagerPosition)

    fun cancelReveal(generation: Long, pagerPosition: Int) {
        if (isPending(generation, pagerPosition)) {
            pendingTarget = null
        }
    }

    fun invalidatePendingReveal() {
        pendingTarget = null
    }

    fun commitReveal(generation: Long, pagerPosition: Int): Boolean {
        if (!isPending(generation, pagerPosition)) return false
        pendingTarget = null
        revealedGeneration = generation
        return true
    }

    private companion object {
        const val NO_GENERATION = -1L
    }
}
