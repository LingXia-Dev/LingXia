# hello-host-js — Shape B: native host app with **JS Logic** lxapp

A macOS host shell that boots, opens one window, and renders an embedded "home" lxapp inside it. The home lxapp's Logic is **JavaScript** (`Page({})` in `index.ts`); the host compiles in the JS AppService runtime (`features.appService: true`). No Rust route — the View can still call Rust if you add one, but this example keeps the slate clean to focus on the host ↔ JS-lxapp wiring.

For the **Rust Logic** variant (no JS runtime, HTML-only view, `#[lingxia::native]` doing all the work), see [`../hello-host-rust/`](../hello-host-rust/README.md). The two examples differ only on the JS vs Rust Logic split; the host wiring is the same.

This is what `lingxia new my-app -t native-app -p macos --package-id com.example.my-app -y` produces in a minimal form. Real scaffolding adds platform-specific subdirectories under `macos/`, generated icons, and CI scripts; those are reproducible from this `lingxia.yaml` and not shown here.

Files:

- `lingxia.yaml` — host project source of truth: app metadata, target platform(s), feature flags, bundled lxapp sources, App UI (surfaces + activators).
- `home/` — the embedded "home" lxapp this host opens by default (same JS-Logic shape as [`../hello-lxapp/`](../hello-lxapp/README.md) — see that example for page Logic + View detail).

What to study:

- `app.homeAppId` matches `resources.bundles[].appId` matches the embedded `lxapp.json.appId`. **All three must agree** or the build fails / the wrong app launches.
- `ui.surfaces[].content.appId` points at the same `homeAppId` — that's what fills the macOS window.
- `features.appService: true` enables the JS Logic runtime. **Required** because the embedded lxapp has a `.ts` Logic file. (For the opposite case, see `../hello-host-rust/`.)

Cross-references:

- `lingxia.yaml` full reference (every field, every section): [`../../app/project.md`](../../app/project.md)
- macOS App UI (surfaces, activators, attach panels, menu-bar): [`../../app/project.md#macos-app-ui`](../../app/project.md#macos-app-ui)
- Apple SDK startup APIs (`Lingxia.quickStart()`): [`../../app/apple-sdk.md`](../../app/apple-sdk.md)
- Sibling Shape C example (Rust Logic): [`../hello-host-rust/`](../hello-host-rust/README.md)
