# App Project Configuration

A LingXia app is a native shell (Host App) that embeds one home lxapp and can open other lxapps at runtime.

For lxapp page development, see [LxApp Development Guide](./lxapp-guide.md).
For quick onboarding, see [Getting Started](./getting-started.md).
For CLI commands, see [CLI Command Reference](./cli.md).

---

## Create a Host App

```bash
lingxia new my-app -t native-app -p android,ios --package-id com.example.myapp -y
```

This scaffolds a host app project with platform directories and an embedded home lxapp (`lingxia-showcase/`). The `-t native-app` flag creates a host app — not a standalone lxapp.

To create a standalone lxapp instead, use `-t lxapp` (see [LxApp Development Guide](./lxapp-guide.md)).

---

## Project Layout

```text
my-app/
├── lingxia.yaml                 # build-time project config
├── android/                     # optional
├── ios/                         # optional
├── macos/                       # optional
├── harmony/                     # optional
└── lingxia-showcase/             # embedded home lxapp source (see LxApp Guide)
```

- `lingxia.yaml` — the single source of truth for host build metadata. CLI reads it and generates the runtime `app.json`.
  CLI also generates `ui.json` from the `ui` section.
- `lingxia-showcase/` — an lxapp project embedded in the host. See [LxApp Development Guide](./lxapp-guide.md) for how to write pages inside it.
- Platform directories (`android/`, `ios/`, etc.) — native project scaffolding, selected by `app.platforms`.

---

## `lingxia.yaml` Configuration

### Minimal Example

```yaml
app:
  projectName: myapp
  productName: My App
  productVersion: 1.0.0
  platforms:
    - macos
  homeLxAppID: lingxia-showcase

macos:
  bundleId: com.example.myapp

ui:
  launch:
    initialSurface: main
    openOnLaunch: true
  surfaces:
    - id: main
      presentation:
        style: window
        resizable: true
        size:
          width: 960
          height: 720
      content:
        kind: lxapp
        appId: lingxia-showcase
        path: /
  activators: []
```

### Root Sections

| Section | Type | Required | Description |
|---|---|---|---|
| `app` | object | Yes (host build) | Core host metadata |
| `android` | object | No | Android platform config |
| `ios` | object | No | iOS platform config |
| `macos` | object | No | macOS platform config |
| `harmony` | object | No | Harmony platform config |
| `ui` | object | Required for macOS product hosts using `Lingxia.quickStart()` | App-level UI config |
| `resources` | object | No | Resource/runtime overrides |

### `app` Fields

| Field | Type | Required | Description |
|---|---|---|---|
| `projectName` | string | Yes (native build) | Technical project identifier, used by native build tooling |
| `productName` | string | Yes | User-facing app name |
| `productVersion` | string | Yes | Host app version |
| `apiServer` | string | No | Optional API server base URL |
| `lingxiaId` | string | No | Logical app publishing ID; required when `lingxia publish --target app` |
| `platforms` | string[] | Yes | Enabled platforms (e.g. `android`, `ios`, `macos`, `harmony`) |
| `homeLxAppID` | string | No | Home lxapp appId to open by default when the host boots |
| `cacheMaxAgeDays` | number | No | Cache TTL days; `0` disables age-based cleanup |
| `cacheMaxSizeMB` | number | No | Cache size limit per lxapp cache dir; `0` disables size-based cleanup |

`app.platforms` is required and must include at least one platform.

### Platform Sections

| Section | Key fields |
|---|---|
| `android` | `packageId`, `minSdk`, `targetSdk`, `compileSdk`, `ndkVersion`, `apiLevel` |
| `ios` | `bundleId`, `deploymentTarget`, `swiftVersion`, `targetName` |
| `macos` | `bundleId`, `deploymentTarget`, `executableName`, `targetName` |
| `harmony` | `bundleName`, `compatibleSdkVersion`, `targetSdkVersion` |

## `ui` Section

The `ui` section describes app-level UI structure. It is intended to be
platform-neutral: the same product model should map to macOS windows/menu bar
and Windows windows/tray/taskbar behavior.

