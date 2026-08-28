# LxApp Development Guide

This guide covers how to write lxapp pages — project layout, the View + Logic architecture, data flow, event handling, and native component integration.

Companion pages in this skill:

- [Adaptive UI](./adaptive-ui.md) - surface size classes, dynamic View
  selection, and Runner device-frame testing.
- [Components](./components.md) — `LxPicker`, `LxVideo`, `LxMediaSwiper`, `LxNavigator` — capabilities, callback shapes, and imperative control (attribute lists live in the exported `@lingxia/elements` types); text input is plain `<input>` / `<textarea>`.
- [Logic runtime and typings](./lx-api.md) — runtime globals and typing wiring; signatures and behavior live in the generated `@lingxia/types` declarations.
- [Bridge Guide](./bridge.md) — `setData`, stream, channel mechanics in depth.
- [App Project](../app/project.md) — host app setup (`lingxia.yaml`, adaptive `surfaces`).

---

## Create an LxApp

```bash
lingxia new my-lxapp -t lxapp -y
```

This creates a standalone lxapp project. To create a native host app, use
`-t native-app` instead. Hosts normally embed a control lxapp, while a desktop
terminal/browser main may use native control with no bundled lxapp (see [App
Project](../app/project.md)).

---

## Project Layout

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

`lxapp.json` holds runtime metadata (`appId`, `appName`, `version`, `pages` — `name` is a legacy alias for `appName`; write `appName` in new projects) and the security policy; `lxapp.config.ts` holds build config (view tooling, aliases, static asset directories).

### Static assets

Use `staticDirs` in `lxapp.config.ts` to declare root-level directories that should be copied into `dist/` as-is for `html`, `react`, and `vue`.

```ts
export default {
  staticDirs: ['public', 'view', 'assets'],
};
```

Rules:

- `public/` and `assets/` are the default static directories. If the project root contains either of them, LingXia copies it to `dist/` even when `staticDirs` is omitted.
- Additional directories must be declared explicitly in `staticDirs`.
- Explicit `staticDirs` entries must exist at the project root. LingXia treats missing configured directories as build errors.
- Paths are preserved. For example, `view/info-panel.js` becomes `dist/view/info-panel.js`.
- LingXia does not scan HTML, manifest files, or arbitrary source strings to discover static assets.

### Security Policy

`lxapp.json` must declare the lxapp security policy. New projects include an explicit deny-by-default policy:

```json
{
  "security": {
    "network": {
      "trustedDomains": []
    },
    "privileges": []
  }
}
```

Rules:

- `security.network.trustedDomains: []` denies all remote hosts.
- Use exact host names, for example `api.example.com` or `cdn.example.com`.
- Do not include scheme, path, or port. `https://api.example.com`, `api.example.com/path`, and `api.example.com:443` are invalid.
- Use `"*"` only when the lxapp intentionally allows all remote hosts, for example during local experiments.
- Do not combine `"*"` with host names. It is an explicit allow-all policy.
- Domain matching is host-only and normalized to lowercase.
- The policy is a host allowlist. It does not distinguish `http` and `https`; prefer HTTPS in production.
- The policy applies to Logic network requests, `lx.downloadFile`, `lx.uploadFile`, and WebView HTTPS resources resolved by LingXia.
- `security.privileges` is for host-defined capabilities such as `downloads` (`lx.downloadFile({ destination: "downloads" })`). Ordinary APIs like media, camera, or location remain guarded by host and platform permission flows.

Example:

```json
{
  "security": {
    "network": {
      "trustedDomains": ["api.example.com", "cdn.example.com"]
    },
    "privileges": ["downloads"]
  }
}
```

### Native client

Views call Rust native APIs through a generated Native client. LxApp projects do not configure Rust source paths. Native host builds generate the client from the native Rust crate's `build.rs` with `lingxia-native-codegen`.

