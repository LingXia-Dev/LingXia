# LxApp Development Guide

This guide covers how to write lxapp pages — project layout, the View + Logic architecture, data flow, event handling, and native component integration.

Companion pages in this skill:

- [Components](./components.md) — `LxPicker`, `LxVideo`, `LxMediaSwiper`, `LxNavigator` — capabilities, callback shapes, and imperative control (attribute lists live in the exported `@lingxia/elements` types); text input is plain `<input>` / `<textarea>`.
- [Logic-side `lx.*` API](./lx-api.md) — capability map + behavioral notes; signatures live in `@lingxia/types` (install steps here too).
- [Bridge Guide](./bridge.md) — `setData`, stream, channel mechanics in depth.
- [App Project](../app/project.md) — host app setup (`lingxia.yaml`, adaptive `surfaces`).

First-time CLI install and platform toolchains are a one-time, human-facing onramp this skill assumes is already done.

---

## Create an LxApp

```bash
lingxia new my-lxapp -t lxapp -y
```

This creates a standalone lxapp project. To create a host app (which contains an embedded home lxapp), use `-t native-app` instead (see [App Project](../app/project.md)).

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

Key files:

- `lxapp.json`: Runtime metadata (`appId`, `appName` or `name`, `version`, `pages`) and lxapp security policy.
- `lxapp.config.ts`: Build config for view tooling, aliases, and static asset directories.
- `pages/<name>/index.tsx` (or `.vue`): View layer — UI rendering in WebView.
- `pages/<name>/index.ts`: Logic layer — page lifecycle and business operations.
- `pages/<name>/index.json`: Page-level config (navigation/title/style and related options).

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

Recommended reading path:

- This guide: page layout, `Page({})`, `useLxPage`, events, and native components.
- [Bridge Guide](./bridge.md): deeper mechanics of `setData`, stream, and channel.

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
| `lx.*` | Global platform APIs (e.g. `lx.setNavigationBarTitle()`, `lx.createVideoContext()`). |

### Private helpers

Functions starting with `_` are private — they are **not** exposed to the View. Use them for internal logic:

```ts
Page({
  data: { total: 0 },

  _calculateTotal: function (items) {
    return items.reduce((sum, item) => sum + item.price, 0);
  },

  onCheckout: function (params) {
    const total = this._calculateTotal(params?.items || []);
    this.setData({ total });
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

```vue
<!-- pages/home/index.vue -->
<template>
  <div>
    <p>Count: {{ data.count }}</p>
    <p>{{ data.message }}</p>
    <button @click="actions.increment()">+1</button>
    <button @click="actions.updateMessage({ text: 'World' })">Update</button>
  </div>
</template>

<script setup lang="ts">
import { useLxPage } from '@lingxia/vue';

type PageData = {
  count: number;
  message: string;
};

type PageActions = {
  increment: () => void;
  updateMessage: (params: { text: string }) => void;
};

const { data, actions } = useLxPage<PageData, PageActions>();
</script>
```

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

// Video — handler receives raw DOM Event
<LxVideo src={url} onPlaying={actions.onPlaying} />
```

**Vue:**

```vue
<script setup lang="ts">
import { useLxPage, LxPicker, LxVideo } from '@lingxia/vue';

const { actions } = useLxPage<PageData, PageActions>();
</script>

<input @input="(e) => actions.onInputChange({ value: e.currentTarget.value })" />

<LxPicker
  :columns="[['A', 'B', 'C']]"
  @confirm="(value) => actions.onPickerConfirm({ field: 'choice', value })"
/>

<LxVideo :src="url" @playing="actions.onPlaying" />
```

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
    const stored = lx.getStorage().get('userId') as string | undefined; // synchronous, untyped
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

## Complete Example: Input Page

**Logic** (`pages/input/index.ts`):

```ts
Page({
  data: {
    inputValue: "",
    syncValue: "",
  },

  onLoad: function () {},

  onInputChange: function (params) {
    // params is { value } passed by the View from the DOM input event
    if (params?.value === undefined) return;
    console.log("input changed:", params.value);
  },

  onSyncInput: function (params) {
    if (params?.value === undefined) return;
    // Write back to data → View re-renders with updated value
    this.setData({ syncValue: String(params.value) });
  },
});
```

**View** (`pages/input/index.tsx`) — text input is a plain `<input>` / `<textarea>`:

```tsx
import { useLxPage } from '@lingxia/react';

type PageData = { syncValue: string };
type PageActions = {
  onInputChange: (params: { value: string }) => void;
  onSyncInput: (params: { value: string }) => void;
};

export default function InputPage() {
  const { data, actions } = useLxPage<PageData, PageActions>();

  return (
    <div>
      <input
        placeholder="Basic input"
        onInput={(e) => actions.onInputChange({ value: e.currentTarget.value })}
      />

      <textarea
        value={data.syncValue}
        placeholder="Synced input"
        onInput={(e) => actions.onSyncInput({ value: e.currentTarget.value })}
      />
      <p>Current: {data.syncValue}</p>
    </div>
  );
}
```

> Logic initializes `data: { syncValue: "" }`, so the field exists from first paint — required in the type.

---

## Tab bar navigation

A **tab bar** is a persistent navigation strip — typically at the bottom of the screen — that shows the lxapp's primary pages. Tapping a tab switches the active page **without** push/pop semantics: the tab bar stays visible across all tab pages, and tab pages do not stack on each other.

