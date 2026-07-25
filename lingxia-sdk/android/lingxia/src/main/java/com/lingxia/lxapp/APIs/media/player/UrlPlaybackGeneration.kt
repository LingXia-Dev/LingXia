package com.lingxia.lxapp.APIs.media.player

internal class UrlPlaybackGeneration {
    private var nextGeneration: Long = 0L
    private var currentMediaId: String? = null

    fun begin(): String {
        nextGeneration += 1L
        return "lingxia-url-$nextGeneration".also { currentMediaId = it }
    }

    fun invalidate() {
        currentMediaId = null
    }

    fun accepts(mediaId: String?): Boolean =
        mediaId != null && mediaId == currentMediaId
}
