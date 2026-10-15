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
| Reopened while still alive | `onShow` with path/query | `lxapp` | `switch_back` |
| Another lxapp opened as the main | `onHide` | `lxapp` | `switch_away` |
| Closed | `onHide` | `lxapp` | `close` |
| Host app went to background / came back | `onHide` / `onShow` | `host` | `background` / `foreground` |
| Screenshot, where the platform reports it | `onUserCaptureScreen` | — | — |

- **An aside does not hide the main.** `lx.shell.openApp(id, { as: 'aside' })`
  opens beside the main, which gets no `onHide`.
- **AppLink arrives once.** A cold AppLink is in `onLaunch` options
  (`scene: 8003`, `url`); the `onShow` that follows carries no link. A warm
  AppLink is in `onShow` with `reason: 'switch_back'`.
- **`onLaunch` runs once per Logic start.** Restarts start Logic again:
  - `lx.update.applyUpdate()` closes the session (`onHide`, `close`) and opens
    a new one: `onLaunch`, `onShow` (`open`), then the initial page's `onLoad`.
  - A dev or code reload keeps the window but starts fresh Logic: `onLaunch`,
    then `onShow` (`open`) if the lxapp is on screen. A hidden lxapp gets its
    `onShow` on the next real show. Open pages replay `onLoad` → `onShow` →
    `onReady`.

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
| Reopen at a named page (link, notification) | `onLoad` with the new query | — |
| Host background / foreground | `onShow` | `onHide` |

- **`onReady` fires once per rendered document, not per entry.** A return to a
  kept page gets `onShow` only.
- **A hidden page can lose its WebView without a hook.** Desktop hosts may
  reclaim a hidden lxapp's inactive tab. `data` survives; the next show brings
  `onShow` and a new `onReady`, but no `onLoad`. Do not keep state only in the
  View.
- **Surface pages** opened with `lx.surface.open` get `onHide` on close and
  `onUnload` 30 seconds later unless reopened first.
