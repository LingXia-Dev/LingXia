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
- `app.homeAppId` controls the home app opened by default; `resources.bundles` controls bundled asset sources.

---

## SDK Startup APIs

Use the product-app startup entry on each platform:

| Platform | Entry |
|---|---|
| Apple | `Lingxia.quickStart()` |
| Android | `Lingxia.quickStart(activity)` |
| Harmony | `Lingxia.quickStart(context, windowStage)` |

`quickStart` means the native app is a LingXia host product. It initializes the
runtime and opens the configured home lxapp through the platform host shell or
navigation container.

Android and Harmony intentionally expose only `quickStart` as the public startup
API today. Advanced embedding into an existing native app should stay internal
until the host-view/session API is designed for those platforms. Do not add
compatibility wrappers such as `Lingxia.initialize(...)`.

---

## Minimal macOS Example

```yaml
app:
  projectName: myapp
  productName: My App
  productVersion: 1.0.0
  platforms:
    - macos
  homeAppId: my-home

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
        kind: window
      content:
        kind: lxapp
        appId: my-home
  activators: []

```

For a native-only menu bar app, see `examples/menubar`. It uses
`features.appService: false`, a `menuBarItem` activator, a panel root surface,
and a `logic: false` HTML home lxapp.

---

## Root Sections

| Section | Required | macOS Status | Description |
|---|---:|---|---|
| `app` | Yes | Required | Host metadata used to generate runtime `app.json` |
| `macos` | For macOS | Required for macOS builds | macOS bundle and SwiftPM target settings |
| `ui` | For macOS product hosts | Required | App UI model used to generate `ui.json` |
| `android` | For Android | Supported | Android host settings |
| `ios` | For iOS | Supported | iOS host settings |
| `harmony` | For Harmony | Supported | Harmony host settings |
| `features` | Recommended | Supported | Native Rust compile-time feature switches |
| `capabilities` | Recommended | Supported | Platform/runtime integrations that may initialize SDK capability flows |
| `resources` | Recommended | Supported | Bundle asset sources copied into native app resources |
| `shell` | When `features.shell` is true | Supported | Shell webui source configuration |
| `storage` | Recommended | Supported | Explicit host temp/cache/data size limits |

---

## `app` Section

| Field | Type | Required | Description |
|---|---|---:|---|
| `projectName` | string | Yes | Technical project identifier. Used by native build tooling and Rust host library naming. |
| `productName` | string | Yes | User-facing app name. |
| `productVersion` | string | Yes | Host app version. |
| `platforms` | string[] | Yes | Enabled platforms, for example `macos`, `android`, `ios`, `harmony`. |
| `homeAppId` | string | Yes | Home app id opened by default. |
| `lingxiaId` | string | No | Logical publishing ID, used by app publishing flows. |
| `lingxiaServer` | string | No | Optional LingXia server base URL paired with `lingxiaId`. |

`homeAppVersion` is not configured in `lingxia.yaml`; the CLI derives it from the matching `resources.bundles` source.

---

## `features` Section

`features` controls native Rust compile-time features. When `appService` is `false`, the CLI builds the host Rust library with `--no-default-features`. When `appService` is `true`, Cargo default features stay enabled and the CLI adds the selected features.

| Field | Type | Default | Description |
|---|---|---:|---|
| `appService` | bool | `true` | Enables JS/TS AppService runtime support. Set `false` for native-only hosts; logic-enabled lxapps will be rejected. |
| `shell` | bool | `false` | Enables product shell/browser chrome: browser, downloads, settings, panels. This can be used by native-only hosts. |
| `devtools` | bool | `false` | Compiles devtools hooks into the host. `lingxia dev` may temporarily enable it without editing YAML. |

`-t lxapp` projects always require an AppService-capable host. `-t native-app` projects may set `appService: false` when they only need native-hosted UI and host APIs.

---

## `capabilities` Section

`capabilities` is for platform/runtime integrations that must be predeclared before the SDK auto-enables them. Do not list ordinary SDK APIs such as camera here; those should request permission only when called.

| Field | Type | Default | Description |
|---|---|---:|---|
| `notifications` | bool | `false` | Enables push/notification integration where supported. iOS/Harmony SDK startup may request notification permission and fetch a push token. |
| `terminal` | bool | `false` | Enables the macOS terminal runtime. When true, the CLI auto-generates a bottom `terminal` App UI attach panel and sidebar activator if they are not already declared. |

---

## `shell` Section

`shell` is used only when `features.shell: true`. Normal apps can omit it and use the SDK default shell webui. Repo development can point to a local checkout; external apps should use the package form.

