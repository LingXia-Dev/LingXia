# hello-host-rust — Shape C: host app with **Rust Logic, no JS runtime**

A native host app whose embedded home lxapp is **HTML-only and has no JS Logic layer**. All state and behavior live in the Rust crate, exposed to the View through `#[lingxia::native]` routes. The JS AppService runtime is not even compiled into the host (`features.appService: false`).

Pick this shape for: menu-bar utilities, system-tray tools, Rust-heavy desktop apps where the UI is mostly a thin view over native logic.

## Contrast with [`../hello-host-js/`](../hello-host-js/README.md)

Same host shell, opposite Logic side:

| | `hello-host-js` (Shape B) | `hello-host-rust` (Shape C) |
|---|---|---|
| `features.appService` | `true` | **`false`** |
| `lxapp.json.logic` | (omitted, defaults true) | **`false`** |
| Logic layer | `Page({ data, …actions })` in `index.ts` | **Rust** `#[lingxia::native]` routes |
| View | React `index.tsx` + `useLxPage` | **plain `index.html` + `<script>`** |
| Bridge surface | `actions.foo()` from `useLxPage()` | `window.native.foo(...)` (CLI-generated browser global) |
| JS AppService runtime in host binary | ✅ shipped | **❌ not compiled in** (smaller binary) |
| Cargo dep on `lingxia` | optional | **required** |

The host project shape (`lingxia.yaml` `ui`, `resources.bundles`, FFI export) is identical. Only the lxapp's authoring model and the `features.appService` switch differ.

## Files

- `lingxia.yaml` — `features.appService: false`; everything else like hello-host-js.
- `Cargo.toml` — host Rust crate declaration.
- `src/lib.rs` — `#[lingxia::native("greet.hello")]` route + `HostAddon` + platform FFI export. (Same shape as a Shape B route — the route signature doesn't change between B and C.)
- `home/lxapp.json` — has `"logic": false` flag; the runtime won't try to spin up a JS Logic layer.
- `home/pages/home/index.html` — the entire UI. `<script src="lingxia://lxapp/.lingxia/native.js">` auto-loads the CLI-generated client; the click handler calls `window.native.greet.hello(...)`.
- `home/pages/home/index.json` — page-level config (navigation bar). The same shape works for both B and C.

## What to study

- **`features.appService: false`** disables the JS runtime. A logic-enabled lxapp (no `"logic": false`) paired with this is **rejected at startup** — flip both together.
- **`"logic": false`** in `lxapp.json` is the lxapp-side declaration that there is no `Page({})` Logic. The CLI build won't look for `pages/*/index.ts` files; the page is the `index.html`.
- **No `@lingxia/react` / `@lingxia/vue` / `@lingxia/html`** dependency. Those packages all assume the AppService bridge (`Page({})` shape). Use plain DOM APIs.
- **`window.native.*`** is the CLI-generated **browser-global** client (different output format from the `@lingxia/native` module client used by React/Vue Shape B). The HTML view just `<script>`-loads it; no bundler involved.
- **The Rust route is identical** to a Shape B route — `#[lingxia::native]`, `HostAddon::install_host_apis`, the macro-generated `<fn>_host()` companion. Only which side owns Logic differs.

## Cross-references in this skill

- Full native Rust development (streams, channels, facades, `lingxia::js` extensions, Android/Harmony FFI): [`../../native/development.md`](../../native/development.md)
- `lingxia.yaml` full reference including the `features` section: [`../../app/project.md`](../../app/project.md)
- Sibling Shape B example: [`../hello-host-js/`](../hello-host-js/README.md)
