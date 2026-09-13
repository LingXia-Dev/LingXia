---
title: Native host apps
description: Configure an installable multi-platform host, embedded lxapps, capabilities, surfaces, and Rust extensions.
sidebar:
  order: 6
---

A native host app is the installable product shell for Android, iOS, macOS, Windows, and HarmonyOS. It owns `lingxia.yaml`, native platform projects, and a Rust host crate. Most products also embed a home lxapp; a desktop terminal- or browser-main host may omit that bundle.

## Scaffold the source of truth

```bash
lingxia new my-app -t native-app -p macos,windows \
  --package-id com.example.myapp -y
```

A desktop product whose main screen is the built-in terminal or browser can skip the embedded control lxapp:

```bash
lingxia new my-terminal -t native-app --main terminal --control native -y
lingxia new my-browser -t native-app -p windows --main browser --control lxapp -y
```

`--main terminal|browser` is currently macOS/Windows. `--control native` leaves out `homeAppId`, `resources.bundles`, and the `lxapp/` tree. `--control lxapp` keeps an embedded lxapp as the trusted [Control app](../control-app/) even when the visible main is the browser.

Read the generated `lingxia.yaml` for the exact fields supported by your installed CLI. `lingxia build` compiles it into runtime `app.json` and `ui.json`; those generated files are never authoring surfaces.

## Keep the home ids aligned

When the host **does** embed a home lxapp, three values must agree:

- `app.homeAppId`
- one `resources.bundles[].appId`
- that bundle's `lxapp.json.appId`

The launch main surface's `lxapp:` value must point at the same home app. Misalignment either fails the build or launches the wrong content. That home session is the Control app; other lxapps are guests even if they share an app id.

## Capabilities and surfaces

Declare host integrations before using them. `capabilities.browser` enables the in-app browser, `terminal` enables the native terminal surface, `process` unlocks trusted desktop process APIs, and `autostart` exposes user-controlled startup registration. Ordinary APIs such as camera are requested when called and do not belong in this list.

Use the top-level `surfaces:` list to describe main, aside, and tray content. See [Adaptive surfaces](../adaptive-surfaces/) for the current schema.

## JavaScript Logic or native-only Rust

Most hosts keep `features.appService: true` and embed a normal lxapp with JS Logic. A native-only host flips both sides together:

- `features.appService: false` in `lingxia.yaml`
- `"logic": false` in the home `lxapp.json` — or omit the control lxapp entirely for a desktop `native: terminal|browser` main

That shape uses an HTML-only View (when a control lxapp remains) and Rust for Logic. A logic-enabled lxapp under an appService-disabled host is rejected at startup.

## Add host-specific Rust APIs

Define host routes with `#[lingxia::native]`, register them through `HostAddon`, then let a native build generate the `@lingxia/native` View client. These routes are not added to `lx.*`. If JS Logic needs reusable cross-page helpers, expose a `lingxia::js` extension instead.

## Environments and release builds

`--env dev|prod` chooses the host environment, including package-id suffix and server config. `--release` chooses the compiler profile. They are independent; a shippable build typically uses both:

```bash
lingxia build --env prod --release
```

Use `lingxia package` when you need staged distributable outputs. Consult `lingxia build --help` and `lingxia package --help` for the version-matched platform and signing flags.
