# SDK source layout & layer boundaries

Every platform SDK is organized around the same two-layer split. When adding a
file, decide its layer first; the directory follows from that.

## The boundary: `app` vs `lxapp`

**`app` (host layer)** — one per host-app process. Code here exists regardless
of how many lxapps are open, and survives them all:

- process bootstrap & SDK entry (`Lingxia`)
- the native↔Rust bridge entry point (`NativeApi` / `NativeBridge`)
- host services: permission requests, host-window screenshot, host app update
- process-wide snapshots (`CurrentLxApp`)

**`lxapp` (runtime layer)** — instantiated per opened lxapp. Everything scoped
to one lxapp session:

- lifecycle & container (`LxApp`, `LxAppActivity` / `LxAppContainer`)
- the WebView shell and its bridges
- `chrome/` — page furniture drawn by the engine: navigation bar, tab bar,
  capsule button & menu, theme
- `APIs/` — implementations of `lx.*` capabilities invoked from Rust
  (one file per capability; private helper views/stores live next to the
  capability they serve)
- `NativeComponents/` — native views embedded into the page (video, picker,
  media swiper)

Dependency rule: `lxapp` code may use `app` services freely. `app` code may
orchestrate lxapp *lifecycles* (open/close/dispatch), but must not reach into
page-scoped internals (chrome widgets, capability impls, components).

## Per-platform map

| Concept | Android (`com.lingxia.*`) | Harmony (`ets/*`) | Apple (`Sources/*`) |
|---|---|---|---|
| Host layer | `app/` | `app/` | `Core/`, `Runner/` |
| Lxapp lifecycle | `lxapp/` | `lxapp/` | `Controller/`, `HostView/` |
| Page chrome | `lxapp/chrome/` | `lxapp/` (NavigationBar, TabBar, …) | `PageChrome/` |
| Capabilities | `lxapp/APIs/` | `lxapp/APIs/` | `Capabilities/` |
| Native components | `lxapp/NativeComponents/` | `lxapp/NativeComponents/` | `Platform/{iOS,macOS}/NativeComponents/` |
| Desktop window shell | — | — | `DesktopShell/` (sidebar/toolbar/chrome skeleton) |
| Mountable surfaces | — | — | `AppUI/` (config-driven: terminal, tray, agent panels) |

Apple-only notes:

- `PageChrome` is the lxapp page chrome — it is *not* the UI layer of
  `DesktopShell` (the old `ShellUI` name implied that; it was renamed).
- `AppUI` surfaces (terminal workspace, tray) are SDK capabilities mounted at
  runtime via `LxAppUIConfig.surfaces`; an IDE-like layout is a composition
  the host app opts into, not a baked-in product.

## Cross-platform placement parity

The same concern must live in the same place on every SDK. Current anchors:

- `BrowserNavigationPolicy`, `LxAppBrowserOverlay` → `lxapp/` root (WebView
  runtime concern, not an `lx.*` capability)
- video player implementation → `APIs/media/player/`
- JNI caveat: Android class FQNs are referenced as strings from Rust
  (`crates/lingxia/src/ffi/android.rs`, `crates/lingxia-platform/src/android/`).
  Moving/renaming a Kotlin class that Rust constructs requires updating those
  strings — grep for the class name in `crates/` before moving.
