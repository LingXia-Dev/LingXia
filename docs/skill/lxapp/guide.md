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
│       ├── index.tsx   # View  — runs in WebView (React; HTML projects use index.html)
│       ├── index.ts    # Logic — runs in native JS runtime
│       └── index.json  # Page config (navigation bar, style)
├── public/
└── shared/
```

- `lxapp.json` — `appId`, `appName`, `version`, `minRuntime`, `pages`, and
  optional `languages` ([Product settings](./lx-api.md#product-settings)). `appName`
  is only a fallback; the app registry's name wins. It declares no network
  hosts or privileges: those are [host grants](../native/permissions.md),
  unrestricted by default.
- `minRuntime` — the lowest host SDK line (major.minor; patch is ignored)
  that may open the package; `lingxia new` writes it and `lingxia upgrade`
  raises it. The CLI builds only for lines from its supported floor up to
  its own, and names the CLI to use otherwise. An older host refuses the
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
- A View reaching for `lx.*`, or calling an action its page never defined,
  fails the build. Other findings are warnings: the artifact is written, the
  code is still wrong.

## Logic — `definePage`

A page's Logic file default-exports `definePage(...)` from `@lingxia/types/page`.

```ts
// pages/order/index.ts
import { definePage, type } from '@lingxia/types/page';

export default definePage({
  params: type<{ id: string }>(),                // what onLoad receives
  result: type<{ status: 'open' | 'done' }>(),   // what close(value) returns to the opener
  data: { order: null as Order | null, saving: false },
  setup(page) {
    let timer: number | undefined;               // private to this instance
    return {
      async onLoad({ id }) {
        const res = await fetch(`/orders/${id}`, { signal: page.signal });
        const order = (await res.json()) as Order;
        page.update(d => { d.order = order; });
      },
      onUnload() { clearInterval(timer); },
      actions: {                                 // everything the View may call
        async markDone() {
          page.update(d => { d.saving = true; });
          await fetch(`/orders/${page.params.id}/done`, { method: 'POST', signal: page.signal });
          await page.close({ status: 'done' });
        },
      },
    };
  },
});
```

- `params`, `result`, `messages`, `data`, `setup` are all optional; `definePage({ data })` is a static page.
- `messages: type<{ in: A; out: B }>()` types the line to the opener: the
  opener's handle sends `A` and receives `B`, `page.opener` the reverse.
  Messages sent before the other side's first `onMessage` wait for it.
- `setup(page)` runs once per instance, before `onLoad`, and is synchronous. Per-instance state lives in its closure, never in module scope.
- It returns lifecycle hooks under their names and `actions`, nothing else: a function beside `actions` or a hook a case away (`onUnLoad`) is a type error and throws.
- Actions take zero or one JSON argument and are Promises to the View. An `async *` action is a stream ([bridge](./bridge.md)).

| `page.` | Description |
| --- | --- |
| `data` | Current state, deep-readonly. |
| `update(d => { d.count += 1; })` | The one way to change data. `page.data` changes at once; the View gets the changes once per turn. The recipe is synchronous and returns nothing: `await` first, then update. Throws inside another `update`; ignored after unload. |
| `await flush()` | Resolves once every update so far reached the View; rejects if the page unloads first or a write was discarded. |
| `params`, `name` | This instance's params, its page name. |
| `signal` | `AbortSignal` aborted as the page unloads, before `onUnload`. Pass it to work the page starts: `fetch(url, { signal: page.signal })`. |
| `setResult(value)` / `close(value?)` | Return a JSON result to the opener. `close` also leaves: a pushed page goes back (top page only), a surface page closes its surface. The opener reads it from the handle's `result` (`lx.nav.go({ page })`, `lx.surface.openPage`): `{ status: 'ok', value }`, or `{ status: 'canceled', reason }` when the page left without one. |
| `opener`, `surface` | The page that pushed this one, or this page's surface; absent otherwise. |

### Lifecycle

Order across `App` and `Page`, and what each navigation fires: [Lifecycle](./lifecycle.md).

| Hook | When it fires |
| --- | --- |
| `onLoad(options)` | Entering the page, with its query: each value keeps the JSON type it was passed with (`{ count: 3 }` arrives as `3`); validate what arrives from outside (an app link): it is usually strings. Non-string values are limited to 8 KB. |
| `onShow()` | Becoming visible, on entry and every return. |
| `onReady()` | The document finished rendering, after the first `onShow`. |
| `onHide()` | Covered by another page, the lxapp went to the background, or the desktop aside it is docked in was collapsed. |
| `onUnload()` | Left the stack (`lx.nav.back`, a replace, a tab switch that drops it, `lx.nav.reset`). |

- **Leaving unloads.** Entering again starts a fresh instance: `data` is back
  to its defaults and the View is a new document. Keep what must survive in
  `lx.storage()` or `App({})`.
- **Hidden keeps state.** A covered or backgrounded page keeps `data`. A
  desktop host may discard a hidden lxapp's inactive tab WebView: `data` stays,
  and the next show brings `onShow` and a new `onReady`.
- Replacing the current page with its own route keeps the instance and calls
  `onLoad` again.
- Every push is its own instance, so `detail {id: 1} → detail {id: 2}`
  stacks. Tab pages are singletons. Refusals carry `error.data.reason`:
  `"duplicate_route"` (a tab already on the stack) or `"stack_full"` (ten pages).
- `await lx.nav.back()` pops one; `lx.nav.back(2)` pops more, stopping at the root.

### What resets

Module scope is evaluated once per app session and is shared by every live
instance of the page; `setup`'s closure belongs to one instance:

```ts
let hits = 0;               // module scope: every instance, never resets

