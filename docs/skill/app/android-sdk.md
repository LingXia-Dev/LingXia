# Android SDK

Android host hooks beyond `Lingxia.quickStart(activity)`.

## URL player engine

Replace the default ExoPlayer URL backend of `lx.previewMedia` (and, opt-in,
`<LxVideo>`) with an engine the host APK ships, such as libmpv. Register on the
main thread before `quickStart`; load native `.so` files earlier, in
`Application.onCreate`.

```kotlin
Lingxia.setUrlPlayerEngineFactory(object : UrlPlayerEngineFactory {
    override fun preferredOutput(kind: UrlPlayerSurfaceKind) =
        UrlPlayerOutputKind.TEXTURE_VIEW // PREVIEW may opt into SURFACE_VIEW on API 24+

    override fun create(request: UrlPlayerEngineRequest): UrlPlayerEngine? {
        if (request.surfaceKind != UrlPlayerSurfaceKind.PREVIEW) return null
        return MpvPlayerEngine(request)
    }
})
Lingxia.quickStart(this) { registerHostAddon() }
```

- `create() == null` keeps ExoPlayer for that surface. `INLINE` (`<LxVideo>`,
  media swiper) output is always `TextureView`.
- Each player snapshots the factory when constructed; a later call does not
  swap live players.
- Emit `FirstFrameRendered` only once a frame reached the surface. No HTTP
  headers are passed to the engine.