The CLI passes the canonical output path through `LINGXIA_NATIVE_CLIENT_OUT` during native cargo builds. React/Vue projects get `.lingxia/native.ts` and import it through `@lingxia/native`; HTML projects get `.lingxia/native.js`, which the build copies explicitly into `dist/.lingxia/native.js`.

### Build

- `lingxia build` builds page assets and runtime artifacts into `dist/`.
- `lingxia build --release --package` produces package archive for publish.

The build enforces the View/Logic boundary: a View reaching for `lx.*`, or
calling an action `Page({})` never defined, fails it. Everything else it reports
is a warning — the artifact is written, the code is still wrong.

---

## Page Architecture

Every page is split into two layers that communicate through a bridge:

```
┌─────────────────────────┐     setData()      ┌──────────────────────────┐
│       View (WebView)    │ ◄────────────────── │   Logic (Native Runtime) │
│  React/Vue + useLxPage  │ ────────────────────► Page({}) instance        │
│                         │   bridge functions   │                          │
└─────────────────────────┘                     └──────────────────────────┘
```

**View** renders UI. **Logic** owns state and business operations. Logic pushes state to View via `setData()`, and View calls Logic functions through auto-generated bridge bindings.

---

## Logic Layer — `Page({})`

The Logic file exports a `Page({})` call. The `Page` function is provided globally by the runtime — you don't import it.

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

### Key concepts

| API | Description |
| --- | --- |
| `this.data` | Current page state. Read-only — use `setData()` to change. |
| `this.setData(patch)` | Merge `patch` into `data` and replicate to View. Triggers re-render. |
| `onLoad(options)` | Lifecycle — page created. `options` are URL query params. |
| `onShow()` | Lifecycle — page becomes visible (including back-navigation). |
| `lx.*` | Global platform APIs (e.g. `lx.navigationBar.update()`, `lx.createVideoContext()`). |

### Page lifecycle

| Hook | When it fires |
| --- | --- |
| `onLoad(options)` | Entering the page. `options` carries the query params. |
| `onShow()` | Becoming visible — on entry, and again every time it comes back. |
| `onReady()` | The page's document has finished rendering. |
| `onHide()` | Another page covered it, or the lxapp went to the background. |
| `onUnload()` | The page left the stack (`lx.navigateBack`, `lx.redirectTo`, a `lx.switchTab` that drops it). |

**Leaving a page ends that page instance.** After `onUnload`, entering the same
page again starts a fresh one: `data` is back to what `Page({ data })` declares,
and the View is a newly rendered document — a dialog left open, a scroll
position, or anything else the page accumulated is gone. Put whatever must
survive in `lx.getStorage()` or in `App({})`.

`onHide` is not `onUnload`. A page covered by another page, or backgrounded with
the lxapp, keeps its instance and its `data`, and simply gets `onShow` again on
return. `lx.switchTab` only hides the tab page it leaves — but any page pushed
on top of a tab is dropped from the stack and unloaded like a `navigateBack`.

`lx.redirectTo` onto the page you are already on is the one exception: the page
never leaves the screen, so it keeps its instance and simply gets `onLoad` again
with the new query.

A route can appear on the page stack more than once: every `lx.navigateTo`
entry is its own page instance with its own data and document, so drill-down
flows like `detail?id=1 → detail?id=2` stack naturally and unwind one entry at
a time. Tab pages are the exception — each tab is a warm singleton, so it can
hold only one stack slot at a time. Navigation rejections carry stable
metadata in `error.data`: `reason` is `"duplicate_route"` (a tab page already
on the stack) or `"stack_full"` (the ten-page limit), with the attempted
`operation` and resolved `target`.

`lx.navigateBack()` pops one page by default; pass `{ delta }` to pop more. It
returns a promise like the other navigation APIs, resolving once the revealed
page's WebView is ready:

```ts
await lx.navigateBack();
await lx.navigateBack({ delta: 2 });
```

### What resets, what survives

