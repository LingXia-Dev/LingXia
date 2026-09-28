# Lxapp pages

How to write lxapp pages: layout, the Logic/View split, events, `App({})`, and
page chrome.

## Scaffold

```bash
lingxia new my-lxapp -t lxapp -y
```

## Layout

```text
my-lxapp/
├── lxapp.json
├── lxapp.config.ts
├── package.json
├── pages/
│   └── home/
│       ├── index.tsx   # View  — runs in WebView (React or Vue)
│       ├── index.ts    # Logic — runs in native JS runtime
│       └── index.json  # Page config (navigation bar, style)
├── public/
└── shared/
```

- `lxapp.json` — `appId`, `appName`, `version`, `minRuntime`, `pages`. `appName`
  is only a fallback; the app registry's name wins. It declares no network
  hosts or privileges: those are [host grants](../native/permissions.md),
  unrestricted by default.
- `minRuntime` — the lowest host SDK that may open the package; `lingxia new`
  writes it and `lingxia upgrade` raises it. An older host refuses the
  package; detect that with `hostUpgradeRequired(error)` from
  `@lingxia/types/error` and ask the user to update the host app.
- `lxapp.config.ts` — build config (view tooling, aliases, `staticDirs`).

### Static assets

`public/` and `assets/` are copied to `dist/` as-is. Declare others in
`staticDirs`; each must exist, and paths are preserved
(`view/info-panel.js` → `dist/view/info-panel.js`). Nothing is discovered by
scanning sources.

```ts
export default {
  staticDirs: ['public', 'view', 'assets'],
};
```

### Build

- `lingxia build` builds into `dist/`; `lingxia build --release --package`
  produces the publish archive.
- A View reaching for `lx.*`, or calling an action `Page({})` never defined,
  fails the build. Other findings are warnings: the artifact is written, the
  code is still wrong.

## Logic — `Page({})`

`Page` is a runtime global; do not import it.

```ts
// pages/home/index.ts
Page({
  data: {
    count: 0,
    message: "Hello",
  },

  onLoad: function (options) {
    // Called when page is created. `options` contains URL query params.
    console.log("query:", options);
  },

  onShow: function () {
    // Called every time the page becomes visible.
  },

  // Action functions — callable from View
  increment: function () {
    this.setData({ count: this.data.count + 1 });
  },

  updateMessage: function (params) {
    // params is whatever the View passes
    this.setData({ message: params?.text || "" });
  },
});
```

| API | Description |
| --- | --- |
| `this.data` | Current state, read-only. Change it with `setData()` / `setPath()`. |
| `this.setData(patch)` | Merge top-level keys into `data` and replicate to View. Nested writes: `setPath` (checked), `setDataPath` (unchecked). |
| `await this.flush()` | Resolves once every `setData` so far reached the View; rejects if the page unloads first or a write was discarded. |
| `this.signal` | `AbortSignal` aborted as the page unloads, before `onUnload`. Pass it to work the page starts: `fetch(url, { signal: this.signal })`. |
| `this.yourMethod()` | Any other member is a page method. A name differing from a hook only in case (`onload`) is rejected. |

### Lifecycle

| Hook | When it fires |
| --- | --- |
| `onLoad(options)` | Entering the page, with its query. |
| `onShow()` | Becoming visible, on entry and every return. |
| `onReady()` | The document finished rendering, after the first `onShow`. |
| `onHide()` | Covered by another page, or the lxapp went to the background. |
| `onUnload()` | Left the stack (`navigateBack`, `redirectTo`, a `switchTab` that drops it). |

- **Leaving unloads.** Entering again starts a fresh instance: `data` is back
  to its defaults and the View is a new document. Keep what must survive in
  `lx.getStorage()` or `App({})`.
- **Hidden keeps state.** A covered or backgrounded page keeps `data`. A
  desktop host may discard a hidden lxapp's inactive tab WebView: `data` stays,
  and the next show brings `onShow` and a new `onReady`.
- `redirectTo` onto the current page keeps the instance and calls `onLoad`
  again.
- Every `navigateTo` is its own instance, so `detail?id=1 → detail?id=2`
  stacks. Tab pages are singletons. Rejections carry `error.data.reason`:
  `"duplicate_route"` or `"stack_full"` (ten pages).
- `await lx.navigateBack()` pops one; `lx.navigateBack({ delta: 2 })` pops more.

### What resets

Module scope (outside `Page({})`) is evaluated once per app session and is
shared by every live instance of the route:

```ts
let hits = 0;          // module scope: shared by every instance, never resets

Page({
  data: { count: 0 },  // instance scope: fresh on every entry
  onLoad() {
    hits += 1;         // counts entries across ALL instances
  },
});
```

