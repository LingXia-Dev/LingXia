# Launch Screen

The `splash:` config gives a host its launch screen (see the `splash`
section of the host project reference). That art is fixed: it ships inside
the app, and it is what the OS launch frame hands over to. Nothing picks it
at runtime, because nothing can — the OS composes the first frame from
build-time resources before your process exists, so art chosen later could
only ever disagree with the frame the user is already looking at. A "white
flash" or a mid-launch image swap is exactly that disagreement.

There is deliberately no dark counterpart. An appearance pair can only ever
follow the *system*, never an in-app appearance choice — HarmonyOS's colour
mode does not survive process death and iOS has no such lever at all — so
the pair's halves are what end up disagreeing at launch. One picture, every
appearance, is the version that always holds.

This page covers the native half: the **campaign**, a screen of your own
shown after the launch face, with a countdown the user can skip.

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

Three rules the design rests on:

- **The launch face is not yours to choose.** It is the configured art in
  every appearance, identical to what the OS frame handed over. The campaign is a
  second screen, and it fades in — the user reads it as content arriving,
  not as the launch stuttering.
- **Selection decides *this* launch.** It runs once the runtime is up, not
  on the cold-start path, so reading a file or checking a clock costs the
  launch nothing. It may only name files already on disk:
  `CampaignChoice::cached(key)` for the store, `CampaignChoice::path(p)` for
  app-owned storage, `CampaignChoice::none()` for no campaign.
  `duration_ms(ms)` sets how long it holds (default 3s, capped at 8s); the
  user can always skip sooner. An answer that arrives after the launch face
  is ready to lift is dropped — a launch that waits on a campaign is worse
  than a launch with no campaign.
- **Acquisition prepares *future* launches.** Hand it to `lingxia::spawn` —
  safe even this early in the process — and land the bytes with
  `splash::store(key, bytes)`: the write is atomic (temp file, then rename),
  so a launch can never select a half-downloaded image.
  Keys are identifiers, not paths: 1–128 ASCII letters, digits, `-`, or `_`.
  Normalize a server campaign ID to that form before passing it to the store.

The store is backed by app data, not the OS cache: a launch must find the
art without any network, and OS caches can be purged at any time. Nothing
deletes it behind your back; `splash::retain([...])` is the whole cleanup
story — keys not listed are deleted in the background. `store` and `retain`
are callable from anywhere, not just the hook.

Packaged art ships as host assets (`assets:` in `lingxia.yaml`, read with
`lingxia::assets::read`), not as bytes embedded in the native library:
packaged assets ride each platform's asset pipeline — store thinning, lazy
loading — and never weigh down library load, which sits on the first-frame
path.

Neither the launch face nor the campaign can change the background color:
that is baked into the OS launch frame at build time.