Leaving a page ends its instance: `data` and the rendered document come back
fresh on the next entry. **Module-level variables do not reset.** A page's
logic file is a JavaScript module, evaluated once per app session; everything
declared outside `Page({})` lives in module scope, survives every page
entry and exit, and is shared by ALL live instances of that route at the same
time — with duplicate routes, two stacked `detail` pages read and write the
same module variable.

```ts
let hits = 0;          // module scope: shared by every instance, never resets

Page({
  data: { count: 0 },  // instance scope: fresh on every entry
  onLoad() {
    hits += 1;         // counts entries across ALL instances
  },
});
```

Pick the container by lifetime:

| State | Container | Lifetime |
| --- | --- | --- |
| Belongs to one entry (form input, counters, request handles, timers) | `data` / `this.xxx` | The page instance — fresh per entry |
| Shared by every instance of the route (constants, memo caches, a singleton connection) | Module scope (outside `Page({})`) | The app's Logic runtime — until the lxapp restarts |
| Must survive restarts | `lx.getStorage()` | Persistent |

Never keep per-entry state (request sequence numbers, timer handles,
in-flight flags) in module scope: it silently couples same-route instances.

### Private helpers

The leading `_` is the whole rule: a method starting with `_` stays private,
and **every** other method becomes a public action the View can call. Nothing
else is special — a name like `onCheckout` reads as a lifecycle hook but is an
ordinary action, so name actions for what they do:

```ts
Page({
  data: { total: 0 },

  _calculateTotal: function (items) {
    return items.reduce((sum, item) => sum + item.price, 0);
  },

  checkout: function (params) {
    const total = this._calculateTotal(params?.items || []);
    this.setData({ total });
  },
});
```

TypeScript types the page config with an index signature, so a method you add
yourself is `unknown` on `this` — calling `this._calculateTotal(...)` compiles
in JavaScript but not under `strict` TypeScript. Give shared behavior a
module-level function that takes the instance instead:

```ts
function calculateTotal(items: Item[]): number { … }

Page<PageData>({
  data: { total: 0 },
  checkout(params) {
    this.setData({ total: calculateTotal(params?.items ?? []) });
  },
});
```

---

## View Layer

The View file can be a standard React component, a Vue component, or an HTML module entry. The framework packages connect View to the Logic layer and expose:

- `data` — reactive page state replicated from Logic via `setData()`
- `actions` — public functions exported from `Page({})`

### Typing `PageData` and `PageActions`

The runtime guarantees that **(a)** `data` reflects Logic's initial `data: { … }` literal by first paint, and **(b)** every public method on `Page({})` is wired into `actions` during page setup. So in your typed shapes:

- **Required by default.** Fields you declare in `data: { … }` are always present; public methods are always callable. Mark them required.
- **Mark `?:` only when the field is genuinely populated lazily** — for example, a field that starts unset and is filled by `this.setData(…)` after an async fetch in `onLoad`.

Using all-`?` fields is a footgun: it propagates `actions.foo?.()` and `data?.x ?? default` through every component for no reason. Don't do that.

### React

```tsx
// pages/home/index.tsx
import { useLxPage } from '@lingxia/react';

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

  return (
    <div>
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

Identical shape via `@lingxia/vue`: `const { data, actions } = useLxPage<PageData, PageActions>()` in `<script setup lang="ts">`, then bind `{{ data.count }}` / `@click="actions.increment()"` in the template. `lingxia new … ` scaffolds the full file.

### HTML

```ts
// pages/home/entry.ts
import { getActions, subscribe } from '@lingxia/html';

type PageData = {
  count: number;
  message: string;
};

type PageActions = {
  increment: () => void;
  updateMessage: (params: { text: string }) => void;
};

const actions = getActions<PageActions>();
const countEl = document.getElementById('count');
const messageEl = document.getElementById('message');

document.getElementById('inc-btn')?.addEventListener('click', () => {
  actions.increment();
});

