# Host project

`lingxia.yaml` configures a native host with an embedded control lxapp, or a
macOS/Windows terminal/browser with native control. UI is a flat, adaptive
[`surfaces:`](#surfaces) list.

## Create

```bash
lingxia new my-app -t native-app -p macos,windows --package-id com.example.myapp -y
lingxia new my-terminal -t native-app --main terminal --control native -y
lingxia new my-browser -t native-app -p windows --main browser --control native -y
```

The first form uses one lxapp for the main UI and host control.
`--main terminal|browser` defaults to native control: no `lxapp/`, `homeAppId`,
or `resources`; the matching capability is enabled and `appService: false`.
Use `--control lxapp` for host control logic in a hidden embedded lxapp.

The scaffold defines the layout:

- `lingxia.yaml` — the source of truth for metadata and UI. `lingxia build`
  generates `app.json` and `ui.json` from it; never edit those.
- `native/` — the Rust host library (`app.rustLibDir`).
- Root `Cargo.toml` owns the workspace, shared dependencies, profiles, patches,
  and lockfile. `native/` and `windows/` are members; Windows dependencies use
  `workspace = true`.
- one directory per platform: `macos/`, `windows/`, `android/`, `ios/`, `harmony/`.
- optionally an embedded control lxapp (scaffold default `lxapp/`).

Network hosts and privileges are [host grants](../native/permissions.md), not
yaml settings.

## Startup entry

| Platform | Entry |
|---|---|
| Apple | `Lingxia.quickStart()` ([Apple SDK](./apple-sdk.md)) |
| Android | `Lingxia.quickStart(activity)` ([Android SDK](./android-sdk.md)) |
| Harmony | `Lingxia.quickStart(context, windowStage)` |

`quickStart` starts the runtime and opens the launch surface.

## Minimal macOS example

```yaml
app:
  projectName: myapp
  packageId: com.example.myapp
  productName: My App
  productVersion: 1.0.0
  platforms:
    - macos
  homeAppId: my-home

macos:
  deploymentTarget: "12.0"
  targetName: MyApp

surfaces:
  - lxapp: my-home       # main screen: your lxapp, by appId
    role: main
    launch: true
```

## Sections

| Section | Required | Purpose |
|---|---:|---|
| [`app`](#app) | Yes | Host identity written into `app.json` |
| [`macos` / `windows`](#macos-and-windows) | Per platform | Desktop bundle, packaging, store identity |
| `android` / `ios` / `harmony` | Per platform | Mobile platform settings |
| [`surfaces`](#surfaces) | For product hosts | Adaptive UI (generates `ui.json`) |
| [`features`](#features) | Recommended | Native Rust compile-time switches |
| [`capabilities`](#capabilities) | Recommended | Predeclared platform/runtime integrations |
| [`theme`](#theme) | Optional | Semantic colors for host-owned native UI |
| [`settingsDestination`](#settingsdestination) | Optional | Target of the host Settings entry |
| [`resources`](#resources) | Conditional | Bundled lxapp sources |
| [`splash`](#splash) | Optional | Launch screen |
| [`assets`](#assets) | Optional | Raw host files |
| [`browser`](#browser) | Optional | In-app browser preferences and webui |
| `appLinks` | Optional | [App link hosts](./applinks.md#applinkshosts) |
| [`storage`](#storage) | Recommended | Temp/cache/data size limits |
| [`update`](#update) | Optional | In-app update keys and channel |

## `app`

- `projectName` — technical id (paths, crate, artifacts).
- `packageId` — OS id for every platform (required). Set `android.packageId` /
  `ios.bundleId` / `macos.bundleId` / `windows.appId` / `harmony.bundleName`
  only when that store listing differs. The env suffix still applies.
- `productName` — display name (required); fallback for locales missing from
  `productNames`.
- `productNames` — optional locale → name map (`zh-CN: 我的应用`). The launcher
  follows the system language; window title, tray, and
  `lx.getAppBaseInfo().productName` follow `lx.host.displayLanguage`.
- `productVersion` — semver, stamped into every OS package. Do not edit
  version fields in platform projects.
- `platforms` — `macos`, `windows`, `ios`, `android`, `harmony`.
- `lingxiaServer`, `lingxiaId` — see [Environment](#environment).
- `homeAppVersion` is not configured; the CLI derives it from the bundle.

With a control lxapp, `app.homeAppId` = a
`resources.bundles[].appId` = that bundle's `lxapp.json.appId`. A launch `main`
lxapp uses the same id; the build rejects mismatches.

`homeAppId` may be omitted only by a macOS/Windows native-main host with
`features.appService: false`.

## `settingsDestination`

Desktop Settings target; omitted means no Settings entry. Pick one `kind`:

- `controlAppPage` — `appId` and `page` of the control lxapp.
- `browserControlPage` — `route` of the browser control UI.
- `nativeAction` — an `actionId` the host registered.

```yaml
settingsDestination:
  kind: controlAppPage
  appId: com.example.control
  page: settings
  query: { tab: general }   # first two kinds only; scalar or null values
```

## `theme`

Host-owned native UI colors and initial appearance; all keys are optional.

```yaml
theme:
  defaultAppearance: dark   # auto (default) | light | dark
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

- Opaque `#RRGGBB` only. A missing role keeps the platform default for that
  scheme; light and dark never fall back to each other.
- Set `pageBackgroundColor` to your page's CSS floor in both schemes; native
  chrome paints it where it borders the page.
- The native refresh indicator uses `accentColor` (else `mutedForegroundColor`).
- `defaultAppearance` applies until the user picks a scheme
  ([product settings](../lxapp/lx-api.md#product-settings)); an lxapp's own
  `appearance` overrides both. The embedded browser's chrome and built-in
  pages follow the product scheme too.
- Lxapp pages do not inherit these colors; they follow `prefers-color-scheme`.

## `update`

```yaml
update:
  trustedPublicKeys: [ ... ]   # 1–2 keys (two = rotation); direct prod updates only
  channel: direct              # default for platforms not listed
  platforms:
    ios: store
    harmony: store
    android: direct
    macos: direct
    windows: direct
```

- `direct` downloads the LingXia feed and self-installs. `store` never
  self-installs: `lx.host.checkUpdate()` reports the version, the built-in
  prompt (at most once per 3 days per version) or `apply()` opens the store
  listing, and `lx.supports('app.selfUpdate')` is false.
- Store updates need no keys and reuse the existing `ios.store` / `macos.store`
  / `harmony.store` / `windows.store` identity. Publish the feed entry only
  after the listing is live.
- Defaults: iOS and Harmony `store`; Android, macOS, Windows `direct`. The value
  is baked into the build; a store-installed process is always `store`.
- Minting keys and publishing: [Distribution](../cli/distribution.md#update-signing-keys).

## Environment

`--env dev|prod` selects the host build environment: `build`/`dev` default to
`dev`, `package` to `prod`. Lxapp channels (`release|draft`) and the `--release`
compiler profile are independent. There is no `preview` env; use `draft` or
a dev build for testing.

| Env | Package id suffix | Launcher icon |
|---|---|---|
| `dev` | `.dev` | red `D` badge |
| `prod` | none | unmodified |

Envs install side by side. The default lxapp channel is always `release`,
independent of the running service env; an omitted selector keeps the channel a
running lxapp already has. An explicit `channel: 'draft'` works
on any host. Lxapp data is kept per channel; switching service env alone does
not select another channel's data.

```yaml
app:
  lingxiaServer: https://api.myapp.com
# lingxiaServer:
#   dev: http://192.168.1.10:8080
#   prod: https://api.myapp.com
```

A map may omit an env, which then cannot be built. A prod build can switch
service servers at runtime; package id, icon, and signed App Link entitlements
stay with the build env. Read the build env with `lx.host.env` (JS) or
`lingxia::app::env()` (Rust), and the running service env with
`lx.host.getServiceEnv()` (JS) or `lingxia::app::service_env()` (Rust).

## `features`

- `appService` (default `true`) — JS Logic runtime. Set it to `false` for
  native control; any control lxapp must also set `"logic": false` or startup
  rejects it. JS-Logic lxapps need it `true`.
- `devtools` (default `false`) — devtools hooks; `lingxia dev` enables it
  transiently.

Browser, terminal, and proxy runtime features come from `capabilities`.

## `capabilities`

Build-time integrations (all default off). APIs such as camera request
permission when called and need no entry here.

- `notifications` — [`lx.host.notification`](../lxapp/lx-api.md#local-notifications)
  and, on iOS/Harmony, push tokens. Declaring never prompts.
- `browser` — the in-app browser; see [`browser`](#browser).
- `terminal` — the built-in terminal (desktop); required by `native: terminal`.
- `proxy` — the in-app browser's HTTP proxy (desktop); requires `browser`.
- `process` — `Rong.spawn`, `Rong.spawnSync`, `Rong.$` and the
  `@lingxia/types/process` declarations for the [Control app](./control-app.md)
  (macOS/Windows), with a host grant.
- `autostart` — `lx.host.autostart` (macOS/Windows, Control app). Declaring
  never registers the app.
- `appUse`, `computerUse`, `browserUse` — agent control; see
  [Driving a shipped product](./agent-control.md).
- `mediaCapture` — realtime visual / system-audio / microphone capture;
  declare only the tracks you need.

## `browser`

Requires `capabilities.browser: true`. `bookmarks` (default `true`) shows
bookmark chrome. Optional `webui` replaces the browser UI with one source:
project-relative `path:` (CLI-built) or npm `package:` with prebuilt
`lxapp.json` + `dist/` (optional `version:`).
`platforms` limits the browser runtime to the listed targets; omit it or use an
empty list for all targets. A mobile-only browser uses `[ios, android, harmony]`.

```yaml
browser:
  platforms: [ios, android, harmony]
  webui:
    path: vendor/browser-shell-webui
```

`homeAppId` is the product control app, never the browser UI.

## `resources`

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

- `appId` must equal the bundle's `lxapp.json.appId` and be unique.
- Use `path:` or `package:` (optional `version:`), never both. With neither,
  `type` and `appId` declare an id supplied by the update provider.
- Bundles do not decide what opens; `homeAppId` and `surfaces` do.

## `storage`

MiB caps: `tempMaxSizeMB`, `cacheMaxSizeMB` (per-lxapp usercache; `0` disables
enforcement), `dataMaxSizeMB`, `appStorageMaxSizeMB`. Eviction behaviour:
[Cleanup and quotas](../lxapp/files.md#cleanup-and-quotas).

## `splash`

A generated launch screen, held until the home page renders, then faded out.

- `image` — PNG, full-screen aspect-fill; also the OS launch frame where the
  platform allows.
- `background` — `#RRGGBB`, **required on Android** (and Harmony): the OS
  splash shows only this colour, so use the art's ground colour.
- `mark` — PNG centred by the OS frame when there is no `image`.
- `minDuration` — ms from process start (default 600).
- One face for every appearance; there is no dark variant.
- iOS needs the platform installed (`xcodebuild -downloadPlatform iOS`); without
  it a dev build shows `background` only and a release build fails.

A campaign screen after launch: [Launch screen](../native/splash.md).

## `assets`

`assets: <dir>` bundles host files, read in Rust with
`lingxia::assets::read("relative/path")`. Lxapps go in `resources`.

## `macos` and `windows`

- `macos` — `deploymentTarget`, `targetName`, optional `executableName`, optional
  `bundleId`, optional `store:` (App Store Connect identity).
- `windows` — optional `executableName`, `publisher` (MSIX, default `CN=<productName>`),
  optional `appId`, optional `store:` (Partner Center id).
  `extraFiles: [path, ...]` copies files beside the executable.
  `portableData: true` keeps data under `<launcher-dir>/data/<appId>`.

`executableName` selects a build target, never a display name. Omit it for a
single executable; Windows also respects Cargo `default-run`. Ambiguous builds
require an explicit selection. Product launcher names follow the system language.

For Microsoft Store, use Partner Center's Package/Identity/Name as the package
id (inherit `app.packageId` or override `windows.appId`) and
Package/Identity/Publisher as `windows.publisher` (a distinguished name,
also matching the signing certificate when signed). `windows.store.appId` is
the separate Store product ID. Auth/upload: [Distribution](../cli/distribution.md#microsoft-store).

## Surfaces

The host maps surfaces to windows/panels/sidebars/tabs/trays by screen size.
Use this list for all platforms; there are no per-platform UI blocks or
`sidebar:` field.

### Fields

Each entry starts with exactly one content key, which is also its identity:

| Field | Type | Description |
|---|---|---|
| `lxapp` | string | An lxapp by appId. Roles `main` \| `aside` \| `float`. |
| `url` | string | A managed-browser page (needs `capabilities.browser`). `main` on macOS/Windows; `aside` on Windows. |
| `native` | string | `terminal` (main or aside) or `browser` (main), macOS/Windows. |
| `role` | `main` \| `aside` \| `float` | Required. `float` = tray popover (needs `tray:`). |
| `launch` | bool | Open on start; at most one `main`. Omit for a tray-launched app. |
| `page` | string | Page name from `lxapp.json`, never a path. Omitted → initial page. |
| `query` | object | Page parameters: string, number, boolean, or null. |
| `edge` | `left`\|`right`\|`top`\|`bottom` | Aside side; default `right`, terminal `bottom` (`top`/`bottom` only). |
| `size` | object | Aside size hint, e.g. `{ width: 320 }`. |
| `tray` | object | `{ icon?, label?, action?, exclusive?, size? }`; `action` is `toggle` or `activate`. |
| `platforms` | string[] | Availability filter. Empty = all. |

### Rules

- Exactly one declared `main` (or one `role: float` tray popover and no main).
  On other targets than macOS/Windows it must be the home lxapp.
- At most one `main` sets `launch: true`; `launch` is invalid elsewhere.
- `edge` and `size` only on `aside`; `page` and `query` only with `lxapp`.
- `url` and `native: browser` need `capabilities.browser`; `native: terminal`
  needs `capabilities.terminal`.
- A content key repeats only across mutually exclusive `platforms`.
- At most one `tray:` per target platform.
- Each `lxapp` surface's app is in `resources.bundles` or provided at runtime.
- The launch `main` is the window's root and cannot be closed.

### Example

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
  - native: terminal     # built-in terminal (needs capabilities.terminal)
    role: aside
    edge: bottom
    platforms: [macos, windows]
```

### Sidebar actions

Declare sidebar entries on every control lxapp Logic launch. `App.onLaunch`
runs once even with a URL/native main, without a hidden WebView. Icons follow
tab-bar [`iconPath`](../lxapp/guide.md#icons); download remote art first.

```ts
const { uri } = await lx.downloadFile({ url: activeBrand.logoUrl }).result;
lx.shell.sidebarActions.replace([
  {
    id: 'brand',
    placement: 'footer',        // header: icon-only, max two
    icon: uri,
    label: activeBrand.shortName,
    onActivate: () => void openBrandPanel(),
  },
]);
```

Use `update(id, { icon })` for presentation changes. Pins are the user's; there
is no API to write them. Open a declared aside with
`lx.surface.openDeclared(id)`.

### Tray apps

A `tray:` entry adds a menu-bar (macOS) or system-tray (Windows) item:

- **Dock + tray** — `role: main` with `tray:`; the tray summons the window.
- **Tray only** — add `exclusive: true`: no dock/taskbar icon.
- **Tray popover** — `role: float` with `tray:`; `tray.size` sets the popover
  (default 360×420). No `main`.

```yaml
surfaces:
  - lxapp: my-panel
    role: float            # tray-anchored popover
    page: tray             # configured page name
    query: { source: tray }
    tray:
      icon: icons/tray.svg
      exclusive: true       # no dock / taskbar icon
      size: { width: 320, height: 480 }
```

The runtime tray content (`lx.tray.*`) and badge (`lx.host.setBadge`) are
Control-app APIs: [Badges](../lxapp/lx-api.md#badges).

### Terminal and browser mains

A terminal main is the default workspace; open or reuse a named one with
`lx.shell.openDeclared('terminal', { key: 'project-a', as: 'main' })`. Equal
keys reuse one workspace; the handle's `id` is not the key. `native: browser`
opens an empty tab in the managed browser; use a `url:` main for a fixed target.

## Icon paths

`tray.icon`: project-relative SVG source, no absolute paths or `..`, ≤ 512 KB,
square viewport from 16×16 to 512×512 px. The build converts it into platform
resources; use source files, not generated lxapp runtime assets.

## Pitfalls

- Adding Settings or Downloads as surfaces — they are built-in browser pages.
- Expecting hidden surfaces to destroy WebViews — hiding keeps state.
- `lingxia build --skip-native` links an existing Rust library, so capability
  changes stay stale; build normally.
- An older `lingxia` on `PATH` after a config schema change.
