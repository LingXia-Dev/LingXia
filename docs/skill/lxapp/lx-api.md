# Logic runtime

What every Logic file (`pages/*/index.ts`, `lxapp.ts`, shared modules) runs
against. Logic imports the runtime from one place:
`import { definePage, lx } from '@lingxia/logic'`. `App` and the Web profile are
globals.

**`@lingxia/types` is the API reference.** Its declarations and JSDoc are
generated from the runtime and are authoritative for signatures, options,
results, defaults, and platform support. The skill docs do not copy them; read
the declarations (and `@lingxia/react` / `@lingxia/bridge` for
View types). This page covers only what the declarations cannot say.

## Install typing

The scaffold configures this. For an existing lxapp, match the CLI version:

```bash
npm install @lingxia/logic@<lingxia-version>
```

```json
{
  "compilerOptions": {
    "lib": ["ES2020"],
    "types": ["@lingxia/types", "@lingxia/types/logic-globals"]
  }
}
```

Keep that in `tsconfig.logic.json`; the View has its own config with the DOM
library, and the root `tsconfig.json` references both.

## Find a method or type

- Type `lx.` in the editor and hover for JSDoc; the full declaration is
  `node_modules/@lingxia/types/dist/generated/logic.d.ts`.
- Import shapes from the same place: `import type { PickCodeResult } from '@lingxia/logic'`.
- `lx` without the import is a type error that names it; a View never imports it
  (`UI_LX` at build), it calls an action.
  Automation types come from `@lingxia/types/automation`; Logic
  `lx.automation()` needs the `automation` (and, for other apps, `host`)
  [grant](../native/permissions.md).
- Navigation takes a configured **page name**, never a path. A name computed
  at runtime needs `page as ConfiguredPageName`; another lxapp's page
  (`lx.nav.go({ app })`, `lx.surface.openApp`) is a plain string.
- Calls live in namespaces by subject: `lx.nav`, `lx.ui`, `lx.app`, `lx.host`,
  `lx.device`, `lx.media`, `lx.network`, `lx.pick`, `lx.fs`, `lx.clipboard`,
  `lx.storage()`, `lx.surface`, `lx.shell`.

## Subscriptions

- `watch(cb)` delivers the current value, then each change; `on<Event>(cb)`
  delivers events and never replays one. The first `watch` callback can run
  before `watch` returns its unsubscribe.
- Every subscription returns its unsubscribe and takes `{ signal }`:
  `lx.network.watch(cb, { signal: page.signal })` ends with the page.

## Web globals

Logic runs in Rong, not a browser. `@lingxia/types/logic-globals` declares what
exists: `fetch`, timers, `URL`, streams, abort signals, `console`. No DOM, no
Node globals, no `WebSocket`. `fetch` follows the host
[network grant](../native/permissions.md).

Consume server-sent events with `Rong.SSE`, not `fetch` (Logic `fetch` hands a
streamed body over in large chunks):

```ts
const events = new (Rong as any).SSE(url, {
  headers: { Authorization: `Bearer ${token}` },
  reconnect: { baseDelayMs: 1000, maxDelayMs: 30000 },
});
for await (const { type, data, id } of events) { /* … */ }
events.close();
```

## Product settings

The product's language and light/dark scheme each have one value and one
writer: the product's Settings screen in the [Control app](../app/control-app.md).
No lxapp keeps its own copy or offers its own picker.

```ts
lx.host.displayLanguage.get();        lx.host.displayLanguage.watch(cb);
lx.host.appearance.get();             lx.host.appearance.watch(cb);

lx.host.displayLanguage.setPreference('zh-CN');
lx.host.appearance.setPreference('dark');
```

- `get`/`watch` report what is in effect; `getPreference` / `setPreference` /
  `watchPreference` report the user's choice (a system change under `'auto'`
  moves only the first pair).
- Use `watch` to re-set strings you hand to native chrome (titles, tab
  labels, modals).
- View: `useLxHost().displayLanguage`. The View's `subscribe(cb)` fires only
  on change; read `get()` first.
- List the languages you ship in lxapp.json `languages` (`["en", "zh"]`, first
  is the fallback). The View's `contentLanguage` (`useLxHost()`, GPU `env`) is
  the display language narrowed to them — `ja-JP` gives `en` — and sets
  `<html lang>` / `<html dir>` and built-in control words. Logic narrows itself.
- `lxdev runner set --language` shadows the effective language; there
  `get()` and `getPreference()` disagree and `setPreference` shows no effect.
  Test a Settings screen without it.