subscribe((data: PageData) => {
  if (countEl) countEl.textContent = String(data.count);
  if (messageEl) messageEl.textContent = data.message;
});
```

```html
<!-- pages/home/index.html -->
<script type="module" src="./entry.ts"></script>
```

### What `useLxPage()` returns

```ts
const { data, actions } = useLxPage<PageData, PageActions>();
```

- **`data`** — Reactive page state, updated whenever Logic calls `setData()`. In React this triggers a re-render; in Vue it's a `reactive()` object.
- **`actions`** — All public functions from `Page({})` (except lifecycle hooks and `_`-prefixed methods). Each action is a bridge function that calls through to the Logic layer.

Use typed `PageActions` interfaces so View and Logic stay aligned as your page grows.

---

## Data Flow

State flows **one way**: Logic `setData()` → bridge replication → View `data` re-render. View never mutates `data` directly — it calls Logic actions, which call `setData()`. Full mechanics (JSON Patch replication, batching, stream/channel): [`./bridge.md`](./bridge.md).

---

## Event Handling

LingXia routes component events two ways automatically — a **Logic short path**
(native → Rust → Logic JS) when the handler is an `actions.*` function, and a
**View DOM path** (native → WebView `CustomEvent` → handler) when it's a local
View function. You never choose: use framework-native syntax (`onX` in React,
`@event` in Vue) and the system routes for you.

### Subscribing to `lx.*` events

Every `lx.on*` subscription returns its own unsubscribe function — that returned
closure is the *only* way to cancel it, so a page that subscribes must keep it
and call it. There is no `lx.off*` counterpart: a call that cancelled by
callback could only match by identity, which silently took out another module's
handler registered with the same function.

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

Unsubscribe in `onUnload`, and keep the closure on the page instance rather than
in `data` — `data` crosses the bridge and a function cannot. A route can be open
more than once, so a subscription left behind is leaked once per page instance,
not once per app. Calling the returned function twice is safe.

The same shape covers `onNetworkChange`, `onWifiConnected`,
`onDeviceOrientationChange`, `onKeyDown`, `onKeyUp`, `lx.surface.onContext`,
`onUpdateReady`, `onUpdateFailed`, and the surface handle's `onMessage` /
`onShow` / `onHide` / `onClose`.

### Native component events

LingXia ships native-backed components (`LxPicker`, `LxVideo`, `LxMediaSwiper`, `LxNavigator`) from `@lingxia/react` and `@lingxia/vue` (HTML views use the raw `<lx-*>` tags); text input is a plain `<input>` / `<textarea>`. Handlers use standard framework-native syntax:

**React:**

```tsx
import { useLxPage, LxPicker, LxVideo } from '@lingxia/react';

const { actions } = useLxPage<PageData, PageActions>();

// Input — read the value off the DOM event
<input onInput={(e) => actions.onInputChange({ value: e.currentTarget.value })} />

// Picker — handler receives resolved value directly
<LxPicker
  columns={[['A', 'B', 'C']]}
  onConfirm={(value) => actions.onPickerConfirm({ field: 'choice', value })}
/>

