# App Project Configuration

A LingXia app project is a native host app. It may embed a control lxapp, or on
macOS/Windows it may be a native-only terminal/browser product with no bundled
lxapp. Either shape can open bundled or runtime-provided lxapps. Its build-time
config lives in `lingxia.yaml`.

The UI is described by a flat, adaptive `surfaces:` list (see [Surfaces](#surfaces-adaptive-ui)) — you declare *what* each surface is and the Host derives the realized platform form (window / panel / sidebar / tab / tray) by screen size. macOS is the most complete runtime today; the same `surfaces:` schema feeds every platform.

For lxapp page development, see [LxApp Development Guide](../lxapp/guide.md).
For CLI commands, see [CLI Command Reference](../cli/lingxia.md).

---

## Create A Host App

```bash
lingxia new my-app -t native-app -p macos --package-id com.example.myapp -y
```

This creates a host app project, not a standalone lxapp. By default it includes
an embedded lxapp that is both the main experience and control app.

To make a desktop native capability the product main, select it explicitly:

```bash
lingxia new my-terminal -t native-app --main terminal --control native -y
lingxia new my-browser -t native-app -p windows --main browser --control native -y
```

Native main defaults to native control, omits `lxapp/`, `app.homeAppId`, and
`resources`, enables the matching capability, and sets
`features.appService: false`. Pass `--control lxapp` only when a non-visible embedded lxapp is actually
needed for host control logic. Runtime/network lxapps are product workspaces or
guest content; they do not become the host's trusted control app.

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
- optionally, an embedded control lxapp source (scaffold default `lxapp/`).

- `lingxia build` generates runtime `app.json` and `ui.json` from `lingxia.yaml`.
- Do not edit generated `app.json` or `ui.json` directly.
- When present, `app.homeAppId` identifies the trusted embedded control lxapp;
  `resources.bundles` resolves its assets. The launch `main` surface determines
  the visible initial experience.

---

## SDK Startup APIs

Use the product-app startup entry on each platform:

| Platform | Entry |
|---|---|
| Apple | `Lingxia.quickStart()` |
| Android | `Lingxia.quickStart(activity)` |
| Harmony | `Lingxia.quickStart(context, windowStage)` |

`quickStart` means the native app is a LingXia host product. It initializes the
runtime and opens the configured launch surface through the platform host shell
or navigation container.

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

For a Rust-controlled menu-bar app, omit `launch: true` and give the main
surface a `tray:` entry, set `features.appService: false`, and use a
`logic: false` HTML control lxapp. The no-lxapp scaffold is intentionally narrower: its
launch main must be the built-in terminal or browser.

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
| `theme` | Optional | Application-wide semantic colors for host-owned native UI |
| `resources` | Conditional | Bundle asset sources; omit when no control/product lxapp is bundled |
| `splash` | Optional | Generated launch placeholder and first-frame cover |
| `assets` | Optional | Raw host files packaged through each platform's asset pipeline |
| `browser` | Optional | Override the in-app browser webui (only used when `capabilities.browser: true`) |
| `appLinks` | Optional | Universal-link / app-link hosts (see [App Links](./applinks.md)) |
| `storage` | Recommended | Explicit host temp/cache/data size limits |

---

## `app` Section

`app` carries host metadata that generates the runtime `app.json`: `projectName` (technical identifier behind native build paths, the Rust host library name, and platform artifact filenames), `productName` (user-facing), `productVersion` (a semver string — the build rejects non-semver), and `platforms` (the enabled set, drawn from `macos`, `windows`, `ios`, `android`, `harmony`). Optional `lingxiaId` / `lingxiaServer` / `packageIdSuffix` drive publishing and per-env builds (see [Environment versions](#environment-versions)).

`homeAppId` is optional only for a macOS/Windows native-main host with
`features.appService: false`. Such a host still declares exactly one launch
main (`native: terminal` or `native: browser`) and enables its capability.

**The id-alignment rule (the one that bites).** When a control lxapp is present,
three ids must line up or the wrong app launches, and the build enforces it:

- `app.homeAppId` = a `resources.bundles[].appId` = that bundle's `lxapp.json.appId`.
- If the launch `main` is an lxapp, its `lxapp:` key points at that same ID. A
  native launch main remains independent from the embedded control lxapp.

`homeAppVersion` is not configured here; the CLI derives it from the matching `resources.bundles` source. The full, current field set is in a freshly scaffolded `lingxia.yaml`.

---

## `theme` Section

`theme` defines application-wide semantic colors for host-owned native UI. It
is host configuration, not an lxapp content theme. Both `light` and `dark` are
optional, and every role inside them is optional:

```yaml
theme:
  light:
    pageBackgroundColor: "#E9EAEE"
    windowBackgroundColor: "#F4F5F7"
    surfaceBackgroundColor: "#FFFFFF"
    foregroundColor: "#111827"
    mutedForegroundColor: "#667085"
    accentColor: "#2865FF"
    separatorColor: "#E5E7EB"
    selectionBackgroundColor: "#EEF3FF"
  dark:
    pageBackgroundColor: "#1B1D21"
    windowBackgroundColor: "#17191C"
    surfaceBackgroundColor: "#23262B"
    foregroundColor: "#F3F4F6"
    mutedForegroundColor: "#9CA3AF"
    accentColor: "#5B8CFF"
    separatorColor: "#343840"
    selectionBackgroundColor: "#303641"
```

Values use opaque `#RRGGBB` sRGB syntax. The build rejects alpha colors,
unknown roles, and unknown scheme names. Missing roles retain the platform's
semantic default for that scheme; values never fall back from light to dark or
from dark to light. On macOS those defaults are dynamic AppKit semantic colors;
on Windows they are Fluent theme tokens, with system colors taking precedence
in a contrast theme.

The Windows and macOS desktop shells consume `windowBackgroundColor` for the
window backdrop and sidebar, and `surfaceBackgroundColor` for native cards and
other raised surfaces. Text, selection, accent, and structural dividers consume
the correspondingly named roles. Other hosts can map the same semantic roles
without adding platform-specific configuration.

`pageBackgroundColor` is the odd one out: not a colour native UI paints
itself, but the host declaring what colour the lxapp paints its page — no
platform can ask a WebView for its document colour in time to paint a frame
already on screen. Native chrome uses it wherever it borders the page (the
strip a pull-to-refresh opens, the canvas behind navigation transitions).
Set it to the page floor the lxapp's CSS uses, both schemes; unset, each
platform keeps its system background, which reads as a pale seam on a
tinted page.

Lxapp page content does not inherit these colors; it responds to the standard
`prefers-color-scheme` surface and owns its CSS design.

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

JS Logic (`pages/*/index.ts`): `lx.app.envVersion` — `'developer' | 'preview' | 'release'`, fixed at app boot. See [Logic runtime and typings](../lxapp/lx-api.md).

Rust host: `lingxia::env_version()` returns the same enum.

The build-time plumbing per platform (Android Gradle properties, iOS bundle id rewrite, Harmony staging mirror, publish-flow id matching) is internal — app authors don't touch it.

---

## `features` Section

`features` controls native Rust compile-time features. `appService` (default on) enables the JS/TS AppService runtime: when it is `false` the CLI builds the host Rust library with `--no-default-features`; when `true`, Cargo default features stay enabled and the CLI adds the derived features. `devtools` (default off) compiles in devtools hooks — `lingxia dev` may enable it transiently without editing YAML.

**Flip `appService` and an embedded control lxapp's `logic` together.** With an
embedded control lxapp, `features.appService: false` requires its `lxapp.json`
to use `"logic": false` (Shape C). A logic-enabled lxapp under
`appService: false` is rejected at startup. A native-main/native-control desktop host has no
control lxapp at all and also sets `appService: false`. `-t lxapp` projects
always require an AppService-capable host.

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
- `appUse` — lets a command line or agent skill on the same machine drive this product's own windows (screenshot, window list, mouse, keyboard), and turns the product's executable into its own command line. macOS/Windows. The local socket this needs is derived, not declared — which IPC carries it is plumbing. Declaring it ships the ability, not the decision: the endpoint stays closed until the user turns it on, the same way `autostart` works.
- `computerUse` — extends that to the machine, and implies `appUse` because it already contains it (an agent that can drive any window can drive this product's): screenshots of any window, synthetic input, the accessibility tree. Named for what the user grants, because they will be asked — macOS prompts for Accessibility and Screen Recording, and the entry in System Settings is this product. Commands run inside the app rather than in the calling process, so that grant stays attached to the product no matter which terminal invoked it.
- `browserUse` — extends it to the in-app browser. Requires `browser`.
- `mediaCapture` — realtime visual / system-audio / microphone capture for a product session. Independent of `computerUse`. Declare only the tracks this product needs; a host that omits the key constructs no provider and receives no capture-specific services, permissions, or entitlements. Snapshot (`lxdev desktop screenshot`, `computerUse` screenshots) stays visual-only and does not enable this.

---

## `browser` Section

`browser` overrides the in-app browser webui, used only when `capabilities.browser: true`. Normal apps omit it and use the SDK default. Set exactly one source under `webui`: a project-relative `path:` to a browser-shell webui lxapp source tree (the CLI builds it — for developing a custom webui alongside the app), or a `package:` npm name shipping a prebuilt `lxapp.json` + `dist/` (with an optional `version:`; the CLI version is used when omitted). Setting both is rejected.

Do not use `app.homeAppId` for browser internals. When present, `homeAppId` is
the trusted product control app; `browser.webui` is the browser UI asset.

---

## `resources` Section

`resources.bundles` declares lxapp asset sources bundled into the native host.
It is optional for a native-main/native-control desktop host. It does not decide
what the app opens; `app.homeAppId` and the `surfaces[]` ids do that. (Raw host
files with no lxapp identity belong in the `assets` section instead.)

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

If a bundle entry has only `type` and `appId`, it declares the appId but does not bundle local assets; the runtime/update provider must make it available. Browser-shell internals (`app.lingxia.browser`) are configured through `browser.webui.*`; other lxapps, including product settings pages, are ordinary resources.

---

## `storage` Section

`storage` makes storage policy visible instead of relying on hidden defaults. Values are MiB: `tempMaxSizeMB` (host temp), `cacheMaxSizeMB` (per-lxapp usercache), `dataMaxSizeMB` (user data), `appStorageMaxSizeMB` (app-scoped). The scaffold seeds the current default caps.

The cache cap has the one non-obvious behavior worth knowing: cleanup triggers at 80% high water and LRU-evicts down to 50% low water, and `cacheMaxSizeMB: 0` disables size enforcement entirely (the scaffold note on `lingxia new` points this out).

---

## `splash` Section

Optional launch screen, generated per platform from a few fields — no hand-built launch UI. The launch experience is "tap the icon, see the art": `image` (PNG, full-screen aspect-fill) is the app's first frame on every cold start, held until the home page first renders, then fading into real content. Where the platform allows it the OS launch frame carries that same art — HarmonyOS's start window, iOS's `UILaunchScreen` — so the two frames are one picture and the handoff has nothing to change.

Android is the exception: its 12+ system splash offers a colour and an icon slot and nothing else, so there the OS beat is `background` (required, `#RRGGBB`) and the art arrives on the app's first frame. Pick the art's own ground for `background` and that beat reads as the art's entrance rather than as a flash. On Android the icon slot follows from whether art is configured: with art it is blanked, since the art is the app's real first face and an icon before it would be a second one; without art the platform draws the real app icon, preserving the launcher's zoom morph. HarmonyOS follows the same rule with a generated transparent icon.

**One face, every appearance.** The launch screen is a brand asset, not a UI surface that follows the system: it has to be identical to a frame the OS composed from build-time resources before the process existed, and one picture is the only thing that always is. There is deliberately no dark counterpart — an appearance pair can only ever follow the *system*, never an in-app appearance choice (HarmonyOS's colour mode does not survive process death and iOS has no such lever at all), so the pair's own halves are what end up disagreeing at launch.

`mark` (PNG, authored at the pixels it should occupy on screen) is what the OS frame centers when no `image` is configured — the documented placeholder-only launch, which holds until the home page is ready.

The launch face is this configured art and nothing else. A host's Rust addon can implement `select_campaign` to show a screen of its own *after* the launch face — its own art, a countdown, skippable — see [Launch Screen](../native/splash.md).

`minDuration` (ms, default 600) is the minimum the launch face stays on screen, measured from process start — the OS frame is already showing the same art, so the user has been looking at it since before this process could count. The maximum is a framework constant.

An iOS release-environment build fails when the asset catalog cannot compile:
without `Assets.car`, `UILaunchScreen` normally cannot resolve the configured
face and iOS would ship a white system frame. There is one narrow device-build
fallback when Xcode has the iOS SDK but no simulator runtime: with a full cover,
the CLI installs that cover under the raw resource name used by
`UILaunchScreen` and keeps the legacy icon output that `actool` did produce. A
mark-only face still requires the iOS platform (`xcodebuild -downloadPlatform
iOS`); developer/preview builds warn when neither path is available.

---

## `assets` Section

`assets: <dir>` packages a project directory into every platform build
through the platform's own asset pipeline; native Rust reads the files back
with `lingxia::assets::read("relative/path")` once the runtime is up. Use it
for host files that should ship with the app — extra launch covers, fonts,
data — instead of embedding bytes in the native library, which bypasses
store optimizations and weighs down library load.

Not for lxapp packages: those are `resources.bundles` — built, appId-addressed,
and served by the runtime, none of which applies to these raw files.

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
| `url` | string | one content key | A page in the managed browser (requires `capabilities.browser: true`). macOS and Windows admit it as `main`; Windows also retains declarative `aside` support, while browser asides may be opened dynamically with `lx.surface.openUrl(url, { as: 'aside' })`. |
| `native` | string | one content key | A built-in host surface: `terminal` or `browser`. On macOS and Windows, terminal supports `main` / `aside`; browser supports `main`. |
| `role` | `main` \| `aside` \| `float` | Yes | `main` = a switchable primary surface; `aside` = a docked companion; `float` = a tray-anchored popover (requires a `tray:`). |
| `launch` | bool | No | Open on start. At most one `main` may set `launch: true` (the initial surface). Omit on all mains for a tray-launched app. |
| `page` | string | No | Configured page name from the lxapp's `lxapp.json`. Omit it to open the initial page; full routes are internal and are not accepted. |
| `query` | object | No | Parameters passed to the selected lxapp page. Values may be strings, numbers, booleans, or null. |
| `edge` | `left`\|`right`\|`top`\|`bottom` | No | Preferred docking side for an aside. `role: aside` chooses the companion region; `edge` places that region when the Host has room to dock it. Defaults to `right`; terminal defaults to `bottom` and accepts only `top`/`bottom`. Compact Hosts may reproject it as a full-screen overlay. |
| `size` | object | No | Aside preferred-size hint, e.g. `{ width: 320 }`. The shell clamps it at layout time. |
| `tray` | object | No | Adds a menu-bar (macOS) / system-tray (Windows) entry: `{ icon?, label?, action?, exclusive?, size? }`. `action`: `toggle` (visible→hide, hidden→show) or `activate` (show + bring to front). `exclusive: true` → no dock / taskbar icon. `size: { width, height }` (on a `role: float` popover) sets the popover content size. |
| `platforms` | string[] | No | Availability filter — `macos`, `windows`, `ios`, `android`, `harmony`. Empty = all platforms. |

Icons (`tray.icon`) are host-root-relative SVG source paths — see [Icon Paths](#icon-paths).

There is **no `sidebar:` entry field**: app-owned sidebar actions are declared at runtime through `lx.shell.sidebarActions`, never in YAML. Each entry chooses `placement: header | footer` and provides `onActivate`; the callback explicitly opens the desired surface or performs the action.

### Rules (enforced at build)

- macOS and Windows admit exactly one declared `main`, whose content may be `lxapp`, `url`, `native: terminal`, or `native: browser`. Other targets still require the home lxapp as their initial main until their native presenters implement this contract. A pure desktop popover app may instead declare one `role: float` surface with a `tray:` and no main. Additional browser/terminal main entries are runtime workspace Surfaces, not extra YAML main declarations.
- After `platforms` filtering, at most one `main` may set `launch: true`; `launch` is invalid on a non-main. macOS and Windows allow it on any admitted main content; other targets currently allow it only on their home lxapp main.
- `edge` and `size` are only valid on `aside`.
- `page` and `query` are valid only with `lxapp` content. Page selection uses the configured page name, matching `lx.navigateTo`, `lx.navigateToApp`, `lx.shell.openApp`, and `lx.surface.openPage`; parameters stay separate in `query`.
- A `url` surface requires `capabilities.browser: true`; declarative URL main is supported on macOS and Windows.
- `native: terminal` requires `capabilities.terminal: true`; an aside uses `edge: top | bottom`. `native: browser` requires `capabilities.browser: true` and supports a macOS or Windows main.
- The same content key may be repeated only when its `platforms` filters are mutually exclusive; after filtering, surface identities remain unique on every target.
- `role: float` requires a `tray:` (it is a tray-anchored popover); a bare `role: float` is rejected.
- At most one effective surface may declare `tray:` on each target platform.

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

- An lxapp in `main` owns the primary content area and appears in the sidebar's main switcher. The content area itself has no tab strip.
- An lxapp in `aside` occupies a companion region at the left, right, top, or bottom of the main and switches through that region's tab strip. It never appears in the sidebar's main switcher.

One lxapp has one live role in a window. Opening it under the other role must move or reopen that same logical app according to the entry point's contract; the Host must never project it as both main and aside.

- **Wide**: full sidebar (pins, main tabs, activators) with up to three docked asides beside the main.
- **Medium**: the sidebar collapses to an icon rail and at most one aside slot is admitted; an explicitly opened slot that cannot preserve the main minimum overlays the content pane.
- **Narrow desktop**: the icon rail remains and `main` keeps its desktop
  workspace; asides overlay the main when they cannot be admitted beside it.
  Browser chrome keeps its top address toolbar; it may collapse secondary
  actions, but never moves to the bottom or into the rail. A narrow desktop does
  not restore an lxapp's mobile bottom tabbar.
- **Mobile / phone Runner**: the sidebar disappears, `main` goes full screen,
  and asides overlay the main full screen.

Asides group into per-engine slots (lxapp / browser / native), each with its own tab strip; switching tabs hides and shows content, and only an explicit close destroys it.
When admission reprojects an aside as an overlay, it covers the main content pane inside the same host window; it is not a second workspace window and never enters the main switcher.

Browser asides adapt their chrome with the slot. Desktop may show the current
URL read-only, but never permits address editing or user-created tabs. On mobile
and phone Runner, the aside is a full-screen browser with a single bottom row
for page history, refresh, its own tab group, and dismissal; it has no address
row or generic top-left Back. System Back, edge Back, and dismissal return to
the main without destroying the aside tabs. The self browser keeps its editable
URL field and a separate tab group; the field accepts URLs, not search queries.

Two sidebar regions have fixed ownership:

- **Pins are the user's** — quick entries for lxapps and websites (eight at most), added and removed through context menus. An lxapp Pin always opens or focuses a main workspace. The Pin tile remains a shortcut while the open lxapp also gets an independent sidebar workspace row for switching and lifecycle controls; hovering the row reveals an explicit ellipsis for its provider-backed menu, and right-click opens the same menu. Unpinning does not close or remove that live row. Its content uses the same rectangle as the home lxapp, with the previous main hidden, no duplicate host window, and no content-area tab strip. It does not inherit a declared aside role. That restriction changes entry role only: a Pin must not add an inset, clip, navigation offset, or alternate content rectangle. Use a sidebar action plus `lx.surface.openDeclared(id)` for the aside entry. There is no production app API to write Pins.
- **Sidebar actions are the control lxapp's** — when one is configured, it may
  declare runtime entries via `lx.shell.sidebarActions` (see the
  `@lingxia/types` declarations). Header actions are icon-only and limited to
  two; footer actions use labeled cells and scroll after five visible rows. The
  shell invokes `onActivate` and performs no built-in navigation; callbacks can
  call `lx.surface.openPage(...)` or run any other app logic. Redeclare them each
  Logic launch.

The initial `main` is admitted first as the window's stable root and cannot be closed. Other
main surfaces expose only the actions their content provider supports: browser
and terminal surfaces may be closed or renamed, while a non-root lxapp
workspace may be closed or restarted through its provider-backed sidebar menu
but cannot be renamed. Closing an active non-root main selects another
remaining main, so the product Host never enters a synthetic zero-main or
empty-state mode.

In a collapsed desktop rail, hovering the current switcher replaces its icon
with a rounded, background-backed close `x` only when that main can be closed.
Inactive switchers keep their icons and click to select; icon-only switchers and
footer actions expose their labels through tooltip and accessibility text. A
subtle divider separates Pins from live switchers when both sections exist.

When `homeAppId` is configured, that lxapp remains the trusted control app even
when the visible desktop main is a URL or native surface. Its Logic worker still
receives `App.onLaunch` once and may register sidebar actions and other host
chrome without creating a hidden WebView. A native-control scaffold has no such
worker or hidden lxapp identity.

### Menu-bar / system-tray apps

A `tray:` entry adds a menu-bar item (macOS) / system-tray icon (Windows). The same declaration drives three shapes:

- **Dock + tray** — `role: main` with a `tray:` (default `exclusive: false`). Keeps the dock / taskbar icon and full window UI; the tray entry summons the window (`action: activate` brings it to front, `toggle` hides on re-click).
- **Tray only** — add `exclusive: true`. No dock / taskbar icon and no flash at launch (macOS sets `LSUIElement`; Windows uses `WS_EX_TOOLWINDOW`). The app lives only in the tray.
- **Tray popover** — `role: float` + a `tray:`. Clicking the tray icon opens the surface as an auto-dismissing popover anchored under the icon. Set its size with `tray.size: { width, height }` (default 360×420). A pure popover app has no `main`.

```yaml
surfaces:
  - lxapp: my-panel
    role: float            # tray-anchored popover
    page: tray             # configured page name, not pages/tray/index
    query: { source: tray }
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

The built-in terminal is gated by `capabilities.terminal`. On macOS and Windows its default declaration may be a main surface or an aside (`edge: top | bottom`, default `bottom`). Omitting `as` uses that declared role and edge; an explicit `as` migrates a non-root live workspace without changing the declaration.

When terminal is declared as `main`, its declaration is the default workspace. The sidebar's global `+` creates another terminal workspace as a separate main Surface; the `+` inside a terminal workspace creates another PTY tab in that workspace. Logic can open or reuse a named workspace with `lx.shell.openDeclared('terminal', { key: 'project-a', as: 'main' })`. Equal keys resolve to the same runtime Surface, distinct keys create distinct entries, and the returned handle's read-only `id` is the runtime `SurfaceId` — it is not the key. `as` controls where the same workspace is presented, independently from `key`.

`native: browser` is a macOS or Windows host-owned browser workspace. It starts with an empty tab and uses the managed browser profile and chrome; use a `url:` main when the declaration should open a specific `https://` or authorized `file://` target.

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

- Builds the configured control lxapp resource bundle when one is present.
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
- A present `homeAppId` not matching any `resources.bundles[].appId` — build
  fails or the wrong control app launches.
- Omitting `homeAppId` while targeting mobile, enabling AppService, or declaring
  a non-native launch main — native-only hosts are desktop terminal/browser
  products, not a way to bypass the control-app contract.
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

- [ ] `lingxia.yaml` validates: every required platform section is present; when
  `homeAppId` exists it resolves to a `resources.bundles[].appId`; when omitted,
  all targets and the launch main satisfy the native-only desktop contract.
- [ ] Exactly one declared `main` surface (or one `role: float` tray popover); it is the stable root, every `aside` has an `edge`, and terminal surfaces have `capabilities.terminal: true`.
- [ ] `features.appService` matches the embedded control lxapp's logic mode, or
  is false when no control lxapp is bundled.
- [ ] All native routes return `lingxia::Result<T>` with `Serialize` outputs.
- [ ] `HostAddon` registers every route and extension; FFI exports present for each target platform.
- [ ] `lingxia doctor` passes; `lingxia dev` boots on a real/simulated device.

## Out Of Scope / Not Implemented Yet

The surface model intentionally does not yet define:

- multiple `main` surfaces open as separate top-level windows simultaneously
- asides nested under other asides
- reusing one lxapp `appId` across multiple surfaces
- native (`native:`) surfaces other than the built-in `terminal` and `browser`
- terminal backend selection in config