- The starting scheme is the host's `theme.defaultAppearance`; a one-scheme
  lxapp declares `"appearance"` in `lxapp.json`
  ([Appearance](./guide.md#appearance)).

## `lx.supports`

```ts
if (lx.supports('surface.window.fullChrome')) {
  // offer a window with full chrome
}
```

- The set is frozen per Logic context. Unknown strings return false;
  non-strings throw `TypeError`.
- `true` is not permission: grants and failures surface at the call.
- Capability members are always present: `lx.host.terminal`, `autostart`,
  `notification`, `banner`. Without the feature their calls fail
  `E_NOT_SUPPORTED`; without the right (`lx.host.cache` outside the Control
  app), `E_PERMISSION_DENIED`.
- Required features need a matching `minRuntime`; optional ones need a
  fallback.
- Ask `lx.app.info.role === 'control'` for Control-app members, not a feature key
  ([Control app](../app/control-app.md)).
- Unsupported cosmetic calls (tray on mobile) are silent no-ops; calls with a
  result reject.

## Badges

`lx.host.setBadge(value, options?)` (Control app) resolves `{ painted, reason? }`:

- `surface: 'auto'` (default) covers dock and menu bar on macOS, taskbar and
  tray on Windows, the home-screen icon on iOS and HarmonyOS.
- No such surface resolves `reason: 'no_surface'`, never rejects. **Android** always does.
- **Apple**: request notification permission first
  (`lx.host.notification.requestPermission()`); a denied macOS app gets no Dock
  badge, and iOS accepts only numbers.
- A badge never prompts, and a notification never sets one. Pass `null` to
  clear.

`lx.host.tray.setIcon` / `setTitle` / `setMenu` / `onClick` / `show` / `hide` change
the tray item declared in [`lingxia.yaml`](../app/project.md#tray-apps).

## Desktop banner

`lx.host.banner` (macOS/Windows; elsewhere calls fail `E_NOT_SUPPORTED`, so ask
`lx.supports('app.banner')`) draws a product card in the top-right corner. No
yaml capability. Guests: see [Guest lxapps](#guest-lxapps).

- Without `actions`: informational, auto-dismisses after 5 s unless
  `timeoutMs`; closing resolves `{ status: 'canceled', reason: 'dismissed' }`.
- With `actions` (at most two): a gate without close control; resolves the
  chosen `action`, or `canceled` on timeout or replacement.
- The same `id` replaces (`reason: 'replaced'`); different ids queue.
- `background`: omitted/`system`, `light`, `dark`, or a `#RGB` / `#RRGGBB` /
  `#RRGGBBAA` fill. Width is 328 pt.

## Local notifications

`lx.host.notification` (needs `capabilities.notifications`; guests: see
[Guest lxapps](#guest-lxapps)):

- An immediate `show` while the product is frontmost resolves
  `delivery: 'suppressed'` and posts nothing; scheduled ones always present.
- `id` replaces on every path; the replaced tap target stops resolving.
- `delivery: 'posted'` means the OS accepted it, nothing more.
- `target` decides where a tap goes, resolved at tap time with no fallback.
  It takes what `lx.nav.go` takes (`{ page, params }`, `{ tab }`, `{ app, page }`),
  plus `{ activate: true }` (the default: bring the product forward),
  `{ route, params }` (a host-registered location) and `{ appLink }` (a URL on a
  configured [App Link](../app/applinks.md) host, delivered to `App.onOpen`).
  A tab root given as `{ page }` is refused; name it with `{ tab }`.
- Android may fire schedules late and drops them on reboot; on HarmonyOS
  `silent` does nothing and `schedule` needs the agent-reminder privilege.

## Guest lxapps

A guest (any lxapp but the Control app) may use `lx.host.notification` and
`lx.host.banner` for itself; refusals reject with `E_PERMISSION_DENIED`.

- Ids are its own; `cancelAll()` clears only its own.
- A notification opens only the guest: omit `target`, or `{ page }`, `{ tab }`,
  or `{ app }` naming itself.
- Per guest: 32 live notifications; 3 banners showing or queued, each up for
  60 s at most and queued behind the product's. Its banner shows its name and icon.
- One permission prompt per launch, only while it is open: ask from a user action.
- A banner action's `target` is where picking it goes, scoped like a
  notification target; the product comes forward and the target opens:

```ts
await lx.host.banner.show({
  title: 'Run finished',
  actions: [{ id: 'open', label: 'Open', target: { page: 'run', params: { id: runId } } }],
})
```

- Only the Control app brings the product forward on its own: `lx.shell.activate()`.

## Errors

A rejection means the operation failed, never that the user said no.
Dismissable APIs (`lx.ui.actionSheet`, every `lx.pick.*`, `lx.clipboard`
reads) resolve a result discriminated on `status`; `lx.ui.confirm` resolves
`false`. `lx.share` reports
`outcome: 'completed' | 'dismissed' | 'unknown'`.

```ts
const scan = await lx.pick.code()
if (scan.status === 'canceled') return                   // the user backed out
lx.ui.toast(scan.value)                    // narrowed: the payload is present
```

Branch on `code` with `isLxError`, never on the localized message:

```ts
import { isLxError, lx } from '@lingxia/logic'

try {
  await lx.media.saveToLibrary(path)
} catch (e) {
  if (isLxError(e) && e.code === 'E_PERMISSION_DENIED') return lx.ui.toast('Allow photo access in Settings')
  throw e
}
```

`instanceof` does not work for these errors yet; `hostUpgradeRequired(e)` says the host app needs updating.

## Logic and native APIs

`lx.*` belongs to Logic. `#[lingxia::native]` routes are called from the View
through `@lingxia/native`; a `lingxia::js` extension adds `lx.<namespace>.*` to
Logic. Both: [Native development](../native/development.md).