// Video — handler receives raw DOM CustomEvent
<LxVideo src={url} onPlaying={actions.onPlaying} />
```

Vue is the same with `@lingxia/vue` and `@event` syntax (`@confirm`, `@playing`, `@input`).

Callback payloads differ by component — some unwrapped, some raw DOM `CustomEvent`. See [Callback shapes by component](./components.md#callback-shapes-by-component) in [`./components.md`](./components.md) for the per-component table and the full attribute/behavior reference (including imperative `LxVideo` control via `lx.createVideoContext()`).

---

## Action Shapes

From a page author's perspective, public `Page({})` methods come in three useful shapes:

| Logic method shape | Use from View | Typical use |
| --- | --- | --- |
| normal function / async function | `actions.foo(...)` from `useLxPage()` | button actions, navigation, one-shot work |
| async generator | `useLxStream(actions.foo, ...)` | progress, incremental output, chat-style streaming |
| channel-style session | `useLxChannel(actions.foo, ...)` | long-lived bidirectional sessions |

Examples:

- `increment()` and `updateMessage()` stay in the normal `actions` bucket.
- `async *onSend(...)` is a stream action and belongs with `useLxStream()`.
- Session-style logic that stays open over time belongs with `useLxChannel()`.

The runtime inspects the Logic method shape and routes it automatically. Use this guide for page authoring; use [Bridge Guide](./bridge.md) for stream/channel lifecycle, cancellation, and transport details.

---

## App-wide lifecycle — `App({})`

`Page({})` defines a single page; **`App({})`** defines the **lxapp-wide singleton** — created once when the lxapp boots, shared by every page. Use it for app-scope state, cross-page coordination, and lifecycle hooks that fire regardless of which page is on screen.

Like `Page`, `App` is a runtime-provided global. Define it in a single file at the lxapp root (conventionally `app.ts`). It is **optional** — many lxapps don't need it.

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
    // Called once when the lxapp boots.
    // `options`: AppLaunchOptions — { path?, query?, scene?, referrerInfo? }
    //   referrerInfo is populated when this lxapp was opened by another lxapp.
    const stored = await lx.getStorage().get<string>('userId');
    if (stored) this.globalData.userId = stored;
  },

  onShow(args) {
    // Called every time the lxapp comes to the foreground.
    // args: AppLifecycleEventArgs
    //   source: 'host' | 'lxapp'
    //   reason: 'foreground' | 'background' | 'screenshot' | 'open' | 'close' | 'switch_back' | 'switch_away'
  },

  onHide(args) {
    // The lxapp is being backgrounded. Same AppLifecycleEventArgs shape.
  },

  onUserCaptureScreen() {
    // The user took a screenshot while this lxapp was active.
  },
});
```

Read app-scope state from any page with `getApp<T>()`:

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

Notes:

