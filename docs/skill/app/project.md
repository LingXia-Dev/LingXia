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
  Permissions are not YAML settings: network hosts and privilege classes are
  [host grants](../native/permissions.md) (default allow until a provider
  constrains them).
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

### Android URL player engine (optional)

Android hosts may replace the default ExoPlayer URL backend used by
`lx.previewMedia` (and, opt-in, `<LxVideo>`) with their own engine — typically
libmpv shipped in the **host** APK. The SDK does not vendor mpv.

Register a process-level factory on the main thread, before any `LxMediaPlayer`
is constructed — immediately before `quickStart`, or in its trailing lambda.
Load native `.so` files in `Application.onCreate` or before the setter; never on
the same line as it.

```kotlin
Lingxia.setUrlPlayerEngineFactory(object : UrlPlayerEngineFactory {
    override fun preferredOutput(kind: UrlPlayerSurfaceKind) =
        UrlPlayerOutputKind.TEXTURE_VIEW // PREVIEW may opt into SURFACE_VIEW on API 24+

    override fun create(request: UrlPlayerEngineRequest): UrlPlayerEngine? {
        if (request.surfaceKind != UrlPlayerSurfaceKind.PREVIEW) return null
        return MpvPlayerEngine(request)
    }
})
Lingxia.quickStart(this) { registerHostAddon() }
```

- `create() == null` means "use the SDK ExoPlayer for this surface". Keep the
  recommended PREVIEW-only default; INLINE (`<LxVideo>` / MediaSwiper) stays on
  ExoPlayer unless the factory answers for `INLINE` too, and INLINE output is
  always `TextureView`.
- Each `LxMediaPlayer` snapshots the factory at construction, so a later setter
  call does not hot-swap live players.
- Emit `FirstFrameRendered` only once a frame reached the output surface
  (`file-loaded` is not enough). The adapter drops loop-on `Ended` so preview
  does not treat a loop point as terminal. v1 passes no HTTP headers to the
  engine.

---

