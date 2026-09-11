package com.lingxia.lxapp.APIs.media.player

import android.view.TextureView
import android.view.View

internal data class SurfaceToken(
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
    private val urlOutputContainer: View,
    private val feedTextureView: TextureView,
    urlOutputView: View,
) {
    private var feedGeneration: Int = 0
    private var stableUrlToken: SurfaceToken = tokenFor(urlOutputView)

    fun setActiveBackend(backend: BackendKind) {
        when (backend) {
            BackendKind.URL -> {
                urlOutputContainer.visibility = View.VISIBLE
                feedTextureView.visibility = View.GONE
            }
            BackendKind.FEED -> {
                feedTextureView.visibility = View.VISIBLE
                urlOutputContainer.visibility = View.GONE
            }
        }
    }

    fun getFeedTextureView(): TextureView = feedTextureView

    fun stableUrlToken(): SurfaceToken = stableUrlToken

    fun replaceUrlTokenFor(view: View): SurfaceToken {
        stableUrlToken = tokenFor(view)
        return stableUrlToken
    }

    fun nextFeedSurfaceToken(): SurfaceToken {
        feedGeneration += 1
        return SurfaceToken(
            id = "feedTextureView@" + System.identityHashCode(feedTextureView).toString(16),
            generation = feedGeneration,
            ownerKey = ownerKey,
        )
    }

    private fun tokenFor(view: View): SurfaceToken = SurfaceToken(
        id = "urlOutput@" + System.identityHashCode(view).toString(16),
        generation = 0,
        ownerKey = ownerKey,
    )
}
