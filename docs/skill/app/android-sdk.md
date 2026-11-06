# Android SDK

Android host hooks beyond `Lingxia.quickStart(activity)`.

## URL player engine

Replace the default ExoPlayer URL backend of `lx.previewMedia` and media
swiper video with an engine the host APK ships, such as libmpv. Register on the
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

- `create() == null` keeps ExoPlayer for that surface. `INLINE` (media swiper)
  output is always `TextureView`.
- Each player snapshots the factory when constructed; a later call does not
  swap live players.
- Emit `FirstFrameRendered` only once a frame reached the surface. No HTTP
  headers are passed to the engine.

## Permission gate

Called each time an lx API needs a permission it lacks (never once granted,
also after a denial), e.g. to explain the purpose first where a store requires
it. The host owns the UI and policy (per build flavor); without a gate LingXia
prompts directly.

```kotlin
Lingxia.setPermissionGate(object : PermissionGate {
    override fun beforePrompt(prompt: PermissionPrompt) {
        showPurposeDialog(prompt.activity, prompt.feature) { agreed ->
            if (agreed) prompt.proceed() else prompt.deny()
        }
    }
    override fun afterPrompt(prompt: PermissionPrompt, outcome: PermissionOutcome) {}
})
```

- `deny()` fails the lx API as permission denied without a system prompt and
  skips `afterPrompt`. An undecided prompt is denied when its activity is
  destroyed; prompts are gated one at a time.
- `prompt.blocked` (before) and `DENIED_PERMANENTLY` (after) mean the system
  will not prompt again: guide the user to Settings instead of explaining.
- Harmony has the same API (`import { Lingxia, PermissionGate, … } from 'lingxia'`),
  except `afterPrompt` is required. A denial there is always permanent, and
  `proceed()` on a blocked prompt opens the system re-grant sheet.
- iOS has the same shape in Swift; see [Apple SDK](./apple-sdk.md).