```yaml
shell:
  webui:
    package: '@lingxia/shell-webui'
    version: '0.5.1'
```

| Field | Type | Required | Description |
|---|---|---:|---|
| `webui.path` | string | One of path/package | Project-relative path to a shell webui lxapp source tree. The CLI builds it. |
| `webui.package` | string | One of path/package | npm package containing prebuilt shell webui `lxapp.json` and `dist/`. |
| `webui.version` | string | With package | npm package version. If omitted, the CLI version is used. |

Do not use `app.homeAppId` for shell internals. `app.homeAppId` is the product home app; `shell.webui` is the shell/browser UI asset.

---

## `resources` Section

`resources.bundles` declares lxapp asset sources bundled into the native host. It does not decide what the app opens; `app.homeAppId` and `ui.surfaces[].content.appId` do that.

| Field | Type | Required | Description |
|---|---|---:|---|
| `bundles[].type` | string | Yes | Currently `lxapp`. |
| `bundles[].appId` | string | Yes | App id provided by this bundle. Must match the bundle `lxapp.json.appId`. |
| `bundles[].path` | string | No | Project-relative local lxapp source path. When set, the CLI builds and bundles it. |
| `bundles[].package` | string | No | npm package containing prebuilt `lxapp.json` and `dist/`. When set, the CLI downloads and bundles it. |
| `bundles[].version` | string | With package | npm package version. If omitted, the CLI version is used. |

Example:

```yaml
resources:
  bundles:
    - type: lxapp
      appId: lingxia-showcase
      path: lingxia-showcase
    - type: lxapp
      appId: app.lingxia.browser
      package: '@lingxia/shell-webui'
      version: '0.5.1'
    - type: lxapp
      appId: lingxia-chat
```

If a bundle entry has only `type` and `appId`, it declares the appId but does not bundle local assets; the runtime/update provider must make it available.

---

## `storage` Section

`storage` makes storage policy visible instead of relying on hidden defaults. Values are MiB except `cacheMaxAgeDays`.

| Field | Type | Default | Description |
|---|---|---:|---|
| `tempMaxSizeMB` | number | `1024` | Maximum host temp storage size. |
| `cacheMaxAgeDays` | number | `7` | Maximum lxapp cache age. `0` disables age cleanup. |
| `cacheMaxSizeMB` | number | `2048` | Maximum lxapp cache size. `0` disables size cleanup. |
| `dataMaxSizeMB` | number | `4096` | Maximum user data storage size. |
| `appStorageMaxSizeMB` | number | `16384` | Maximum app-scoped storage size. |

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

For menu-bar apps, use `openOnLaunch: false` and add a `menuBarItem` activator that toggles a panel anchored to the activator.

### `surfaces`

A surface is a visible macOS container.

One sentence model: **a surface defines what can be shown, presentation defines
how it is shown, and activators define who opens it.**

Current macOS supported presentation kinds:

| Kind | Status | Description |
|---|---|---|
| `window` | Supported | Normal app window. |
| `panel` | Supported | Floating panel. Use `anchor: activator` to position it from the entry point that opened it. |
| `attachPanel` | Supported | Panel attached to the single root window/panel. |
| `embedded` | Rejected | Not implemented in macOS runtime. |

Current macOS rules:

- Exactly one root surface is required.
- The root surface must be `window` or `panel`.
- Menu-bar panels use `presentation.kind: panel` and `presentation.anchor: activator`; when opened by a `menuBarItem`, the activator is the menu-bar icon.
- `attachPanel` must set `presentation.attachTo`.
- `attachPanel.attachTo` must reference the root surface.
- `attachPanel.edge` must be `leading`, `trailing`, or `bottom`.
- `attachPanel.edge: top` is rejected.
- The stable `content.kind` today is `lxapp`.
- Each `lxapp` surface must currently use a unique `content.appId`.

Common presentation fields:

| Field | Applies To | Description |
|---|---|---|
| `kind` | all surfaces | `window`, `panel`, or `attachPanel` in the product model. Current runtime may still map this to platform-specific styles internally. |
| `anchor` | `panel` | Optional anchor. Use `activator` to position the panel from the native entry point that opened it. |
| `size.width` | `window`, `panel` | Optional initial width. Omit it to use the shell's native default. |
| `size.height` | `window`, `panel` | Optional initial height. Omit it to use the shell's native default. |
| `resizable` | `window`, `panel` | Whether the native window can resize. Defaults to `true`. |
| `showTrafficLights` | `window`, `panel` | Whether macOS traffic lights are shown. Defaults to `true` for `window` and `false` for menu-bar panels. |
| `attachTo` | `attachPanel` | Parent/root surface id. |
| `edge` | `attachPanel` | `leading`, `trailing`, or `bottom`. |

