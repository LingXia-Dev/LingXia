# Logic-side `lx.*` API

Every lxapp Logic file (`pages/*/index.ts`) runs against a global `lx` object exposing platform capabilities — navigation, file I/O, media, networking, device info, UI chrome, and more.

**`@lingxia/types` is the authoritative `lx.*` surface.** It declares the exact signatures, option shapes, and result types of every method, globally. This page does **not** re-list them — that mirror only drifts. Instead it covers the three things the type declarations can't tell you:

1. **What standard web globals exist** in the Logic runtime (`fetch`, `setTimeout`, …) — runtime facts, not on `lx`.
2. **A capability map** — which method names exist in each group, so you know what to reach for; read the `.d.ts` for the signature.
3. **Behavioral notes / gotchas** that the signatures don't spell out.

For exact signatures, install the types (below) and let TypeScript drive, or open `node_modules/@lingxia/types/dist/index.d.ts` and read the `interface Lx { … }` block.

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

That's it. `lx`, `Page`, `App`, `getApp`, and `getCurrentPages` are now globally typed.

```ts
// pages/home/index.ts — no imports needed for the lx surface
Page({
  data: { name: '' },

  async onLoad() {
    const info = lx.getDeviceInfo();   // typed
    lx.setNavigationBarTitle({ title: `Hello, ${info.model}` });
  },

  async pickFile() {
    const res = await lx.chooseFile({ count: 1 });
    this.setData({ name: res.files[0]?.name ?? '' });
  },
});
```

| Global | Purpose |
|---|---|
| `lx` | The full platform API surface (capability map below). |
| `Page(config)` | Define a page. `config.data` initializes state; public methods become bridge-callable actions. |
| `App(config)` | Define the app-wide lifecycle (`onLaunch`, `onShow`, `onHide`, …). |
| `getApp<T>()` | Return the current `AppInstance` or `null`. |
| `getCurrentPages<T>()` | Stack of currently mounted pages, top of stack last. |

---

## Standard Web APIs (built-in globals)

The Logic JS runtime is **not** a stripped-down sandbox. It's the Rong runtime with the standard Web API set wired in, so you write Logic the same way you'd write any modern JS — `fetch`, `setTimeout`, `URL`, `console`, all global, no import.

| Group | Globals provided |
|---|---|
| **Timers** | `setTimeout`, `setInterval`, `clearTimeout`, `clearInterval`, `queueMicrotask` |
| **HTTP** | `fetch`, `Request`, `Response`, `Headers`, `FormData` |
| **Encoding** | `TextEncoder`, `TextDecoder`, `btoa`, `atob` |
| **URL** | `URL`, `URLSearchParams` |
| **Streams** | `ReadableStream`, `WritableStream`, `TransformStream`, `ByteLengthQueuingStrategy`, `CountQueuingStrategy` |
| **Events / abort** | `Event`, `EventTarget`, `CustomEvent`, `AbortController`, `AbortSignal` |
| **Exception** | `DOMException` |
| **Buffer** | `Buffer` (Node-style — handy for binary work alongside `ArrayBuffer`) |
| **Console** | `console.log` / `.info` / `.warn` / `.error` / `.debug` / `.trace` |

```ts
const res = await fetch('https://api.example.com/items', {
  method: 'POST',
  headers: { 'content-type': 'application/json' },
  body: JSON.stringify({ id: 42 }),
  signal: AbortSignal.timeout(5000),
});
```

**Gating.** `fetch` (and `WebSocket`) is constrained by the lxapp's `security.network.trustedDomains` in `lxapp.json`. A request to a host not on that list **silently fails** — see [LxApp guide → Security Policy](./guide.md#security-policy). For HTTP use this global `fetch`, **not** the `lx.*` networking calls (those are WiFi / network-info only).

### AppService-only extras

When the host has `features.appService: true`, the wider **AppService scope** (the JS service hosting all per-page Logic contexts) adds:

- **`cron`** — scheduled-task module for app-lifetime jobs (heartbeat, badge refresh, polling) that should run as long as the lxapp is loaded, not tied to one page's lifecycle.
- **App-wide `storage`** — durable key/value at the lxapp scope, via `lx.getStorage()` (values persist across pages and launches).

The Rong `cron` surface **isn't declared in `@lingxia/types`** yet — inspect the runtime's live globals (`console.log(globalThis)` from `App({}).onLaunch`) for the exact API.

---