> **Scope.** Tab bar is an **lxapp-internal navigation concept** declared in `lxapp.json`. It has nothing to do with host surfaces — `lingxia.yaml` `surfaces` live one layer up and describe the native shell (windows, asides, sidebar/tray). A host shell renders an lxapp; that lxapp may have its own tab bar inside.

### Declaring the tab bar in `lxapp.json`

Add a `tabBar` block alongside `pages`:

```json
{
  "appId": "my-app",
  "version": "0.1.0",
  "pages": [
    { "name": "home",     "path": "pages/home/index" },
    { "name": "discover", "path": "pages/discover/index" },
    { "name": "profile",  "path": "pages/profile/index" }
  ],
  "tabBar": {
    "color":           "#999999",
    "selectedColor":   "#1677ff",
    "backgroundColor": "#ffffff",
    "borderStyle":     "#eeeeee",
    "position":        "bottom",
    "list": [
      {
        "text":             "Home",
        "pagePath":         "pages/home/index",
        "iconPath":         "public/home.png",
        "selectedIconPath": "public/home_selected.png",
        "selected":         true
      },
      {
        "text":     "Discover",
        "pagePath": "pages/discover/index",
        "iconPath": "public/discover.png"
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

Rules:

- Every `list[].pagePath` must match a registered page path under `pages[]`.
- `iconPath` / `selectedIconPath` are project-relative — usually under `public/` so they're copied verbatim into `dist/` by the default static-assets rule.
- `selected: true` on one entry picks the initial tab; if omitted, the first entry is selected.
- `position`: `"bottom"` (default) or `"top"`.

### Switching tabs at runtime

From Logic, use `lx.switchTab(...)`. **`lx.navigateTo` and `lx.redirectTo` do not work on tab pages** — the runtime rejects them with errors like `"redirectTo cannot navigate to a tabBar page"`. Switching is the only way in and out of tabs:

```ts
lx.switchTab({ url: '/pages/profile/index' });
```

When driving a running app from `lxdev`, use the page name from `lxapp.json` rather than the path:

```bash
lxdev lxapp nav switch-tab profile
```

`lx.navigateBack` still works for popping non-tab pages that were pushed on top of the current tab.

### Modifying the tab bar after declaration

The `lx.setTabBar*` family **mutates an already-declared tab bar** — none of these create or remove tabs. If the lxapp has no `tabBar` in `lxapp.json`, every call returns `false`.

```ts
lx.setTabBarItem({ index: 1, text: 'Inbox', iconPath: 'public/inbox.png' });
lx.setTabBarBadge({ index: 1, text: '3' });
lx.removeTabBarBadge({ index: 1 });
lx.showTabBarRedDot({ index: 0 });
lx.hideTabBarRedDot({ index: 0 });
lx.setTabBarStyle({ selectedColor: '#ff0000' });
lx.showTabBar();
lx.hideTabBar();
```

Full option shapes: [`./lx-api.md#page-chrome--ui`](./lx-api.md#page-chrome--ui).

---

## Common Pitfalls

- Mixing view logic and page logic in one file; keep `index.tsx` and `index.ts` roles clear.
- Mutating `data` directly in View instead of calling Logic actions.
- Re-documenting bridge behavior inside page code instead of leaning on [Bridge Guide](./bridge.md) for stream/channel details.
- Assuming every component's event handler receives the same shape — `LxPicker` hands you the resolved value, `LxVideo` passes the raw DOM `Event`. See [Components](./components.md#callback-shapes-by-component).
- Skipping `@lingxia/types` in the lxapp's devDependencies and losing intellisense on the entire `lx.*` surface. See [Logic-side `lx.*` API](./lx-api.md).
- Forgetting that only public `Page({})` methods become actions; lifecycle hooks and `_`-prefixed helpers are not exposed.
- Mutating `App({}).globalData` and expecting page views to re-render — `globalData` is not reactive. Propagate to a page's `data` via `setData`.
- Calling `lx.navigateTo` / `lx.redirectTo` on a tab page — rejected by the runtime. Use `lx.switchTab` for tab-page entry; `navigateBack` for non-tab stack pops.
- Treating the tab bar as a host UI surface — it is an lxapp-internal feature declared in `lxapp.json`, orthogonal to top-level `surfaces:` in `lingxia.yaml`.

---

## Pre-ship checklist

- [ ] `lxapp.json` lists every page; `appId` set; `version` bumped if shipping.
- [ ] `security.network.trustedDomains` covers every external host (exact host names, no scheme/port/path).
- [ ] One view-framework file per page.
- [ ] Public actions typed in `PageActions`; private helpers prefixed `_`.
- [ ] `lingxia dev` runs cleanly.

## Tips

- **Type your data**: Define a `PageData` type in both Logic and View to catch mismatches early.
- **Keep Logic pure**: Logic has no DOM access. Use `lx.*` APIs for platform operations, `setData()` for state.
- **Avoid heavy View state**: Prefer Logic-managed state via `setData()` over local `useState`/`ref`. This keeps state consistent across the bridge boundary.
- **Private with `_` prefix**: Functions starting with `_` won't be exposed to View. Use them for internal helpers.
- **Page config**: `index.json` controls navigation bar title, background color, and other page-level settings.
