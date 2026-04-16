# App Project Configuration

A LingXia app project is a native host app that embeds one home lxapp and can open bundled or runtime lxapps.

This page focuses on the macOS host app path because that is the current product App UI runtime. Android, iOS, and Harmony still use their platform host scaffolding, but the `ui` section below is implemented for macOS first.

For lxapp page development, see [LxApp Development Guide](./lxapp-guide.md).
For quick onboarding, see [Getting Started](./getting-started.md).
For CLI commands, see [CLI Command Reference](./cli.md).

---

## Create A Host App

```bash
lingxia new my-app -t native-app -p macos --package-id com.example.myapp -y
```

This creates a host app project, not a standalone lxapp. A host app owns native platform directories, a `lingxia.yaml`, and one embedded home lxapp.

To create a standalone lxapp instead, use `-t lxapp`.

---

## Project Layout

```text
my-app/
├── lingxia.yaml                  # build-time host project config
├── macos/                        # macOS Swift Package host
├── android/                      # optional Android host
├── ios/                          # optional iOS host
├── harmony/                      # optional Harmony host
└── lingxia-showcase/             # embedded home lxapp source
```

- `lingxia.yaml` is the source of truth for host build metadata and macOS App UI.
- `lingxia build` generates runtime `app.json` and `ui.json` from `lingxia.yaml`.
- Do not edit generated `app.json` or `ui.json` directly.
- `resources.bundles` controls lxapp/static bundles copied into native resources.

---

## Minimal macOS Example

```yaml
app:
  projectName: myapp
  productName: My App
  productVersion: 1.0.0
  platforms:
    - macos
  homeLxAppID: my-home

macos:
  bundleId: com.example.myapp
  deploymentTarget: "12.0"
  targetName: MyApp
  executableName: MyApp

ui:
  launch:
    initialSurface: main
  surfaces:
    - id: main
      presentation:
        style: window
      content:
        kind: lxapp
        appId: my-home
  activators: []

resources:
  bundles:
    - type: lxapp
      path: my-home
```

---

## Root Sections

| Section | Required | macOS Status | Description |
|---|---:|---|---|
| `app` | Yes | Required | Host metadata used to generate runtime `app.json` |
| `macos` | For macOS | Required for macOS builds | macOS bundle and SwiftPM target settings |
| `ui` | For macOS product hosts | Required | App UI model used to generate `ui.json` |
| `resources` | No | Supported | Extra bundled lxapps and static resources |
| `android` | For Android | Supported | Android host settings |
| `ios` | For iOS | Supported | iOS host settings |
| `harmony` | For Harmony | Supported | Harmony host settings |

---

## `app` Section

| Field | Type | Required | Description |
|---|---|---:|---|
| `projectName` | string | Yes | Technical project identifier. Used by native build tooling and Rust host library naming. |
| `productName` | string | Yes | User-facing app name. |
| `productVersion` | string | Yes | Host app version. |
| `platforms` | string[] | Yes | Enabled platforms, for example `macos`, `android`, `ios`, `harmony`. |
| `homeLxAppID` | string | No | Home lxapp appId opened by default. |
| `lingxiaId` | string | No | Logical publishing ID, used by app publishing flows. |
| `apiServer` | string | No | Optional API server base URL written into runtime metadata. |
| `cacheMaxAgeDays` | number | No | Cache TTL in days. `0` disables age-based cleanup. |
| `cacheMaxSizeMB` | number | No | Per-lxapp cache capacity in MiB. `0` disables size-based cleanup. |

`homeLxAppVersion` is not configured in `lingxia.yaml`; the CLI derives it from built lxapp metadata.

---

## `macos` Section

| Field | Type | Required | Description |
|---|---|---:|---|
| `bundleId` | string | No | macOS bundle identifier. |
| `deploymentTarget` | string | No | macOS deployment target, for example `"12.0"`. |
| `targetName` | string | Recommended | SwiftPM executable target name used for resource lookup. |
| `executableName` | string | Recommended | SwiftPM executable product/binary name. |

