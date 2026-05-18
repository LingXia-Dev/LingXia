---
name: lingxia
description: Build apps on the LingXia cross-platform framework — standalone lxapps (page-based mini-apps with a View+Logic split), native host apps (Android/iOS/macOS/Harmony shells embedding an lxapp), and Rust native extensions. TRIGGER on `lingxia` CLI, `lxapp`, `lingxia.yaml`, `lxapp.json`, `#[lingxia::native]`, `HostAddon`, `useLxPage`, `LxAppController`, or an lxapp-flavored `Page({})`. SKIP if the project imports `@tarojs/*`, `wx.*`, `uni-app`, `@dcloudio/*`, or `@remax/*` — those share the `Page({})` shape but are different runtimes. **Always read §"Step 0" before generating any file.**
license: MIT
allowed-tools: Read, Grep, Glob, Edit, Write, Bash(lingxia:*), Bash(npm:*), Bash(npx:*), Bash(test:*), Bash(ls:*), Bash(cat:*), Bash(cargo:*)
---

# LingXia App Development

LingXia is a cross-platform app framework. This skill is the entry router — it carries the decision tree, fast-path recipes, and pointers into supporting reference files. **Read sub-files only when you need them**; do not load the whole tree up front.

This skill is installed via `npx @lingxia/skill install` (the `lingxia` CLI prints a hint pointing at that command after `lingxia new`, but does not install the skill itself). When you read this you can usually assume:

- The `lingxia` CLI is on `PATH` (verify with `lingxia --version`); if not, point the human at `docs/quick-start.md` in the LingXia repo for the install steps.
- You are inside a LingXia project — but **confirm the shape** with the probe in Step 0 rather than guessing.

For first-time CLI + platform-toolchain setup (one-time, human-facing onramp), the repo's `docs/quick-start.md` is the source. This skill does not duplicate it.

---

## Step 0 — Decide before scaffolding

**Always do this first.** Before generating a single file:

### 0a. Identify what you're inside

```bash
test -f lingxia.yaml   && echo "host-app"
test -f lxapp.json     && echo "lxapp"
test -f lxplugin.json  && echo "lxplugin"
```

If none match, you're about to scaffold a new project — continue to 0b. If one matches, jump to the fast-path for that shape.

### 0b. Pick the shape (ask the user or infer)

1. **Standalone lxapp or host app?** (A vs B/C below)
2. **If host app:** which platforms? `android`, `ios`, `macos`, `harmony`, any combination.
3. **If host app:** JS Logic for the home lxapp, or native-only Rust? (B vs C)
4. **View framework:** React **or** Vue **or** HTML — pick **one**. The LingXia repo's `lingxia-showcase` example deliberately mixes all three; real apps do not.

Then scaffold:

```bash
# Shape A — standalone lxapp
lingxia new my-lxapp -t lxapp -y

# Shape B/C — native host app
lingxia new my-app -t native-app -p macos --package-id com.example.myapp -y
# -p accepts: android, ios, macos, harmony, all  (comma-separated)
```

`lingxia doctor` verifies platform toolchains.

---

## What you build (pick one shape)

| Shape | What it is | Pick when |
|---|---|---|
| **A. Standalone lxapp** | Page-based mini-app that runs in any LingXia host (e.g. macOS Runner). | UI/page work, no native shell. |
| **B. Host app + JS lxapp** | Native installable app (Android/iOS/macOS/Harmony) embedding a home lxapp whose Logic is JS. | Most product apps. |
| **C. Host app + native Rust logic** | Same shell, but the home lxapp's Logic is in Rust. The lxapp is HTML-only with `logic: false`. | Native-only hosts (e.g. menu-bar utilities), or when the heavy lifting belongs in Rust. |

C is just B with `features.appService: false` and Rust replacing the JS Logic. You can also mix: a JS-Logic lxapp that **calls** Rust routes via `#[lingxia::native]` — that is still B, with native Rust as an *API surface* rather than the Logic layer.

---

## `@lingxia/*` npm packages at a glance

Every published package and what to import from each. Don't guess imports from the package name — use this table.

