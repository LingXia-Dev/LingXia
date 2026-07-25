# Logic-side `lx.*` API

Every lxapp Logic file (`pages/*/index.ts`) runs against a global `lx` object exposing platform capabilities — navigation, file I/O, media, networking, device info, UI chrome, and more.

**`@lingxia/types` is the authoritative `lx.*` surface.** It declares the exact signature, option shapes, result types, **and JSDoc contract** (platform support, restrictions, task semantics) of every method, globally. This page does **not** re-list any of that — a mirror only drifts. It covers only what the declarations can't tell you: runtime globals, typing wiring, and cross-cutting behavior.

For page mechanics (`data`, `setData`, lifecycle), see [`./guide.md`](./guide.md). For bridge details (stream, channel), see [`./bridge.md`](./bridge.md).

---

## Install typing

`@lingxia/types` declares everything globally — no `import` needed in Logic files.

```bash
npm install --save-dev @lingxia/types@<lingxia-version>
```

Match the version to your `lingxia` CLI — the CLI, the skill, and `@lingxia/types` release in lockstep. Then in `tsconfig.json`:

```json
{
  "compilerOptions": {
    "types": ["@lingxia/types"]
  }
}
```

That's it. `lx`, `Page`, `App`, `getApp`, and `getCurrentPages` are now globally typed:

```ts
// pages/home/index.ts — no imports needed for the lx surface
Page({
  data: { name: '' },
  async pickFile() {
    const res = await lx.chooseFile({ count: 1 });
    this.setData({ name: res.files[0]?.name ?? '' });
  },
});
```

The scaffold type-checks each layer against its real runtime: Logic (`tsconfig.logic.json`) uses `lib: ["ES2020"]` + `@lingxia/types/logic-globals` — web-standard globals but **no** browser DOM. View (`tsconfig.view.json`) keeps the full DOM. The root `tsconfig.json` references both, so editors route each file automatically.

---

## Finding a method or type

- Type `lx.` in your editor — completion lists every member, and hovering shows the JSDoc contract.
- Or open `node_modules/@lingxia/types/dist/generated/logic.d.ts` → the `interface Lx { … }` declarations.
- Or grep a hunch: `grep -rn "scanCode" node_modules/@lingxia/types`.
- Every option/result type is importable from the package root — `import type { ScanCodeResult } from '@lingxia/types'` — when typing your own helpers.

**Nested namespaces** (the rest of `lx.*` is flat): `lx.env` (abstract `lx://` paths), `lx.app` (host-app control), `lx.tray` (desktop status item), and `lx.shell.activators` (home-lxapp-owned desktop activator declarations).

---

## Standard Web APIs (built-in globals)

The Logic JS runtime is **not** a stripped-down sandbox. It's the Rong runtime with the standard Web API set wired in — `fetch`, `setTimeout`, `URL`, `console`, all global, no import.

**The authoritative portable list is `@lingxia/types/logic-globals`** (`node_modules/@lingxia/types/dist/logic-globals.d.ts`, which includes the generated `dist/generated/logic-web.d.ts` runtime profile). It is generated against the modules available outside development, so application code neither over- nor under-promises. By group: timers, HTTP (`fetch` family), encoding, URL, streams, compression, events & abort, `DOMException`, and `console`. If a name is not declared there, it is not portable application functionality — `document`, `window`, `localStorage`, and Node globals like `Buffer` correctly error.

**OS processes are an explicit product capability.** Set `capabilities.process: true` on a macOS/Windows host and add `"process"` to the home lxapp's `security.privileges`; it then receives `Rong.spawn`, `Rong.spawnSync`, and `Rong.$` in both development and release builds. Add `@lingxia/types/process` to the Logic tsconfig `types` list. It is a narrow declaration-only entry—do not install the full `@rongjs/rong` types.

**Gating.** `fetch` (and `WebSocket`) is constrained by the lxapp's `security.network.trustedDomains` in `lxapp.json`. A request to a host not on that list **silently fails** — see [LxApp guide → Security Policy](./guide.md#security-policy). For HTTP use this global `fetch`, **not** the `lx.*` networking calls (those are WiFi / network-info only).

---

## Cross-cutting behavior

Facts that span the whole surface, so no single method's JSDoc carries them:

- **Unsupported platforms no-op; they don't throw.** A capability some platforms lack — a *cosmetic / optional* one, e.g. the desktop tray, with no mobile equivalent — is a **silent no-op** there, so portable code calls it unconditionally: no platform guards, no `try/catch`. A method that **returns a result** you depend on, or whose failure is a genuine bug (permissions, bad arguments), **throws** instead. Result-bearing platform-exclusive capabilities are **optional members** that are simply absent off-platform (e.g. `lx.app.autostart?`) — presence *is* the support check. Each method's JSDoc states its platform support.
- **Storage is async and values are untyped.** Await `get` / `set` / `delete` / `clear` / `list` / `info`; cast values returned by `get` at the call site. For path-based data use `FileManager` (`lx://` storage-class paths — see [`../reference/file-lifecycle.md`](../reference/file-lifecycle.md)).
- **Two distinct update flows.** `lx.getUpdateManager()` updates the **lxapp bundle** (every lxapp, callback model); `lx.app.checkUpdate()` updates the **host app shell** (home lxapp only, task model). Don't mix them.
- **The tab bar is declared, not built.** The `setTabBar*` / `showTabBar` / `hideTabBar` family mutates a tab bar configured statically in `lxapp.json` — see [LxApp guide → Tab bar navigation](./guide.md#tab-bar-navigation).
- **Shell activators are app-owned; Pins are user-owned.** Only the home lxapp may atomically declare `lx.shell.activators`. Every runtime-scoped entry supplies a stable id, `label`, bundle-relative `icon`, and `onActivate`; the shell only invokes that callback, so opening an lxapp/native surface is explicit app code (usually `lx.openSurface(...)`). Redeclare activators each Logic launch. Sidebar Pins (lxapps and websites, mixed order, eight maximum) are intentionally not exposed to Logic.
- **Float interaction is explicit when needed.** `lx.openSurface({ page, as: 'float', interaction: { closeButton, dismiss, modal } })` controls the native circular close button, outside-click dismissal, and blocking independently. Defaults remain no button, `tapOutside`, and non-modal.

---

## Calling native Rust routes from Logic

`lx.*` is the JS-only surface. Host-app-specific routes defined in Rust with `#[lingxia::native(...)]` are **not** on `lx` — you call them from the **View** layer via the CLI-generated client at `@lingxia/native`.

If you need cross-page business helpers callable from Logic as `lx.<yourNamespace>.foo(...)`, define a `lingxia::js` extension in the host Rust crate — see [`../native/development.md` → JS AppService Extensions](../native/development.md#js-appservice-extensions).

The `.d.ts` (with its JSDoc) is the source of truth; this page is just orientation.