Content fields:

| Field | Required | Description |
|---|---:|---|
| `kind` | Yes | Use `lxapp` or `terminal`. `terminal` is currently macOS-only and must be used as a bottom `attachPanel`. |
| `appId` | For `lxapp` | Lxapp appId to open in this surface. |
| `path` | No | Initial route/path for `lxapp` content. |

Example root window:

```yaml
surfaces:
  - id: main
    presentation:
      kind: window
    content:
      kind: lxapp
      appId: my-home
```

`attachPanel` is supported by the macOS runtime. If the panel uses another lxapp, list it in `resources.bundles` for local bundling, or let the runtime/update provider fetch it.

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
  - id: homeSidebar
    kind: sidebarItem
    hostSurface: main
    label: Home
    icon: icons/home.svg
    action:
      kind: focusSurface
      surface: main
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

This shape uses one main app window plus an attached AI assistant panel. Settings and Downloads remain shell built-ins. `lingxia-chat` can be bundled locally through `resources.bundles`, or omitted there and fetched by runtime/update flow.

```yaml
ui:
  launch:
    initialSurface: main
  surfaces:
    - id: main
      presentation:
        kind: window
      content:
        kind: lxapp
        appId: lingxia-showcase
    - id: assistant
      presentation:
        kind: attachPanel
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

```

### Terminal Panel (Native Host, Shared Rust Engine)

The App UI runtime supports a native terminal surface that can be opened from product chrome and shown as a bottom panel. macOS is the first host, but the runtime boundary is designed for Windows to reuse the same Rust terminal engine.

Current scope and limits:

- macOS first; Windows should attach to the same Rust session/snapshot API instead of reimplementing terminal semantics.
- `content.kind: terminal` only.
- `presentation.kind: attachPanel` only.
- `presentation.edge: bottom` only.
- `content.backend` is not product config. If present, validation rejects it.
- Rust owns terminal sessions, PTY/conpty transport, `libghostty-vt` terminal semantics, themes, and the stable snapshot/input protocol.
- Platform SDKs own only native view rendering, focus/input event capture, clipboard/menu integration, and host UX such as tabs/splits/panel lifecycle.

Reference material used for backend direction:

- [con-ghostty](https://github.com/nowledge-co/con/tree/main/crates/con-ghostty)
- [libghostty-rs bindings](https://github.com/Uzaaft/libghostty-rs/blob/master/crates/libghostty-vt-sys/src/bindings.rs)

Ghostty preparation is handled by `crates/lingxia-terminal/build.rs`. Terminal builds use a pinned Ghostty git checkout and build `libghostty-vt`; the build script does not fetch release tarballs.

```bash
cargo build -p lingxia --features shell-runtime
```

Supported build-time inputs:

| Environment | Description |
|---|---|
| `LINGXIA_GHOSTTY_SOURCE_DIR=/path/to/ghostty` | Uses an existing local checkout/source tree. |
| `LINGXIA_GHOSTTY_REV=<rev>` | Overrides the pinned Ghostty git revision. If omitted, LingXia uses the pinned revision in `crates/lingxia-terminal/build.rs`. |
| `LINGXIA_GHOSTTY_REPO=<url>` | Overrides the git repo used with `LINGXIA_GHOSTTY_REV`. |
| `LINGXIA_GHOSTTY_ZIG=/path/to/zig` | Overrides the `zig` executable. |
| `LINGXIA_GHOSTTY_ZIG_ARGS="..."` | Appends extra `zig build` arguments. |

Target behavior:

- A terminal icon in product chrome toggles the terminal surface.
- Terminal surface is attached to the main window with `attachPanel.edge: bottom`.
- The terminal workspace supports multi-tab sessions.
- Each tab supports pane split in `left`, `right`, `up`, and `down` directions.
- Hiding the surface should preserve terminal sessions (same lifecycle expectation as `closeSurface`).

Example shape:

```yaml
ui:
  launch:
    initialSurface: main
  surfaces:
    - id: main
      presentation:
        kind: window
      content:
        kind: lxapp
        appId: lingxia-showcase
    - id: terminal
      presentation:
        kind: attachPanel
        attachTo: main
        edge: bottom
        size:
          height: 320
      content:
        kind: terminal
  activators:
    - id: terminalSidebar
      kind: sidebarItem
      hostSurface: main
      label: Terminal
      icon: icons/terminal.svg
      action:
        kind: toggleSurface
        surface: terminal
```

Engine/platform boundary:

- `crates/lingxia-terminal` is the product terminal engine. Public APIs use `terminal_*` naming; `portable-pty` is an internal transport detail.
- The engine emits JSON snapshots containing grid size, cells, grapheme text, colors, attributes, cursor state, title, alternate-screen state, and lifecycle state.
- macOS renders snapshots into `NSView`; Windows should render the same snapshots into its native view while sharing session create/write/resize/close behavior.
- Do not add backend selectors to `lingxia.yaml`; backend choice is owned by the runtime.

Implementation notes for native hosts:

- Terminal content is mounted as the platform-native terminal view (`NSView` on macOS, Windows-native view later) into the attachPanel container.
- Keep terminal runtime state in native host scope: `surface -> tabs -> split tree -> panes`.
- Drive split layouts with native split containers and map each pane to one terminal session.
- Add native terminal commands for: new tab, close tab, split left/right/up/down, and focus movement between panes.

Future phases:

1. Phase 1: bottom terminal surface with single tab/single pane.
2. Phase 2: multi-tab support with stable session lifecycle.
3. Phase 3: four-direction split and pane focus/resize behavior.

### Menu Bar Panel Example

This example uses a single declarative `panel` surface opened by the native
menu-bar icon. The current macOS runtime supports exactly one root
`window`/`panel` surface, so this example does not also declare a separate main
window.

`menuBarItem` is the native entry point. `presentation.kind: panel` describes
how the `menu` surface is displayed. `presentation.anchor: activator` means the
panel is positioned from whichever native entry point opened it; in this example
that entry point is the menu-bar icon.

```yaml
ui:
  launch:
    initialSurface: menu
    openOnLaunch: false
  surfaces:
    - id: menu
      presentation:
        kind: panel
        anchor: activator
        resizable: false
        showTrafficLights: false
        size:
          width: 360
          height: 480
      content:
        kind: lxapp
        appId: monitor-home
        path: pages/menubar/index
  activators:
    - id: status
      kind: menuBarItem
      label: Monitor
      icon: icons/monitor.svg
      action:
        kind: toggleSurface
        surface: menu
```

Runtime behavior:

- If the app only has menu-bar activators and no app-activation activator, it can run as an accessory-style menu-bar app.
- Clicking the menu-bar icon toggles the `menu` surface.
- The `menu` surface renders `monitor-home/pages/menubar/index`.
- The string `menu` is a surface ID, not a page path.
- A menu-bar anchored panel is positioned from the source menu-bar item.

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

Do not reference generated lxapp runtime assets such as `app.lingxia.browser/public/LingXia.png` for native chrome icons. Use a host-root-relative SVG source file instead; it is fine for that source file to live inside the home lxapp project, because the CLI converts and copies it into native host resources.

---

## Generated Files

During `lingxia build`, the CLI generates platform resources:

- `app.json`: runtime app metadata.
- `ui.json`: macOS App UI structure.
- `icons/*.pdf`: generated macOS native chrome icons.
- `splash.png`: optional copied splash image when `ui.launch.splash.path` is configured.
- bundled lxapp directories from `resources.bundles`.
- bundled shell webui directory when `features.shell: true`.
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

- Builds the configured home lxapp resource bundle.
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
- Using `attachPanel` without `attachTo` or `edge`.
- Attaching a panel to another panel instead of the root surface.
- Using `attachPanel.edge: top`, which is not supported yet.
- Expecting `closeSurface` to destroy WebViews; it hides the surface.
- Using PNG or generated lxapp runtime images for `ui.activators[].icon`; App UI icons must be host-root-relative SVG source files.
- Defining `content.kind: terminal` outside `attachPanel` or with non-`bottom` edge; current terminal surfaces are bottom attach panels only.
- Adding terminal backend selectors to product config; the runtime owns backend selection.
- Editing generated `app.json` or `ui.json`.
- Running an older `lingxia` binary from `PATH` after changing config schema or CLI validation.

---

## Out Of Scope / Not Implemented In macOS App UI

This page intentionally does not define product behavior for:

- splash presentation
- multiple root windows
- embedded native host surfaces
- attach panels nested under other panels
- top-attach panels
- reusing one lxapp appId across multiple surfaces
- page-owned App UI APIs for toggling or closing surfaces from lxapp content
- terminal surfaces outside macOS `attachPanel` bottom shape
- terminal backend selection in App UI config