export default definePage({
  data: { count: 0 },       // fresh on every entry
  setup(page) {
    let opened = Date.now(); // this instance only
    return { onLoad() { hits += 1; } };
  },
});
```

| State | Container | Lifetime |
| --- | --- | --- |
| One entry (form input, timers, request handles) | `data`, or a `let` in `setup` | The page instance |
| Every instance of the page (constants, caches) | Module scope | Until the lxapp restarts |
| Must survive restarts | `lx.storage()` | Persistent |

Never keep per-entry state in module scope.

### Types for the View

The page's type is its contract: `View<typeof page>`, `useLxPage<typeof page>()`
and `t.app.page<typeof page>()` read `data` and `actions` from
`import type page from './index.ts'`. That file must be in the View's
tsconfig. A shared `PageContract<Data, Actions>` still works.

## View

A View is a React component or an HTML module entry. It renders `data`
and calls `actions`; it never mutates `data`.

The page mounts once its first state (the `definePage({ data })` defaults) has
arrived, so `data` is whole from the first render. Type `PageData` and
`PageActions` fields as **required**; use `?:` only for a field filled later.
Never write `actions.foo?.()`. If Logic fails to deliver state, the page shows
a panel naming itself.

| Hook | Returns | Changes |
|---|---|---|
| `useLxPage<typeof page>()` | `{ data, actions }` — this page's Logic state and actions | on every update |
| `useLxHost()` | [host facts](#host-facts) | only when a field changes |
| `useLxLeaveGuard(dirty, onRequest)` | nothing — [guards leaving](#leaving-a-page-with-unsaved-changes) while `dirty` | — |

Any component may call either. Geometry is CSS, not a hook:
[page chrome CSS](#page-chrome-css).

`data` is typed `DeepReadonly<PageData>`. In a dev session a write throws.
Keep what the user edits as View state and submit it with an action; Logic's
`page.update` brings the result back:

```tsx
const { data, actions } = useLxPage<PageData, PageActions>();
const [draft, setDraft] = useState(data.message);
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
const { sizeClass, canDock, displayLanguage, contentLanguage, formFactor, os, runner } = useLxHost();
```

`getHost()` / `subscribeHost(cb)` in `@lingxia/html`;
`window.LingXiaBridge.host.get()` / `.subscribe(cb)` in a page that bundles
nothing. It never changes while a window is dragged.

- `sizeClass` (`compact` | `regular`) and `canDock` — how much room:
  [Adaptive UI](./adaptive-ui.md).
- `formFactor` (`mobile` | `desktop`) — which machine. Branch on it for what a
  phone must not show. A narrow desktop window is `desktop`; a tablet or
  unfolded fold is `mobile`.
- `os` — `'iOS' | 'macOS' | 'Android' | 'Windows' | 'Harmony' | 'unknown'`, for
  OS-specific features only. `runner` is `true` in the dev Runner.
- `displayLanguage` — the product language; `contentLanguage` — it narrowed to
  lxapp.json `languages`, which sets `<html lang>` and `<html dir>`; pick copy by
  it. See [Product settings](./lx-api.md#product-settings).

`formFactor`, `os`, and `runner` are fixed for a page's life. A Runner phone
frame reports `mobile` while `os` names the desktop; switching form factor
re-serves the page. Logic reads the OS with `lx.device.getDeviceInfo()`.

## Events

Use framework syntax (`onX` in React). A handler can be an
`actions.*` function or a local View function; either receives what the
component reports, and an `actions.*` handler forwards it to Logic as its
payload. Native component callbacks and their payload shapes:
[Components](./components.md#callback-shapes).

### Subscriptions

Pass `{ signal: page.signal }` to every `watch` and `on<Event>` a page makes;
it ends with the page. A route can be open more than once, so a subscription
without it leaks once per instance.

```ts
export default definePage({
  data: { online: true },
  setup: page => ({
    onLoad() {
      lx.network.watch(info => page.update(d => { d.online = info.isConnected; }), { signal: page.signal });
    },
  }),
});
```

Work started with `page.signal` needs no teardown; ignore its `AbortError`.

## Action shapes

| Action shape | Use from View | Typical use |
| --- | --- | --- |
| function / async function | `actions.foo(...)` | buttons, navigation, one-shot work |
| async generator | `useLxStream(actions.foo, ...)` | progress, streaming output |
| channel handler | `useLxChannel(actions.foo, ...)` | long-lived two-way sessions |

The runtime routes by shape. Details: [Bridge](./bridge.md).

## `App({})`

The optional lxapp-wide singleton, in one root file (conventionally `app.ts`),
created once at boot and shared by every page:

```ts
// shared/session.ts — app state is a module the App and pages import
export const session = { userId: '', theme: 'light' as 'light' | 'dark' };
```

```ts
// app.ts
import { session } from './shared/session';

