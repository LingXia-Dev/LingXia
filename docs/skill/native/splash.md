# Launch Cover

The `splash:` config gives a host its launch screen (see the `splash`
section of the host project reference). This page covers the native half: a
host addon can substitute the cover *file* per launch by implementing
`select_splash`.

```rust
impl lingxia::HostAddon for AppHostAddon {
    fn select_splash(&self, launch: &lingxia::splash::Launch) -> lingxia::splash::SplashChoice {
        use lingxia::splash;

        // Covers not listed here are deleted in the background.
        splash::retain(["campaign", "night"]);

        // Prepare future launches; never blocks this one. Sources: the
        // network, or a cover packaged via `assets:` in lingxia.yaml.
        lingxia::spawn(async {
            if let Ok(bytes) = lingxia::assets::read("covers/night.png") {
                let _ = splash::store("night", &bytes);
            }
            if let Ok(bytes) = download_campaign_cover().await {
                let _ = splash::store("campaign", &bytes);
            }
        });

        // Decide this launch, from files already on disk only.
        // `launch.is_dark()` is the appearance the launch frame is already
        // showing — match it rather than re-reading system settings.
        if launch.is_dark() && launch.cached("night").is_some() {
            return splash::SplashChoice::cached("night");
        }
        if launch.cached("campaign").is_some() {
            return splash::SplashChoice::cached("campaign");
        }
        splash::SplashChoice::bundled()
    }
}
```

The two halves must not be mixed:

- **Selection** decides *this* launch. It is synchronous, runs on the
  cold-start path under a small budget (the bundled cover wins on overrun),
  and may only pick files already on disk: `SplashChoice::cached(key)` for
  the store, `SplashChoice::path(p)` for app-owned storage,
  `SplashChoice::bundled()` for the configured cover.
  `SplashChoice::min_duration_ms(ms)` lengthens the hold for a launch that
  deserves it.
- **Acquisition** prepares *future* launches. Hand it to `lingxia::spawn` —
  safe even this early in the process — and land the bytes with
  `splash::store(key, bytes)`: the write is atomic (temp file, then
  rename), so a launch can never select a half-downloaded cover.

The store is backed by app data, not the OS cache: the next cold start must
find the cover before any code or network runs, and OS caches can be purged
at any time. Nothing deletes it behind your back; `splash::retain([...])`
is the whole cleanup story — covers whose keys aren't listed are deleted in
the background. `store` and `retain` are callable from anywhere, not just
the hook.

Extra covers ship as packaged host assets (`assets:` in `lingxia.yaml`,
read with `lingxia::assets::read`), not as bytes embedded in the native
library: packaged assets ride each platform's asset pipeline — store
thinning, lazy loading — and never weigh down library load, which sits on
the first-frame path. `lingxia::assets` needs the initialized runtime, so
the copy into the store happens in acquisition; from the second launch on,
the cover is selectable.

The hook cannot change the background color: that is baked into the OS
launch frame at build time, so a runtime override could only disagree with
the frame already on screen.