| Package | What it is | Imported by | Typical import |
|---|---|---|---|
| `@lingxia/react` | React hooks + framework-wrapped native components | lxapp View (React) | `useLxPage`, `useLxStream`, `useLxChannel`, `LxInput`, `LxVideo`, … |
| `@lingxia/vue` | Vue composables + framework-wrapped native components | lxapp View (Vue) | same surface as React, Vue-flavored |
| `@lingxia/html` | DOM helpers for HTML-only views (`subscribe`, `getActions`, …) | lxapp View (HTML) | `import { getActions, subscribe } from '@lingxia/html'` |
| `@lingxia/elements` | Pure-JS custom elements (`<lx-video>`, `<lx-input>`, …) | rarely direct — `@lingxia/react`/`vue` re-export wrappers around these | `registerVideoComponent`, `LxVideoElement` |
| `@lingxia/types` | **TypeScript declarations for the Logic-side `lx.*` API + `Page({})` / `App({})` globals** | lxapp Logic (`pages/*/index.ts`) | install as dev dep; types apply globally |
| `@lingxia/bridge` | Bridge runtime + low-level invocation helpers | rarely direct (advanced) | only when bypassing the framework wrappers |
| `@lingxia/native` | Virtual module — points at the **CLI-generated** native client (`#[lingxia::native]` routes) | lxapp View | `import { native } from '@lingxia/native'` — only after a native build runs |
| `@lingxia/page-runtime` | Internal — shared impl behind react/vue/html | **don't import directly** | — |
| `@lingxia/skill` | This skill itself | install via `npx @lingxia/skill install` | not imported in code |

**Logic-side typing.** Add `@lingxia/types` to the lxapp's `devDependencies` so `pages/*/index.ts` gets full intellisense for `lx.navigateTo(...)`, `lx.chooseMedia(...)`, the `Page({ data, onLoad, … })` shape, and so on. The declarations are global — no `import` needed in your Logic files.

For the full surface of `lx.*` and which namespace covers what, see [`./lxapp/lx-api.md`](./lxapp/lx-api.md).

## Reference map (inside this skill)

| Need | File |
|---|---|
| Every CLI command, flag, env var (daily use) | [`./cli/reference.md`](./cli/reference.md) |
| Page authoring: `Page({})`, `useLxPage`, events | [`./lxapp/guide.md`](./lxapp/guide.md) |
| **Native components: `LxInput`, `LxVideo`, `LxMediaSwiper`, `LxPicker`, `LxNavigator`, `LxTextarea`** | [`./lxapp/components.md`](./lxapp/components.md) |
| **Logic-side `lx.*` API surface map** | [`./lxapp/lx-api.md`](./lxapp/lx-api.md) |
| Bridge mechanics: `setData`, stream, channel | [`./lxapp/bridge.md`](./lxapp/bridge.md) |
| Host project: `lingxia.yaml` reference, macOS App UI | [`./app/project.md`](./app/project.md) |
| Native Rust: `HostAddon`, `#[lingxia::native]`, facades, JS extensions | [`./native/development.md`](./native/development.md) |
| iOS/macOS SDK embedding, public startup APIs | [`./app/apple-sdk.md`](./app/apple-sdk.md) |
| Universal links / app links setup | [`./app/applinks.md`](./app/applinks.md) |
| File API lifecycle (storage classes, downloadFile, FileManager) | [`./reference/file-lifecycle.md`](./reference/file-lifecycle.md) |

## Bundled hello-world examples

Three minimal end-to-end shapes ship with this skill — one per shape, named to make the JS-vs-Rust Logic split obvious. Read them first when you need to see the exact file layout for a shape. They are **not buildable starters** (no lockfiles, generated artifacts, or platform host scaffolding); they exist so you can match the shape and write real code with `lingxia new`.

| Example | Shape | Logic in | What it shows |
|---|---|---|---|
| [`./examples/hello-lxapp/`](./examples/hello-lxapp/README.md) | A — standalone lxapp | JS | `Page({})` Logic + React `useLxPage` View + `lxapp.json` security policy |
| [`./examples/hello-host-js/`](./examples/hello-host-js/README.md) | B — host + JS lxapp | JS | minimal macOS `lingxia.yaml` + embedded JS-Logic lxapp |
| [`./examples/hello-host-rust/`](./examples/hello-host-rust/README.md) | C — host + Rust Logic | **Rust** | `features.appService: false` + `lxapp.json` `"logic": false` + HTML view calling `window.native.*` |

`hello-host-js` and `hello-host-rust` share the host shell wiring (`lingxia.yaml` `ui`, `resources.bundles`, FFI export) — the diff is entirely on the Logic side: who owns state and how the View talks to it. Read both side-by-side when picking B vs C.

For a real, buildable starter, run `lingxia new` — the CLI emits a working project that matches the version on `PATH` and is regenerated per release.

