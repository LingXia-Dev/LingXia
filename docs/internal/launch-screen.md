# Launch screen

The launch screen of a mobile host: the OS launch frame, the cover the app
draws on its first frame, and the campaign a host may show after them. Read
before changing `splash:` in `lingxia.yaml`, the CLI generation in
`tools/lingxia-cli/src/splash.rs`, the shared runtime in
`crates/lingxia/src/splash.rs` and `crates/lingxia-app-context`, or an SDK's
splash overlay (`SplashOverlay.kt` on Android, `LingXiaSplashOverlay.swift` on
Apple, `SplashOverlay.ets` and `LxAppNavigation.ets` on HarmonyOS).

`splash:` (`background`, optional `image`, `mark`, `minDuration`) drives the
launch screen; hosts build none of it by hand. Two OS-composed beats: the
launch frame, then the app's first frame, which is the cover (`image`,
aspect-fill over the same `background`), held until the home page first
renders. One shared `background` makes the frame read as the cover's entrance.

- **The launch frame carries what the OS can render sharp and on time.**
  HarmonyOS draws `image` full-screen in the start window (fit `Cover`, with
  a blank `startWindowIcon`); without `image` it centres `mark`, unscaled.
  iOS uses a generated launch storyboard with the same three variants: cover
  (aspect-fill), centred mark, or plain `background`. Android 12+ draws only
  `background` and its icon slot: the slot is blanked when `image` is set and
  keeps the launcher icon otherwise; `mark` is not used on Android.
  `background` is the only required field.
- **The cover is drawn on the first app frame** and is fully opaque, so on
  HarmonyOS and iOS it continues the launch frame without a seam and on
  Android the frame's own exit is the one transition onto it. Nothing heavier
  than building that frame may run before it: runtime initialization happens
  under the cover, never in front of it.
- **Without `image`, hold instead**: Android suspends the first draw so the
  system splash persists until home is ready; HarmonyOS and iOS render a first
  frame reproducing the launch frame exactly.
- Dismissal: the once-per-process home-first-render signal, held by the core
  until `minDuration` (default 600 ms, at most 6000 ms) measured from process
  start; Android measures its hold again from the cover's own first draw. A
  6 s timeout MUST dismiss regardless and MUST NOT be configurable. While up,
  the cover swallows input.
- **The launch face is build-time art.** No hook may substitute it: the OS
  frame is composed from build-time resources before the process exists, so
  a runtime choice can only disagree with what is already on screen.
- **One face, every appearance.** `splash:` has no dark counterpart: an
  appearance pair can only follow the system, never an in-app choice
  (measured: HarmonyOS `setColorMode` does not survive process death, iOS has
  no lever), so the pair's halves are what disagree at launch. The runtime
  still pushes an in-app appearance into the platform's own night mode where
  one persists (Android `UiModeManager`, API 31 and later), for everything the
  app draws after the launch.
- **Runtime hook.** A host MAY implement `HostAddon::select_campaign` to
  show its own screen after the launch face, with a countdown the user can
  skip. Selection runs after runtime init on its own thread, is on-disk only,
  and is dropped if it misses the moment the launch face lifts; acquisition
  goes through `lingxia::spawn` and MUST NOT block it. Duration defaults to
  3 s and is capped at 8 s. The store sits under app data, exempt from OS and
  framework eviction; `splash::store` writes it atomically and
  `splash::retain` bounds it. The hook cannot choose the background — that is
  baked into the launch frame at build time.
- Generated resource names are the CLI↔SDK contract: Android
  `lingxia_splash_background` / `lingxia_splash_image` /
  `lingxia_splash_light_bars` / `Theme.LingXia.Splash`; Apple
  `LingXiaSplashBackground` / `LingXiaSplash` / `LingXiaSplashMark` /
  `LingXiaSplashGround` / `LingXiaLaunchMark` and the `LingXiaLaunchScreen`
  storyboard; HarmonyOS `$color:lingxia_splash_background` /
  `$media:lingxia_splash` / `$media:lingxia_splash_mark` /
  `$media:lingxia_splash_blank`. Missing resources disable the runtime half on
  Apple and HarmonyOS; Android still holds the first draw on its normal theme,
  without a minimum hold. A host without `splash:` is unchanged.
- On Apple the runtime half MUST NOT depend on the compiled asset catalog
  (`actool` can fail): the cover and mark ship as plain bundle resources and
  the colour as an Info.plist key. The catalog keeps the colour only as a
  fallback for an older Info.plist.
- Desktop shells take no overlay. On macOS the ready signal is a no-op; on
  Windows it shows the first home window.

Not implemented: a Windows launch screen, and any channel that downloads
campaign art (hosts call `splash::store` themselves).
