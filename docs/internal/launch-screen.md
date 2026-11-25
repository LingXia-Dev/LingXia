# Launch screen

The launch screen of a mobile host: the OS launch frame, the cover the app
draws on its first frame, and the campaign a host may show after them. The
`splash:` block of `lingxia.yaml` (`background`, optional `image`, `mark`,
`minDuration`) drives all of it; hosts build none of it by hand.

| Piece | Owner |
| --- | --- |
| Resource generation | `lingxia-cli` |
| Hold time, home-ready signal | `lingxia-app-context` |
| Campaign hook and store | `lingxia` (`lingxia::splash`) |
| Overlay | Android, Apple, and HarmonyOS platform SDKs |

Invariants:

- The launch face is build-time art. No runtime hook may substitute it.
- Nothing heavier than building the first app frame runs before it: runtime
  initialization happens under the cover, never in front of it.
- A 6 s timeout always dismisses the cover and is not configurable.
- `splash:` has one face for every appearance.

## Two beats

The OS composes two frames: the launch frame, then the app's first frame,
which is the cover (`image`, aspect-fill over the same `background`), held
until the home page first renders. One shared `background` makes the launch
frame read as the cover's entrance. `background` is the only required field.

The launch frame carries only what the OS can render sharp and on time:

| Platform | Launch frame |
| --- | --- |
| HarmonyOS | `image` full-screen in the start window (fit `Cover`, blank `startWindowIcon`); without `image`, `mark` centred and unscaled |
| iOS | A generated launch storyboard: cover (aspect-fill), centred mark, or plain `background` |
| Android 12+ | `background` and the icon slot only: the slot is blank when `image` is set and keeps the launcher icon otherwise; `mark` is unused |

The cover is fully opaque, so on HarmonyOS and iOS it continues the launch
frame without a seam; on Android the system splash's own exit is the one
transition onto it.

Without `image` the app holds instead: Android suspends the first draw so the
system splash stays until home is ready; HarmonyOS and iOS draw a first frame
that reproduces the launch frame exactly.

## Dismissal

- The signal is the once-per-process home-first-render, held by the core until
  `minDuration` (default 600 ms, capped at 6000 ms) measured from process
  start. Android measures its hold again from the cover's own first draw,
  because its system splash cannot carry the art.
- The 6 s timeout dismisses regardless, so a broken home page cannot strand
  the user on the cover.
- While up, the cover swallows input.

## One face, every appearance

`splash:` has no dark counterpart. An appearance pair can only follow the
system, never an in-app choice (HarmonyOS `setColorMode` does not survive
process death; iOS has no lever), so the two halves would disagree at launch.
The runtime still pushes an in-app appearance into the platform's own night
mode where one persists (Android `UiModeManager`, API 31 and later), for
everything the app draws after the launch.

## Campaign hook

A host may implement `HostAddon::select_campaign` to show its own screen after
the launch face, with a countdown the user can skip.

- Selection runs after runtime init on its own thread, picks only among files
  already on disk, and is dropped if it misses the moment the launch face
  lifts. A launch that waits on a campaign is worse than one with no campaign.
- Acquisition (downloading) goes through `lingxia::spawn`, must not block
  selection, and lands for a later launch. The runtime downloads nothing
  itself.
- Duration defaults to 3 s and is capped at 8 s; the host owns the duration,
  the framework owns the ceiling.
- The store sits under app data, not the OS cache, so a launch finds the art
  offline and nothing evicts it. `splash::store` writes it atomically under an
  identifier key; `splash::retain` bounds it.
- The hook cannot choose the background or the launch face: both are baked
  into the launch frame before the process exists, and a runtime choice could
  only disagree with what is already on screen.

## Resource contract

Generated resource names are the contract between the CLI and the SDKs:

| Platform | Names |
| --- | --- |
| Android | `lingxia_splash_background`, `lingxia_splash_image`, `lingxia_splash_light_bars`, `Theme.LingXia.Splash` |
| Apple | `LingXiaSplashBackground`, `LingXiaSplash`, `LingXiaSplashMark`, `LingXiaSplashGround`, `LingXiaLaunchMark`, storyboard `LingXiaLaunchScreen` |
| HarmonyOS | `$color:lingxia_splash_background`, `$media:lingxia_splash`, `$media:lingxia_splash_mark`, `$media:lingxia_splash_blank` |

Missing resources disable the runtime half on Apple and HarmonyOS; Android
still holds the first draw on its normal theme, without a minimum hold. A host
without `splash:` is unchanged.

On Apple the runtime half must not depend on the compiled asset catalog,
because `actool` can fail: the cover and mark ship as plain bundle resources
and the colour as the `LingXiaSplashBackground` Info.plist key. The catalog
colour is only a fallback when that key is absent.

Desktop shells take no overlay. On macOS the ready signal is a no-op; on
Windows it shows the first home window.
