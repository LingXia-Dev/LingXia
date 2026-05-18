# Logic-side `lx.*` API

Every lxapp Logic file (`pages/*/index.ts`) runs against a global `lx` object that exposes platform capabilities — navigation, file I/O, media, networking, device info, UI chrome, and more. The shape of `lx` (and the `Page({})` / `App({})` globals) is published as TypeScript declarations in **`@lingxia/types`**.

This page is the routing index for `lx.*`. It tells you which namespace owns which capability and how to wire up typing.

For page mechanics (`data`, `setData`, lifecycle), see [`./guide.md`](./guide.md).
For bridge details (stream, channel), see [`./bridge.md`](./bridge.md).

---

## Install typing

`@lingxia/types` declares everything globally — no `import` needed in Logic files.

```bash
npm install --save-dev @lingxia/types@<lingxia-version>
```

Match the version to your `lingxia` CLI. The skill's `package.json` version, the CLI version, and `@lingxia/types` are released in lockstep.

Then in `tsconfig.json`:

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

---

## Globals

| Global | Purpose |
|---|---|
| `lx` | The full platform API surface (see namespaces below). |
| `Page(config)` | Define a page. `config.data` initializes state; public methods become bridge-callable actions. |
| `App(config)` | Define the app-wide lifecycle (`onLaunch`, `onShow`, `onHide`, …). |
| `getApp<T>()` | Return the current `AppInstance` or `null`. |
| `getCurrentPages<T>()` | Stack of currently mounted pages, top of stack last. |

---

## Standard Web APIs (built-in globals)

The Logic JS runtime is **not** a stripped-down sandbox. It's the [Rong](https://github.com/) runtime with the standard Web API set wired in, so you write Logic code the same way you'd write any modern JS — `fetch`, `setTimeout`, `URL`, `console`, all available globally with no import.

Available everywhere (every lxapp Logic file):

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
| **Console** | `console.log`, `console.info`, `console.warn`, `console.error`, `console.debug`, `console.trace` |

```ts
// Standard fetch, just works.
const res = await fetch('https://api.example.com/items', {
  method: 'POST',
  headers: { 'content-type': 'application/json' },
  body: JSON.stringify({ id: 42 }),
  signal: AbortSignal.timeout(5000),
});
const data = await res.json();
```

