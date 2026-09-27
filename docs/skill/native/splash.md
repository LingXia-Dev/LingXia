# Launch screen

A campaign screen of your own, shown after the fixed launch face
([`splash`](../app/project.md#splash)), with a countdown the user can skip.

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

- **Selection decides this launch** from files already on disk:
  `CampaignChoice::cached(key)`, `CampaignChoice::path(p)` for app-owned
  storage, or `CampaignChoice::none()`. `duration_ms(ms)` holds it (default
  3 s, max 8 s); the user can always skip. An answer arriving after the launch
  face is ready is dropped.
- **Acquisition prepares future launches.** Download in `lingxia::spawn` and
  save with `splash::store(key, bytes)`, which writes atomically. Keys are 1–128
  ASCII letters, digits, `-`, or `_`.
- **`splash::retain([...])` is the cleanup**: unlisted keys are deleted. It and
  `store` work from anywhere. The store is app data, so art survives offline.

Ship packaged art as host [`assets`](../app/project.md#assets), not bytes in the
native library. The background colour is fixed at build time.
