# Lxapp pages

How to write lxapp pages: layout, Logic, `App({})`, navigation and page chrome. The View is
a GPU View by default ([authoring](./gpu/authoring.md)); React or HTML in a WebView is the
second path ([WebView pages](./webview/guide.md)). One kind per lxapp.

**Which one when** (GPU View names; React pages: [WebView pages](./webview/guide.md)):

| Need | Use |
| --- | --- |
| State Logic reads or keeps | `data`; change it with `page.update(d => …)`, or `page.set({ key })` for whole keys |
| State only one piece of UI shows (a draft, an open menu) | `useLocal` in the View |
| State several pages show (session, cart) | `defineStore`, changed with its `set`/`update`; a page shows it with `follow: { key: store.pick(…) }` |
| Wait or repeat | `page.after(ms, fn)` / `page.every(ms, fn)` in Logic; the View has no timers |
| Call Logic from a handler | the action itself (`onClick={actions.refresh}`); add `useAction` to show `pending`/`error` |
| Load when the component first renders, again when params change | `useRequest(name, actions.x, params)` |
| Progress or streamed output | an `async *` action, with `useStream` |
| A long-lived two-way line | a channel action, with `useChannel` |
| Go to another page | `<link to={{ page, params }}>` in the View; `lx.nav.go` in Logic when it decides |

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
│       ├── index.tsx   # View  — GPU View (`export function render`); React default export or index.html in a WebView
│       ├── index.ts    # Logic — runs in native JS runtime
│       └── index.json  # Page config, optional (navigation bar, style)
├── public/
└── shared/
```

- `lxapp.json` — `appId`, `appName`, `version`, `minRuntime`, `pages`, and
  optional `languages` ([Product settings](./core.md#product-settings)). `appName`
  is only a fallback; the app registry's name wins. It declares no network
  hosts or privileges: those are [host grants](../native/permissions.md),
  unrestricted by default.
- `minRuntime` — the lowest host SDK line (major.minor; patch is ignored)
  that may open the package; `lingxia new` writes it and `lingxia upgrade`
  raises it. The CLI builds only for lines from its supported floor up to
  its own, and names the CLI to use otherwise. A host below that line refuses the
  package with `reason: 'hostUpgradeRequired'`; ask the user to update the host app.
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

- `lingxia build` builds into `dist/`; `lingxia package` builds a release and
  produces the publish archive.
- Type errors and View authoring problems (a View reaching for `lx.*`, calling
  an action its page never defined) fail the build. Performance, control and
  style findings are warnings: the artifact is written, the code is still wrong.

## Logic — `definePage`

A page's Logic file default-exports `definePage(...)`; it and `lx` come from `@lingxia/logic`.

```ts
// pages/order/index.ts
import { definePage, lx, type } from '@lingxia/logic';

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

- `params`, `result`, `messages`, `data`, and `setup` or `actions` are all optional; `definePage({ data })` is a static page.
- `messages: type<{ in: A; out: B }>()` types the line to the opener: the
  opener's handle sends `A` and receives `B`, `page.opener` the reverse.
  Messages sent before the other side's first `onMessage` wait for it.
- `setup(page)` is the form for lifecycle hooks and per-instance state: it runs once per instance, before `onLoad`, and is synchronous. Per-instance state lives in its closure, never in module scope.
- `actions: page => ({ … })` is the short form for a page with only actions; it also runs once per instance. Giving it beside `setup` is a type error and throws.
- It returns lifecycle hooks under their names and `actions`, nothing else: a function beside `actions` or a hook a case away (`onUnLoad`) is a type error and throws.
- Actions take zero or one JSON argument and are Promises to the View. An `async *` action is a stream ([bridge](./bridge.md)).

| `page.` | Description |
| --- | --- |
| `data` | Current state, deep-readonly. |
| `update(d => { d.count += 1; })` | The one way to change data. `page.data` changes at once; the View gets the changes once per turn. The recipe is synchronous and returns nothing: `await` first, then update. Throws inside another `update`; ignored after unload. |
| `await flush()` | Resolves once every update so far reached the View; rejects if the page unloads first or a write was discarded. |
| `params`, `name` | This instance's params, its page name. |
| `visible` | `Live<boolean>` of `onShow`/`onHide`; already `true` in `setup` for a page that opens showing. |
| `signal` | `AbortSignal` aborted as the page unloads, before `onUnload`. Pass it to work the page starts: `fetch(url, { signal: page.signal })`. |
| `setResult(value)` / `close(value?)` | Return a JSON result to the opener. `close` also leaves: a pushed page goes back (top page only), a surface page closes its surface. The opener reads it from the handle's `result` (`lx.nav.go({ page })`, `lx.surface.openPage`): `{ status: 'ok', value }`, or `{ status: 'canceled', reason }` when the page left without one. It settles when the page is closed: a `setResult` in `onUnload` is ignored. |
| `opener`, `surface` | The page that opened this one, or this page's surface; absent otherwise. |