## `lx` capability map

The `lx` object is mostly flat, with a few nested namespaces (`lx.app`, `lx.tray`, `lx.env`); the rest is grouped logically below. These are method **names for discovery** — open `@lingxia/types` for the signatures and option/result types (the "Types" column shows the sub-module). The non-obvious behavior is in [Behavioral notes](#behavioral-notes) below.

| Capability | Methods | Types |
|---|---|---|
| **Navigation** | `navigateTo` `navigateBack` `redirectTo` `switchTab` `reLaunch` `navigateToLxApp` `navigateBackLxApp` | `navigator`, `app` |
| **Media** | `chooseMedia` `previewMedia` `saveImageToPhotosAlbum` `saveVideoToPhotosAlbum` `getImageInfo` `compressImage` `compressVideo` `getVideoInfo` `extractVideoThumbnail` `scanCode` `createVideoContext` | `media` |
| **File & transfer** | `openFile` `chooseFile` `chooseDirectory` `downloadFile` `uploadFile` `getFileManager` | `file`, `transfer` |
| **Device / system** | `getDeviceInfo` `getScreenInfo` `getSystemSetting` `vibrateShort` `vibrateLong` `makePhoneCall` `openExternal` | `device`, `system` |
| **Networking** | `startWifi` `stopWifi` `connectWifi` `getWifiList` `getConnectedWifi` `onWifiConnected`/`off…` `getNetworkInfo` `onNetworkChange`/`off…` | — |
| **Display / orientation** | `setDeviceOrientation` `onDeviceOrientationChange`/`off…` | — |
| **Location** | `getLocation` | — |
| **Keyboard / hardware** | `onKeyDown`/`off…` `onKeyUp`/`off…` (TV/desktop hosts) | — |
| **Storage (k/v)** | `getStorage` → `{ get, set, remove, clear, keys, has, size }` | `storage` |
| **Host app** (`lx.app`) | `envVersion` `getBaseInfo` `checkUpdate` `exit` `screenshot` `setBadge` `autostart?.isEnabled` `autostart?.setEnabled` | — |
| **Tray** (`lx.tray`, desktop) | `setIcon` `setTitle` `setBadge` `setMenu` `onClick` `show` `hide` | — |
| **Surfaces** | `openSurface` `onSurfaceContext` | `ui` |
| **Runtime info** | `env` (`USER_DATA_PATH` / `USER_CACHE_PATH`) `getLxAppInfo` `getUpdateManager` | `update` |

### Page chrome / UI

`lx.setNavigationBarTitle` / `setNavigationBarColor` / `hideHomeButton`, `showToast` / `hideToast`, `showModal`, `showActionSheet`, `startPullDownRefresh` / `stopPullDownRefresh`, `getCapsuleRect`, and the `setTabBar*` family. Signatures and option shapes are in `@lingxia/types/ui`.

The `setTabBar*` / `showTabBar` / `hideTabBar` family **mutates an already-declared tab bar** — the tab bar itself is configured statically in `lxapp.json`. For the declarative shape, `lx.switchTab`, and why the tab bar is lxapp-internal (unrelated to host surfaces), see [LxApp guide → Tab bar navigation](./guide.md#tab-bar-navigation).

### File and transfer

`FileManager` (`lx.getFileManager()`) gives low-level `exists` / `stat` / `readDir` / `mkdir` / `readFile` / `writeFile` / `copyFile` / `rename` / `remove` — **every method is async (`Promise`)**. Path strings use the `lx://` storage-class scheme (`lx://temp/…`, `lx://userdata/…`, `lx://usercache/…`); bundle-relative paths (`images/a.png`) resolve against the lxapp bundle. Storage classes, auto-cleanup, size caps, and how `downloadFile` paths interact with them are in [`../reference/file-lifecycle.md`](../reference/file-lifecycle.md). Signatures: `@lingxia/types/file` and `/transfer`.

---

## Behavioral notes

The signatures don't capture these — get them wrong and the code compiles but misbehaves:

- **Unsupported platforms no-op; they don't throw.** The `lx.*` surface is one shared, cross-platform type. A capability that some platforms lack — a *cosmetic / optional* one, e.g. the desktop tray (`lx.tray.*`), which has no equivalent on mobile — is a **silent no-op** there, never a thrown error. So portable code can call it unconditionally; no `if (platform === …)` guards, no `try/catch`. This holds only for cosmetic capabilities: a method that **returns a result** you depend on, or whose failure is a genuine bug (permissions, bad arguments), **throws** instead. A method's JSDoc states its platform support (e.g. `lx.tray.*` is *desktop only*; `lx.app.setBadge` is cross-platform including the mobile app icon). When adding a new `lx.*` API, follow this: no-op off-platform for cosmetic chrome, throw for result-bearing or correctness-critical calls. A third shape exists for **result-bearing platform-exclusive** capabilities: an **optional member** that is simply absent off-platform (e.g. `lx.app.autostart?` — macOS/Windows only, and only with `capabilities.autostart` declared in `lingxia.yaml`). Presence is the support check (`if (lx.app.autostart)`), so a stub never has to fake a result.
- **Storage is synchronous and untyped.** `get(key)` returns `unknown` with no generic and is **not** a promise — never `await` it. Cast at the call site: `lx.getStorage().get('userId') as string | undefined`. For larger or path-based storage use `FileManager`.
- **Transfer/transcode tasks are `PromiseLike` *and* `AsyncIterable`.** `DownloadTask`, `UploadTask`, `CompressVideoTask`: `await` the task for the final result, or `for await` it for live progress. `break`-ing out of the loop **stops iteration without cancelling** the transfer — call `task.cancel()` to actually abort. `pause()`/`resume()` and the `abort()` alias are download-only.
- **`previewMedia` returns a handle synchronously**, not a promise — attach `onChange` listeners before the first event; `await handle.presented` / `handle.completed` for lifecycle.
- **`downloadFile` defaults to `destination: 'app'`.** Use `destination: 'downloads'` to save into the user's Downloads dir and surface it in the built-in downloads page — that requires `"downloads"` in `lxapp.json` `security.privileges`. The `filePath` is then a sanitized filename hint only.
- **`lx.app.checkUpdate()`, `screenshot()`, and `autostart.*` are home-lxapp only** (others get a permission error). `checkUpdate()` also **opts the whole app into custom update handling** — the built-in update UI is suppressed afterward. `apply()` (direct package handoff) works on Android/macOS; elsewhere point users at the store.
- **Two distinct `envVersion`s.** `lx.app.envVersion` (`'developer' | 'preview' | 'release'`, fixed at boot, from `lingxia … --env`) is **not** the navigator-module `envVersion` (`'develop' | 'preview' | 'release'`) used in cross-lxapp URLs.
- **Two distinct update flows.** `lx.getUpdateManager()` updates the **lxapp bundle** (every lxapp, callback model: `onUpdateReady` → `applyUpdate()`); `lx.app.checkUpdate()` updates the **host app shell** (home lxapp only, Promise + `update.apply()` task). Don't mix them.
- **`lx.openSurface(spec)`** opens dynamic / host-declared surfaces. `{ page, as: 'aside'|'float'|'window' }` returns a full `Surface` (with `postMessage` / `onMessage` / `show` / `hide` / `close`); `{ surface }` (declared in `lingxia.yaml`) returns a smaller `SurfaceHandle`; `{ url }` opens a browser tab (no handle), `{ url, as: 'aside' }` a browser aside. `hide()` preserves the page's JS state; `close()` tears it down. URL surfaces have no page-side receiver.
- **`lx.onSurfaceContext(handler)`** reports the adaptive `SurfaceContext` (`sizeClass: 'compact' | 'medium' | 'expanded'`, with hysteresis) so an lxapp can self-adapt (e.g. column count). The handler fires on each change; returns an unsubscribe fn.

---

## Calling native Rust routes from Logic

`lx.*` is the JS-only surface. Host-app-specific routes defined in Rust with `#[lingxia::native(...)]` are **not** on `lx` — you call them from the **View** layer via the CLI-generated client at `@lingxia/native`.

If you need cross-page business helpers callable from Logic as `lx.<yourNamespace>.foo(...)`, define a `lingxia::js` extension in the host Rust crate — see [`../native/development.md` → JS AppService Extensions](../native/development.md#js-appservice-extensions).

---

## Can't remember a method name?

1. Open `@lingxia/types/dist/index.d.ts` and search the `interface Lx { … }` block.
2. Or grep: `grep -r "scanCode" node_modules/@lingxia/types`.
3. The sub-module layout (`@lingxia/types/media`, `/file`, `/ui`, …) groups option/result types — useful when typing your own helpers.

The `.d.ts` is the source of truth; this page is just orientation.
