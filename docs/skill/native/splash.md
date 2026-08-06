# Launch Cover

`splash:` in `lingxia.yaml` gives a host its launch screen without building
any UI: the OS launch frame (the `background` color with a small centered
`mark`) hands off to the cover (`image`), rendered full-screen as the app's
very first frame and held until the home page first renders. The runtime
boots underneath the cover, never in front of it.

A host addon can substitute the cover *file* per launch — a downloaded
campaign, a seasonal variant — by implementing `select_splash`:

```rust
impl lingxia::HostAddon for AppHostAddon {
    fn select_splash(&self, launch: &lingxia::splash::Launch) -> lingxia::splash::SplashChoice {
        // Fetch next launch's campaign; never blocks this one.
        let dest = launch.cache_dir().join("campaign.png");
        lingxia::spawn(async move { download_campaign_cover(dest).await });

        // Decide this launch, from files already on disk only.
        if launch.cached("campaign").is_some() {
            return lingxia::splash::SplashChoice::cached("campaign");
        }
        lingxia::splash::SplashChoice::bundled()
    }
}
```

The two halves must not be mixed:

- **Selection** decides *this* launch. It is synchronous, runs on the
  cold-start path under a small budget (the bundled cover wins on overrun),
  and may only pick files already on disk: `SplashChoice::cached(key)` for
  the managed cache, `SplashChoice::path(p)` for app-owned storage,
  `SplashChoice::bundled()` for the configured cover.
- **Acquisition** prepares the *next* launch. Hand it to `lingxia::spawn` —
  safe even this early in the process — and write into
  `launch.cache_dir()`; the file becomes selectable on the next cold start.

The hook cannot change the background color: that is baked into the OS
launch frame at build time, so a runtime override could only disagree with
the frame already on screen. `launch.is_dark()` exposes the appearance that
frame is showing, and `SplashChoice::min_duration_ms` can lengthen the hold
for a launch that deserves it.
