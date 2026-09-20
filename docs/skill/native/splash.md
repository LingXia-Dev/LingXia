# Launch Screen

This page covers the **campaign**: a screen of your own, shown after the
launch face, with a countdown the user can skip.

The launch face itself is not on this page and is not yours to pick at
runtime — it is the fixed `splash:` art, one picture in every appearance.
See the `splash` section of the host project reference for why.

```rust
impl lingxia::HostAddon for AppHostAddon {
    fn select_campaign(&self, launch: &lingxia::splash::Launch) -> lingxia::splash::CampaignChoice {
        use lingxia::splash;

        // Art not listed here is deleted in the background.
        splash::retain(["promo", "night"]);

        // Prepare future launches; never blocks this one.
        lingxia::spawn(async {
            if let Ok(bytes) = download_promo().await {
                let _ = splash::store("promo", &bytes);
            }
        });

        // Decide this launch, from files already on disk only.
        // `launch.is_dark()` is the appearance the launch face is showing —
        // match it rather than re-reading system settings.
        if launch.is_dark() && launch.cached("night").is_some() {
            return splash::CampaignChoice::cached("night").duration_ms(3000);
        }
        if launch.cached("promo").is_some() {
            return splash::CampaignChoice::cached("promo");
        }
        splash::CampaignChoice::none()
    }
}
```

Three rules:

- **The campaign is a second screen, not the launch face.** It fades in, so it
  reads as content arriving rather than the launch stuttering.
- **Selection decides *this* launch.** It runs once the runtime is up, off the
  cold-start path, and may only name files already on disk:
  `CampaignChoice::cached(key)` for the store, `CampaignChoice::path(p)` for
  app-owned storage, `CampaignChoice::none()` for none. `duration_ms(ms)` sets
  how long it holds (default 3s, capped at 8s) and the user can always skip
  sooner. An answer arriving after the launch face is ready to lift is dropped.
- **Acquisition prepares *future* launches.** Hand it to `lingxia::spawn` —
  safe even this early — and land the bytes with `splash::store(key, bytes)`,
  which writes atomically, so a launch can never select a half-downloaded
  image. Keys are identifiers, not paths: 1–128 ASCII letters, digits, `-`, or
  `_`; normalize a server campaign id before storing it.

The store is app data, not OS cache: a launch must find the art with no network,
and OS caches can be purged at any time. `splash::retain([...])` is the whole
cleanup story — unlisted keys are deleted in the background — and both it and
`store` are callable from anywhere, not just the hook.

Packaged art ships as host assets (`assets:` in `lingxia.yaml`, read with
`lingxia::assets::read`), never as bytes embedded in the native library, which
sits on the first-frame path.

Neither the launch face nor the campaign can change the background color:
that is baked into the OS launch frame at build time.