- `globalData` is a plain object. **Mutations are not reactive** — pages don't re-render when you change `app.globalData.x`. To propagate changes into the View, write to a page's `data` via `setData`.
- Lifecycle order on cold start: `App.onLaunch` → `App.onShow` → first page's `Page.onLoad` → `Page.onShow`. On foregrounding: `App.onShow` → top page's `Page.onShow`.
- `getCurrentPages()` returns the active page stack (top of stack last) when you need to coordinate across pages.
- Type declarations for `App`, `AppConfig`, `AppInstance`, `AppLaunchOptions`, `AppLifecycleEventArgs`, `getApp`, `getCurrentPages` come from [`@lingxia/types`](./lx-api.md#install-typing).

---

## Tab bar & Page Chrome

**Page Chrome** is the native UI the host renders around a page's View: the navigation bar with its capsule buttons, the tab bar, and the lxapp's light/dark appearance. It is declared in JSON and mutated at runtime through the `lx.*` namespaces below.

A **tab bar** is a persistent navigation strip — typically at the bottom of the screen — that shows the lxapp's primary pages. Tapping a tab switches the active page **without** push/pop semantics: the tab bar stays visible across all tab pages, and tab pages do not stack on each other.

> **Scope.** Tab bar is an **lxapp-internal navigation concept** declared in `lxapp.json`. It has nothing to do with host surfaces — `lingxia.yaml` `surfaces` live one layer up and describe the native shell (windows, asides, sidebar/tray). A host shell renders an lxapp; that lxapp may have its own tab bar inside.

### Declaring the tab bar in `lxapp.json`

Add a `tabBar` block alongside `pages`:

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
        "pagePath": "pages/home/index",
        "iconPath": "public/home.png"
      },
      {
        "text":     "Profile",
        "pagePath": "pages/profile/index",
        "iconPath": "public/profile.png"
      }
    ]
  }
}
```

All style keys are optional and inherit the host theme. `presentation` is
`"standard"` (the View ends above the bar) or `"immersive"` (the View extends
behind it). An immersive bar must omit `backgroundColor` and `dividerColor`.

Rules:

- `items` holds **2 to 10** entries.
- Every `items[].pagePath` must match a registered page path under `pages[]`.
- `iconPath` is project-relative — usually under `public/`, so the default
  static-assets rule copies it verbatim into `dist/`.
- The first item is initially selected. Placement and dimensions are host-owned.

### Icons

One icon per item, drawn as a **template**: the host tints it with
`foregroundColor` / `selectedForegroundColor` and marks the active tab with a
circle behind it. Ship a monochrome glyph — a multi-colour PNG is flattened to
one colour. There is no second "selected" icon to author.

### More than five tabs

A phone strip fits five slots. Past that the host shows the first four, then a
**More** slot; tapping it opens the rest in a panel above the bar. Desktop and
tablet hosts have the room and list every item in their sidebar instead.

The split is host-owned — nothing to configure, no API to open the panel. What
follows:

- Order the list by use: the first four are the ones always one tap away.
- "More" carries the selected tint while a folded page is active, and shows a
  red dot if any folded item has a badge or dot.
- Only items holding a slot are warmed at launch, so ten tabs start as fast as
  four.

### Switching tabs at runtime

From Logic, use `lx.switchTab(...)`. **`lx.navigateTo` and `lx.redirectTo` do not work on tab pages** — the runtime rejects them with errors like `"redirectTo cannot navigate to a tabBar page"`. Switching is the only way in and out of tabs.

Like the whole JavaScript navigation family, it takes the page **name**
registered in `lxapp.json`. Full routes are runtime implementation details and
are not accepted as API input; there is no `path` or `url` selector.

```ts
lx.switchTab({ page: 'profile' }); // page name from lxapp.json
```

When driving a running app from `lxdev`, use the page name from `lxapp.json` rather than the path:

```bash
lxdev lxapp nav switch-tab profile
```

`lx.navigateBack` still works for popping non-tab pages that were pushed on top of the current tab.

### Declaring the navigation bar in page JSON

Each page's `index.json` declares its navigation bar:

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

`title` and all `style` keys are optional and inherit the host theme.
`navigationStyle: "custom"` renders no native bar; on mobile the floating
capsule buttons remain over the page's own header.

### Updating Page Chrome at runtime

`lx.tabBar.update()` **mutates an already-declared tab bar** — it does not create
or remove tabs. If the lxapp has no `tabBar` in `lxapp.json`, the promise rejects.

```ts
await lx.tabBar.update({
  style: { selectedForegroundColor: '#ff0000' },
  items: [{ index: 1, text: 'Inbox', badge: '3' }],
});
await lx.tabBar.update({ items: [{ index: 1, text: null, badge: null }] });
await lx.tabBar.update({ visibility: 'hidden' });
await lx.tabBar.update({ visibility: 'auto' });
```

`lx.navigationBar.update()` patches the current page's bar the same way:

```ts
await lx.navigationBar.update({ title: 'Account' });
await lx.navigationBar.update({ style: { backgroundColor: '#111827' } });
await lx.navigationBar.update({ style: null, homeButton: 'auto' });
```

Each `update()` is one transaction: `null` resets a field to its declared
value, omitted fields keep their current state, and an invalid patch rejects
without applying anything.

`await lx.appearance.set('auto' | 'light' | 'dark')` sets the lxapp's own
light/dark branch independently of the host shell; the preference persists per
lxapp, and `lx.appearance.get()` synchronously returns it alongside the
resolved branch. The runtime projects the resolved branch into every page as
`color-scheme` plus a `data-theme="light|dark"` attribute on `<html>` — key
theme CSS off `[data-theme]` (with a `prefers-color-scheme` fallback for
no-JS first paint), since platform media queries may lag an in-place switch.

### Laying out under immersive chrome

A `standard` tab bar shortens the View, so page CSS needs no tab-bar inset. An
`immersive` tab bar overlaps the View. Never hard-code its platform height; use
the page-chrome CSS variables for ordinary layout:

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

Use the framework helper when placement needs the exact capsule rectangle or
must react in JavaScript:

```tsx
// React
import { useLxPageChrome } from '@lingxia/react';