## Minimal macOS Example

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
| `settingsDestination` | Optional | Static target for the host-owned Settings entry |
| `resources` | Conditional | Bundle asset sources; omit when no control/product lxapp is bundled |
| `splash` | Optional | Generated launch placeholder and first-frame cover |
| `assets` | Optional | Raw host files packaged through each platform's asset pipeline |
| `browser` | Optional | Browser chrome preferences and webui override (used when `capabilities.browser: true`) |
| `appLinks` | Optional | Universal-link / app-link hosts (see [App Links](./applinks.md)) |
| `storage` | Recommended | Explicit host temp/cache/data size limits |
| `update` | Optional | In-app update keys (1–2 `trustedPublicKeys`) and per-platform channel. See [`update`](#update). |

---

## `app` Section

`app` is the host identity written into `app.json`:

- `projectName` — technical id (paths, crate, artifacts)
- `packageId` — required OS id for every platform. **Breaking:** a platform-only id is no longer enough; copy the reverse-DNS onto `app.packageId` yourself. Keep `android.packageId` / `ios.bundleId` / `macos.bundleId` / `windows.appId` / `harmony.bundleName` only when that store listing differs. Env suffix still applies.
- `productName` — default display name (required). Fallback for any locale not in `productNames`.
- `productNames` — optional locale → name map (`zh-CN: 我的应用`). Do not repeat `productName`. Launcher follows the **system** language; window title / tray / `lx.getAppBaseInfo().productName` follow `lx.host.displayLanguage` (`auto` = system).
- `productVersion` — semver, stamped into every OS package
- `platforms` — enabled set (`macos`, `windows`, `ios`, `android`, `harmony`)

Optional `lingxiaId` / `lingxiaServer`: [Environment](#environment).

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

## `settingsDestination` Section

`settingsDestination` declares where the product's Settings entry leads. It is a
static, data-only descriptor: the CLI copies it into the generated `app.json`
unchanged (edit `lingxia.yaml`, never that file). Unconfigured, the key is
omitted entirely, the macOS and Windows shells show no Settings entry, and the
native resolver returns `SettingsDestinationResolveError::NotConfigured` rather
than falling back to the home runtime or a focus-only route.

The three `kind`s are mutually exclusive — pick one:

- `controlAppPage` — a page of the control lxapp; `appId` and `page` required, both non-empty.
- `browserControlPage` — a route of the browser control UI; `route` required, non-empty.
- `nativeAction` — a native action the host registered; `actionId` required, non-empty.

```yaml
settingsDestination:
  kind: controlAppPage
  appId: com.example.control
  page: settings
  query:
    tab: general
    highlight: true
```

The first two may carry `query`. Every key must be non-empty, and every value
must be a JSON scalar (string, number, boolean) or `null` — arrays and objects
are rejected during config validation. The schema is a strict tagged union, so a
misspelled field, or a field belonging to another `kind`, is an error too.

Browser and pure-native products look like this:

```yaml
settingsDestination:
  kind: browserControlPage
  route: /settings/privacy

# or
settingsDestination:
  kind: nativeAction
  actionId: openPreferences
```

---

## `theme` Section

`theme` defines application-wide semantic colors for host-owned native UI and
the scheme the product starts in. It is host configuration, not an lxapp
content theme. Every key and role is optional:

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

Values use opaque `#RRGGBB` sRGB syntax. The build rejects alpha colors,
unknown roles, and unknown scheme names. Missing roles retain the platform's
semantic default for that scheme; values never fall back from light to dark or
from dark to light. On macOS those defaults are dynamic AppKit semantic colors;
on Windows they are Fluent theme tokens, with system colors taking precedence
in a contrast theme.

`defaultAppearance` applies until the user picks a scheme through
`lx.host.control?.appearance`; the saved choice, `auto` included, wins after
that. An lxapp's own `appearance` in `lxapp.json` overrides both. The launch
screen follows the system.

The Windows and macOS desktop shells consume `windowBackgroundColor` for the
window backdrop and sidebar, and `surfaceBackgroundColor` for native cards and
other raised surfaces. Text, selection, accent, and structural dividers consume
the correspondingly named roles. Other hosts can map the same semantic roles
without adding platform-specific configuration.

`pageBackgroundColor` is the odd one out: it tells native chrome what colour the
lxapp's page will be, because no platform can ask a WebView for its document
colour in time to paint a frame already on screen. Chrome uses it wherever it
borders the page (the strip a pull-to-refresh opens, the canvas behind
navigation transitions). Set it to the page floor your CSS uses, in both
schemes; unset, the system background reads as a pale seam on a tinted page.

Lxapp page content does not inherit these colors; it responds to the standard
`prefers-color-scheme` surface and owns its CSS design.

---

## `update`

In-app host updates. Omit the table to skip prod `checkUpdate` (`dev` still checks, unsigned). If present, it must list 1–2 `trustedPublicKeys` (two = key rotation) — the signed feed is the version signal on every channel, including `store`. How to mint the seed/public pair: [Distribution](../cli/distribution.md#lingxia-publish).

`channel` / `platforms` choose who installs the package. This is **not** the `ios.store` / `android.googlePlayStore` listing identity used by `lingxia store`. A freshly scaffolded `lingxia.yaml` already carries this block, commented out — uncomment it and add your key.

```yaml
update:
  trustedPublicKeys: [ ... ]
  channel: direct          # default for platforms not listed
  platforms:
    ios: store
    harmony: store
    android: direct        # next Play-signed build: store
    macos: direct          # MAS flavor: store
    windows: direct
```

`direct` — download the LingXia feed and self-install. `store` — never self-install. The feed is still the version signal: `lx.host.checkUpdate()` returns `hasUpdate: true` when a newer host version exists, the built-in flow offers a prompt, and `apply()` (or confirming that prompt) opens the store listing. `lx.supports('app.selfUpdate')` is false. The store is only ever opened by a user action; nothing is opened automatically. The prompt repeats at most once every 3 days per version.

**Publish the feed package for a `store` platform only after the store listing is live.** The feed is what tells users a new version exists — if it lands while the listing is still in review, everyone is sent to a page with nothing to update.

Reuse the existing store identity — no extra yaml, no country URL. The ids are
baked into `app.json` as `storeListingIds`, and a `store` platform that needs one
but has none warns at build time.

- **Apple** — `ios.store.appId` / `macos.store.appId` (numeric Apple ID) open
  `itms-apps://apps.apple.com/app/id…`, with the HTTPS listing as fallback; the
  storefront follows the signed-in Apple ID.
- **Android** — the listing opens in the store that installed the APK (Play /
  Huawei / Honor / Xiaomi / OPPO / vivo / Samsung / Amazon / Yingyongbao):
  `market://details?id=` addressed to that store's package, then that store's
  own scheme, then an HTTPS page. The `android.*Store` blocks are publish
  identity, not a second listing URL.
- **Harmony** — `appmarket://details?id=` with the bundle name, falling back to
  the AppGallery HTTPS page from `harmony.store.appId`.
- **Windows** — `windows.store.appId`.

Do not put a store URL in `appLinks.hosts`: those hosts open *this* app, so a tap
would bounce back into the old binary instead of the marketplace.

Omitted values keep today's defaults: iOS and HarmonyOS are `store`; Android, macOS, and Windows are `direct`.

The value is baked into **this build**. Changing yaml and shipping a new package does not rewrite already-installed binaries. A process actually installed by a store (Play / App Store / MAS receipt / Microsoft Store-signed package) always behaves as `store`, even when this build still says `direct` — so a sideloaded Android APK that the user later updates from Play flips channel in place. Same `applicationId` and a matching signing key; uninstall is only required when the store package cannot overlay the sideload signature. Sideloaded or developer-signed MSIX stays on this build's channel.

---

## Environment

A host build is `dev` or `prod`, selected via `lingxia {build,dev,package} --env <env>`. Default: `dev` for `build`/`dev`, `prod` for `package`. This is **not** the lxapp channel (`release` | `draft`) and not the `--release` compiler profile. `developer` / `preview` are not env names; `envVersion` is not a field.

**What each env produces:**

| Env | Package id suffix | Launcher icon | Default `lingxia dev/build` | Default `lingxia package` |
|---|---|---|---|---|
| `dev` | `.dev` | red `D` badge | ✓ | |
| `prod` | `(none)` | unmodified | | ✓ |

There is no host `preview` env. Testers publish the **draft** channel (same-version overwrite via checksum) or use a `dev` env for a staging server.

Different envs install **side by side** because their package ids differ. No git-tracked file changes when you switch envs.

Default lxapp channel is derived from env (`dev` → `draft`, `prod` → `release`) and can be overridden when opening an app with `channel`. Prod hosts are allowed to open the `draft` channel; the registry decides.

### Per-env `lingxiaServer`

```yaml
app:
  lingxiaServer: https://api.myapp.com
# lingxiaServer:
#   dev: http://192.168.1.10:8080
#   prod: https://api.myapp.com
```

### Per-env `appLinks.hosts`

Same list-or-map shape. `lingxia build --env` writes that env's hosts into `app.json` and platform association files.

```yaml
appLinks:
  hosts: [app.example.com]
# hosts:
#   dev: [app-dev.example.com]
#   prod: [app.example.com]
```

Omit an env → no App Links for that build. See [App Links](./applinks.md).

Empty `lingxiaServer` is rejected. Per-env maps must set at least one of `dev` or `prod`. Unknown keys are YAML parse errors.

### Reading the env at runtime

JS: `lx.host.env` — `'dev' | 'prod'`, fixed at boot.

Rust: `lingxia::app::env()` returns `AppEnv`.

The build-time plumbing per platform is internal — app authors don't touch it.

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

- `notifications` — unlocks [`lx.host.notification`](../lxapp/lx-api.md#local-notifications) ([Control app](./control-app.md) only) and, on iOS/Harmony, push-token registration. Declaring it never prompts; the product asks.
- `browser` — the in-app browser (its newtab / settings / downloads pages and shell runtime). Cross-platform; bundles the browser webui, overridable via the [`browser`](#browser-section) section.
- `terminal` — the built-in terminal runtime. Required before a `native: terminal` surface can be declared (desktop only).
- `proxy` — the in-app browser's HTTP proxy (desktop). Requires `browser`.
- `process` — OS process launch/management for trusted Agent-style products (macOS/Windows). Available only to the [Control app](./control-app.md) — the session created from `app.homeAppId` — and still needs a host privilege grant plus a native Process grant; adds `Rong.spawn`, `Rong.spawnSync`, and `Rong.$` plus the opt-in `@lingxia/types/process` declarations.
- `autostart` — unlocks `lx.host.autostart` (launch at system startup; macOS/Windows, [Control app](./control-app.md) only). Declaring it never registers the app by itself — enabling is a runtime user decision via the API.
- `appUse` — lets a command line or agent skill on the same machine drive this product's own windows (screenshot, window list, mouse, keyboard), and turns the product's executable into its own command line. macOS/Windows. The local socket this needs is derived, not declared — which IPC carries it is plumbing. Declaring it ships the ability, not the decision: the endpoint stays closed until the user turns it on, the same way `autostart` works.
- `computerUse` — extends that to the machine, and implies `appUse` because it already contains it (an agent that can drive any window can drive this product's): screenshots of any window, synthetic input, the accessibility tree. Named for what the user grants, because they will be asked — macOS prompts for Accessibility and Screen Recording, and the entry in System Settings is this product. Commands run inside the app rather than in the calling process, so that grant stays attached to the product no matter which terminal invoked it.
- `browserUse` — extends it to the in-app browser. Requires `browser`.
- `mediaCapture` — realtime visual / system-audio / microphone capture for a product session. Independent of `computerUse`. Declare only the tracks this product needs; a host that omits the key constructs no provider and receives no capture-specific services, permissions, or entitlements. Snapshot (`lxdev desktop screenshot`, `computerUse` screenshots) stays visual-only and does not enable this.

---

## `browser` Section

`browser.bookmarks` defaults to `true`. Set it to `false` to hide bookmark chrome; pin context menus offer “Manage Pinned Sites” at `lingxia://bookmarks`. That route, the store, and full `bookmarks.list/watch` results remain available so the frontend can migrate legacy bookmarks.

Trusted browser webui calls `shell.pins` for `{items, max}` — the ordered `{kind: "lxapp" | "bookmark", key}` list, and how many Pins the sidebar holds in total — then `shell.reorderPins({items})` with every current item exactly once, which returns the same shape. The budget covers pinned lxapps as well, so read `max` instead of counting your own rows. Pinning past it rejects with code `SHELL_PIN_LIMIT` and `data.max`, so a caller writes that message in its own language rather than matching the host's. `bookmarks.reorder` only orders the bookmark manager. Control apps may also call these shell routes. Automation callers use `lx.automation().shell.reorderPins({items})` with the complete list from `lx.automation().shell.pins()`.

`tabs.recentlyClosed` lists up to 25 normal website tabs, newest first, for this process. `tabs.reopen({id?})` restores one (latest if omitted); private tabs and aside/standalone surfaces are excluded. Desktop shortcuts are ⌘⇧T / Ctrl+Shift+T.

`browser` overrides the in-app browser webui, used only when `capabilities.browser: true`. Normal apps omit it and use the SDK default. Set exactly one source under `webui`: a project-relative `path:` to a browser-shell webui lxapp source tree (the CLI builds it — for developing a custom webui alongside the app), or a `package:` npm name shipping a prebuilt `lxapp.json` + `dist/` (with an optional `version:`; the CLI version is used when omitted). Setting both is rejected. Protocol version is declared on the webui `lxapp.json` (`controlProtocolVersion` equal to the current runtime constant — wire `v` for BrowserControl documents), not in `lingxia.yaml`. Missing, older, and unknown future values fail the build. The SDK's built-in catalog is native code already on that protocol, so a host that does not replace the webui has nothing to pin.

```yaml
browser:
  webui:
    path: vendor/browser-shell-webui
```

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

Optional launch screen, generated per platform from a few fields — no
hand-built launch UI. `image` (PNG, full-screen aspect-fill) is the app's first
frame on every cold start, held until the home page first renders, then fading
into content. Where the platform allows it, the OS launch frame carries the same
art (HarmonyOS's start window, a generated iOS storyboard), so the two frames
are one picture.

- **Android** is the exception: its 12+ system splash offers only a colour and
  an icon slot, so `background` (required, `#RRGGBB`) is the OS beat and the art
  arrives on the app's first frame. Use the art's own ground colour. Configured
  art blanks the icon slot; without art the platform draws the app icon and
  keeps the launcher's zoom morph. HarmonyOS follows the same rule.
- **One face, every appearance.** There is no dark counterpart. The OS composes
  this frame from build-time resources, so an appearance pair could only follow
  the *system*, never the product's own light/dark choice — and then disagree
  with it at launch.
- `mark` (PNG, authored at the pixels it occupies on screen) is what the OS
  frame centers when no `image` is configured.
- `minDuration` (ms, default 600) is measured from process start, not from first
  paint. The maximum is a framework constant.
- **iOS** needs the platform installed (`xcodebuild -downloadPlatform iOS`) to
  compile its generated storyboard. Without it a dev build degrades to
  `background` alone; a release build fails outright.

A host's Rust addon can show a screen of its own *after* the launch face — its
own art, a countdown, skippable: see [Launch Screen](../native/splash.md).

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

`macos` sets the deployment target and the SwiftPM `targetName` / `executableName`. `bundleId` is an optional override of `app.packageId`. Optional `store:` is the App Store Connect identity for `lingxia store`. The scaffold writes a starting `macos:`; read it for the keys.

## `windows` Section

`windows` is the desktop host for Windows, on the same adaptive `surfaces:` model as macOS (no per-platform UI block). You don't hand-wire the Windows SDK: scaffold with `lingxia new -t native-app -p windows` (combine with other platforms, e.g. `-p macos,windows`) and the generated project drops in the `windows/` Rust host crate and its packaging wired to crates.io `lingxia-windows-sdk` — read the generated project rather than pasting dependency tables here.

`windows:` packaging: `executableName` (`windows/Cargo.toml` binary), `publisher` (MSIX `Publisher`, default `CN=<productName>`), optional `appId` override of `app.packageId`, optional `store:` Partner Center id for `lingxia store`.

`windows.extraFiles: [path, ...]` copies project-relative files/directories beside
the executable (basename preserved; collisions fail). Sibling Cargo-output DLLs
are included automatically. `windows.portableData: true` stores portable data
under `<launcher-dir>/data/<appId>`; default `false` uses the normal per-user
state location. Portable always extracts program resources to a temporary
folder and cleans them after exit. User data must never live in that folder.

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

On desktop the main window is a sidebar plus a main area plus docked asides.

- An lxapp in `main` owns the primary content area and appears in the sidebar's
  main switcher. That area has no tab strip.
- An lxapp in `aside` occupies a companion region at the left, right, top, or
  bottom of the main and switches through that region's tab strip. It never
  appears in the main switcher.
- One lxapp holds one live role per window. Opening it under the other role
  moves or reopens that same logical app; the Host never projects it as both.

The shell picks the realized form from the window width:

| Width | What the shell does |
|---|---|
| Wide | full sidebar (pins, main tabs, activators), up to three docked asides |
| Medium | sidebar collapses to an icon rail; at most one aside slot, overlaying the content pane when it cannot preserve the main's minimum |
| Narrow desktop | icon rail stays and `main` keeps its desktop workspace; asides overlay it. Browser chrome keeps its top address toolbar, and no mobile bottom tab bar comes back |
| Mobile / phone Runner | no sidebar; `main` is full screen and asides overlay it full screen |

Asides group into per-engine slots (lxapp / browser / native), each with its own
tab strip: switching tabs hides and shows content, and only an explicit close
destroys it. An aside reprojected as an overlay stays inside the same host
window — never a second workspace window, never in the main switcher.

Browser asides adapt their chrome to the slot — a read-only URL at most on
desktop, a single bottom row (history, refresh, tab group, dismissal) on mobile
and phone Runner, and no user-created tabs anywhere. Only the self browser has
an editable URL field, and it takes URLs, not search queries.

Two sidebar regions have fixed ownership:

- **Pins are the user's** — up to eight shortcuts to lxapps and websites, added
  and removed through context menus. An lxapp Pin opens or focuses a main
  workspace in the same rectangle the home lxapp uses; it never inherits a
  declared aside role and never changes insets, clipping, or the content
  rectangle. For an aside entry, use a sidebar action plus
  `lx.surface.openDeclared(id)`. There is no production API to write Pins.
- **Sidebar actions are the control lxapp's** — when one is configured, it may
  declare runtime entries via `lx.shell.sidebarActions` (see the
  `@lingxia/types` declarations). Header actions are icon-only and limited to
  two; footer actions use labeled cells and scroll after five visible rows. The
  shell invokes `onActivate` and performs no built-in navigation; callbacks can
  call `lx.surface.openPage(...)` or run any other app logic. Redeclare them each
  Logic launch.

`icon` uses the same lxapp-local path model as a runtime tab-bar `iconPath`: a
bundled relative path, or an `lx://temp`, `lx://usercache`, or `lx://userdata`
path returned by a LingXia file API. Network URLs, `file:` URLs, native absolute
paths, and parent traversal are rejected. Download a remote brand logo before
declaring the action. SVG is host-tinted as a template glyph; raster
PNG/JPEG/WebP keeps its colours and is center-cropped into the square icon slot.
Use square raster artwork when cropping would remove meaningful content.

```ts
const { uri } = await lx.downloadFile({ url: activeBrand.logoUrl }).result;
lx.shell.sidebarActions.replace([
  {
    id: 'brand',
    placement: 'footer',
    icon: uri,
    label: activeBrand.shortName,
    onActivate: () => void openBrandPanel(),
  },
]);
```

The declaration and an `lx://temp` download are both process-local. Repeat the
download and `replace()` after every Logic launch; never persist a temp path for
the next launch. Use `update(id, { icon })` for presentation-only changes to an
existing action, or `replace()` when its placement or callback changes.

The initial `main` is the window's stable root and cannot be closed. Other main
surfaces expose only the actions their content provider supports: browser and
terminal surfaces may be closed or renamed, while a non-root lxapp workspace may
be closed or restarted through its provider-backed sidebar menu but not renamed.
Closing the active non-root main selects another, so the product Host never
enters a zero-main empty state.

When `homeAppId` is configured, that lxapp remains the trusted control app even
when the visible desktop main is a URL or native surface. Its Logic worker still
receives `App.onLaunch` once and may register sidebar actions and other host
chrome without creating a hidden WebView. A native-control scaffold has no such
worker or hidden lxapp identity.

### Menu-bar / system-tray apps

A `tray:` entry adds a menu-bar item (macOS) / system-tray icon (Windows). The same declaration drives three shapes:

- **Dock + tray** — `role: main` with a `tray:` (default `exclusive: false`). Keeps the dock / taskbar icon and full window UI; the tray entry summons the window (`action: activate` brings it to front, `toggle` hides on re-click).
- **Tray only** — add `exclusive: true`. No dock / taskbar icon and no flash at launch (macOS sets `LSUIElement`; Windows uses `WS_EX_TOOLWINDOW`). The app lives only in the tray. Host self-updates prompt from the tray (menu item + balloon on Windows, badge + menu on macOS). Dock + tray keeps the window callout.
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

- `lx.tray.setIcon(path)` / `lx.tray.setTitle(text)` / `lx.tray.setMenu(items)` / `lx.tray.onClick(fn)` / `lx.tray.show()` / `lx.tray.hide()` — the status item's own appearance and behaviour.
- `lx.host.setBadge(value, options?)` — the count, wherever this platform shows one: `surface: 'auto'` (the default) marks the dock *and* the menu-bar item on macOS, the taskbar *and* the notification-area item on Windows, the home-screen icon on iOS and HarmonyOS. There is no separate tray badge call. Call it directly; it resolves whether anything was painted and returns `false` on Android.

All of these are the product's own chrome, not the calling lxapp's, so they are Control app only; a guest lxapp gets a permission error.

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
- Editing Android `versionName` / Harmony `versionName` / Apple `CFBundleShortVersionString` in the platform project — those are scaffold placeholders; `app.productVersion` is written at build time.
- A present `homeAppId` not matching any `resources.bundles[].appId` — build
  fails or the wrong control app launches.
- Omitting `homeAppId` while targeting mobile, enabling AppService, or declaring
  a non-native launch main — native-only hosts are desktop terminal/browser
  products, not a way to bypass the control-app contract.
- Declaring more than one `main` with `launch: true`, or `launch: true` on an `aside`.
- An `aside` without an `edge`, or an `edge` on a `main`.
- `native:` on anything but `terminal` or `browser`, a built-in surface without its `capabilities` flag, or a terminal `edge` other than `top`/`bottom`.
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