App({
  async onLaunch(options) {
    // Once per worker. AppLaunchOptions: path?, query?, scene?, url?, referrerInfo?
    // Cold AppLink: scene 8003 + the original `url` here. referrerInfo when
    // opened by another lxapp.
    const stored = await lx.storage().get<string>('userId');
    if (stored) session.userId = stored;
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
import { session } from '../../shared/session';

export default definePage({
  data: { userId: '' },
  setup: page => ({
    onLoad() {
      page.update(d => { d.userId = session.userId; });
    },
  }),
});
```

- There is no `getApp()` or `globalData`; `App({ globalData })` throws. Module
  state is not reactive; push changes to a page with `page.update`.
- Cold start: `App.onLaunch` → `App.onShow` → `Page.onLoad` → `Page.onShow`;
  `Page.onLoad` waits for an `async onLaunch` to settle.
  Foregrounding: `App.onShow` → top page's `onShow`.
- There is no `getCurrentPages()`; a page reaches itself through `page`, and
  another page through the handle `lx.nav.go` returned.

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
  `backgroundColor`. `lx.app.tabBar.update()` cannot change style.
- A phone shows the first four items plus **More**; order items by use.
  Desktop lists all in the sidebar.
- `"showOn": ["mobile"]` or `["desktop"]` limits an item to one form factor.
  Indices stay as declared, and each host needs at least two items.
- `text` is a string, or one per language — see
  [localized labels and titles](#localized-labels-and-titles).

### Icons

One icon per item. SVG is a template glyph tinted by the style colours; raster
PNG/JPEG/WebP keeps its colours and is centre-cropped to a square. In
`lxapp.json` `iconPath` is project-relative (usually `public/`). At runtime it
may also be an `lx://temp`, `lx://usercache`, or `lx://userdata` path; network
URLs, `file:` URLs, `..`, and absolute paths are rejected. Download remote art
first, and re-download `lx://temp` art after each Logic launch:

```ts
const { uri } = await lx.network.download(brand.logoUrl).result;
await lx.app.tabBar.update({
  items: [{ index: 0, text: brand.shortName, iconPath: uri }],
});
```

`iconPath: null` restores the declared icon.

### Navigating

`lx.nav` takes a target naming one destination: `{ page, params? }`,
`{ tab }`, `{ app, page?, params? }` or `{ url }`. Names come from
`lxapp.json`; `params` is checked against what the page declares.

```ts
const order = lx.nav.go({ page: 'order', params: { id } }); // handle, at once
await order.opened;                          // on screen; rejects if it could not open
const r = await order.result;                // { status: 'ok', value } | { status: 'canceled', reason }

await lx.nav.go({ page: 'login' }, { replace: true }); // inherits this page's opener and result
await lx.nav.go({ tab: 'profile' });
await lx.nav.reset({ page: 'home' });        // new stack; discarded pages' results settle canceled
await lx.nav.go({ app: 'lingxia-chat', page: 'chat' });
await lx.nav.leaveApp();                     // back to the lxapp that opened this one
```

A tab page cannot replace another page. `{ url }` opens the in-app browser and
does not join the stack; `lx.openExternal(url)` hands a URL to the system.

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

### Localized labels and titles

`tabBar.items[].text` and `navigationBar.title` take a string, or an object
with a required `default` plus BCP-47 language tags. The host resolves it
against the effective display language and repaints when the language
changes — no Logic needed. Other manifest strings are not localizable.

```json
{ "page": "profiles", "text": { "default": "Profiles", "zh-CN": "节点" } }
```

```json
{ "navigationBar": { "title": { "default": "Profiles", "zh-CN": "节点" } } }
```

Resolution: the exact tag; else an entry of the same language, matching
script first, then region (a Chinese region implies its script, so
`zh-Hant-TW` reads a `zh-TW` entry and never a `zh-CN` one); else `default`.
Key order does not matter. A runtime value from `lx.app.tabBar.update` / `page.navigationBar.update`
wins until reset with `null`, so do not also translate the declared text
through those calls. The map form needs a runtime that has it: raise
`minRuntime` when you adopt it.

The language is the host's display language. A host app that keeps its own
language setting must forward it with
`lingxia::app::set_display_language_preference` (or
`lx.host.displayLanguage.setPreference` from Logic); otherwise the
page content follows the app's setting and the declared chrome text does not.

### Leaving a page with unsaved changes

A pushed page can hold itself while it has unsaved changes. When the user
then tries to leave — navigation bar back or home, Android/Harmony system
back, the edge swipe — the page stays and your handler is asked. Return
`true` to let the user go (the host then does what they asked), `false` to
stay. Works without Logic.

```tsx
import { useLxLeaveGuard } from '@lingxia/react';

useLxLeaveGuard(dirty, () => confirmDiscard()); // boolean | Promise<boolean>
```

Without a framework, hold while dirty and release when clean:

```ts
import { holdLeaveGuard } from '@lingxia/bridge'; // or @lingxia/html,
                                                  // window.LingXiaBridge.leaveGuard.hold
const release = holdLeaveGuard(({ reason }) => confirmDiscard()); // reason: 'back' | 'home'
release();
```

- Several guards on one page are independent: the page is held while any is
  live, and all must return `true` to leave.
- While a request is being answered, further back/home presses are ignored.
- Not guarded: programmatic navigation (`lx.nav`, `<lx-navigator>`), tab switches, closing the lxapp from the
  capsule, and the root page (nothing to pop).
- The guard belongs to the current document: a reload, or leaving the stack,
  clears it.

### Runtime updates

`lx.app.tabBar.update()` changes a declared tab bar (it rejects without one);
`page.navigationBar.update()` patches that page's bar, even while another
page is on top. Each call is one
transaction: `null` resets a field, omitted fields stay, an invalid patch
applies nothing.

```ts
await lx.app.tabBar.update({ items: [{ index: 1, text: 'Inbox', badge: '3' }] });
await lx.app.tabBar.update({ items: [{ index: 1, text: null, badge: null }] });
await lx.app.tabBar.update({ visibility: 'hidden' });   // or 'auto'
await page.navigationBar.update({ title: 'Account' });
await page.navigationBar.update({ style: null, homeButton: 'auto' });
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
- Expecting changes to shared module state to re-render.
- Pushing a tab page instead of `lx.nav.go({ tab })`.
- A page subscription without `{ signal: page.signal }`.
- Missing `@lingxia/types`: see [Install typing](./lx-api.md#install-typing).
