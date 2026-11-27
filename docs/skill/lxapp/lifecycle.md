# Lifecycle

When `App({})` and page hooks fire, in what order, and what each
navigation does to the pages it enters and leaves. Hook signatures are in
[Pages → Lifecycle](./guide.md#lifecycle) and [`App({})`](./guide.md#app).

## Two layers

`App({})` hooks belong to the lxapp session. Page hooks belong to one
page instance. They advance independently.

- **Every page `onLoad` waits for `App.onLaunch` to settle**, so a page can
  read what an `async onLaunch` set up:

  ```ts
  // shared/session.ts
  export const session = { userId: '' };
  declare global { interface LxStorageSchema { userId: string } }

  // lxapp.ts
  App({
    async onLaunch() {
      session.userId = (await lx.storage.get('userId')) ?? '';
    },
  });

  // pages/home/index.ts
  export default definePage({
    data: { userId: '' },
    setup: page => ({
      onLoad() {
        page.update(d => { d.userId = session.userId; });
      },
    }),
  });
  ```

  The first frame is not held: the page paints its initial `data` meanwhile.
  An `onLaunch` that never settles leaves every `onLoad` waiting. `App.onShow`
  does not wait for `onLaunch`.
- **Page hooks of one page run in order, each awaited.** An `async onLoad`
  settles before that page's `onShow` starts. Different pages do not wait on
  each other.
- **`page.signal` aborts before `onUnload`.**
- **`onRefresh` holds the indicator until it settles.** A pull and
  `page.refresh()` both run it; return the fetch's promise. There is no stop call.
  Both need `"enablePullDownRefresh": true` in the page's `index.json`;
  without it `page.refresh()` throws (`reason: 'refreshNotEnabled'`).

## App hooks

Every `onShow` / `onHide` receives `{ source, reason }`:

| What happened | Hook | `source` | `reason` |
| --- | --- | --- | --- |
| First open of this session | `onLaunch()`, then `onShow` | `lxapp` | `open` |
| Reopened while alive (link, notification tap, `lx.nav.go({ app })`, `openApp`, a banner action) | `onShow` with path/query | `lxapp` | `switch_back` |
| Another lxapp opened as the main | `onHide` | `lxapp` | `switch_away` |
| Closed | `onHide` | `lxapp` | `close` |
| Host app went to background / came back | `onHide` / `onShow` | `host` | `background` / `foreground` |
| Screenshot, where the platform reports it | `onScreenshot` | — | — |

- **One `onShow` per foreground.** A link that brings the app forward reaches `onOpen` and
  then a single `onShow`; with `onOpen` defined, a link arriving just after another show
  brought the app forward reaches `onOpen` alone.

- **An aside does not hide the main.** `lx.surface.openApp(id, { as: 'aside' })`
  (Control app only) opens beside the main, which gets no `onHide`. Collapsing a docked aside is
  a page event, not an App event: the aside lxapp's App hooks stay quiet (see
  Navigation below). Reopening it while docked brings it back in place; route
  from the `path` / `query` in App `onShow`.
- **Moving between aside and main does not restart.** Dragging an aside's tab
  onto the sidebar, clicking its Pin, or `lx.surface.openApp(id, { as })` naming
  the other role moves the live lxapp: no `onLaunch`, page stack and state
  kept; the previous main gets `onHide` (`switch_away`). Desktop only: mobile
  hosts reject with `E_NOT_SUPPORTED`, the root main with `E_SURFACE_ROLE_LOCKED`,
  and an app already open in the other role with `E_SURFACE_CONFLICT`.
- **`onLaunch` may pick the first page.** Return a page or tab target (local
  decisions only); with a cold link's `onOpen` it holds the launch screen at
  most 1.5 s, then the start page opens and a late target is navigated to.
- **AppLink arrives once.** Route it in `App.onOpen(link)` and return a
  target: cold, before the first page when `onLaunch` returned none (its
  target is the first page); warm, before `onShow` (`reason: 'switch_back'`).
  `onLaunch` sees it as `{ link }`; `onShow` does not.
- **`onLaunch` runs once per Logic start.** Restarts start Logic again:
  - `lx.app.update.apply()` closes the session (`onHide`, `close`) and opens
    a new one: `onLaunch`, `onShow` (`open`), then the initial page's `onLoad`.
  - A dev or code reload, or a device switch in the Runner, keeps the window
    but starts fresh Logic: `onLaunch`, then `onShow` (`open`) if the lxapp is
    on screen. A hidden lxapp gets its `onShow` on the next real show. Open
    pages replay `onLoad` → `onShow` → `onReady`. A closed lxapp cannot be
    reloaded this way; open it again.

## Cold open

1. The previous main, if any: `App.onHide` (`switch_away`).
2. `App.onLaunch()`.
3. `App.onShow` (`open`).
4. Landing page: `onLoad(query)` → `onShow` → `onReady`.

Step 4's `onLoad` waits for `onLaunch` to settle; step 3 does not. Other tab pages may be preloaded
but get no hook until first selected.

## Navigation

| Action | Page entered | Page left |
| --- | --- | --- |
| `lx.nav.go({ page })` | New instance: `onLoad` → `onShow` → `onReady` | `onHide`, `data` kept |
| `go(…, { replace: true })` to another page | New instance: `onLoad` → `onShow` → `onReady` | `onUnload` |
| `lx.nav.back` | `onShow` only, `data` kept | Each popped page: `onUnload` |
| `go(…, { reuse: true })`, an instance open | `onShow` only, `data` kept | Each page above it: `onUnload` |
| `go({ tab })`, first visit | `onLoad` → `onShow` → `onReady` | Left tab: `onHide`; pushed pages: `onUnload` |
| `go({ tab })`, later visits | `onShow` only, `data` kept | Same |
| `lx.nav.reset` | New instance: `onLoad` → `onShow` → `onReady` | Every loaded page, tabs included: `onUnload` |
| Reopen at a named page (link, notification) in main, not the current page | `onLoad` with the new query | — |
| Reopen at the current page, or on a live aside | App `onShow` (`switch_back`) carries `path` and `query`; the page stack is kept | — |
| Host background / foreground | `onShow` | `onHide` |
| Desktop aside collapsed / restored (the lxapp is docked as an aside) | `onShow`, `data` kept | `onHide` |

- **`onReady` fires once per rendered document, not per entry.** A return to a
  kept page gets `onShow` only.
- **A hidden page can lose its WebView without a hook.** Desktop hosts may
  reclaim a hidden lxapp's inactive tab. `data` survives; the next show brings
  `onShow` and a new `onReady`, but no `onLoad`. Do not keep state only in the
  View.
- **A collapsed aside hides its page.** On desktop the user can collapse the
  aside an lxapp is docked in (the slot control, or Cmd+Option+\ / Ctrl+Alt+\).
  The page it was showing gets `onHide`; restoring the aside with that page
  still current gets `onShow`. Logic, the page and its WebView stay alive, and
  App `onHide` does not fire, so pause polling or animation in page `onHide`
  and treat "the user left the app" as App `onHide`. Native components on the
  page (video, camera) are paused and resumed with it, as they are for a main
  page. Maximizing an aside over the main fires nothing, for the aside or for
  the main underneath: both keep their frames and stay shown.
- **Surface pages** get `onHide` then `onUnload` as soon as the surface
  closes. `hide()` keeps the page (only `onHide`); under memory pressure the
  host may unload a hidden one, and its handle closes with `reclaimed`.