**SDK-author internals** (bridge wire protocol, logging, webview lifecycle, env-version build-time plumbing) live in the LingXia repo under `docs/internal/` and `docs/draft/`. The skill summarizes the app-author-facing surface where relevant (e.g. env-version → [`./app/project.md`](./app/project.md#environment-versions)); the internal docs themselves are not needed for building on LingXia.

---

## Symptom router — error → file

Jump straight here when the user reports a concrete failure:

| Symptom | Where to look |
|---|---|
| `homeAppId` doesn't match any bundle / wrong app launches | [`./app/project.md`](./app/project.md) → `resources.bundles` |
| `fetch()` silently fails from an lxapp | [`./lxapp/guide.md`](./lxapp/guide.md) → "Security Policy" (`trustedDomains`) |
| "Is `fetch` / `setTimeout` / `URL` available in Logic?" | [`./lxapp/lx-api.md`](./lxapp/lx-api.md#standard-web-apis-built-in-globals) — yes, full Rong runtime |
| Need to read/write files (not just `lx.downloadFile`) | [`./lxapp/lx-api.md`](./lxapp/lx-api.md#file-and-transfer) → `lx.getFileManager()` |
| Need a scheduled task running across pages | [`./lxapp/lx-api.md`](./lxapp/lx-api.md#appservice-only-extras) → AppService `cron` |
| `attachPanel` validation rejected | [`./app/project.md`](./app/project.md) → "surfaces" rules |
| `setData` not reflecting in View | [`./lxapp/bridge.md`](./lxapp/bridge.md) → "How replication works" |
| Native route returns `BRIDGE_METHOD_NOT_FOUND` | [`./native/development.md`](./native/development.md) → Host Addon registration |
| `#[lingxia::native]` compiles but View can't call it | [`./native/development.md`](./native/development.md) → "Generated Native Client" |
| Stream cancels never trigger cleanup | [`./lxapp/bridge.md`](./lxapp/bridge.md) → `finally` block / `stream.on('cancel')` |
| `lingxia.yaml` change ignored after rebuild | [`./cli/reference.md`](./cli/reference.md) → `lingxia clean`, then rebuild |
| iOS dev app can't reach Mac dev server | [`./cli/reference.md`](./cli/reference.md) → `lingxia dev` (LAN reachability) |
| `Lingxia.initialize(...)` not found | [`./app/apple-sdk.md`](./app/apple-sdk.md) → use `Lingxia.quickStart()` (legacy removed) |
| TS doesn't know about `lx.foo()` / `Page({})` in Logic | install `@lingxia/types` as a devDependency; see [`./lxapp/lx-api.md`](./lxapp/lx-api.md) |
| `<LxVideo>` / `<LxPicker>` attribute not recognized by TS or runtime | [`./lxapp/components.md`](./lxapp/components.md) → component attribute table |
| Event handler on `LxVideo` fires DOM CustomEvent, not unwrapped detail | [`./lxapp/components.md`](./lxapp/components.md) → "Callback shapes by component" |

---

## Fast-path recipes

These cover the 80% case. For anything beyond, jump to the linked reference file.

### Recipe 1 — A standalone lxapp page

**Logic** (`pages/home/index.ts`):

```ts
Page({
  data: { count: 0 },
  onLoad(options) { /* options = URL query params */ },
  increment() { this.setData({ count: this.data.count + 1 }); },
  _privateHelper(n) { return n * 2; }, // _-prefixed = NOT exposed to View
});
```

**View — React** (`pages/home/index.tsx`):

```tsx
import { useLxPage } from '@lingxia/react';

type PageData = { count: number };
type PageActions = { increment: () => void };

export default function HomePage() {
  const { data, actions } = useLxPage<PageData, PageActions>();
  return <button onClick={() => actions.increment()}>{data.count}</button>;
}
```

> **On the `?:` question.** The runtime guarantees `data` reflects Logic's initial `data` object by first paint, and `actions` is fully wired during page setup. Mark `PageData` / `PageActions` fields **required** unless your Logic genuinely populates them lazily (e.g. via an async `setData` after `onLoad`). Earlier drafts used all-`?` fields and produced unnecessary `actions.foo?.()` and `data?.x ?? default` noise — avoid that pattern.

**Page config** (`pages/home/index.json`):

```json
{ "navigationBarTitleText": "Home", "navigationStyle": "custom" }
```

**Register** in `lxapp.json`:

```json
{ "pages": [{ "name": "home", "path": "pages/home/index" }] }
```

**Run:** `lingxia dev` (launches macOS Runner).

Vue / HTML variants, action shapes (stream, channel), native components, `trustedDomains` security policy: [`./lxapp/guide.md`](./lxapp/guide.md). Bridge mechanics: [`./lxapp/bridge.md`](./lxapp/bridge.md).

### Recipe 2 — A macOS host app window

**`lingxia.yaml`:**

```yaml
app:
  projectName: myapp
  productName: My App
  productVersion: 1.0.0
  platforms: [macos]
  homeAppId: my-home

macos:
  bundleId: com.example.myapp
  deploymentTarget: "12.0"
  targetName: MyApp
  executableName: MyApp

features: { appService: true, shell: false, devtools: false }

resources:
  bundles:
    - { type: lxapp, appId: my-home, path: my-home }

ui:
  launch: { initialSurface: main }
  surfaces:
    - id: main
      presentation: { kind: window }
      content: { kind: lxapp, appId: my-home }
  activators: []
```

**Run:** `lingxia dev` (build, install, launch).

Menu-bar shape, sidebar + attachPanel shape, every section of `lingxia.yaml`, surfaces/activators rules, icon paths: [`./app/project.md`](./app/project.md).

### Recipe 3 — A Rust native route called from the View

**Rust** (host crate `src/lib.rs`):

```rust
use std::sync::Arc;

#[derive(serde::Deserialize)]
struct OpenInput { title: String }

// The #[lingxia::native(...)] attribute macro generates a sibling fn
// `pick_document_host()` that returns the host-entry registration value.
// You do NOT write `pick_document_host` yourself — it is macro-generated.
#[lingxia::native("editor.pickDocument")]
async fn pick_document(
    app: Arc<lingxia::LxApp>,            // optional, FIRST when present
    input: OpenInput,                     // optional JSON payload
    cancel: lingxia::host::HostCancel,    // optional, LAST when present
) -> lingxia::Result<String> {
    let path = lingxia::app::state_file_for(&app, &format!("{}.md", input.title))?;
    Ok(path.to_string_lossy().into_owned())
}

struct AppHostAddon;
impl lingxia::HostAddon for AppHostAddon {
    fn install_host_apis(&self) {
        // pick_document_host() is generated by the macro on `pick_document`.
        lingxia::host::register_host_entry(pick_document_host());
    }
    fn start_services(&self) {}
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
#[unsafe(no_mangle)]
pub extern "C" fn lingxia_register_host_addon() {
    lingxia::register_host_addon(Box::new(AppHostAddon));
}
```

**Call from the View** (after a native build regenerates the client):

```ts
import { native } from '@lingxia/native';   // module form, React/Vue
const path = await native.editor.pickDocument({ title: 'notes' });
```

Streams, channels, JS AppService extensions, facade modules (`lingxia::app/task/file/media/update`), platform FFI for Android/Harmony, generated-client paths: [`./native/development.md`](./native/development.md).

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
| Surfaces, panels, activators (macOS) | `lingxia.yaml` | `ui` |
| Bundled lxapp sources | folder + `resources.bundles` | `lingxia.yaml` |

---

## Top pitfalls (one per layer — full lists in the sub-files)

**LxApp** — see [`./lxapp/guide.md` → Common Pitfalls](./lxapp/guide.md#common-pitfalls):

- Generating `.tsx` + `.vue` + `.html` for one page. Pick **one** view framework per project.
- `fetch()` to a host not in `security.network.trustedDomains` fails silently.

**Host app** — see [`./app/project.md` → Common Pitfalls](./app/project.md#common-pitfalls):

- Editing generated `app.json` / `ui.json` instead of `lingxia.yaml`. They're regenerated every build.
- `homeAppId` not matching any `resources.bundles[].appId` — build fails or the wrong app launches.

**Native Rust** — see [`./native/development.md`](./native/development.md):

- Importing internal crates (`lingxia_logic`, `rong`) directly. Use `lingxia::*` facades.
- `app: Arc<LxApp>` not first, or `HostCancel` not last, in a `#[lingxia::native]` signature.

---

## Pre-ship checklist

**LxApp:**

- [ ] `lxapp.json` lists every page; `appId` set; `version` bumped if shipping.
- [ ] `security.network.trustedDomains` covers every external host (exact host names, no scheme/port/path).
- [ ] One view-framework file per page.
- [ ] Public actions typed in `PageActions`; private helpers prefixed `_`.
- [ ] `lingxia dev` runs cleanly.

**Host app:**

- [ ] `lingxia.yaml` validates: every required platform section present; `homeAppId` resolvable.
- [ ] `features.appService` matches the embedded lxapp's logic mode.
- [ ] All native routes return `lingxia::Result<T>` with `Serialize` outputs.
- [ ] `HostAddon` registers every route and extension.
- [ ] FFI exports for each target platform present.
- [ ] `lingxia doctor` passes; `lingxia dev` boots on a real/simulated device.
