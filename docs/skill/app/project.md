# App Project Configuration

A LingXia app project is a native host app that embeds one home lxapp and can open bundled or runtime lxapps. Its build-time config lives in `lingxia.yaml`.

The UI is described by a flat, adaptive `surfaces:` list (see [Surfaces](#surfaces-adaptive-ui)) — you declare *what* each surface is and the Host derives the realized platform form (window / panel / sidebar / tab / tray) by screen size. macOS is the most complete runtime today; the same `surfaces:` schema feeds every platform.

For lxapp page development, see [LxApp Development Guide](../lxapp/guide.md).
For CLI commands, see [CLI Command Reference](../cli/lingxia.md).

---

## Create A Host App

```bash
lingxia new my-app -t native-app -p macos --package-id com.example.myapp -y
```

This creates a host app project, not a standalone lxapp. A host app owns native platform directories, a `lingxia.yaml`, and one embedded home lxapp.

To create a standalone lxapp instead, use `-t lxapp`.

---

## Project Layout

Don't reach for a frozen tree — scaffold one and read it:

```bash
lingxia new my-app -t native-app -p macos,windows --package-id com.example.myapp -y
```

The CLI emits the authoritative layout for the `lingxia` on your `PATH`; a hand-written sample drifts, the generated one can't. At a conceptual level a host app owns:

- `lingxia.yaml` — the build-time host project config and source of truth for metadata + UI.
- a native Rust crate in `native/` — the host library (routes, addons); `lingxia.yaml` records its directory as `app.rustLibDir`.
- one per-platform host directory for each enabled platform — `macos/`, `windows/`, `android/`, `ios/`, `harmony/`.
- the embedded home lxapp source (scaffold default `lxapp/`, and its directory name doubles as the home `appId`).

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

surfaces:
  - lxapp: my-home       # main screen: your lxapp, by appId
    role: main
    launch: true
```

For a native-only menu-bar app, omit `launch: true` and give the main surface a
`tray:` entry, set `features.appService: false`, and use a `logic: false` HTML
home lxapp.

---

## Root Sections

The authoritative, version-matched field list is a freshly scaffolded `lingxia.yaml` — run `lingxia new -t native-app -p <platforms>` and read the generated file (the CLI seeds every section a host needs, commented). This page covers the *model* behind those sections — what each is for and the rules the build enforces — not a field-by-field mirror, which would only drift from the serde structs in the CLI.

| Section | Required | Purpose |
|---|---:|---|
| `app` | Yes | Host metadata used to generate runtime `app.json` |
| `macos` / `windows` | Per platform | Desktop host bundle / packaging + Store identity |
| `android` / `ios` / `harmony` | Per platform | Mobile platform host settings |
| `surfaces` | For product hosts | Adaptive UI surface list (generates `ui.json`) |
| `features` | Recommended | Native Rust compile-time feature switches |
| `capabilities` | Recommended | Platform/runtime integrations that may initialize SDK capability flows |
| `resources` | Recommended | Bundle asset sources copied into native app resources |
| `browser` | Optional | Override the in-app browser webui (only used when `capabilities.browser: true`) |
| `appLinks` | Optional | Universal-link / app-link hosts (see [App Links](./applinks.md)) |
| `storage` | Recommended | Explicit host temp/cache/data size limits |

---

## `app` Section

`app` carries host metadata that generates the runtime `app.json`: `projectName` (technical identifier behind native build paths and the Rust host library name), `productName` (user-facing), `productVersion` (a semver string — the build rejects non-semver), and `platforms` (the enabled set, drawn from `macos`, `windows`, `ios`, `android`, `harmony`). Optional `lingxiaId` / `lingxiaServer` / `packageIdSuffix` drive publishing and per-env builds (see [Environment versions](#environment-versions)).

**The id-alignment rule (the one that bites).** Three ids must line up or the wrong app launches, and the build enforces it:

- `app.homeAppId` = a `resources.bundles[].appId` = that bundle's `lxapp.json.appId`.
- The launch `main` surface's `lxapp:` key is the appId it renders — point it at the same home app.

`homeAppVersion` is not configured here; the CLI derives it from the matching `resources.bundles` source. The full, current field set is in a freshly scaffolded `lingxia.yaml`.

---

## Environment versions

A LingXia host build is always one of three envs — `developer`, `preview`, or `release` — selected via `lingxia {build,dev,package} --env <env>`. The default is `developer` for `build`/`dev` and `release` for `package`.

**What each env produces:**

| Env | Default `packageIdSuffix` | Launcher icon | Default `lingxia dev/build` | Default `lingxia package` |
|---|---|---|---|---|
| `developer` | `.dev` | red `D` badge | ✓ | |
| `preview` | `.preview` | red `P` badge | | |
| `release` | `(none)` | unmodified | | ✓ |

Different envs of the same app install **side by side** because their bundle/package ids differ. No git-tracked file changes when you switch envs — every effect lands in a build-output directory or is passed at build time.

### Per-env `lingxiaServer`

Single URL (same for every env):

```yaml
app:
  lingxiaServer: https://api.myapp.com
```

Per-env map (omit envs you don't have a server for — typical for `developer`):

```yaml
app:
  lingxiaServer:
    developer: http://192.168.1.10:8080
    preview: https://preview.api.myapp.com
    release: https://api.myapp.com
```

### Per-env `packageIdSuffix`

Built-in defaults (`.dev` / `.preview` / `(none)`) cover most apps. Override only when you need custom suffixes:

```yaml
app:
  packageIdSuffix:
    developer: .internal   # → com.example.myapp.internal
    preview: ".preview"    # quote when starting with .
    release: ""            # "" = opt out of any suffix
```

Validation rules:

- Each suffix must match `^\.[a-z0-9]+(\.[a-z0-9]+)*$` (start with `.`, lowercase a-z 0-9 segments) — or be `""` to opt out.
- Empty `lingxiaServer` string is rejected. Per-env map must have at least one entry set.
- Unknown keys (e.g. `enviroments:` typo) surface as YAML parse errors, not silent ignores.

### Reading the env at runtime

JS Logic (`pages/*/index.ts`): `lx.app.envVersion` — `'developer' | 'preview' | 'release'`, fixed at app boot. See [Logic-side `lx.*` API](../lxapp/lx-api.md).

Rust host: `lingxia::env_version()` returns the same enum.

The build-time plumbing per platform (Android Gradle properties, iOS bundle id rewrite, Harmony staging mirror, publish-flow id matching) is internal — app authors don't touch it.

---

## `features` Section

`features` controls native Rust compile-time features. `appService` (default on) enables the JS/TS AppService runtime: when it is `false` the CLI builds the host Rust library with `--no-default-features`; when `true`, Cargo default features stay enabled and the CLI adds the derived features. `devtools` (default off) compiles in devtools hooks — `lingxia dev` may enable it transiently without editing YAML.

**Flip `appService` and the home lxapp's `logic` together.** A native-only host sets `features.appService: false` *and* the home lxapp's `lxapp.json` `"logic": false` (Shape C). A logic-enabled lxapp under `appService: false` is rejected at startup. `-t lxapp` projects always require an AppService-capable host; `-t native-app` projects may opt out when they only need native-hosted UI and host APIs.

The browser, terminal, and HTTP-proxy runtime features are **not** set here — they are derived from the [`capabilities`](#capabilities-section) below.

---

## `capabilities` Section

`capabilities` is for platform/runtime integrations that must be predeclared before the SDK auto-enables them. Each one toggles the corresponding native runtime feature at build (all default off). Do not list ordinary SDK APIs such as camera here; those request permission only when called.

- `notifications` — push/notification integration where supported. iOS/Harmony SDK startup may request notification permission and fetch a push token.
- `browser` — the in-app browser (its newtab / settings / downloads pages and shell runtime). Cross-platform; bundles the browser webui, overridable via the [`browser`](#browser-section) section.
- `terminal` — the built-in terminal runtime. Required before a `native: terminal` surface can be declared (desktop only).
- `proxy` — the in-app browser's HTTP proxy (desktop). Requires `browser`.
- `process` — OS process launch/management for trusted Agent-style products (macOS/Windows). Available only to the home lxapp, which must also declare `security.privileges: [process]`; adds `Rong.spawn`, `Rong.spawnSync`, and `Rong.$` plus the opt-in `@lingxia/types/process` declarations.
- `autostart` — unlocks `lx.app.autostart` (launch at system startup; macOS/Windows, home lxapp only). Declaring it never registers the app by itself — enabling is a runtime user decision via the API.

---

## `browser` Section

`browser` overrides the in-app browser webui, used only when `capabilities.browser: true`. Normal apps omit it and use the SDK default. Set exactly one source under `webui`: a project-relative `path:` to a browser-shell webui lxapp source tree (the CLI builds it — for developing a custom webui alongside the app), or a `package:` npm name shipping a prebuilt `lxapp.json` + `dist/` (with an optional `version:`; the CLI version is used when omitted). Setting both is rejected.

Do not use `app.homeAppId` for browser internals. `app.homeAppId` is the product home app; `browser.webui` is the browser UI asset.

---

## `resources` Section

`resources.bundles` declares lxapp asset sources bundled into the native host. It does not decide what the app opens; `app.homeAppId` and the `surfaces[]` ids do that.

Each bundle entry has a `type` (currently `lxapp`) and an `appId` that **must match** the bundle's `lxapp.json.appId` (the id-alignment rule again). Its asset source is exactly one of: a project-relative `path:` (the CLI builds and bundles it) or a `package:` npm name shipping a prebuilt `lxapp.json` + `dist/` (optional `version:`; CLI version when omitted). Setting both is rejected, and appIds must be unique across bundles.

Example:

```yaml
resources:
  bundles:
    - type: lxapp
      appId: home
      path: home
    - type: lxapp
      appId: settings
      path: ../settings
```

If a bundle entry has only `type` and `appId`, it declares the appId but does not bundle local assets; the runtime/update provider must make it available. SDK-reserved appIds (e.g. `app.lingxia.browser`) are not listed here — use `browser.webui.*` instead.

---

## `storage` Section

`storage` makes storage policy visible instead of relying on hidden defaults. Values are MiB: `tempMaxSizeMB` (host temp), `cacheMaxSizeMB` (per-lxapp usercache), `dataMaxSizeMB` (user data), `appStorageMaxSizeMB` (app-scoped). The scaffold seeds the current default caps.

The cache cap has the one non-obvious behavior worth knowing: cleanup triggers at 80% high water and LRU-evicts down to 50% low water, and `cacheMaxSizeMB: 0` disables size enforcement entirely (the scaffold note on `lingxia new` points this out).

---

## `macos` Section

`macos` sets the macOS bundle id, deployment target, and the SwiftPM `targetName` (resource lookup) / `executableName` (product binary). All are optional — the CLI tries reasonable defaults and falls back to inference — but explicit names give reproducible builds. An optional `store:` block holds the App Store Connect identity (`bundleId` / `appId`) used by `lingxia store`. The scaffold writes a starting `macos:` for you; read it for the exact keys.

## `windows` Section

`windows` is the desktop host for Windows, on the same adaptive `surfaces:` model as macOS (no per-platform UI block). You don't hand-wire the Windows SDK: scaffold with `lingxia new -t native-app -p windows` (combine with other platforms, e.g. `-p macos,windows`) and the generated project drops in the `windows/` Rust host crate and its packaging wired to the right SDK refs — read the generated project rather than pasting git refs or patch blocks here.

The `windows:` section carries the packaging identity — `appId` (env suffixes apply like other platforms' package ids), `executableName` (the `windows/Cargo.toml` binary), and `publisher` (the MSIX `Publisher` distinguished name, defaulting to `CN=<productName>`). An optional `store:` block holds the Microsoft Store (Partner Center) `appId` for `lingxia store`. Build with `lingxia build --platform windows`; submit to the MS Store with `lingxia store --platform windows`. As always, the scaffolded `lingxia.yaml` is the authoritative field list.

---

## Surfaces (adaptive UI)

A host app's UI is a flat list under top-level `surfaces:`. You declare *what* each surface is and *how it relates* to the others; the Host derives the realized platform form (window / panel / sidebar / tab / tray) from screen size at runtime — there are **no** per-platform `macos:` / `windows:` UI blocks.

`lingxia build` compiles `surfaces:` into the internal `ui.json` the runtime consumes. Do not hand-write `ui.json`.

### Surface fields

Each entry starts with its **content key** — exactly one of `lxapp` / `url` / `native` — whose value names the content and doubles as the surface's identity (there is no separate `id` and no `render` field):

| Field | Type | Required | Description |
|---|---|---:|---|
| `lxapp` | string | one content key | An lxapp, by appId. Roles: `main` \| `aside` \| `float`. |
| `url` | string | one content key | A page in the in-app browser (requires `capabilities.browser: true`). Roles: `main` \| `aside`. |
| `native` | string | one content key | A host-native capability — only the built-in `terminal` today. Role: `aside`. |
| `role` | `main` \| `aside` \| `float` | Yes | `main` = a switchable primary surface; `aside` = a docked companion; `float` = a tray-anchored popover (requires a `tray:`). |
| `launch` | bool | No | Open on start. At most one `main` may set `launch: true` (the initial surface). Omit on all mains for a tray-launched app. |
| `edge` | `left`\|`right`\|`top`\|`bottom` | No | Aside docking edge. Defaults to `right`; the terminal defaults to `bottom` (and only accepts `top`/`bottom`). |
| `size` | object | No | Aside preferred-size hint, e.g. `{ width: 320 }`. The shell clamps it at layout time. |
| `tray` | object | No | Adds a menu-bar (macOS) / system-tray (Windows) entry: `{ icon?, label?, action?, exclusive?, size? }`. `action`: `toggle` (visible→hide, hidden→show) or `activate` (show + bring to front). `exclusive: true` → no dock / taskbar icon. `size: { width, height }` (on a `role: float` popover) sets the popover content size. |
| `platforms` | string[] | No | Availability filter — `macos`, `windows`, `ios`, `android`, `harmony`. Empty = all platforms. |

Icons (`tray.icon`) are host-root-relative SVG source paths — see [Icon Paths](#icon-paths).

There is **no `sidebar:` entry field**: app-owned sidebar activators are declared at runtime through `lx.shell.activators`, never in YAML. Each entry provides `onActivate`; the callback explicitly opens the desired surface or performs the action.

### Rules (enforced at build)

- A config needs at least one `lxapp` `main` surface — or, for a pure popover app, a `role: float` surface with a `tray:` and no `main`.
- At most one `main` may set `launch: true`; `launch` is invalid on a non-main surface and only supported on an lxapp main.
- `edge` and `size` are only valid on `aside`.
- A `url` surface requires `capabilities.browser: true` and supports `role: main | aside`.
- `native` supports only `terminal`, requires `capabilities.terminal: true`, and its `edge` must be `top` or `bottom`.
- The same content key may be declared at most once.
- `role: float` requires a `tray:` (it is a tray-anchored popover); a bare `role: float` is rejected.
- At most one surface may declare `tray:`.

### Example — main + assistant aside + terminal

```yaml
capabilities:
  browser: true
  terminal: true

surfaces:
  - lxapp: my-home       # main screen: your lxapp, by appId
    role: main
    launch: true
    tray:
      icon: icons/tray.svg
      label: My App
      action: activate
  - lxapp: assistant     # right-docked companion lxapp
    role: aside
    edge: right
    size: { width: 320 }
  - native: terminal     # built-in native terminal (needs capabilities.terminal)
    role: aside
    edge: bottom
    platforms: [macos, windows]   # desktop-only
```

Each `lxapp` surface needs its assets bundled — list its appId in `resources.bundles`, or let the runtime/update flow provide it.

### How the desktop shell realizes surfaces

On desktop the main window is a sidebar plus a main area plus docked asides, and the shell picks the realized form from the window width:

- **Wide**: full sidebar (pins, main tabs, activators) with up to three docked asides beside the main.
- **Medium**: the sidebar collapses to an icon rail and at most one aside stays docked.
- **Narrow** (and mobile): the sidebar disappears, `main` goes full screen, and asides overlay the main full screen.

Asides group into per-engine slots (lxapp / browser / native), each with its own tab strip; switching tabs hides and shows content, and only an explicit close destroys it.

Browser asides adapt their chrome with the slot. Desktop may show the current
URL read-only, but never permits address editing or user-created tabs. On mobile
and phone Runner, the aside is a full-screen browser with a single bottom row
for page history, refresh, its own tab group, and dismissal; it has no address
row or generic top-left Back. System Back, edge Back, and dismissal return to
the main without destroying the aside tabs. The self browser keeps its editable
address and a separate tab group.

Two sidebar regions have fixed ownership:

- **Pins are the user's** — quick entries for lxapps and websites (eight at most), added and removed through context menus. There is no app API to write them.
- **Activators are the app's** — runtime entries the home lxapp declares via `lx.shell.activators` (see the `@lingxia/types` declarations). The shell invokes `onActivate` and performs no built-in navigation; callbacks can call `lx.openSurface(...)` or run any other app logic. Redeclare them each Logic launch.

### Menu-bar / system-tray apps

A `tray:` entry adds a menu-bar item (macOS) / system-tray icon (Windows). The same declaration drives three shapes:

- **Dock + tray** — `role: main` with a `tray:` (default `exclusive: false`). Keeps the dock / taskbar icon and full window UI; the tray entry summons the window (`action: activate` brings it to front, `toggle` hides on re-click).
- **Tray only** — add `exclusive: true`. No dock / taskbar icon and no flash at launch (macOS sets `LSUIElement`; Windows uses `WS_EX_TOOLWINDOW`). The app lives only in the tray.
- **Tray popover** — `role: float` + a `tray:`. Clicking the tray icon opens the surface as an auto-dismissing popover anchored under the icon. Set its size with `tray.size: { width, height }` (default 360×420). A pure popover app has no `main`.

```yaml
surfaces:
  - lxapp: my-panel
    role: float            # tray-anchored popover
    tray:
      icon: icons/tray.svg
      exclusive: true       # no dock / taskbar icon
      size: { width: 320, height: 480 }
```

#### Runtime tray / dock APIs (JS)

The tray's dynamic content is updated from page/app logic:

- `lx.tray.setIcon(path)` / `lx.tray.setTitle(text)` / `lx.tray.setBadge(value)` — update the status item's icon, its text (macOS), and a badge (e.g. an unread count).
- `lx.app.setBadge(value)` — the dock (macOS) / taskbar (Windows) badge.

Pass `null` / empty to clear a badge or title. The tray *shape* is declared in `lingxia.yaml`; these APIs only change its runtime content.

### Terminal surface

The built-in terminal is a native aside (`native: terminal`, `edge: top | bottom`, default `bottom`) gated by `capabilities.terminal`. To expose it as an activator, declare a runtime entry whose `onActivate` calls `lx.openSurface({ native: 'terminal' })`.

It shares a single cross-platform Rust engine that owns sessions, PTY transport, terminal semantics, and the snapshot/input protocol; platform SDKs only render snapshots into a native view and capture input. Backend selection is owned by the runtime — there is no backend selector in `lingxia.yaml`.

---

## Icon Paths

Surface `tray.icon` values are source icon paths relative to the host project root.

The current UI supports SVG source icons only. During `lingxia build`, the CLI validates each source icon, converts it to a platform resource, copies it into generated `icons/`, and rewrites the generated `ui.json` to reference that generated resource path.

Example:

```yaml
tray:
  icon: icons/tray.svg
```

Validation rules:

| Check | Rule |
|---|---|
| Source format | SVG only |
| Path | Relative to host project root; absolute paths and `..` are rejected |
| File size | Maximum 512 KB |
| SVG viewport size | 16x16 px through 512x512 px |
| Aspect ratio | Must be square, within a small tolerance |

Do not reference generated lxapp runtime assets such as `app.lingxia.browser/public/LingXia.png`. Use a host-root-relative SVG source file instead; it is fine for that file to live inside the home lxapp project, because the CLI converts and copies it into native host resources.

---

## Generated Files

During `lingxia build`, the CLI generates platform resources:

- `app.json`: runtime app metadata.
- `ui.json`: the UI structure compiled from `surfaces:`.
- `icons/*`: generated native chrome icons.
- bundled lxapp directories from `resources.bundles`.
- bundled browser webui directory when `capabilities.browser: true`.
- `bridge-runtime.js`.

For macOS, these are copied into the SwiftPM target resource directory, usually `macos/Sources/<targetName>/Resources` unless the target declares a custom `path`.

Generated files are build artifacts. Edit `lingxia.yaml` instead.

---

## Build

Build macOS from the host project root:

```bash
lingxia build --platform macos
```

The macOS host build does the following:

- Builds the configured home lxapp resource bundle.
- Generates `app.json` and `ui.json` from `surfaces:`.
- Builds the Rust host static library with the native features derived from `features` + `capabilities` (e.g. `capabilities.browser` adds the browser/shell runtime, `capabilities.terminal` the terminal runtime).
- Builds the SwiftPM macOS app.
- Packages the `.app` under `target/lingxia/macos/`.

Example output:

```text
target/lingxia/macos/My App.app
```

If `--skip-native` is used, SwiftPM links an existing Rust static library. That can leave runtime capability bits stale (including browser/terminal). For UI debugging, prefer a normal build without `--skip-native`.

---

## Common Pitfalls

- Hand-writing `ui.json` or editing generated `app.json` / `ui.json` — author `surfaces:` in `lingxia.yaml`; they are regenerated every build.
- `homeAppId` not matching any `resources.bundles[].appId` — build fails or the wrong app launches.
- Declaring more than one `main` with `launch: true`, or `launch: true` on an `aside`.
- An `aside` without an `edge`, or an `edge` on a `main`.
- `native:` on anything but `terminal`, or a terminal surface without `capabilities.terminal: true`, or a terminal `edge` other than `top`/`bottom`.
- Using `role: float` without a `tray:` — a float surface is only valid as a tray-anchored popover.
- Reusing one lxapp `appId` across multiple surfaces.
- Adding Settings or Downloads as their own surfaces — those are built-in browser pages, opened by built-in chrome when `capabilities.browser` is on.
- Expecting browser chrome without `capabilities.browser: true` — browser shell UI is opt-in.
- Using PNG or generated lxapp runtime images for surface icons; icons must be host-root-relative SVG source files.
- Expecting hidden surfaces to destroy WebViews — hiding preserves state.
- Running an older `lingxia` binary from `PATH` after changing config schema or CLI validation.

---

## Pre-ship checklist

- [ ] `lingxia.yaml` validates: every required platform section present; `homeAppId` resolvable to a `resources.bundles[].appId`.
- [ ] Exactly one `main` surface (or a `role: float` tray popover); every `aside` has an `edge`; terminal surfaces have `capabilities.terminal: true`.
- [ ] `features.appService` matches the embedded lxapp's logic mode.
- [ ] All native routes return `lingxia::Result<T>` with `Serialize` outputs.
- [ ] `HostAddon` registers every route and extension; FFI exports present for each target platform.
- [ ] `lingxia doctor` passes; `lingxia dev` boots on a real/simulated device.

## Out Of Scope / Not Implemented Yet

The surface model intentionally does not yet define:

- splash / launch screens — LingXia does not provide them; host apps own their launch UX
- multiple `main` surfaces open as separate top-level windows simultaneously
- asides nested under other asides
- reusing one lxapp `appId` across multiple surfaces
- native (`native:`) surfaces other than the built-in `terminal`
- terminal backend selection in config
