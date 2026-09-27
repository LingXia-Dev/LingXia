---
name: lingxia
description: Build apps on the LingXia cross-platform framework — standalone lxapps (page-based mini-apps with a View+Logic split), native host apps (Android/iOS/macOS/Harmony/Windows shells embedding an lxapp), and Rust native extensions. TRIGGER on `lxapp.json`, `lingxia.yaml`, the `lingxia` / `lxdev` CLIs, `#[lingxia::native]`, `HostAddon`, `useLxPage`, or the user naming LingXia. Not on `Page({})` alone — other runtimes share it. **Always read §"Step 0" before generating any file.**
license: MIT
allowed-tools: Read, Grep, Glob, Edit, Write, Bash(lingxia:*), Bash(lxdev:*), Bash(npm:*), Bash(npx:*), Bash(test:*), Bash(ls:*), Bash(cat:*), Bash(cargo:*)
---

# LingXia

This file routes you to the one doc you need. Read sub-files only when needed.
Assume `lingxia` is on `PATH` (`lingxia version`), but confirm the project
shape with Step 0.

API signatures are never copied into these docs: `@lingxia/types` (and each
package's own declarations) and every CLI's `--help` are the source of truth.

## Step 0 — Decide before scaffolding

### 0a. What are you inside?

```bash
test -f lingxia.yaml   && echo "host-app"
test -f lxapp.json     && echo "lxapp"
```

Neither matches: an empty directory, or someone else's project. Scaffold only
into an empty directory or one the user asked you to initialise; otherwise say
so and stop. Scaffolding writes pages, config, and dependencies.

### 0b. Pick the shape

1. Standalone lxapp or host app? (A vs B/C)
2. Host app: which platforms (`android`, `ios`, `macos`, `windows`, `harmony`)?
3. Host app: lxapp or native terminal/browser as the main experience; lxapp or
   Rust as host control? (B vs C)
4. View framework: React, Vue, or HTML — one per project.

### 0c. Scaffold and read the result

```bash
lingxia new hello -t lxapp -y                                   # Shape A — standalone lxapp
lingxia new hello -t native-app -p macos --package-id com.example.hello -y   # Shape B/C — host app
```

The generated tree is the authoritative layout for your CLI version. In
`pages/home/`, `index.ts` is Logic, `index.tsx` (or `.vue` / `.html`) is the
View, `index.json` is page config. More forms: [`lingxia new`](./cli/lingxia.md#lingxia-new).
`lingxia doctor` checks toolchains.

## The development loop

`lingxia dev` starts a session (build → install → launch → dev websocket);
`lxdev` drives it.

| You changed | Do |
|---|---|
| lxapp code (View, Logic, `lxapp.json`), standalone or embedded | Save. The session rebuilds and reloads in place. |
| host code (`lingxia.yaml`, native Rust, platform projects) | Re-run `lingxia dev`; it takes over the previous session. |

```bash
lingxia dev --background     # start (or take over) this project's session; returns when live
lingxia dev stop
```

**A successful build is not done.** Done means you watched the change work:

1. Navigate to it (`lxdev lxapp nav to ...`) and use it
   (`lxdev lxapp page click/type ...`). A new control gets clicked.
2. Assert the effect: DOM via `lxdev lxapp page eval`, Logic state via
   `lxdev lxapp eval`. Screenshot only visual changes.
3. Check `lxdev logs` for new errors.

On failure, fix and repeat. Report what you observed, not what the edit should
do. Keep regressions as `lxdev test` specs: [Testing](./lxapp/testing.md).

## Shapes

```
┌─────────────────────┐                          ┌──────────────────────┐
│ View (WebView)      │ ◄── setData(patch) ───── │ Logic (JS runtime)   │
│ React / Vue / HTML  │                          │ Page({}) instance    │
│ renders `data`      │ ──── actions.foo() ────► │ owns state + `lx.*`  │
└─────────────────────┘                          └──────────────────────┘
```

View never mutates `data`; it calls an action and Logic answers with
`setData`.

| Shape | What it is | Pick when |
|---|---|---|
| **A. Standalone lxapp** | Page-based mini-app that runs in any LingXia host (e.g. macOS Runner). | UI/page work, no native shell. |
| **B. Host app + JS lxapp** | Native installable app (Android/iOS/macOS/Windows/Harmony) embedding a home lxapp whose Logic is JS. | Most product apps. |
| **C. Host app + native Rust control** | Either an HTML control lxapp with `logic: false`, or a macOS/Windows terminal/browser main with no bundled lxapp. | Rust-controlled utilities and products whose main experience is a built-in native capability. |

A JS-Logic lxapp that calls Rust routes is still B. B's id rule:
[`app`](./app/project.md#app). C's `appService`/`logic` rule:
[`features`](./app/project.md#features).

## `@lingxia/*` packages

| Package | What it is | Imported by | Typical import |
|---|---|---|---|
| `@lingxia/react` | React hooks + framework-wrapped native components | lxapp View (React) | `useLxPage`, `useLxHost`, `useLxStream`, `LxNativeRoot`, `LxVideo`, … |
| `@lingxia/vue` | Vue composables + framework-wrapped native components | lxapp View (Vue) | same surface as React, Vue-flavored |
| `@lingxia/html` | DOM helpers for HTML-only views | lxapp View (HTML) | `pageReady`, `getPage`, `subscribePage`, `getHost`, `subscribeHost` |
| `@lingxia/elements` | Pure-JS custom elements (`<lx-video>`, …) | rarely direct — react/vue re-export wrappers | `registerVideoComponent`, `LxVideoElement` |
| `@lingxia/types` | Declarations for Logic `lx.*`, `Page({})`, `App({})` | lxapp Logic (devDependency, global) | [Install typing](./lxapp/lx-api.md#install-typing) |
| `@lingxia/test` | Spec authoring SDK for `lxdev test` | lxapp tests | `import { spec } from '@lingxia/test'` |
| `@lingxia/bridge` | Bridge runtime + low-level helpers | rarely direct | types such as `LxStream`, `LxChannel` |
| `@lingxia/native` | Virtual module for the CLI-generated native client | lxapp View | `import { native } from '@lingxia/native'` |
| `@lingxia/page-runtime` | Internal — shared impl behind react/vue/html | **don't import directly** | — |

The CLI and host ship `@lingxia/polyfills` and `@lingxia/terminal-settings`;
never add them to a project.

## Reference map

| Need | File |
|---|---|
| **Loop** | |
| `lingxia` commands: new, dev, build, package, devices, upgrade | [cli/lingxia.md](./cli/lingxia.md) |
| Drive a running session: `lxdev` lxapp/runner/host/browser/logs | [cli/lxdev.md](./cli/lxdev.md) |
| Write and run specs (`@lingxia/test`, `lxdev test`) | [lxapp/testing.md](./lxapp/testing.md) |
| Mocks and scenarios: `mocks/`, `lxdev mock`, `lingxia dev --mock`, `t.app.mock.use` | [lxapp/mock.md](./lxapp/mock.md) |
| **Lxapp** | |
| Pages: `Page({})`, View hooks, lifecycle, events, `App({})`, tab bar, page chrome | [lxapp/guide.md](./lxapp/guide.md) |
| `setData`, streams, channels | [lxapp/bridge.md](./lxapp/bridge.md) |
| Native components: `LxNativeRoot` + `LxVideo`, `LxPicker`, `LxMediaSwiper`, `LxNavigator`; text inputs | [lxapp/components.md](./lxapp/components.md) |
| Logic runtime: typing, Web globals, product settings, `lx.supports`, badges, notifications, errors | [lxapp/lx-api.md](./lxapp/lx-api.md) |
| Size classes and per-size Views | [lxapp/adaptive-ui.md](./lxapp/adaptive-ui.md) |
| Files: `lx://` storage, `downloadFile`, `lx.fs`, `uploadFile`, quotas | [lxapp/files.md](./lxapp/files.md) |
| **Host app** | |
| `lingxia.yaml`: sections, surfaces, tray, theme, update, env | [app/project.md](./app/project.md) |
| Session classes: Control app vs control surface vs guest | [app/control-app.md](./app/control-app.md) |
| Agent/CLI control of a shipped product (`appUse`, `computerUse`, `browserUse`) | [app/agent-control.md](./app/agent-control.md) |
| Apple entry points and embedding | [app/apple-sdk.md](./app/apple-sdk.md) |
| Android URL player engine | [app/android-sdk.md](./app/android-sdk.md) |
| Universal links / app links | [app/applinks.md](./app/applinks.md) |
| **Native Rust** | |
| `HostAddon`, `#[lingxia::native]`, audience, native client, facades, JS extensions | [native/development.md](./native/development.md) |
| Network/privilege grants and the app registry | [native/permissions.md](./native/permissions.md) |
| Campaign launch screen | [native/splash.md](./native/splash.md) |
| **Ship** | |
| Publish, update keys, signing, stores, `lingxia auth` | [cli/distribution.md](./cli/distribution.md) |

## Where code goes

| Job | Lives in | Surface |
|---|---|---|
| UI rendering, page state | lxapp `pages/index.{tsx,vue,html}` | View |
| Page lifecycle, `setData`, action handlers (JS) | lxapp `pages/index.ts` | `Page({})` Logic |
| Cross-page business helpers callable as `lx.X(...)` | host Rust crate | `lingxia::js` extension (needs `standard` feature) |
| Page-scoped native UI (file/media picker, native browser) | host Rust crate | `#[lingxia::native]` route |
| Background services (devtool, push, ipc) | host Rust crate | `HostAddon::start_services` |
| Platform integrations needing predeclaration | `lingxia.yaml` | `capabilities`, `features` |
| Surfaces (windows, asides, sidebar/tray, terminal) | `lingxia.yaml` | `surfaces` |
| Bundled lxapp sources | folder + `resources.bundles` | `lingxia.yaml` |

## Symptom router

| Symptom | Where to look |
|---|---|
| Wrong control app launches / `homeAppId` matches no bundle | [project.md → `app`](./app/project.md#app) |
| `fetch()` fails from an lxapp | [permissions.md](./native/permissions.md) (host grant; default allow) |
| Is `fetch` / `setTimeout` / `URL` available in Logic? | [lx-api.md → Web globals](./lxapp/lx-api.md#web-globals) |
| Read/write files | [files.md](./lxapp/files.md) |
| Surface config rejected (`aside` edge, one `main`, terminal capability) | [project.md → Surfaces](./app/project.md#rules) |
| `setData` not reaching the View | [bridge.md → `setData`](./lxapp/bridge.md#setdata) |
| Native route returns `BRIDGE_METHOD_NOT_FOUND` | [development.md → Registration](./native/development.md#registration) |
| `#[lingxia::native]` compiles but the View can't call it | [development.md → Generated native client](./native/development.md#generated-native-client) |
| Stream cancel never cleans up | [bridge.md → Stream](./lxapp/bridge.md#stream) |
| `lingxia.yaml` change ignored after rebuild | [`lingxia clean`](./cli/lingxia.md#lingxia-clean) |
| iOS dev app can't reach the Mac | [`lingxia dev`](./cli/lingxia.md#lingxia-dev) |
| TS doesn't know `lx.foo()` / `Page({})` | [Install typing](./lxapp/lx-api.md#install-typing) |
| `<LxVideo>` / `<LxPicker>` prop not recognized | [components.md](./lxapp/components.md) |
| Build rejects `<video>` / `<audio>` / `new Audio()` | [components.md → LxVideo](./lxapp/components.md#lxvideo) |
| Handler gets an event instead of a payload (or vice versa) | [Callback shapes](./lxapp/components.md#callback-shapes) |
| `NATIVE_ROOT_INVALID_STRUCTURE` | [Inline native island](./lxapp/components.md#inline-native-island) |
| `E_PERMISSION_DENIED` naming a class | [control-app.md → Refusal](./app/control-app.md#refusal) |
