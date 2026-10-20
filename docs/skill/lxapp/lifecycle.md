# Lifecycle

When `App({})` and `Page({})` hooks fire, in what order, and what each
navigation does to the pages it enters and leaves. Hook signatures are in
[Pages → Lifecycle](./guide.md#lifecycle) and [`App({})`](./guide.md#app).

## Two layers

`App({})` hooks belong to the lxapp session. `Page({})` hooks belong to one
page instance. They advance independently.

- **App hooks do not wait for each other.** An `async onLaunch` is still
  running when `App.onShow` and the first page's `onLoad` run. Share start-up
  work as a promise instead of assuming `globalData` is filled:

  ```ts
  // app.ts
  App({
    globalData: { ready: null as Promise<void> | null, userId: '' },
    onLaunch() {
      this.globalData.ready = (async () => {
        this.globalData.userId = (await lx.getStorage().get<string>('userId')) ?? '';
      })();
    },
  });

  // pages/home/index.ts
  Page({
    async onLoad() {
      const app = getApp<AppInstance & { globalData: { ready: Promise<void>; userId: string } }>();
      await app?.globalData.ready;
      this.setData({ userId: app?.globalData.userId ?? '' });
    },
  });
  ```

- **Page hooks of one page run in order, each awaited.** An `async onLoad`
  settles before that page's `onShow` starts. Different pages do not wait on
  each other.
- **`this.signal` aborts before `onUnload`.**

## App hooks

Every `onShow` / `onHide` receives `{ source, reason }`:

| What happened | Hook | `source` | `reason` |
| --- | --- | --- | --- |
| First open of this session | `onLaunch(options)`, then `onShow` | `lxapp` | `open` |
| Reopened while alive (link, notification tap, `navigateToApp`, `openApp`, `bringToFront`) | `onShow` with path/query | `lxapp` | `switch_back` |
| Another lxapp opened as the main | `onHide` | `lxapp` | `switch_away` |
| Closed | `onHide` | `lxapp` | `close` |
| Host app went to background / came back | `onHide` / `onShow` | `host` | `background` / `foreground` |
| Screenshot, where the platform reports it | `onUserCaptureScreen` | — | — |

- **An aside does not hide the main.** `lx.shell.openApp(id, { as: 'aside' })`
  opens beside the main, which gets no `onHide`. Collapsing a docked aside is
  a page event, not an App event: the aside lxapp's App hooks stay quiet (see
  Navigation below). Reopening it while docked brings it back in place; route
  from the `page` / `query` in App `onShow`.
- **An aside the user promotes to main does not restart.** On macOS, dragging
  its tab onto the sidebar (or clicking its Pin) moves the live lxapp: no
  `onLaunch`, page stack and state kept; the previous main gets `onHide`
  (`switch_away`). Code cannot do this: `openApp` with the other role fails.
- **AppLink arrives once.** A cold AppLink is in `onLaunch` options
  (`scene: 8003`, `url`); the `onShow` that follows carries no link. A warm
  AppLink is in `onShow` with `reason: 'switch_back'`.
- **`onLaunch` runs once per Logic start.** Restarts start Logic again:
  - `lx.update.applyUpdate()` closes the session (`onHide`, `close`) and opens
    a new one: `onLaunch`, `onShow` (`open`), then the initial page's `onLoad`.
  - A dev or code reload, or a device switch in the Runner, keeps the window
    but starts fresh Logic: `onLaunch`, then `onShow` (`open`) if the lxapp is
    on screen. A hidden lxapp gets its `onShow` on the next real show. Open
    pages replay `onLoad` → `onShow` → `onReady`. A closed lxapp cannot be
    reloaded this way; open it again.

## Cold open

1. The previous main, if any: `App.onHide` (`switch_away`).
2. `App.onLaunch(options)`.
3. `App.onShow` (`open`).
4. Landing page: `onLoad(query)` → `onShow` → `onReady`.

Steps 2–4 are not awaited across each other. Other tab pages may be preloaded
but get no hook until first selected.

## Navigation

| Action | Page entered | Page left |
| --- | --- | --- |
| `navigateTo` | New instance: `onLoad` → `onShow` → `onReady` | `onHide`, `data` kept |
| `redirectTo` another page | New instance: `onLoad` → `onShow` → `onReady` | `onUnload` |
| `navigateBack` | `onShow` only, `data` kept | Each popped page: `onUnload` |
| `switchTab`, first visit | `onLoad` → `onShow` → `onReady` | Left tab: `onHide`; pushed pages: `onUnload` |
| `switchTab`, later visits | `onShow` only, `data` kept | Same |
| `reLaunch` | New instance: `onLoad` → `onShow` → `onReady` | Every loaded page, tabs included: `onUnload` |
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
- **Surface pages** opened with `lx.surface.open` get `onHide` on close and
  `onUnload` 30 seconds later unless reopened first.