### Lifecycle

Order across `App` and `Page`, and what each navigation fires: [Lifecycle](./lifecycle.md).

| Hook | When it fires |
| --- | --- |
| `onLoad(options)` | Entering the page, with its query: each value keeps the JSON type it was passed with (`{ count: 3 }` arrives as `3`); validate what arrives from outside (an app link): it is usually strings. Non-string values are limited to 8 KB. |
| `onShow({ params? })` | Becoming visible, on entry and every return. `params` only on the show a `go` with `params` caused: an open, a tab switch, a `reuse`. |
| `onReady()` | The document finished rendering, after the first `onShow`. |
| `onHide()` | Covered by another page, the lxapp went to the background, or the desktop aside it is docked in was collapsed. |
| `onUnload()` | Left the stack (`lx.nav.back`, a replace, a tab switch that drops it, `lx.nav.reset`). |

- **Leaving unloads.** Entering again starts a fresh instance: `data` is back
  to its defaults and the View is a new document. Keep what must survive in
  `lx.storage` or `App({})`.
- **Hidden keeps state.** A covered or backgrounded page keeps `data`. A
  desktop host may discard a hidden lxapp's inactive tab WebView: `data` stays,
  and the next show brings `onShow` and a new `onReady`.
- Replacing the current page with its own route keeps the instance and calls
  `onLoad` again.
- Every push is its own instance, so `detail {id: 1} → detail {id: 2}`
  stacks. Tab pages are singletons. Refusals carry `error.reason`:
  `'tabAlreadyOpen'` (a tab already on the stack) or `'stackFull'` (ten pages).
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
| What several pages show (session, cart) | `defineStore`, read with `follow` | Until the lxapp restarts, or persisted |
| Must survive restarts | `lx.storage` (JSON values; key types in `LxStorageSchema`; a plain file, readable by any same-user process on macOS and Windows, so no tokens) | Persistent |

Never keep per-entry state in module scope.

### Types for the View

A GPU View takes its types from its Logic: `import type { View } from './$types'`
(see [GPU View authoring](./gpu/authoring.md)). React, HTML and tests read the page type itself:
`useLxPage<typeof page>()`, `getPage<typeof page>()` and `t.app.page<typeof page>()` with
`import type page from './index.ts'`.

## Subscriptions

Pass `{ signal: page.signal }` to every `watch` and `on<Event>` a page makes;
it ends with the page. A route can be open more than once, so a subscription
without it leaks once per instance.

```ts
export default definePage({
  data: { online: true },
  setup: page => ({
    onLoad() {
      lx.network.status.watch(info => page.update(d => { d.online = info.isConnected; }), { signal: page.signal });
    },
  }),
});
```

Work started with `page.signal` needs no teardown; ignore its `AbortError`.

Timers: `page.after(ms, fn)` and `page.every(ms, fn)` end with the page and return `stop()`. `every` waits
`ms` after each run ends and pauses while the page is hidden (`{ whenHidden: 'run' }` keeps it going);
plain `setTimeout` is for module work that outlives a page.

## Action shapes

| Action shape | Use from View | Typical use |
| --- | --- | --- |
| function / async function | `actions.foo(...)` | buttons, navigation, one-shot work |
| async generator | `useStream` (GPU), `useLxStream` (React) | progress, streaming output |
| channel handler | `useChannel` (GPU), `useLxChannel` (React) | long-lived two-way sessions |

The runtime routes by shape. Details: [Bridge](./bridge.md).

## `App({})`

The optional lxapp-wide singleton, in `lxapp.ts` at the lxapp root,
created once at boot and shared by every page:

```ts
// shared/session.ts — app state is a store the App and pages import
import { defineStore } from '@lingxia/logic';
export const session = defineStore(
  { userId: '', theme: 'light' as 'light' | 'dark' },
  { persist: { key: 'session' } },  // or { load, save } for your own keys and migration
);
```

```ts
// lxapp.ts
import { session } from './shared/session';

App({
  async onLaunch() {
    // Once per worker. An App Link goes to `onOpen`, not here.
    const loaded = await session.ready;  // { ok: true } or { ok: false, error }; never rejects
    if (!loaded.ok) console.warn('session not restored', loaded.error);
  },

  onShow(args) {
    // Foreground. source: host|lxapp. reason: foreground|background|screenshot|open|close|switch_back|switch_away.
    // A switch-back also carries the landing `path` and `query`.
  },

  onHide(args) {
    // The lxapp is being backgrounded. Same AppLifecycleEventArgs shape.
  },

  onScreenshot() {
    // The user took a screenshot while this lxapp was active.
  },
});
```