If `targetName` or `executableName` are omitted, the CLI tries reasonable defaults and then falls back to inference. Explicit names are preferred for reproducible builds.

---

## `resources` Section

`resources.bundles` copies lxapp/static bundles into native host resources.
`resources.i18n` and `resources.icons` are optional project-relative resource
directories used by platform host asset generation when present.

```yaml
resources:
  bundles:
    - type: lxapp
      path: my-home
    - type: lxapp
      path: ../examples/lingxia-chat
```

Bundle entries can be short strings or objects:

```yaml
resources:
  bundles:
    - my-home
    - type: lxapp
      path: ../examples/lingxia-chat
      target: app.lingxia.browser
```

| Field | Type | Description |
|---|---|---|
| `type` | `lxapp` or `npm` | Bundle build/copy strategy. Defaults to `lxapp`. |
| `path` | string | Project-relative bundle path. |
| `target` | string | Resource target directory override. Required for `npm` bundles; optional for `lxapp` bundles. |

For the built-in browser/settings/downloads UI, include `../crates/lingxia-shell/webui` as an lxapp bundle when the host app exposes browser shell features.

---

## macOS App UI

The `ui` section describes product-level macOS UI: windows, panels, and native chrome entry points.

`Lingxia.quickStart()` loads bundled `app.json` and `ui.json`, initializes the runtime, creates the macOS shell window, and then applies this App UI model.

The model has three parts:

- `launch`: startup behavior.
- `surfaces`: UI containers.
- `activators`: native entry points that operate on surfaces.

### Important Boundary

Settings and Downloads are built-in shell/browser entries. Do not duplicate them in `ui.activators`.

When the macOS native host is built with the `shell` feature, the shell provides built-in Settings and Downloads chrome. The example app should configure only product-specific entries such as Browser/Assistant panels.

### `launch`

```yaml
ui:
  launch:
    initialSurface: main
```

| Field | Type | Required | Description |
|---|---|---:|---|
| `initialSurface` | string | Yes | Surface opened first. Must reference `ui.surfaces[].id`. |
| `openOnLaunch` | bool | No | Defaults to open-on-launch behavior. Set `false` for menu-bar style apps. |
| `splash.path` | string | No | Optional PNG source path copied into native resources as `splash.png`; macOS App UI does not present it yet. |

For menu-bar apps, use `openOnLaunch: false` and add a `menuBarItem` activator that toggles the status panel.

### `surfaces`

A surface is a visible macOS container.

Current macOS supported styles:

| Style | Status | Description |
|---|---|---|
| `window` | Supported | Normal app window. |
| `statusPanel` | Supported | Menu-bar anchored floating panel. |
| `attachedPanel` | Supported | Panel attached to the single root window/status panel. |
| `sheet` | Rejected | Not implemented in macOS runtime. |
| `embedded` | Rejected | Not implemented in macOS runtime. |

Current macOS rules:

- Exactly one root surface is required.
- The root surface must be `window` or `statusPanel`.
- `attachedPanel` must set `presentation.attachTo`.
- `attachedPanel.attachTo` must reference the root surface.
- `attachedPanel.edge` must be `leading`, `trailing`, or `bottom`.
- `attachedPanel.edge: top` is rejected.
- The stable `content.kind` today is `lxapp`.
- Each `lxapp` surface must currently use a unique `content.appId`.

Common presentation fields:

| Field | Applies To | Description |
|---|---|---|
| `style` | all surfaces | `window`, `statusPanel`, or `attachedPanel` on macOS. |
| `size.width` | `window`, `statusPanel` | Optional initial width. Omit it to use the shell's native default. |
| `size.height` | `window`, `statusPanel` | Optional initial height. Omit it to use the shell's native default. |
| `resizable` | `window`, `statusPanel` | Whether the native window can resize. Defaults to `true`. |
| `showTrafficLights` | `window`, `statusPanel` | Whether macOS traffic lights are shown. Defaults to `true` for `window` and `false` for `statusPanel`. |
| `attachTo` | `attachedPanel` | Parent/root surface id. |
| `edge` | `attachedPanel` | `leading`, `trailing`, or `bottom`. |