const chrome = useLxPageChrome();
const capsule = chrome.capsuleRect;
```

```ts
// Vue
import { computed } from 'vue';
import { useLxPageChrome } from '@lingxia/vue';

const chrome = useLxPageChrome(); // Readonly<Ref<PageChromeLayoutSnapshot>>
const capsule = computed(() => chrome.value.capsuleRect);
```

```ts
// HTML
import {
  getPageChromeLayout,
  subscribePageChromeLayout,
} from '@lingxia/html';

const initial = getPageChromeLayout();
const unsubscribe = subscribePageChromeLayout((next) => {
  // Reposition geometry-dependent UI from next.bottomInset/capsuleRect.
});
```

Snapshots are frozen and revisioned. `window.lxPageChrome.layout` and the
`lxpagechromechange` event remain the low-level View contract; framework code
should prefer the helpers so subscriptions are cleaned up with the component.
Capsule geometry is View-owned; Logic has no capsule measurement API.

Full Logic patch shapes are exported by `@lingxia/types`; View snapshot types
are exported by `@lingxia/react`, `@lingxia/vue`, and `@lingxia/html`.

### Migrating Page Chrome configuration

This contract is a breaking replacement rather than a compatibility layer.
Move flat page navigation fields into `navigationBar`, rename `tabBar.list` to
`tabBar.items`, and move tab colors into `tabBar.style`; app-controlled tab
placement and dimensions are gone. Replace the flat mutation functions with a
`lx.navigationBar.update()` or `lx.tabBar.update()` patch. The CLI rejects
removed configuration fields with the complete field path and its replacement.

---

## Common Pitfalls

- Mixing view logic and page logic in one file; keep `index.tsx` and `index.ts` roles clear.
- Mutating `data` directly in View instead of calling Logic actions.
- Touching the DOM from Logic — Logic has no DOM access; use `lx.*` for platform operations and `setData()` for state.
- Keeping business state in View `useState`/`ref` instead of Logic-managed `setData()` — state drifts across the bridge boundary.
- Assuming every component's event handler receives the same shape — `LxPicker` hands you the resolved value, `LxVideo` passes the raw DOM `CustomEvent`. See [Components](./components.md#callback-shapes-by-component).
- Skipping `@lingxia/types` in the lxapp's devDependencies and losing intellisense on the entire `lx.*` surface. See [Logic runtime and typings](./lx-api.md).
- Forgetting that only public `Page({})` methods become actions; lifecycle hooks and `_`-prefixed helpers are not exposed.
- Mutating `App({}).globalData` and expecting page views to re-render — `globalData` is not reactive. Propagate to a page's `data` via `setData`.
- Calling `lx.navigateTo` / `lx.redirectTo` on a tab page — rejected by the runtime. Use `lx.switchTab` for tab-page entry; `navigateBack` for non-tab stack pops.
- Treating the tab bar as a host UI surface — it is an lxapp-internal feature declared in `lxapp.json`, orthogonal to top-level `surfaces:` in `lingxia.yaml`.
- Dropping the function an `lx.on*` call returns — it is the only handle that cancels the subscription, and the same route can be open more than once, so the leak multiplies per page instance.
- Using `<video>`, `<audio>`, `video.srcObject`, or `new Audio()` — video is native-owned and audio is not available yet; see [`./components.md`](./components.md) → `LxVideo`.

---

## Pre-ship checklist

- [ ] `lxapp.json` lists every page; `appId` set; `version` bumped if shipping.
- [ ] `security.network.trustedDomains` covers every external host (exact host names, no scheme/port/path).
- [ ] One view-framework file per page.
- [ ] Public actions typed in `PageActions`; private helpers prefixed `_`.
- [ ] `lingxia dev` runs cleanly.