```ts
// pages/profile/index.ts
import { session } from '../../shared/session';

export default definePage({
  follow: { userId: session.pick(s => s.userId) },  // data.userId, read-only, current until unload
  actions: {
    signOut() { session.update(d => { d.userId = ''; }); },  // every following page updates
  },
});
```

- `defineStore(initial, { name?, persist? })` holds one JSON value for every page: `set` replaces it,
  `update` changes an object or array in place, `pick` and `derive([a, b], fn)` give read-only `Live`s.
- `follow: { key: live }` puts a store, pick, derive or host `Live` in `data` from the first frame;
  write it through its source, never `page.update`.
- `persist` loads at definition and saves each change, one save at a time with the newest value;
  a change made before the load wins, and `saved` reports the last save.
- `onError(failure)` in `App({})` sees each failure nothing handled (page hooks and timers, App hooks,
  store callbacks), one call at a time; it only observes. A promise nothing awaited or caught reaches it
  (`kind: 'app'`, `hook: null`) only on an engine that tracks rejections: not on iOS or macOS.
- There is no `getApp()` or `globalData`; `App({ globalData })` throws. Plain module
  state is not reactive; share what pages show through a store.
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
- With more than five items a phone shows the first four plus **More**; order
  items by use. A tablet shows all; desktop lists all in the sidebar.
- `"showOn": ["mobile"]` or `["desktop"]` limits an item to mobile or desktop hosts.
  Each host needs at least two items.