Content fields:

| Field | Required | Description |
|---|---:|---|
| `kind` | Yes | Use `lxapp` for current macOS App UI. `terminal` is reserved for a future runtime implementation. |
| `appId` | For `lxapp` | Lxapp appId to open in this surface. |
| `path` | No | Initial route/path for `lxapp` content. |

Example root window:

```yaml
surfaces:
  - id: main
    presentation:
      style: window
    content:
      kind: lxapp
      appId: my-home
```

Example attached assistant panel:

```yaml
surfaces:
  - id: assistant
    presentation:
      style: attachedPanel
      attachTo: main
      edge: trailing
    content:
      kind: lxapp
      appId: lingxia-chat
```

Do not define separate `settings` or `downloads` surfaces for the built-in browser app. Those pages are opened by built-in shell controls.

### `activators`

An activator is a native entry point that performs an action on a surface.

Supported macOS activator kinds:

| Kind | Scope | Status | Rules |
|---|---|---|---|
| `menuBarItem` | App-level | Supported | Must not set `hostSurface`. |
| `appActivation` | App-level | Supported | Must not set `hostSurface`. Runs when the app becomes active. |
| `sidebarItem` | Surface-owned | Supported | Must set `hostSurface`. Rendered in shell sidebar chrome. |
| `toolbarItem` | Surface-owned | Supported | Must set `hostSurface`. Rendered in navigation toolbar chrome. |
| `titlebarItem` | Surface-owned | Supported | Must set `hostSurface`. Rendered as a titlebar accessory action strip. |

`trayItem` is not a macOS App UI activator.

Supported action kinds:

| Action | Description |
|---|---|
| `toggleSurface` | If the surface is visible, close it; otherwise open it. |
| `openSurface` | Open the surface; if it already exists, focus/show it. |
| `closeSurface` | Hide the surface. Does not destroy the lxapp session or WebView. |
| `focusSurface` | Bring an already-visible surface to the front. Does not open it. |

All current App UI actions require:

```yaml
action:
  kind: toggleSurface
  surface: someSurfaceId
```

Examples:

```yaml
activators:
  - id: assistantSidebar
    kind: sidebarItem
    hostSurface: main
    label: AI Chat
    icon: lingxia-chat/chat.svg
    action:
      kind: toggleSurface
      surface: assistant
```


```yaml
activators:
  - id: menuBar
    kind: menuBarItem
    label: My App
    icon: icons/menu.svg
    action:
      kind: toggleSurface
      surface: main
```

Surface-owned activators are visible only while their `hostSurface` is visible.

### Assistant Panel Example

This is the current example shape: one main app window plus an attached AI Chat assistant panel. Settings and Downloads remain shell built-ins.

```yaml
ui:
  launch:
    initialSurface: main
  surfaces:
    - id: main
      presentation:
        style: window
      content:
        kind: lxapp
        appId: lingxia-showcase
    - id: assistant
      presentation:
        style: attachedPanel
        attachTo: main
        edge: trailing
      content:
        kind: lxapp
        appId: lingxia-chat
  activators:
    - id: assistantSidebar
      kind: sidebarItem
      hostSurface: main
      label: AI Chat
      icon: lingxia-chat/chat.svg
      action:
        kind: toggleSurface
        surface: assistant

resources:
  bundles:
    - type: lxapp
      path: lingxia-showcase
    - type: lxapp
      path: lingxia-chat
```

### Menu Bar Panel Example

```yaml
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
  activators:
    - id: status
      kind: menuBarItem
      label: Monitor
      icon: icons/monitor.svg
      action:
        kind: toggleSurface
        surface: monitor
```

Runtime behavior:

