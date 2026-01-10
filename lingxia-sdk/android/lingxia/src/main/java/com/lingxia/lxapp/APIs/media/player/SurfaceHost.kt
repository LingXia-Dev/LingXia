package com.lingxia.lxapp.APIs.media.player

import android.view.TextureView
import android.view.View
import androidx.media3.ui.PlayerView

data class SurfaceToken(
    val id: String,
    val generation: Int,
    val ownerKey: String
) {
    fun isSameOwner(other: SurfaceToken): Boolean = ownerKey == other.ownerKey

    fun isNewerThan(other: SurfaceToken): Boolean =
        ownerKey == other.ownerKey && generation > other.generation
}

internal class SurfaceHost(
    private val ownerKey: String,
    private val urlPlayerView: PlayerView,
    private val feedTextureView: TextureView,
) {
    private var feedGeneration: Int = 0

    fun setActiveBackend(backend: BackendKind) {
        when (backend) {
            BackendKind.URL -> {
                urlPlayerView.visibility = View.VISIBLE
                feedTextureView.visibility = View.GONE
            }
            BackendKind.FEED -> {
                feedTextureView.visibility = View.VISIBLE
                urlPlayerView.visibility = View.GONE
            }
        }
    }

    fun getFeedTextureView(): TextureView = feedTextureView

    fun nextFeedSurfaceToken(): SurfaceToken {
        feedGeneration += 1
        return SurfaceToken(
            id = "feedTextureView@" + System.identityHashCode(feedTextureView).toString(16),
            generation = feedGeneration,
            ownerKey = ownerKey,
        )
    }
}
