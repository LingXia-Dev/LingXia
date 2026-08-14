---
name: lingxia
description: Build apps on the LingXia cross-platform framework — standalone lxapps (page-based mini-apps with a View+Logic split), native host apps (Android/iOS/macOS/Harmony/Windows shells embedding an lxapp), and Rust native extensions. TRIGGER on the `lingxia` CLI, `lxdev`, `lxapp`, `lingxia.yaml`, `lxapp.json`, `#[lingxia::native]`, `HostAddon`, `useLxPage`, or an lxapp-flavored `Page({})`. SKIP if the project imports `@tarojs/*`, `wx.*`, `uni-app`, `@dcloudio/*`, or `@remax/*` — those share the `Page({})` shape but are different runtimes. **Always read §"Step 0" before generating any file.**
license: MIT
allowed-tools: Read, Grep, Glob, Edit, Write, Bash(lingxia:*), Bash(lxdev:*), Bash(npm:*), Bash(npx:*), Bash(test:*), Bash(ls:*), Bash(cat:*), Bash(cargo:*)
---

# LingXia App Development

LingXia is a cross-platform app framework. This skill is the entry router — it carries the decision tree and pointers into supporting reference files. **Read sub-files only when you need them**; do not load the whole tree up front.

When you read this you can usually assume the `lingxia` CLI is on `PATH` (verify with `lingxia version`; installing it is outside this skill's scope) and that you are inside a LingXia project — but **confirm the shape** with the probe in Step 0 rather than guessing.

---

## Step 0 — Decide before scaffolding

**Always do this first.** Before generating a single file:

### 0a. Identify what you're inside

```bash
test -f lingxia.yaml   && echo "host-app"
test -f lxapp.json     && echo "lxapp"
```

If none match, you're about to scaffold a new project — continue to 0b. If one matches, jump to the fast-path for that shape.

### 0b. Pick the shape (ask the user or infer)

1. **Standalone lxapp or host app?** (A vs B/C below)
2. **If host app:** which platforms? `android`, `ios`, `macos`, `windows`, `harmony`, any combination.
3. **If host app:** lxapp or terminal/browser as the main experience, and lxapp
   or native Rust as host control? (B vs C)
4. **View framework:** React, Vue, or HTML when an lxapp is scaffolded — chosen
   once; a project has exactly one.

Then scaffold:

```bash
# Shape A — standalone lxapp
lingxia new my-lxapp -t lxapp -y

# Shape B/C — native host app
lingxia new my-app -t native-app -p macos --package-id com.example.myapp -y
# -p accepts: android, ios, macos, windows, harmony, all  (comma-separated)
```

`lingxia doctor` verifies platform toolchains.

---

## The development loop

Two binaries, one split: **`lingxia dev` starts a session** (build → install →
launch → dev websocket), **`lxdev` connects to that session** and drives it.
Memorize the split; everything else is detail in the two CLI docs.

After an edit, pick the loop by **what you changed** — this is the decision
that matters:

| You changed | Do |
|---|---|
| **lxapp code** (View / Logic / `lxapp.json`) — an embedded resource or a standalone lxapp project | `lxdev lxapp reload` — rebuilds the current bundle and reloads it. Use `--app <appid>` for a non-home resource. No new session. |
| **host/app code** (`lingxia.yaml`, native Rust, platform projects) | re-run `lingxia dev` — it automatically stops the project's previous same-platform session and takes over. |

```bash
lingxia dev --background     # start (or take over) this project's session; returns when live
lingxia dev <http(s)://url> --headless --background
lxdev lxapp reload           # lxapp inner loop: rebuild + reload in place
```

**A successful edit (or build) is not "done."** Done means you drove the change
in the running app and watched it behave. Close the loop with `lxdev`:

1. Apply the change with the loop above (reload / dev takeover).
2. Exercise the change itself: navigate to the page (`lxdev lxapp nav to ...`)
   and interact with it (`lxdev lxapp page click/type ...`). A new control gets
   clicked, not just rendered.
3. Confirm the expected effect where it lives: page DOM via
   `lxdev lxapp page eval`, Logic state via `lxdev lxapp eval` — assertable
   values beat screenshot-squinting. Screenshot before/after only when the
   change is visual.
4. Check `lxdev logs` for new errors or warnings from the interaction.

Any step fails → fix and rerun from step 1. Don't hand back partially verified
work, and report what you actually observed — not what the edit should do.

Keep regressions and repeatable business flows as `lxdev test` cases:
`tests/api/` for `lx.*` contracts, `tests/pages/` for page behavior, and
`tests/flows/` for cross-page journeys.
One-off visual polish needs live verification, not necessarily a permanent test.

Command details: [`lingxia` CLI](./cli/lingxia.md) · [`lxdev`](./cli/lxdev.md).

---

## What you build (pick one shape)

| Shape | What it is | Pick when |
|---|---|---|
| **A. Standalone lxapp** | Page-based mini-app that runs in any LingXia host (e.g. macOS Runner). | UI/page work, no native shell. |
| **B. Host app + JS lxapp** | Native installable app (Android/iOS/macOS/Windows/Harmony) embedding a home lxapp whose Logic is JS. | Most product apps. |
| **C. Host app + native Rust control** | Either an HTML control lxapp with `logic: false`, or a macOS/Windows terminal/browser main with no bundled lxapp. | Rust-controlled utilities and products whose main experience is a built-in native capability. |

C sets `features.appService: false` and puts control in Rust. It may retain a
`logic: false` HTML view, or omit the control lxapp for a desktop native main.
You can also mix: a JS-Logic lxapp that **calls** Rust routes via
`#[lingxia::native]` is still B, with native Rust as an API surface rather than
the control layer.

---

## `@lingxia/*` npm packages at a glance

Every published package and what to import from each. Don't guess imports from the package name — use this table.

| Package | What it is | Imported by | Typical import |
|---|---|---|---|
| `@lingxia/react` | React hooks + framework-wrapped native components | lxapp View (React) | `useLxPage`, `useLxPageChrome`, `useLxStream`, `LxVideo`, … |
| `@lingxia/vue` | Vue composables + framework-wrapped native components | lxapp View (Vue) | same surface as React, Vue-flavored |
| `@lingxia/html` | DOM helpers for HTML-only views (`subscribe`, `getActions`, …) | lxapp View (HTML) | `getActions`, `subscribe`, `getPageChromeLayout` |
| `@lingxia/elements` | Pure-JS custom elements (`<lx-video>`, `<lx-input>`, …) | rarely direct — `@lingxia/react`/`vue` re-export wrappers around these | `registerVideoComponent`, `LxVideoElement` |
| `@lingxia/types` | **TypeScript declarations for the Logic-side `lx.*` API + `Page({})` / `App({})` globals** | lxapp Logic (`pages/*/index.ts`) | install as dev dep; types apply globally |
| `@lingxia/bridge` | Bridge runtime + low-level invocation helpers | rarely direct (advanced) | only when bypassing the framework wrappers |
| `@lingxia/native` | Virtual module — points at the **CLI-generated** native client (`#[lingxia::native]` routes) | lxapp View | `import { native } from '@lingxia/native'` — only after a native build runs |
| `@lingxia/page-runtime` | Internal — shared impl behind react/vue/html | **don't import directly** | — |
| `@lingxia/skill` | This skill itself | install via `npx @lingxia/skill install` | not imported in code |

**Logic-side typing**: install `@lingxia/types` as a devDependency (declarations are global — no `import`). Runtime boundaries and typing setup: [`./lxapp/lx-api.md`](./lxapp/lx-api.md).

## Reference map (inside this skill)

| Need | File |
|---|---|
| The `lingxia` CLI — daily commands (build, dev, package, install) | [`./cli/lingxia.md`](./cli/lingxia.md) |
| Drive a running `lingxia dev` session — `lxdev` (browser/app/lxapp/logs automation; run `lxdev <cmd> --help` for exact flags) | [`./cli/lxdev.md`](./cli/lxdev.md) |
| Ship it: publish to the LingXia server, platform signing, app-store submission, developer accounts | [`./cli/distribution.md`](./cli/distribution.md) |
| Page authoring: `Page({})`, `useLxPage`, events | [`./lxapp/guide.md`](./lxapp/guide.md) |
| Adaptive Views: surface size classes, runtime switching, Runner device frames | [`./lxapp/adaptive-ui.md`](./lxapp/adaptive-ui.md) |
| **Native components: `LxVideo`, `LxMediaSwiper`, `LxPicker`, `LxNavigator` (text input is plain `<input>`/`<textarea>`)** | [`./lxapp/components.md`](./lxapp/components.md) |
| **Logic runtime globals and typing setup** | [`./lxapp/lx-api.md`](./lxapp/lx-api.md) |
| Bridge mechanics: `setData`, stream, channel | [`./lxapp/bridge.md`](./lxapp/bridge.md) |
| Host project: `lingxia.yaml` reference, adaptive `surfaces` | [`./app/project.md`](./app/project.md) |
| Let a command line or agent drive a shipped product — `appUse` / `computerUse` / `browserUse`, the product's own CLI, generated agent skills | [`./app/agent-control.md`](./app/agent-control.md) |
| Native Rust: `HostAddon`, `#[lingxia::native]`, facades, JS extensions | [`./native/development.md`](./native/development.md) |
| Launch screen (`splash:`) and the per-launch cover hook | [`./native/splash.md`](./native/splash.md) |
| iOS/macOS SDK embedding, public startup APIs | [`./app/apple-sdk.md`](./app/apple-sdk.md) |
| Universal links / app links setup | [`./app/applinks.md`](./app/applinks.md) |
| File API lifecycle (storage classes, downloadFile, `lx.fs`) | [`./reference/file-lifecycle.md`](./reference/file-lifecycle.md) |

## See a real layout — scaffold one

Don't reach for a frozen example tree. The CLI emits a working, version-matched project per shape — generate one in a scratch dir and read it:

```bash
lingxia new hello -t lxapp -y                                   # Shape A — standalone lxapp
lingxia new hello -t native-app -p macos --package-id com.example.hello -y   # Shape B/C — host app
```

The output is the authoritative layout for the `lingxia` on your `PATH`; it can't drift the way a hand-written sample does. What to look at per shape:

- **A — standalone lxapp** (JS). `pages/home/`: `index.ts` is **Logic** (`Page({ data, …actions })`, runs in the JS runtime), `index.tsx` is **View** (React + `useLxPage`, runs in the WebView), `index.json` is page config. Type View `PageData`/`PageActions` fields as **required**. A `_`-prefixed method stays private to Logic. `lxapp.json` `security.network.trustedDomains` starts `[]` (all `fetch` denied) — set real hostnames before networking.
- **B — host + JS lxapp** (most product apps). Adds a `lingxia.yaml` with `features.appService: true`. Three ids must line up or the wrong app launches: `app.homeAppId` = a `resources.bundles[].appId` = that bundle's `lxapp.json.appId`. The launch `main` surface's `lxapp:` content key is the appId it renders, so point it at that same home app. View talks to Logic via `actions.foo()` from `useLxPage()`.
- **C — host + Rust control.** With an embedded HTML control lxapp, use `features.appService: false`, `lxapp.json` `"logic": false`, `window.native.*`, and `#[lingxia::native]` routes. A macOS/Windows `native: terminal|browser` main may instead omit the control lxapp, `homeAppId`, and resources entirely: `lingxia new … --main terminal|browser --control native`. Runtime/downloaded lxapps remain guest workspaces, not the trusted control app. Flip `appService` and embedded `logic` together; don't add `@lingxia/react|vue|html` to a logic-disabled view.

Run any shape with `lingxia dev`. Full recipes: [LxApp page](./lxapp/guide.md#logic-layer--page) · [host `lingxia.yaml`](./app/project.md#minimal-macos-example) · [Rust route](./native/development.md#native-routes).

---

## Symptom router — error → file

Jump straight here when the user reports a concrete failure:

| Symptom | Where to look |
|---|---|
| A configured `homeAppId` doesn't match a bundle / wrong control app launches | [`./app/project.md`](./app/project.md) → `resources.bundles` |
| `fetch()` silently fails from an lxapp | [`./lxapp/guide.md`](./lxapp/guide.md) → "Security Policy" (`trustedDomains`) |
| "Is `fetch` / `setTimeout` / `URL` available in Logic?" | [`./lxapp/lx-api.md`](./lxapp/lx-api.md#standard-web-apis-built-in-globals) — yes, full Rong runtime |
| Need to read/write files (not just `lx.downloadFile`) | `lx.fs` — paths & lifecycle in [`./reference/file-lifecycle.md`](./reference/file-lifecycle.md) |
| Surface config rejected (`aside` needs `edge`, one `main`, terminal needs capability) | [`./app/project.md`](./app/project.md#surfaces-adaptive-ui) → Rules |
| `setData` not reflecting in View | [`./lxapp/bridge.md`](./lxapp/bridge.md) → "How replication works" |
| Native route returns `BRIDGE_METHOD_NOT_FOUND` | [`./native/development.md`](./native/development.md) → Host Addon registration |
| `#[lingxia::native]` compiles but View can't call it | [`./native/development.md`](./native/development.md) → "Generated Native Client" |
| Stream cancels never trigger cleanup | [`./lxapp/bridge.md`](./lxapp/bridge.md) → use the generator form + `finally` (the explicit handle has no cancel hook) |
| `lingxia.yaml` change ignored after rebuild | [`./cli/lingxia.md`](./cli/lingxia.md) → `lingxia clean`, then rebuild |
| iOS dev app can't reach Mac dev server | [`./cli/lingxia.md`](./cli/lingxia.md) → `lingxia dev` (LAN reachability) |
| `Lingxia.initialize(...)` not found | [`./app/apple-sdk.md`](./app/apple-sdk.md) → use `Lingxia.quickStart()` (legacy removed) |
| TS doesn't know about `lx.foo()` / `Page({})` in Logic | install `@lingxia/types` as a devDependency; see [`./lxapp/lx-api.md`](./lxapp/lx-api.md) |
| `<LxVideo>` / `<LxPicker>` attribute not recognized by TS or runtime | [`./lxapp/components.md`](./lxapp/components.md) → component attribute table |
| Event handler on `LxVideo` fires DOM CustomEvent, not unwrapped detail | [`./lxapp/components.md`](./lxapp/components.md) → "Callback shapes by component" |

---

## Where does this code go?

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

---

## Top pitfalls (one per layer — full lists in the sub-files)

**LxApp** — see [`./lxapp/guide.md` → Common Pitfalls](./lxapp/guide.md#common-pitfalls):

- Generating `.tsx` + `.vue` + `.html` for one page. A project has one view framework — match the existing pages.
- `fetch()` to a host not in `security.network.trustedDomains` fails silently.

**Host app** — see [`./app/project.md` → Common Pitfalls](./app/project.md#common-pitfalls):

- Editing generated `app.json` / `ui.json` instead of `lingxia.yaml`. They're regenerated every build.
- A configured `homeAppId` not matching any `resources.bundles[].appId` — build
  fails or the wrong control app launches.

**Native Rust** — see [`./native/development.md`](./native/development.md):

- Importing internal crates (`lingxia_logic`, `rong`) directly. Use `lingxia::*` facades.
- `app: Arc<LxApp>` not first, or `HostCancel` not last, in a `#[lingxia::native]` signature. The macro **generates** the `<fn>_host()` registration companion — never write it yourself.

---

## Pre-ship checklist

Run the per-layer checklists before shipping: [LxApp](./lxapp/guide.md#pre-ship-checklist) · [Host app](./app/project.md#pre-ship-checklist).
