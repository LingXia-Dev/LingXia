package com.lingxia.lxapp.APIs.media.player

import com.lingxia.app.media.UrlPlayerOutputKind
import com.lingxia.app.media.UrlPlayerSurfaceKind

internal data class UrlOutputResolution(
    val kind: UrlPlayerOutputKind,
    val forceReason: String?,
)

internal object UrlOutputPlanner {
    const val SURFACE_VIEW_MIN_SDK = 24

    fun resolve(
        surfaceKind: UrlPlayerSurfaceKind,
        preferred: UrlPlayerOutputKind,
        sdkInt: Int,
    ): UrlOutputResolution {
        if (preferred != UrlPlayerOutputKind.SURFACE_VIEW) {
            return UrlOutputResolution(UrlPlayerOutputKind.TEXTURE_VIEW, forceReason = null)
        }
        if (surfaceKind != UrlPlayerSurfaceKind.PREVIEW) {
            return UrlOutputResolution(
                UrlPlayerOutputKind.TEXTURE_VIEW,
                forceReason = "inline-surface-view",
            )
        }
        if (sdkInt < SURFACE_VIEW_MIN_SDK) {
            return UrlOutputResolution(
                UrlPlayerOutputKind.TEXTURE_VIEW,
                forceReason = "api-lt-24",
            )
        }
        return UrlOutputResolution(UrlPlayerOutputKind.SURFACE_VIEW, forceReason = null)
    }

    fun shouldEagerCreate(kind: UrlPlayerOutputKind): Boolean =
        kind == UrlPlayerOutputKind.SURFACE_VIEW
}