The model below is the product contract. Current runtime support is narrower,
especially on macOS; unsupported pieces are called out in
[Current macOS Runtime Support](#current-macos-runtime-support) and
[Implementation Gaps](#implementation-gaps).

Think about it in three parts:

- `launch` — what should happen when the app starts
- `surfaces` — the visible UI containers
- `activators` — the entry points users click

A valid `ui` config contains `launch`, `surfaces`, and `activators`. Use
`activators: []` when the app has no extra entry points.

### Platform Mapping

Use these terms consistently:

- A `surface` is content the app can show.
- An `activator` is an entry point that opens, closes, toggles, or focuses a surface.
- App-level activators are owned by the OS or app process, not by a window.
- Surface-owned activators are chrome inside a visible surface and must set `hostSurface`.

Platform mapping:

| UI kind | macOS mapping | Windows mapping |
|---|---|---|
| `window` surface | `NSWindow` | Native app window |
| `statusPanel` surface | Menu-bar anchored floating panel | Tray-anchored popup window |
| `attachedPanel` surface | Panel attached to a root window | Panel docked to a root window |
| `menuBarItem` activator | `NSStatusItem` in the menu bar | No Windows mapping |
| `trayItem` activator | No macOS mapping | Notification-area tray icon |
| `appActivation` activator | Dock/app activation | Taskbar/app activation |
| `sidebarItem` / `toolbarItem` / `titlebarItem` | Window chrome item | Window chrome item |

Windows mappings are part of the platform-neutral model, but the Windows App UI
runtime is not implemented yet.

### `launch`

The most important field is:

- `initialSurface`

That is the first surface the app opens.

Example:

```yaml
ui:
  launch:
    initialSurface: main
    openOnLaunch: true
```

Useful fields:

- `initialSurface` — the first surface to open
- `openOnLaunch` — whether the app should open that surface immediately on launch

Typical status-item/tray behavior:

- `openOnLaunch: false`
- install the menu bar or tray item
- wait for the user to click it

### `surfaces`

A `surface` is a visible container.

Examples:

- a normal app window
- a menu-bar anchored status panel
- a panel attached to another surface

Each surface defines:

- an `id`
- a `presentation`
- a `content`

Example:

```yaml
ui:
  launch:
    initialSurface: main
  surfaces:
    - id: main
      presentation:
        style: window
        resizable: true
        size:
          width: 960
          height: 720
      content:
        kind: lxapp
        appId: lingxia-showcase
        path: /
  activators: []
```

#### Presentation Styles

Surface ids must be unique. `launch.initialSurface` must reference an existing
surface.

Presentation styles:

- `window` — a normal app window
- `statusPanel` — a menu bar style panel shown below a status item
- `attachedPanel` — a panel attached to another surface
- `sheet` — a modal sheet attached to another surface
- `embedded` — content embedded inside another native host surface

Useful presentation fields:

- `size.width`
- `size.height`
- `resizable`
- `attachTo`
- `edge`
- `showTrafficLights`

If `size` is omitted, the SDK uses its built-in default window size.

Omitting `size` does not mean auto-size-to-content.

`size` applies to desktop surfaces. Mobile platforms use the native viewport or
scene size and ignore desktop window dimensions.

Platform-neutral rules:

- `window` and `statusPanel` are root surfaces unless `attachTo` is explicitly supported by the platform.
- `attachedPanel`, `sheet`, and `embedded` must set `attachTo`.
- `edge` is required for `attachedPanel`.
- `content.kind` must be `lxapp`.
- `showTrafficLights` applies on platforms with window traffic lights. Other platforms ignore it.

#### Current macOS Runtime Support

macOS v1 supports a smaller subset:

- Exactly one root surface with style `window` or `statusPanel`.
- `attachedPanel` surfaces attached to that root surface.
- `attachedPanel.edge` values `leading`, `trailing`, and `bottom`.
- `content.kind: lxapp` only.
- `menuBarItem`, `appActivation`, `sidebarItem`, `toolbarItem`, and
  `titlebarItem` activators.

Current macOS limitations:

- Multiple root windows are rejected.
- `sheet`, `embedded`, and `terminal` content are rejected.
- `attachedPanel` attached to another panel is rejected.
- `attachedPanel.edge: top` is rejected.
- Each surface must currently use a unique `content.appId`. Reusing the same
  lxapp appId across multiple surfaces is not supported yet, even with different
  paths.

Example for a menu bar panel:

```yaml
ui:
  launch:
    initialSurface: main
    openOnLaunch: false
  surfaces:
    - id: main
      presentation:
        style: statusPanel
        resizable: false
        showTrafficLights: false
        size:
          width: 520
          height: 720
      content:
        kind: lxapp
        appId: lingxia-showcase
        path: /
  activators:
    - id: menuBar
      kind: menuBarItem
      label: LingXia
      icon: AppIcon.pdf
      action:
        kind: toggleSurface
        surface: main
```

### Example: Menu Bar Monitor App

This shape is useful for a lightweight monitor app:

- Launching the app does not open a window.
- The app installs a menu bar item.
- Clicking the menu bar item opens a fixed-size panel below the icon.
- The panel has no macOS traffic lights.
- A native chrome activator can close the panel with `action.kind: closeSurface`.
  A view-facing App UI API for page-owned close buttons is planned but not
  implemented yet.

```yaml
app:
  projectName: monitor
  productName: Monitor
  productVersion: 1.0.0
  platforms:
    - macos
  homeLxAppID: monitor-home

macos:
  bundleId: com.example.monitor

ui:
  launch:
    initialSurface: monitor
    openOnLaunch: false

  surfaces:
    - id: monitor
      presentation:
        style: statusPanel
        resizable: false
        showTrafficLights: false
        size:
          width: 360
          height: 480
      content:
        kind: lxapp
        appId: monitor-home
        path: /

  activators:
    - id: status
      kind: menuBarItem
      label: Monitor
      icon: AppIcon.pdf
      action:
        kind: toggleSurface
        surface: monitor

resources:
  bundles:
    - type: lxapp
      path: monitor-home
```

The planned Windows equivalent uses `trayItem` with the same `statusPanel`
surface:

```yaml
ui:
  launch:
    initialSurface: monitor
    openOnLaunch: false
  activators:
    - id: tray
      kind: trayItem
      label: Monitor
      icon: AppIcon.ico
      action:
        kind: toggleSurface
        surface: monitor
```

### `activators`

An `activator` is a user entry point.

Examples:

- a menu bar item
- a tray item
- an app activation entry
- a toolbar item
- a sidebar item

Each activator defines:

- where the user clicks
- which visible host surface owns that chrome, for surface-owned items
- which surface it controls
- what action should happen

App-level activators do not use `hostSurface`.

Surface-owned activators must set `hostSurface` to an existing surface id. The
item is only visible while that host surface is visible.

Examples:

```yaml
ui:
  activators: []
```

```yaml
ui:
  activators:
    - id: menuBar
      kind: menuBarItem
      label: LingXia
      icon: AppIcon.pdf
      action:
        kind: toggleSurface
        surface: main
```

```yaml
ui:
  activators:
    - id: dock
      kind: appActivation
      action:
        kind: focusSurface
        surface: main
```

```yaml
ui:
  activators:
    - id: browser
      kind: toolbarItem
      hostSurface: main
      label: Browser
      icon: app.lingxia.browser/public/LingXia.png
      action:
        kind: toggleSurface
        surface: browserPanel
```

For `menuBarItem` and `trayItem`, click behavior is simple:

- left click triggers the configured action
- right click triggers the same action

Icons are relative to the generated `ui.json` resource directory. These icons
are chrome icons, not app icons: keep them small, square, and cheap to decode.

Current CLI behavior: `ui.activators[].icon` files are not copied as loose
assets by the normal host build. Use icons that already live in generated host
resources, such as files inside a bundled lxapp directory copied by
`resources.bundles`.

Recommended icon assets:

| Use | Recommended asset | Notes |
|---|---|---|
| `menuBarItem` | PDF template asset | macOS menu bar icons should render cleanly in light and dark mode |
| `trayItem` | `.ico` with 16, 20, 24, 32, 48, and 256 px entries | Windows tray scaling depends on display scale and taskbar settings |
| `sidebarItem` / `toolbarItem` / `titlebarItem` | 64x64 or 128x128 PNG, SVG, or PDF | Runtime renders these into small chrome buttons |

Icon limits:

| Check | Limit |
|---|---|
| Raster dimensions | Maximum 512x512 px |
| Single icon file size | Maximum 512 KB |
| Total `ui.activators[].icon` assets | Maximum 2 MB |

Icons should be square. Raster icons smaller than 24x24 px may look blurry on
high-DPI displays.

The limits above are the intended product constraints. CLI validation for icon
paths, file types, file sizes, raster dimensions, and platform format
recommendations is not implemented yet.

Activator kinds:

| Kind | Scope | Platforms | Notes |
|---|---|---|---|
| `menuBarItem` | App-level | macOS | System menu bar status item |
| `trayItem` | App-level | Windows | Notification-area tray icon; ignored by current macOS runtime |
| `appActivation` | App-level | macOS, Windows | Dock/taskbar/app activation |
| `deepLink` | App-level | Planned | External URL or OS deep link |
| `sidebarItem` | Surface-owned | macOS, Windows | Requires `hostSurface` |
| `toolbarItem` | Surface-owned | macOS, Windows | Requires `hostSurface` |
| `titlebarItem` | Surface-owned | macOS, Windows | Requires `hostSurface` |

Supported action kinds:

- `toggleSurface`
- `openSurface`
- `closeSurface`
- `focusSurface`

The same action model is intended to be available to lxapp content through a
view-facing App UI API. That page-owned API is not implemented yet; today the
macOS runtime supports these actions through configured native activators.

On current macOS, `closeSurface` hides the window or panel. It does not destroy
the lxapp session or release the WebView.

### What the CLI Generates

The source of truth is:

- `lingxia.yaml`

The CLI generates:

- `app.json`
- `ui.json`

`app.json` contains runtime core metadata.
`ui.json` contains app-level UI structure.

Current CLI validation is partial. It requires `ui` for macOS host projects,
validates the current macOS runtime subset, and checks `ui.launch.splash.path`
when a splash is configured. It does not yet validate future platform mappings
or icon asset constraints.

For macOS product hosts that use `Lingxia.quickStart()`, `ui` is required. The
generated `app.json` and `ui.json` are loaded together from the target app's
bundled resources.

### Notes

- `homeLxAppVersion` is not configured here. CLI derives it from home lxapp build metadata and writes it into runtime `app.json`.
- Missing cache fields are allowed. Runtime defaults are applied from `app.json` parsing.
- If `ui.launch.splash.path` is configured, CLI expects a PNG image and copies
  it into native host resources as `splash.png`. The current macOS runtime does
  not present the splash screen yet.
- Do not edit generated `app.json` or `ui.json` directly.

### Implementation Gaps

The guide above describes the product contract. These parts still need runtime
or CLI work:

- Windows App UI runtime.
- macOS `deepLink`.
- macOS `launch.splash` presentation.
- macOS `sheet`, `embedded`, and `terminal` surfaces/content.
- macOS multiple root windows.
- macOS support for reusing the same lxapp `content.appId` across multiple
  surfaces.
- macOS `attachedPanel` support beyond panels attached to the root window.
- macOS `attachedPanel.edge: top`.
- View-facing App UI API for page-owned controls such as open, close, focus, or
  toggle surface.
- CLI build-time validation for the full cross-platform `ui` schema.
- CLI build-time validation for `ui.activators[].icon` size, dimensions, and platform format recommendations.
- CLI copying or packaging rules for loose `ui.activators[].icon` assets.

---

## Build

- `lingxia build` in the host project builds home lxapp assets and native platform resources.
- Runtime `app.json` and `ui.json` are generated from `lingxia.yaml` + built home lxapp metadata.
- Do not edit generated files directly — they are regenerated on every build.

---

## Common Pitfalls

- Editing generated runtime `app.json` directly instead of `lingxia.yaml`.
- Editing generated runtime `ui.json` directly instead of `lingxia.yaml`.
- Confusing `projectName` (technical) with `productName` (display name).
- Setting `homeLxAppVersion` in `lingxia.yaml` (it is generated).
