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
test -f .lingxia/template.json && echo "from a template"
```

From a template: commit `.lingxia/template.json`, no other `.lingxia/` content.
`lingxia build` and `lingxia dev` run the project's `lingxia:prepare` script, so
don't run its generators yourself. See [projects from a template](./cli/lingxia.md#projects-from-a-template).

Neither matches: an empty directory, or someone else's project. Scaffold only
into an empty directory or one the user asked you to initialise; otherwise say
so and stop. Scaffolding writes pages, config, and dependencies.

### 0b. Pick the shape

1. Standalone lxapp or host app? (A vs B/C)
2. Host app: which platforms (`android`, `ios`, `macos`, `windows`, `harmony`)?
3. Host app: lxapp or native terminal/browser as the main experience; lxapp or
   Rust as host control? (B vs C)
4. View framework: GPU View (the default), or React or HTML in a WebView — one per project.

### 0c. Scaffold and read the result

```bash
lingxia new hello -t lxapp -y                                   # Shape A — standalone lxapp
lingxia new hello -t native-app -p macos --package-id com.example.hello -y   # Shape B/C — host app
```

The generated tree is the authoritative layout for your CLI version. In
`pages/home/`, `index.ts` is Logic, `index.tsx` (or `.html`) is the
View, `index.json` is page config. More forms: [`lingxia new`](./cli/lingxia.md#lingxia-new).
`lingxia doctor` checks toolchains.

## The development loop

`lingxia dev` starts a session (build → install → launch → dev websocket);
`lxdev` drives it.

| You changed | Do |
|---|---|
| lxapp code (View, Logic, `lxapp.json`), standalone or embedded | Save. [Auto-reload](./cli/dev-reload.md). |
| host code (`lingxia.yaml`, native Rust, platform projects) | Re-run `lingxia dev`; it takes over the previous session. |

```bash
lingxia dev --background     # start (or take over) this project's session; returns when live
lingxia dev stop
```

**A successful build is not done.** Done means you watched the change work:

1. Navigate to it (`lxdev lxapp nav go ...`) and use it
   (`lxdev lxapp page click '[data-testid="save"]'`, `page type`). A new control gets clicked.
2. Assert the effect: the page's accessibility tree via `lxdev lxapp page tree` or
   `page find ROLE --name NAME`, Logic state via `lxdev lxapp eval`. A WebView page also takes
   `lxdev lxapp page eval` (DOM). Screenshot (`page screenshot`) only visual changes.
3. Check `lxdev logs` for new errors.

`lingxia check` lists type and authoring errors without building; a GPU View component is
unit-tested in Node with `@lingxia/view/testing` ([GPU tests](./lxapp/gpu/testing.md)).

On failure, fix and repeat. Report what you observed, not what the edit should
do. Keep regressions as `lxdev test` cases: [Testing](./lxapp/testing.md).

A bare `lxdev test` runs every test. Run `lxdev test --changed` after an edit
and `lxdev test --last-failed` while fixing; run everything once before pushing.

## Shapes

```
┌─────────────────────┐                          ┌──────────────────────┐
│ View                │ ◄── page.update(…) ───── │ Logic (JS runtime)   │
│ GPU TSX (default),  │                          │ definePage instance  │
│ or React/HTML in a  │ ──── actions.foo() ────► │ owns state + `lx.*`  │
│ WebView             │                          │                      │
└─────────────────────┘                          └──────────────────────┘
```

View never mutates `data`; it calls an action and Logic answers with
`page.update`.

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
| `@lingxia/view` | GPU View primitives' types, hooks and helpers | lxapp View (GPU) | `useLocal`, `useAction`, `useRequest`, `useStream`, `List`, `t`; tests: `@lingxia/view/testing` |
| `@liulidesign/core` | Liuli: accessible GPU components (buttons, fields, rows, sheets, icons) | lxapp View (GPU) | `import { Button } from '@liulidesign/core/Button'`; the project's `liuli` skill says how to find one |
| `@liulidesign/charts` | Liuli charts and stats | lxapp View (GPU) | `import { LineChart } from '@liulidesign/charts/LineChart'` |
| `@lingxia/react` | React hooks + framework-wrapped native components | lxapp View (React) | `useLxPage`, `useLxHost`, `useLxStream`, `LxPicker`, `LxMediaSwiper`, … |
| `@lingxia/html` | DOM helpers for HTML-only views | lxapp View (HTML) | `pageReady`, `getPage`, `subscribePage`, `getHost`, `subscribeHost` |
| `@lingxia/elements` | Pure-JS custom elements (`<lx-picker>`, …) | rarely direct — react re-exports wrappers | `registerPickerComponent`, `LxPickerElement` |
| `@lingxia/logic` | Logic's `lx`, `definePage`, `type`, `LxError` and every API type | lxapp Logic | [Install typing](./lxapp/core.md#install-typing) |
| `@lingxia/test` | Test authoring SDK for `lxdev test` | lxapp tests | `import { test } from '@lingxia/test'` |
| `@lingxia/bridge` | Bridge runtime + low-level helpers | rarely direct | types such as `LxStream`, `LxChannel` |
| `@lingxia/native` | Virtual module for the CLI-generated native client | lxapp View | `import { native } from '@lingxia/native'` |
| `@lingxia/page-runtime` | Internal — shared impl behind react/html | **don't import directly** | — |

The CLI and host ship `@lingxia/polyfills` and `@lingxia/terminal-settings`;
never add them to a project.

## Reference map

| Need | File |
|---|---|
| **Loop** | |
| `lingxia` commands: new, dev, build, package, devices, skill, upgrade | [cli/lingxia.md](./cli/lingxia.md) |
| Saving an lxapp, `lingxia dev listen`, attaching one the host does not declare | [cli/dev-reload.md](./cli/dev-reload.md) |
| Drive a running session: `lxdev` lxapp/runner/host/browser/logs | [cli/lxdev.md](./cli/lxdev.md) |
| Write and run tests (`@lingxia/test`, `lxdev test`) | [lxapp/testing.md](./lxapp/testing.md) |
| Mocks and scenarios: `mocks/`, `lxdev mock`, `lingxia dev --mock`, `t.scenario.use` | [lxapp/mock.md](./lxapp/mock.md) |
| **Lxapp** | |
| Pages: `definePage`, lifecycle, `App({})`, navigation, tab bar, page chrome, catalogues | [lxapp/guide.md](./lxapp/guide.md) |
| Hook order, `App`/`Page` triggers, what each navigation or restart fires | [lxapp/lifecycle.md](./lxapp/lifecycle.md) |
| GPU View: elements, layout, hooks, events, dialogs, inputs, accessibility, video | [lxapp/gpu/authoring.md](./lxapp/gpu/authoring.md) |
| GPU View: Tailwind classes, theme roles, `theme/pack.json`, transitions, drag and snap | [lxapp/gpu/styling-motion.md](./lxapp/gpu/styling-motion.md) |
| GPU View: tests in Node, checking the running page | [lxapp/gpu/testing.md](./lxapp/gpu/testing.md) |
| Page state, streams, channels | [lxapp/bridge.md](./lxapp/bridge.md) |
| WebView pages: React/HTML hooks, host facts, leave guard, page chrome CSS | [lxapp/webview/guide.md](./lxapp/webview/guide.md) |
| WebView native elements: `LxPicker`, `LxMediaSwiper`, `LxNavigator`; text inputs, HTML `<video>` / `<audio>` | [lxapp/webview/native-elements.md](./lxapp/webview/native-elements.md) |
| Logic runtime: typing, Web globals, product settings, `lx.supports`, errors; look up a member with `lingxia api` | [lxapp/core.md](./lxapp/core.md) |
| Size classes and per-size Views | [lxapp/adaptive-ui.md](./lxapp/adaptive-ui.md) |
| Files: `lx://` storage, `lx.network.download`, `lx.fs`, `lx.network.upload`, quotas | [lxapp/files.md](./lxapp/files.md) |
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
| UI rendering, page state | lxapp `pages/<name>/index.tsx` (or `.html`) | View |
| Page lifecycle, state, actions (JS) | lxapp `pages/index.ts` | `definePage` Logic |
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
| Is `fetch` / `setTimeout` / `URL` available in Logic? | [core.md → Web globals](./lxapp/core.md#web-globals) |
| Read/write files | [files.md](./lxapp/files.md) |
| Surface config rejected (`aside` edge, one `main`, terminal capability) | [project.md → Surfaces](./app/project.md#rules) |
| `page.update` not reaching the View | [bridge.md → State](./lxapp/bridge.md#state) |
| Native route returns `BRIDGE_METHOD_NOT_FOUND` | [development.md → Registration](./native/development.md#registration) |
| `#[lingxia::native]` compiles but the View can't call it | [development.md → Generated native client](./native/development.md#generated-native-client) |
| Stream cancel never cleans up | [bridge.md → Stream](./lxapp/bridge.md#stream) |
| `lingxia.yaml` change ignored after rebuild | [`lingxia clean`](./cli/lingxia.md#lingxia-clean) |
| iOS dev app can't reach the Mac | [`lingxia dev`](./cli/lingxia.md#lingxia-dev) |
| A save did not update the running lxapp | [dev-reload.md](./cli/dev-reload.md) |
| `lxdev test` takes too long / runs unrelated tests | [testing.md → Running](./lxapp/testing.md#running) |
| TS doesn't know `lx.foo()` / `definePage` | [Install typing](./lxapp/core.md#install-typing) |
| A GPU build error (`UI_*`) | [GPU authoring](./lxapp/gpu/authoring.md), [styling](./lxapp/gpu/styling-motion.md) |
| `<LxPicker>` / `<LxMediaSwiper>` prop not recognized | [native-elements.md](./lxapp/webview/native-elements.md) |
| WebView handler gets an event instead of a payload (or vice versa) | [Callback shapes](./lxapp/webview/native-elements.md#callback-shapes) |
| `E_PERMISSION_DENIED` naming a class | [control-app.md → Refusal](./app/control-app.md#refusal) |
