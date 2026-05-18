# hello-lxapp — Shape A: a standalone lxapp

A page-based mini-app. Runs in any LingXia host (e.g. macOS Runner via `lingxia dev`). No native shell, no Rust.

This is the smallest meaningful starter. **Read it; don't copy-paste it whole** — scaffold a real project with `lingxia new my-app -t lxapp -y` and refer back here for shape.

Files:

- `lxapp.json` — appId, version, pages list, security policy
- `lxapp.config.ts` — build config (view framework, static dirs, …)
- `package.json` — runtime + dev deps you actually need
- `pages/home/`
  - `index.ts` — **Logic** (`Page({…})`) — state and actions, runs in the native JS runtime
  - `index.tsx` — **View** (React + `useLxPage`) — renders UI, runs in the WebView
  - `index.json` — page-level config (navigation bar, etc.)

What to study:

- How **Logic** initializes `data: {…}` and exposes public methods to View.
- How `_privateHelper` is hidden from View by the `_` prefix convention.
- How **View** types `PageData` / `PageActions` with **required** fields (the runtime guarantees first-paint data and wired-up actions).
- How `lxapp.json.security.network.trustedDomains` is set to `[]` to deny all external `fetch()` calls — flip to actual hostnames before running anything that talks to a server.

Cross-references in this skill:

- Full page authoring: [`../../lxapp/guide.md`](../../lxapp/guide.md)
- Bridge mechanics (`setData`, stream, channel): [`../../lxapp/bridge.md`](../../lxapp/bridge.md)
- Native components (`LxInput`, `LxVideo`, …): [`../../lxapp/components.md`](../../lxapp/components.md)
- Logic-side `lx.*` API: [`../../lxapp/lx-api.md`](../../lxapp/lx-api.md)