- `text` is a string, or a catalogue key `{ "message": "tab.home" }` — see
  [text in several languages](#text-in-several-languages).

### Icons

One icon per item. SVG is a template glyph tinted by the style colours; raster
PNG/JPEG/WebP keeps its colours and is centre-cropped to a square. In
`lxapp.json` `iconPath` is project-relative (usually `public/`). At runtime it
may also be an `lx://temp`, `lx://usercache`, or `lx://userdata` path; network
URLs, `file:` URLs, `..`, and absolute paths are rejected. Download remote art
first, and re-download `lx://temp` art after each Logic launch:

```ts
const { path } = await lx.network.download(brand.logoUrl).result;
await lx.app.tabBar.update({
  items: { home: { label: brand.shortName, icon: path } },
});
```

Items are keyed by tab page name; `icon: null` restores the declared icon, `badge: true` is a dot.

### Navigating

`lx.nav` takes a target naming one destination: `{ page, params? }`,
`{ tab, params? }`, `{ app, page?, params? }` or `{ url }`. Names come from
`lxapp.json`; `params` is checked against what the page declares.

```ts
const order = lx.nav.go({ page: 'order', params: { id } }); // handle, at once
await order.opened;                          // on screen; rejects if it could not open
const r = await order.result;                // { status: 'ok', value } | { status: 'canceled', reason }

await lx.nav.go({ page: 'login' }, { replace: true }); // inherits this page's opener and result
await lx.nav.go({ page: 'cart' }, { reuse: true }); // back to the open cart, or open one
await lx.nav.go({ tab: 'profile' });
await lx.nav.reset({ page: 'home' });        // new stack; discarded pages' results settle canceled
await lx.nav.go({ app: 'lingxia-showcase-web', page: 'chat' });
await lx.nav.leaveApp();                     // back to the lxapp that opened this one
```

`reuse` goes to the topmost open instance, closing the pages above it as `back(n)` would; already
on top, only `params` show it again. With none open it is a plain `go` (a replace with
`replace: true`); with one open, `replace` is ignored. It returns no handle.
`lx.nav.current` is a `Live` of the main stack's top page, `{ page, params } | null`.

A page has one current `page.params`: a reuse or tab switch with `params` replaces it and hands it
to that `onShow`; one without keeps it. A tab page cannot replace another page.
Moves run one at a time in the order issued; past 8 waiting, another throws `E_INVALID_STATE`. `{ url }` opens the in-app browser and
does not join the stack; `lx.openExternal(url)` hands a URL to the system.

`lx.surface.openPage('detail', { as: 'float', params })` returns the same
handle, plus `show`/`hide`; the floated page reaches it through `page.opener`.
A handle is not a promise: await `.opened`. Its methods can be called at once;
`close()` before `opened` settles stops the open (`opened` rejects `E_ABORT`,
`result` is `canceled`).

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
`pageOrientation` (`portrait`, `landscape`, `auto`) applies unless the page
calls `page.orientation.lock()`; the lock ends with `unlock()` or when that page
unloads. Unset (or `auto`), the page follows the host app's orientation: leave it
unset unless the page only works one way. `lx.device.orientation.get()` / `watch(cb)` report the orientation in effect.

### Text in several languages

List the languages in `lxapp.json` (`"languages": ["en-US", "zh-CN"]`; the first is the source) and
put one catalogue per language in `i18n/<tag>.json`:

```json
{
  "cart.title": { "defaultMessage": "Cart", "description": "Heading of the cart page." },
  "cart.items": { "defaultMessage": "{count, plural, one {# item} other {# items}}", "description": "count = items in the cart." }
}
```

Keys are dot-separated camelCase. The source catalogue needs a `description` per key; a translation
has only `defaultMessage`. Messages are ICU MessageFormat: `{arg}`, `{n, number}`, `{d, date, short}`,
`plural`, `selectordinal`, `select`, paired tags. `lingxia build` and `lingxia check` fail on a missing
translation, an argument a translation adds or drops, and a catalogue for an undeclared language;
`lingxia dev` warns about a translation's key and shows the source message for it.

- View: `t('cart.items', { count })` from `@lingxia/view`, called while rendering; module-level code
  cannot call it. It follows a language switch.
- An argument is wrapped in bidi isolates (U+2066–2069) when the language is right to left or the
  argument holds right-to-left text, so it cannot reorder the sentence around it.
- Logic: `lx.app.t('cart.items', { count })` for toasts, dialogs and action sheets.
- WebView (React): `const t = useText()` from `@lingxia/react`; `t` from `@lingxia/bridge` elsewhere.
- The View's own words (picker Cancel/Done, dialog OK) are `@view.*` keys; a catalogue that names
  one, e.g. `"@view.done"`, overrides it in that language only. The build checks an override against
  the installed package: an unknown key or a changed argument name stops it.
- A package with words of its own ships its source catalogue and names it in its `package.json`:
  `"lingxia": { "messages": "./messages/en.json" }`; every key starts with its `@namespace.`.
- A dev session also builds `en-XA`: the source accented, ~40% longer and bracketed, to find
  clipped or untranslated text; and `ar-XB`: the source forced right-to-left on a right-to-left
  page, to check mirroring. Pick one with `lx.host.displayLanguage.set('ar-XB')` from
  the Control app (`lxdev lxapp eval`); neither ships.
- Keys and arguments are typed from the source catalogue: an unknown key, or a missing or wrong-kind
  argument, fails `tsc`, and a literal key the catalogue lacks fails the build. A key built at run
  time takes a cast, `` t(`state.${s}` as MessageKey) `` (`LxAppMessageKey` in Logic), and its
  arguments go unchecked.
- Never put translated text in `data`: it would not change on a switch. Store a key (and its
  arguments) and let the View call `t`.
- Tab labels and titles name a key: `"text": { "message": "tab.home" }`,
  `"navigationBar": { "title": { "message": "title.cart" } }`. A plain string is one text for every
  language. The build fails on a key the source catalogue lacks.

An lxapp that must show a language the host does not (a language course, a kiosk) keeps its own
choice and renders its own text; `t`, View words, Liuli labels and host chrome stay in the host's.

The language is the host's display language narrowed to the declared ones: the exact tag, else the
same language, else the source. A runtime value from `lx.app.tabBar.update` /
`page.navigationBar.update` wins until reset with `null`, so do not also translate the declared text
through those calls. A host app that keeps its own language setting must forward it with
`lingxia::app::set_display_language_preference` (or `lx.host.displayLanguage.set` from the
Control app's Logic).

### Runtime updates

`lx.app.tabBar.update()` changes a declared tab bar (it rejects without one);
`page.navigationBar.update()` patches that page's bar, even while another
page is on top. Each call is one
transaction: `null` resets a field, omitted fields stay, an invalid patch
applies nothing.

```ts
await lx.app.tabBar.update({ items: { profile: { label: 'Me', badge: '3' } } });
await lx.app.tabBar.update({ items: { profile: { label: null, badge: null } } });
await lx.app.tabBar.update({ visibility: 'hidden' });   // or 'auto'
await page.navigationBar.update({ title: 'Account' });
await page.navigationBar.update({ style: null, homeButton: 'auto' });
```

### Appearance

Light/dark is a [product setting](./core.md#product-settings). An lxapp whose
UI works in only one scheme declares it in `lxapp.json`:

```json
{ "appearance": "dark" }
```

A View never mutates `data`: it calls an action and Logic answers with `page.update`. A page
holds itself while it has unsaved changes with `useLeaveGuard` (GPU) or
[`useLxLeaveGuard`](./webview/guide.md#leaving-a-page-with-unsaved-changes) (WebView).

## Pitfalls

- More than one view framework in a project; match the existing pages.
- Mutating `data` in the View, or keeping business state in `useState`/`ref`.
- Touching the DOM from Logic; Logic has no DOM.
- Expecting `_`-prefixed helpers or hooks to be actions.
- Expecting changes to plain module state to re-render; use a store and `follow`.
- Pushing a tab page instead of `lx.nav.go({ tab })`.
- A page subscription without `{ signal: page.signal }`.
- Missing `@lingxia/logic`: see [Install typing](./core.md#install-typing).