**Gating.** `fetch` (and `WebSocket` if you reach for it) is constrained by the lxapp's `security.network.trustedDomains` in `lxapp.json`. A request to a host that isn't on that list silently fails — see [LxApp guide → Security Policy](./guide.md#security-policy).

### AppService-only extras

When the host has `features.appService: true`, the wider **AppService scope** (the JS service that hosts all per-page Logic contexts) adds:

- **`cron`** — scheduled-task module for app-lifetime jobs. Useful for periodic checks (heartbeat, badge refresh, polling) that should run as long as the lxapp is loaded, not tied to a single page lifecycle.
- **App-wide `storage`** — durable key/value at the lxapp scope.

The Rong-supplied cron surface isn't yet declared in `@lingxia/types`; check the runtime's current globals (e.g. via `console.log(globalThis)` from `App({}).onLaunch`) for the exact API, or look at `crates/lingxia-lxapp/Cargo.toml`'s `rong_modules` feature list for what's enabled. App-scope key/value is available via the page-level `lx.getStorage()` (see [Storage](#storage-keyvalue)) — values written there persist across pages and app launches.

---

## `lx` surface — by capability

The `lx` object is flat (no nested namespaces in code), but the surface logically groups into the capabilities below. The "Sub-module" column shows where the types come from in `@lingxia/types/<sub>` if you want to import option/result types directly.

### Navigation (in-app and cross-lxapp)

Sub-module: `@lingxia/types/navigator`, `@lingxia/types/app`

```ts
lx.navigateTo(options)        // push a page
lx.navigateBack(options)      // pop
lx.redirectTo(options)        // replace current page
lx.switchTab(options)         // switch to tab page
lx.reLaunch(options)          // restart at a new page

lx.navigateToLxApp(options)   // jump to another lxapp
lx.navigateBackLxApp()        // return to caller lxapp
```

For declarative navigation in markup, prefer the `LxNavigator` component — see [`./components.md` → LxNavigator](./components.md#lxnavigator).

### Page chrome / UI

Sub-module: `@lingxia/types/ui`

```ts
lx.setNavigationBarTitle({ title })
lx.setNavigationBarColor(options)
lx.hideHomeButton()

lx.showToast(options) / lx.hideToast()
lx.showModal(options) -> Promise<ModalResult>
lx.showActionSheet(options) -> Promise<ActionSheetResult>

lx.showTabBar() / hideTabBar()
lx.setTabBarStyle(options)
lx.setTabBarItem(options)
lx.setTabBarBadge(options) / removeTabBarBadge(options)
lx.showTabBarRedDot(options) / hideTabBarRedDot(options)

lx.startPullDownRefresh() / stopPullDownRefresh()
lx.getCapsuleRect() -> Promise<CapsuleRect>
```

> The `setTabBar*` family mutates an already-declared tab bar — the tab bar itself is configured statically in `lxapp.json`. For the declarative shape, switching tabs (`lx.switchTab`), and the rule that tab bar is an lxapp-internal concept (unrelated to host App UI surfaces), see [LxApp guide → Tab bar navigation](./guide.md#tab-bar-navigation).

### Media (images, video, scanning, preview)

Sub-module: `@lingxia/types/media`

```ts
lx.chooseMedia(options?) -> Promise<ChosenMediaEntry[]>
lx.previewMedia(options) -> PreviewMediaHandle    // returns handle synchronously
lx.saveImageToPhotosAlbum(options)
lx.saveVideoToPhotosAlbum(options)

lx.getImageInfo(options) -> Promise<ImageInfo>
lx.compressImage(options) -> Promise<CompressImageResult>
lx.compressVideo(options) -> Promise<CompressVideoResult>
lx.getVideoInfo(options) -> Promise<VideoInfo>
lx.extractVideoThumbnail(options) -> Promise<ExtractVideoThumbnailResult>

lx.scanCode(options?) -> Promise<ScanCodeResult>

// Imperative video player control (pair with a <LxVideo id=…>)
lx.createVideoContext(componentId) -> VideoContext
```

`previewMedia` returns a handle, not a promise. Await `handle.completed` for the final session result; subscribe to `handle.presented` to know when the first pixel hits the screen.

### File and transfer

Sub-module: `@lingxia/types/file`, `@lingxia/types/transfer`

Top-level file operations (open in a native viewer, pick a file, transfer):

```ts
lx.openFile(options)                                  // mode: 'auto' | 'review'
lx.chooseFile(options?) -> Promise<ChooseFileResult>
lx.chooseDirectory(options?) -> Promise<ChooseDirectoryResult>

lx.downloadFile(options) -> DownloadTask
lx.uploadFile(options) -> UploadTask
```

**`DownloadTask` / `UploadTask`** are both `PromiseLike` **and** `AsyncIterable` — `await` them for the final result, or iterate for live progress events:

```ts
// Simple form — await for the final result
const result = await lx.downloadFile({ url: 'https://cdn.example.com/big.zip', filePath: 'lingxia://cache/big.zip' });

// Progress form — iterate
const task = lx.downloadFile({ url, filePath });
for await (const event of task) {
  // event.progress: 0..100 ; event.totalBytesWritten / event.totalBytesExpected
  setData({ percent: event.progress });
}
const final = await task.wait();
```

Control methods (all `Promise<void>`):

| Method | DownloadTask | UploadTask | Notes |
|---|:---:|:---:|---|
| `pause()` | ✓ | — | Pauses bytes flowing; `resume()` continues. Not all backends support it. |
| `resume()` | ✓ | — | |
| `cancel()` | ✓ | ✓ | Aborts the underlying transfer; the task promise rejects. |
| `abort()` | ✓ | — | Alias for `cancel()` — matches browser / mini-program naming. |
| `wait()` | ✓ | ✓ | Awaits the final result. Equivalent to `await task`. Use when you stopped iterating partway. |

`return()` on the iterator (e.g. `break` inside `for await`) stops iteration **without** cancelling the underlying transfer — call `cancel()` explicitly if you want to abort.

For low-level read/write/stat/list/mkdir/copy/rename/remove, get a `FileManager`:

```ts
const fm = lx.getFileManager();
```

`FileManager` methods (every method returns a `Promise<…>` — async-only):

| Method | Signature | Notes |
|---|---|---|
| `exists` | `({ path }) → boolean` | |
| `stat` | `({ path }) → FileStats` | `{ isFile, isDirectory, isSymlink, size, lastModifiedTime?, lastAccessedTime?, createTime? }` |
| `readDir` | `({ path }) → AsyncIterableIterator<DirEntry>` | `for await (const entry of await fm.readDir(...))` — `DirEntry = { name, isFile, isDirectory, isSymlink }` |
| `mkdir` | `({ path, recursive? })` | `recursive: true` for `mkdir -p` behavior |
| `readFile` | `({ filePath, encoding: 'utf8' \| 'base64' }) → { data: string }`<br>or `({ filePath }) → { data: ArrayBuffer }` | Pass `encoding` for text, omit for binary. Three overloads share one impl. |
| `writeFile` | `({ filePath, data: string, encoding?: 'utf8' \| 'base64', overwrite? })`<br>or `({ filePath, data: ArrayBuffer \| ArrayBufferView, overwrite? })` | `overwrite` defaults to `false` — write fails if target exists. |
| `copyFile` | `({ srcPath, destPath, overwrite? })` | |
| `rename` | `({ oldPath, newPath, overwrite? })` | Use for moves too. |
| `remove` | `({ path, recursive? })` | `recursive: true` removes directories with content. |

`path` / `filePath` strings use the storage-class scheme described in [`../reference/file-lifecycle.md`](../reference/file-lifecycle.md) (e.g. `lingxia://temp/...`, `lingxia://cache/...`). That doc also covers when each storage class is auto-cleaned and how `downloadFile` paths interact with the lifecycle.

### Device / system

Sub-module: `@lingxia/types/device`, `@lingxia/types/system`

```ts
lx.getDeviceInfo() -> DeviceInfo
lx.getScreenInfo() -> ScreenInfo
lx.getSystemSetting() -> SystemSettingInfo
lx.vibrateShort() / vibrateLong()
lx.makePhoneCall(options)
lx.openURL(options)                                   // hand off to OS browser/app
```

### Networking

```ts
lx.startWifi() / stopWifi()
lx.connectWifi(options) -> Promise<void>
lx.getWifiList() -> Promise<WifiInfo[]>
lx.getConnectedWifi() -> Promise<WifiInfo>
lx.onWifiConnected(cb) / offWifiConnected(cb?)

lx.getNetworkInfo() -> Promise<NetworkInfo>
lx.onNetworkChange(cb) / offNetworkChange(cb?)
```

Network requests from Logic must respect `security.network.trustedDomains` in `lxapp.json` — see [`./guide.md` → Security Policy](./guide.md#security-policy). The `lx.*` networking calls above are for WiFi / network-info — for actual **HTTP requests**, use the standard global `fetch` (see [Standard Web APIs](#standard-web-apis-built-in-globals)).

### Display / orientation

```ts
lx.setDeviceOrientation(orientation)
lx.onDeviceOrientationChange(cb) / offDeviceOrientationChange(cb?)
```

### Location

```ts
lx.getLocation(options?) -> Promise<LocationInfo>
```

### Keyboard / hardware input

```ts
lx.onKeyDown(cb) / offKeyDown(cb?)
lx.onKeyUp(cb) / offKeyUp(cb?)
```

Useful on TV/desktop hosts where physical-key events matter.

### Storage (key/value)

Sub-module: `@lingxia/types/storage`

```ts
interface Storage {
  get(key: string): unknown;          // synchronous, untyped — cast as needed
  set(key: string, value: unknown): void;
  remove(key: string): void;
  clear(): void;                       // wipes the whole namespace
  keys(): string[];                    // every key currently stored
  has(key: string): boolean;
  size(): number;                      // count of stored entries (not byte size)
}

const storage = lx.getStorage();
storage.set('lastSeenTip', 3);
if (!storage.has('userId')) await prompt();
for (const key of storage.keys()) console.log(key, storage.get(key));
```

All methods are **synchronous**. For larger or path-based storage, use the `FileManager` from `lx.getFileManager()` instead.

### `lx.app` — host-app metadata and control (`HostAppApi`)

```ts
interface HostAppApi {
  readonly envVersion: 'developer' | 'preview' | 'release';
  getBaseInfo(): AppBaseInfo;
  checkUpdate(): Promise<HostAppUpdateCheckResult>;
  exit(): void;
}

interface AppBaseInfo {
  language: string;
  productName: string;
  version: string;
  SDKVersion: string;
}
```

**`envVersion`** — synchronous, fixed at app boot. Use to branch behavior:

```ts
if (lx.app.envVersion === 'developer') enableVerboseLogging();
```

Configured via `lingxia build/dev/package --env <env>` and `lingxia.yaml` (`app.lingxiaServer`, `app.packageIdSuffix`) — see [App Project → Environment versions](../app/project.md#environment-versions). **Not** the same as the navigator-module `envVersion` (`'develop' | 'preview' | 'release'`) used in cross-lxapp URLs.

**`getBaseInfo()`** — language, product/host versions:

```ts
const info = lx.app.getBaseInfo();
console.log(info.productName, info.version, info.SDKVersion);
```

**`checkUpdate()`** — **home-lxapp only**. Non-home lxapps get a permission error. Calling this **opts the host app into custom update handling** for the process; LingXia's built-in update UI is suppressed afterward. Returns either `{ hasUpdate: false }` or `{ hasUpdate: true, update: HostAppUpdateInfo }` where `update.apply()` returns a `HostAppUpdateTask` (PromiseLike + AsyncIterable of progress events):

```ts
const result = await lx.app.checkUpdate();
if (result.hasUpdate) {
  const task = result.update.apply();
  for await (const event of task) {
    // event.state: 'downloading' | 'downloaded' | 'installRequested' | 'failed'
    // 'downloading' carries optional downloadedBytes and progress
  }
}
```

Direct package handoff is supported on Android and macOS today. Other platforms reject `apply()` with an unsupported-operation error — use `update.version` and `update.releaseNotes` to point users at the app store.

**`exit()`** — terminate the host app immediately. **No confirmation dialog** — show one yourself with `lx.showModal(...)` first if needed.

### `lx.surface` — dynamic surfaces (`SurfaceApi`)

This is the JS API for **opening surfaces dynamically at runtime** — distinct from the **declarative** `lingxia.yaml.ui.surfaces` in host config. Use this when you need to pop an overlay or open a secondary window from Logic.

```ts
interface SurfaceApi {
  open(options: SurfaceOpenOptions): Promise<Surface>;
}
```

**Two kinds:**

| `kind` | Where | Notes |
|---|---|---|
| `'overlay'` | **all platforms** | A webview composited on top of the host activity. Use `position` + `size` for placement. |
| `'window'` | **macOS only** | A new native window. Mobile platforms reject with `surface_open_failed`. |

**Target — open a page from this lxapp, a page from another path, or an external URL:**

```ts
// open a registered page of this lxapp
await lx.surface.open({ kind: 'overlay', page: 'detail', query: { id: '42' } });

// open by raw path
await lx.surface.open({ kind: 'overlay', path: 'pages/detail/index', position: 'bottom', size: { height: '60%' } });

// open an external HTTPS URL (subject to trustedDomains)
await lx.surface.open({ kind: 'overlay', url: 'https://example.com/help' });

// open a window on macOS (rejected on mobile)
await lx.surface.open({ kind: 'window', page: 'settings', size: { width: 600, height: 400 } });
```

`OverlaySurfaceOptions.position`: `'center' | 'bottom' | 'left' | 'right' | 'top'`. `OverlaySurfaceSizeValue`: a positive number (absolute) or a `"N%"` template string (0 < N ≤ 100).

**Surface returned**: a stable handle for the opened page. The opener and the opened page can talk to each other:

```ts
interface Surface {
  readonly id: string;
  readonly kind: 'overlay' | 'window';
  readonly visible: boolean;     // tracks native show/hide events
  readonly alive: boolean;       // false after close()

  postMessage(message: unknown): void;
  onMessage(handler: (message: unknown) => void): () => void;  // returns unsubscribe

  show(): Promise<void>;         // idempotent
  hide(): Promise<void>;         // hides without destroying — page state survives
  close(): Promise<void>;        // tears down the page instance

  onShow(handler: (event: SurfaceVisibilityEvent) => void): () => void;
  onHide(handler: (event: SurfaceVisibilityEvent) => void): () => void;
  onClose(handler: (event: SurfaceClosedEvent) => void): () => void;
}
```

**Page side** — the opened page receives the opener's port via `lx.navigateTo`-style messaging or via the surface API. URL surfaces have no page-side receiver (you can't `postMessage` from arbitrary external HTML back into Logic).

**`hide()` vs `close()`**: hide preserves the page's JS state, scroll position, and form input — a subsequent `show()` restores everything. `close()` destroys it; `onClose` fires; further `show()` / `hide()` calls reject.

### `lx.env` — runtime paths (`LxEnv`)

```ts
interface LxEnv {
  USER_DATA_PATH: string;   // durable per-lxapp data root
  USER_CACHE_PATH: string;  // evictable per-lxapp cache root
}
```

Use these as roots when building `FileManager` paths or `downloadFile` targets. The storage-class model (when each path is auto-cleaned, size caps) is detailed in [`../reference/file-lifecycle.md`](../reference/file-lifecycle.md).

```ts
const profilePath = `${lx.env.USER_DATA_PATH}/profile.json`;
```

### `lx.getLxAppInfo()` — manifest at runtime

Returns `LxAppInfo` with the lxapp's `appId`, `version`, `appName`, and other manifest fields. Useful for showing the user "you're on v1.2.3" or branching by `appId` when the same Logic is reused across embedded lxapps.

### Updates (lxapp self-update — distinct from `lx.app.checkUpdate`)

Sub-module: `@lingxia/types/update`. The **lxapp** update manager — different from `lx.app.checkUpdate()` (host app update). This one handles the runtime swapping a newer lxapp bundle into place:

```ts
interface UpdateManager {
  applyUpdate(): void;
  onUpdateReady(callback: (info: UpdateReadyInfo) => void): void;
  onUpdateFailed(callback: (info: UpdateFailedInfo) => void): void;
}

const manager = lx.getUpdateManager();
manager.onUpdateReady(({ version, isForceUpdate, channel }) => {
  // a new lxapp bundle is staged; channel: 'release' | 'preview' | 'developer'
  manager.applyUpdate();   // restart into the new bundle
});
manager.onUpdateFailed(({ error, version }) => { /* log/report */ });
```

---

## Calling native Rust routes from Logic

`lx.*` is the JS-only surface. For host-app-specific routes defined in Rust with `#[lingxia::native(...)]`, you call them from the **View** layer via the CLI-generated client at `@lingxia/native`. They are not on `lx`.

If you need cross-page business helpers callable from Logic as `lx.<yourNamespace>.foo(...)`, define a `lingxia::js` extension in the host Rust crate — see [`../native/development.md` → JS AppService Extensions](../native/development.md#js-appservice-extensions).

---

## Quick reference — finding a method

If you can't remember the exact name:

1. **Open `@lingxia/types/dist/index.d.ts`** in your editor and search the `interface Lx { … }` block.
2. **Or grep `node_modules/@lingxia/types`** for a keyword (`grep -r "scanCode" node_modules/@lingxia/types`).
3. The submodule structure (`@lingxia/types/media`, `/file`, `/ui`, …) groups option/result types — useful when typing your own helpers.

The `lx` interface is the authoritative source. This page is a routing index — when in doubt, read the `.d.ts`.
