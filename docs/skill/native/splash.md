# Launch Cover

The `splash:` config gives a host its launch screen (see the `splash`
section of the host project reference). This page covers the native half: a
host addon can substitute the cover *file* per launch — a downloaded
campaign, a seasonal variant — by implementing `select_splash`:

```rust
impl lingxia::HostAddon for AppHostAddon {
    fn select_splash(&self, launch: &lingxia::splash::Launch) -> lingxia::splash::SplashChoice {
        // Covers not listed here are deleted in the background.
        lingxia::splash::retain(["campaign"]);

        // Fetch next launch's campaign; never blocks this one.
        lingxia::spawn(async {
            if let Ok(bytes) = download_campaign_cover().await {
                let _ = lingxia::splash::store("campaign", &bytes);
            }
        });

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
  safe even this early in the process — and land the bytes with
  `splash::store(key, bytes)`: the write is atomic (temp file, then
  rename), so a launch can never select a half-downloaded cover.

The store is backed by app data, not the OS cache: the next cold start must
find the cover before any code or network runs, and OS caches can be purged
at any time. Nothing deletes it behind your back; `splash::retain([...])`
is the whole cleanup story — covers whose keys aren't listed are deleted in
the background. Both are callable from anywhere, not just the hook.

The hook cannot change the background color: that is baked into the OS
launch frame at build time, so a runtime override could only disagree with
the frame already on screen. `launch.is_dark()` exposes the appearance that
frame is showing, and `SplashChoice::min_duration_ms` can lengthen the hold
for a launch that deserves it.
