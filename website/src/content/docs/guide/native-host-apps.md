---
title: Native host apps
description: Configure an installable multi-platform host, embedded lxapps, capabilities, surfaces, and Rust extensions.
sidebar:
  order: 6
---

A native host app is the installable product shell for Android, iOS, macOS, Windows, and HarmonyOS. It owns `lingxia.yaml`, native platform projects, a Rust host crate, and one embedded home lxapp.

## Scaffold the source of truth

```bash
lingxia new my-app -t native-app -p macos,windows \
  --package-id com.example.myapp -y
```

Read the generated `lingxia.yaml` for the exact fields supported by your installed CLI. `lingxia build` compiles it into runtime `app.json` and `ui.json`; those generated files are never authoring surfaces.

## Keep the home ids aligned

Three values must agree:

- `app.homeAppId`
- one `resources.bundles[].appId`
- that bundle's `lxapp.json.appId`

The launch main surface's `lxapp:` value must point at the same home app. Misalignment either fails the build or launches the wrong content.

## Capabilities and surfaces

Declare host integrations before using them. `capabilities.browser` enables the in-app browser, `terminal` enables the native terminal surface, `process` unlocks trusted desktop process APIs, and `autostart` exposes user-controlled startup registration. Ordinary APIs such as camera are requested when called and do not belong in this list.

Use the top-level `surfaces:` list to describe main, aside, and tray content. See [Adaptive surfaces](../adaptive-surfaces/) for the current schema.

## JavaScript Logic or native-only Rust

Most hosts keep `features.appService: true` and embed a normal lxapp with JS Logic. A native-only host flips both sides together:

- `features.appService: false` in `lingxia.yaml`
- `"logic": false` in the home `lxapp.json`

That shape uses an HTML-only View and Rust for Logic. A logic-enabled lxapp under an appService-disabled host is rejected at startup.

## Add host-specific Rust APIs

Define host routes with `#[lingxia::native]`, register them through `HostAddon`, then let a native build generate the `@lingxia/native` View client. These routes are not added to `lx.*`. If JS Logic needs reusable cross-page helpers, expose a `lingxia::js` extension instead.

## Environments and release builds

`--env developer|preview|release` chooses the environment slot, including package-id suffix and server config. `--release` chooses the compiler profile. They are independent; a shippable build typically uses both:

```bash
lingxia build --env release --release
```

Use `lingxia package` when you need staged distributable outputs. Consult `lingxia build --help` and `lingxia package --help` for the version-matched platform and signing flags.
