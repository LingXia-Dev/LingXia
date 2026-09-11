package com.lingxia.example.lxapp.mpv

import com.lingxia.app.media.UrlPlayerEngine
import com.lingxia.app.media.UrlPlayerEngineFactory
import com.lingxia.app.media.UrlPlayerEngineRequest
import com.lingxia.app.media.UrlPlayerOutputKind
import com.lingxia.app.media.UrlPlayerSurfaceKind

/** Opt-in showcase factory. Off by default; see MainActivity. */
class MpvUrlPlayerEngineFactory : UrlPlayerEngineFactory {
    override fun preferredOutput(kind: UrlPlayerSurfaceKind): UrlPlayerOutputKind =
        if (kind == UrlPlayerSurfaceKind.PREVIEW) {
            UrlPlayerOutputKind.SURFACE_VIEW
        } else {
            UrlPlayerOutputKind.TEXTURE_VIEW
        }

    override fun create(request: UrlPlayerEngineRequest): UrlPlayerEngine? {
        if (!MpvNative.available) return null
        return MpvPlayerEngine(request.context)
    }
}