- If the app only has menu-bar activators and no app-activation activator, it can run as an accessory-style menu-bar app.
- Clicking the status item triggers the configured action.
- `statusPanel` is positioned from the source status item when opened by a menu-bar activator.

### App Activation Example

Use `appActivation` when clicking the Dock icon or activating the app should focus/open a known surface.

```yaml
activators:
  - id: activateMain
    kind: appActivation
    action:
      kind: openSurface
      surface: main
```

If an app has `appActivation` activators, the macOS runtime uses regular app activation behavior.

---

## Icon Paths

`ui.activators[].icon` is a source icon path relative to the host project root.

Current macOS App UI supports SVG source icons only. During `lingxia build`, the CLI validates each source icon, converts it to a PDF resource, copies it into generated `icons/`, and rewrites the generated `ui.json` to reference that generated resource path.

Example:

```yaml
icon: icons/browser.svg
```

Validation rules:

| Check | Rule |
|---|---|
| Source format | SVG only |
| Path | Relative to host project root; absolute paths and `..` are rejected |
| File size | Maximum 512 KB |
| SVG viewport size | 16x16 px through 512x512 px |
| Aspect ratio | Must be square, within a small tolerance |

Generated macOS resource paths look like:

```text
icons/browser-<hash>.pdf
```

Do not reference generated lxapp runtime assets such as `app.lingxia.browser/public/LingXia.png` for native chrome icons. Use a host-root-relative SVG source file instead; it is fine for that source file to live inside a bundled lxapp project such as `lingxia-chat/chat.svg`, because the CLI converts and copies it into native host resources.

---

## Generated Files

During `lingxia build`, the CLI generates platform resources:

- `app.json`: runtime app metadata.
- `ui.json`: macOS App UI structure.
- `icons/*.pdf`: generated macOS native chrome icons.
- `splash.png`: optional copied splash image when `ui.launch.splash.path` is configured.
- bundled lxapp directories from `resources.bundles`.
- `bridge-runtime.js`.

For macOS, these are copied into the SwiftPM target resource directory, usually `macos/Sources/<targetName>/Resources` unless the target declares a custom `path`.

Generated files are build artifacts. Edit `lingxia.yaml` instead.

---

## Build

Build macOS from the host project root:

```bash
lingxia build --platform macos --framework vue
```

The macOS host build does the following:

- Builds configured lxapp resource bundles.
- Generates `app.json` and `ui.json`.
- Builds the Rust host static library.
- Enables the native `shell` feature for macOS builds by default.
- Builds the SwiftPM macOS app.
- Packages the `.app` under `macos/.lingxia/`.

Example output:

```text
macos/.lingxia/My App.app
```

If `--skip-native` is used, SwiftPM links an existing Rust static library. That can leave runtime capabilities stale, including shell/browser capability bits. For App UI debugging, prefer a normal build without `--skip-native`.

---

## Common Pitfalls

- Adding Settings or Downloads to `ui.activators`: these are built-in shell entries, not product UI activators.
- Adding `trayItem` to macOS `ui.activators`: it is not a macOS App UI activator.
- Defining multiple surfaces with the same `content.appId`: current macOS runtime rejects this.
- Using `attachedPanel` without `attachTo` or `edge`.
- Attaching a panel to another panel instead of the root surface.
- Using `attachedPanel.edge: top`, which is not supported yet.
- Expecting `closeSurface` to destroy WebViews; it hides the surface.
- Using PNG or generated lxapp runtime images for `ui.activators[].icon`; App UI icons must be host-root-relative SVG source files.
- Editing generated `app.json` or `ui.json`.
- Running an older `lingxia` binary from `PATH` after changing config schema or CLI validation.

---

## Out Of Scope / Not Implemented In macOS App UI

This page intentionally does not define product behavior for:

- splash presentation
- multiple root windows
- sheets and embedded native host surfaces
- attached panels nested under other panels
- top-attached panels
- reusing one lxapp appId across multiple surfaces
- page-owned App UI APIs for toggling or closing surfaces from lxapp content