| State | Container | Lifetime |
| --- | --- | --- |
| One entry (form input, timers, request handles) | `data` / `this.xxx` | The page instance |
| Every instance of the route (constants, caches) | Module scope | Until the lxapp restarts |
| Must survive restarts | `lx.getStorage()` | Persistent |

Never keep per-entry state in module scope.

### Private helpers

A method starting with `_` stays private; every other method is a public
action the View can call (`onCheckout` too). Name the `data` shape by
annotating `data`, not with a type argument on `Page`, so `this` keeps your
methods:

```ts
Page({
  data: { total: 0 } as PageData,

  _calculateTotal(items) {
    return items.reduce((sum, item) => sum + item.price, 0);
  },

  checkout(params) {
    this.setData({ total: this._calculateTotal(params?.items ?? []) });
  },
});
```

## View

A View is a React or Vue component, or an HTML module entry. It renders `data`
and calls `actions`; it never mutates `data`.

The page mounts once its first state (the `Page({ data })` defaults) has
arrived, so `data` is whole from the first render. Type `PageData` and
`PageActions` fields as **required**; use `?:` only for a field filled later.
Never write `actions.foo?.()`. If Logic fails to deliver state, the page shows
a panel naming itself.

| Hook | Returns | Changes |
|---|---|---|
| `useLxPage<PageData, PageActions>()` | `{ data, actions }` — this page's Logic state and public methods | on every `setData` |
| `useLxHost()` | [host facts](#host-facts) | only when a field changes |

Any component may call either. Geometry is CSS, not a hook:
[page chrome CSS](#page-chrome-css).

`data` is typed `DeepReadonly<PageData>` (Vue hands out a readonly proxy; in a
dev session a write throws). Keep what the user edits as View state and submit
it with an action; Logic's `setData` brings the result back:

```tsx
const { data, actions } = useLxPage<PageData, PageActions>();
const [draft, setDraft] = useState(data.message);   // Vue: ref(data.message)
<input value={draft} onChange={(e) => setDraft(e.target.value)} />
<button onClick={() => actions.updateMessage({ text: draft })}>Save</button>
```

### React

```tsx
// pages/home/index.tsx
import { useLxHost, useLxPage } from '@lingxia/react';

type PageData = {
  count: number;
  message: string;
};

type PageActions = {
  increment: () => void;
  updateMessage: (params: { text: string }) => void;
};

export default function HomePage() {
  const { data, actions } = useLxPage<PageData, PageActions>();
  const { formFactor } = useLxHost();

  return (
    <div data-form-factor={formFactor}>
      <p>Count: {data.count}</p>
      <p>{data.message}</p>
      <button onClick={() => actions.increment()}>+1</button>
      <button onClick={() => actions.updateMessage({ text: 'World' })}>
        Update
      </button>
    </div>
  );
}
```

### Vue

The same with `@lingxia/vue` in `<script setup lang="ts">`: `const { data,
actions } = useLxPage<PageData, PageActions>()`, `const host = useLxHost()`,
then `{{ data.count }}`, `host.sizeClass`, `@click="actions.increment()"`.
`data` is deep-reactive, so destructuring it stays live; never `v-model` it —
bind a `ref` draft instead. Read `host.sizeClass` rather than destructuring `host`.

### HTML

```ts
// pages/home/entry.ts — index.html loads it with <script type="module">
import { getPage, pageReady, subscribePage } from '@lingxia/html';

const render = () => draw(getPage<PageData, PageActions>().data);
document.getElementById('inc-btn')?.addEventListener('click', () => getPage<PageData, PageActions>().actions.increment());

// Plain HTML has no mount to gate: wait for the first state, then follow it.
void pageReady().then(() => {
  render();
  subscribePage(render);
});
```

`pageReady()` shows a startup fault after 10 seconds and still mounts when
state arrives. Pass `{ timeoutMs }` only when the caller handles rejection.
Read `useLxPage()` during component setup/render; pass its actions into helpers.

### Host facts

```ts
const { sizeClass, aside, displayLanguage, formFactor, os, runner } = useLxHost();
```

`getHost()` / `subscribeHost(cb)` in `@lingxia/html`;
`window.LingXiaBridge.host.get()` / `.subscribe(cb)` in a page that bundles
nothing. It never changes while a window is dragged.

- `sizeClass` (`compact` | `regular`) and `aside` — how much room:
  [Adaptive UI](./adaptive-ui.md).
- `formFactor` (`mobile` | `desktop`) — which machine. Branch on it for what a
  phone must not show. A narrow desktop window is `desktop`; a tablet or
  unfolded fold is `mobile`.
- `os` — `'iOS' | 'macOS' | 'Android' | 'Windows' | 'Harmony' | 'unknown'`, for
  OS-specific features only. `runner` is `true` in the dev Runner.
- `displayLanguage` — the product language; the runtime also sets
  `<html lang>` and `<html dir>`. See [Product settings](./lx-api.md#product-settings).

`formFactor`, `os`, and `runner` are fixed for a page's life. A Runner phone
frame reports `mobile` while `os` names the desktop; switching form factor
re-serves the page. Logic reads the OS with `lx.device.getDeviceInfo()`.

## Events

Use framework syntax (`onX` in React, `@event` in Vue). A handler that is an
`actions.*` function is delivered straight to Logic; a local View function gets
a DOM event. Native component callbacks and their payload shapes:
[Components](./components.md#callback-shapes).

### `lx.on*` subscriptions

Every `lx.on*` call returns its unsubscribe function, the only way to cancel
it. Keep it on the page instance (never in `data`) and call it in `onUnload`:

```ts
Page({
  data: { online: true },

  onLoad() {
    this._offNetwork = lx.onNetworkChange((info) => {
      this.setData({ online: info.isConnected });
    });
  },

  onUnload() {
    this._offNetwork?.();
    this._offNetwork = null;
  },
});
```

The same applies to `onWifiConnected`, `onDeviceOrientationChange`,
`onKeyDown`, `onKeyUp`, `lx.surface.watchContext`, `onUpdateReady`,
`onUpdateFailed`, and a surface handle's `onMessage` / `onShow` / `onHide` /
`onClose`. A route can be open more than once, so a leak multiplies per
instance. Work started with `this.signal` needs no teardown; ignore its
`AbortError`.

## Action shapes

| Logic method shape | Use from View | Typical use |
| --- | --- | --- |
| function / async function | `actions.foo(...)` | buttons, navigation, one-shot work |
| async generator | `useLxStream(actions.foo, ...)` | progress, streaming output |
| channel handler | `useLxChannel(actions.foo, ...)` | long-lived two-way sessions |

The runtime routes by shape. Details: [Bridge](./bridge.md).

## `App({})`

The optional lxapp-wide singleton, in one root file (conventionally `app.ts`),
created once at boot and shared by every page:

```ts
// app.ts
interface AppGlobals {
  userId: string;
  theme: 'light' | 'dark';
}

App({
  globalData: <AppGlobals>{
    userId: '',
    theme: 'light',
  },

  async onLaunch(options) {
    // Once per worker. AppLaunchOptions: path?, query?, scene?, url?, referrerInfo?
    // Cold AppLink: scene 8003 + the original `url` here. referrerInfo when
    // opened by another lxapp.
    const stored = await lx.getStorage().get<string>('userId');
    if (stored) this.globalData.userId = stored;
  },

  onShow(args) {
    // Foreground. source: host|lxapp. reason: foreground|background|screenshot|open|close|switch_back|switch_away.
    // Warm AppLink: scene 8003 + url + query. Cold onShow is visibility only (8003 already went to onLaunch).
  },

  onHide(args) {
    // The lxapp is being backgrounded. Same AppLifecycleEventArgs shape.
  },

  onUserCaptureScreen() {
    // The user took a screenshot while this lxapp was active.
  },
});
```

```ts
// pages/profile/index.ts
Page({
  data: { userId: '' },
  onLoad() {
    const app = getApp<AppInstance & { globalData: AppGlobals }>();
    if (app) this.setData({ userId: app.globalData.userId });
  },
});
```

- `globalData` is not reactive; push changes to a page with `setData`.
- Cold start: `App.onLaunch` → `App.onShow` → `Page.onLoad` → `Page.onShow`.
  Foregrounding: `App.onShow` → top page's `onShow`.
- `getCurrentPages()` returns the stack, top last.

## Tab bar and page chrome

Page chrome is the native UI around a View: navigation bar with capsule, tab
bar, appearance. The tab bar is lxapp-internal (`lxapp.json`), unrelated to
host `surfaces`.

### Tab bar

```json
{
  "appId": "my-app",
  "version": "0.1.0",
  "pages": [
    { "name": "home",    "path": "pages/home/index" },
    { "name": "profile", "path": "pages/profile/index" }
  ],
  "tabBar": {
    "presentation": "standard",
    "style": {
      "foregroundColor": "#999999",
      "selectedForegroundColor": "#1677ff",
      "backgroundColor": "#ffffff"
    },
    "items": [
      {
        "text":     "Home",
        "page":     "home",
        "iconPath": "public/home.png"
      },
      {
        "text":     "Profile",
        "page":     "profile",
        "iconPath": "public/profile.png"
      }
    ]
  }
}
```

- 2 to 10 `items`; each `page` is a `pages[].name`. The first is selected.
- `presentation`: `"standard"` (View ends above the bar) or `"immersive"` (View
  extends behind it; omit `backgroundColor` and `dividerColor`).
- Style keys are optional. Mobile uses them as declared (no OS dark mode by
  itself); desktop draws the sidebar from the host `theme` and ignores
  `backgroundColor`. `lx.tabBar.update()` cannot change style.
- A phone shows the first four items plus **More**; order items by use.
  Desktop lists all in the sidebar.
- `"showOn": ["mobile"]` or `["desktop"]` limits an item to one form factor.
  Indices stay as declared, and each host needs at least two items.

### Icons

One icon per item. SVG is a template glyph tinted by the style colours; raster
PNG/JPEG/WebP keeps its colours and is centre-cropped to a square. In
`lxapp.json` `iconPath` is project-relative (usually `public/`). At runtime it
may also be an `lx://temp`, `lx://usercache`, or `lx://userdata` path; network
URLs, `file:` URLs, `..`, and absolute paths are rejected. Download remote art
first, and re-download `lx://temp` art after each Logic launch:

```ts
const { uri } = await lx.downloadFile({ url: brand.logoUrl }).result;
await lx.tabBar.update({
  items: [{ index: 0, text: brand.shortName, iconPath: uri }],
});
```

`iconPath: null` restores the declared icon.

### Switching tabs

`navigateTo` and `redirectTo` reject tab pages; use `lx.switchTab`. Navigation
always takes a page name ([lx-api](./lx-api.md#find-a-method-or-type)).

```ts
lx.switchTab({ page: 'profile' }); // page name from lxapp.json
```

### Navigation bar

Each page's `index.json`:

```json
{
  "navigationStyle": "default",
  "navigationBar": {
    "title": "Profile",
    "style": {
      "backgroundColor": "#ffffff",
      "foregroundColor": "#111111"
    }
  }
}
```

All keys are optional. `navigationStyle: "custom"` draws no native bar; on
mobile the capsule stays over the page's header.

### Runtime updates

`lx.tabBar.update()` changes a declared tab bar (it rejects without one);
`lx.navigationBar.update()` patches the current page's bar. Each call is one
transaction: `null` resets a field, omitted fields stay, an invalid patch
applies nothing.

```ts
await lx.tabBar.update({ items: [{ index: 1, text: 'Inbox', badge: '3' }] });
await lx.tabBar.update({ items: [{ index: 1, text: null, badge: null }] });
await lx.tabBar.update({ visibility: 'hidden' });   // or 'auto'
await lx.navigationBar.update({ title: 'Account' });
await lx.navigationBar.update({ style: null, homeButton: 'auto' });
```

### Appearance

Light/dark is a [product setting](./lx-api.md#product-settings). An lxapp whose
UI works in only one scheme declares it in `lxapp.json`:

```json
{ "appearance": "dark" }
```

The runtime sets `color-scheme` and `data-theme="light|dark"` on `<html>`. Key
theme CSS off `[data-theme]`, with a `prefers-color-scheme` fallback for first
paint.

### Page chrome CSS

A `standard` tab bar shortens the View. For an `immersive` one, and for the
capsule, use the CSS variables; never hard-code platform heights:

```css
.page-scroll {
  padding-bottom: var(--lx-page-chrome-bottom-inset);
}

.floating-action {
  bottom: calc(16px + var(--lx-page-chrome-bottom-inset));
}

/* Apply this only to controls in the capsule's top band, not the whole page. */
.page-header {
  padding-inline-end: var(--lx-page-chrome-capsule-inline-end-inset);
}
```

```css
.page-header {
  padding-top: calc(var(--lx-page-chrome-capsule-bottom) + 12px);
}
```

- `--lx-page-chrome-capsule-{top,right,bottom,left,width,height}` are `0px`
  without a capsule. `--lx-page-chrome-top-inset` covers a
  [full-chrome window](./adaptive-ui.md#edge-to-edge-windows).
- In Runner, `env(safe-area-inset-*)` alone does not describe simulated chrome;
  use these variables.
- For JavaScript placement, `window.lxPageChrome.layout` holds the snapshot and
  `lxpagechromechange` reports changes. Logic has no capsule API.

## Pitfalls

- More than one view framework in a project; match the existing pages.
- Mutating `data` in the View, or keeping business state in `useState`/`ref`.
- Touching the DOM from Logic; Logic has no DOM.
- Expecting `_`-prefixed helpers or hooks to be actions.
- Expecting `App({}).globalData` changes to re-render.
- `navigateTo` / `redirectTo` on a tab page.
- Dropping the function an `lx.on*` call returns.
- Using `<video>`, `<audio>`, or `new Audio()`: see
  [LxVideo](./components.md#lxvideo).
- Missing `@lingxia/types`: see [Install typing](./lx-api.md#install-typing).
