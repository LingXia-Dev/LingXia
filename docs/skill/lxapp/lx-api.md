# Logic runtime

The globals every Logic file (`pages/*/index.ts`) runs against: `lx`, `Page`,
`App`, and the Web profile.

**`@lingxia/types` is the API reference.** Its declarations and JSDoc are
generated from the runtime and are authoritative for signatures, options,
results, defaults, and platform support. The skill docs do not copy them; read
the declarations (and `@lingxia/react` / `@lingxia/vue` / `@lingxia/bridge` for
View types). This page covers only what the declarations cannot say.

## Install typing

The scaffold configures this. For an existing lxapp, match the CLI version:

```bash
npm install --save-dev @lingxia/types@<lingxia-version>
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
- Import shapes from the root: `import type { ScanCodeResult } from '@lingxia/types'`.
  Automation types come from `@lingxia/types/automation`; Logic
  `lx.automation()` needs the `automation` (and, for other apps, `host`)
  [grant](../native/permissions.md).
- Navigation takes a configured **page name**, never a path. A name computed
  at runtime needs `page as ConfiguredPageName`; another lxapp's page
  (`lx.navigateToApp`, `lx.shell.openApp`) is a plain string.
- Most methods are flat on `lx`; namespaces include `lx.env`, `lx.host`,
  `lx.clipboard`, `lx.navigationBar`, `lx.tabBar`, `lx.tray`, `lx.shell`.

## Web globals

Logic runs in Rong, not a browser. `@lingxia/types/logic-globals` declares what
exists: `fetch`, timers, `URL`, streams, abort signals, `console`. No DOM, no
Node globals, no `WebSocket`. `fetch` follows the host
[network grant](../native/permissions.md). Process APIs are opt-in:
[`capabilities.process`](../app/project.md#capabilities).

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

lx.host.control?.displayLanguage.setPreference('zh-CN');
lx.host.control?.appearance.setPreference('dark');
```

- `get`/`watch` report what is in effect; `getPreference` / `setPreference` /
  `watchPreference` report the user's choice (a system change under `'auto'`
  moves only the first pair).
- Logic `watch(cb)` calls `cb` with the current value synchronously, before it
  returns its unsubscribe. Use it to re-set strings you hand to native chrome
  (titles, tab labels, modals).
- View: `useLxHost().displayLanguage`, and `<html lang>` / `<html dir>` are set
  for you. The View's `subscribe(cb)` fires only on change; read `get()` first.
- Narrowing to the catalogs you ship is yours: `ja-JP` with only `en`/`zh`
  renders `en` while the product stays `ja-JP`.
- `lingxia dev --display-language` shadows the effective language; there
  `get()` and `getPreference()` disagree and `setPreference` shows no effect.
  Test a Settings screen without the flag.
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
- `true` is not permission: grants and failures surface at the call. With
  `capabilities.process`, `lx.supports('process')` is true yet every call throws
  until the host grants the process resource.
- Optional namespaces share it: `terminal`, `app.autostart`,
  `app.notification`, `app.banner`.
- Required features need a matching `minRuntime`; optional ones need a
  fallback.
- Use `lx.host.control?.…` for Control-app members, not a feature key
  ([Control app](../app/control-app.md)).
- Unsupported cosmetic calls (tray on mobile) are silent no-ops; calls with a
  result reject.

## Badges

`lx.host.setBadge(value, options?)` (Control app) resolves whether anything was
painted:

- `surface: 'auto'` (default) covers dock and menu bar on macOS, taskbar and
  tray on Windows, the home-screen icon on iOS and HarmonyOS.
- Nothing to paint on resolves `false`, never rejects. **Android** always
  resolves `false`.
- **Apple**: request notification permission first
  (`lx.host.notification.requestPermission()`); a denied macOS app gets no Dock
  badge, and iOS accepts only numbers.
- A badge never prompts, and a notification never sets one. Pass `null` to
  clear.

`lx.tray.setIcon` / `setTitle` / `setMenu` / `onClick` / `show` / `hide` change
the tray item declared in [`lingxia.yaml`](../app/project.md#tray-apps).

## Desktop banner

`lx.host.banner` (Control app, macOS/Windows; present exactly when
`lx.supports('app.banner')`) draws a product card in the top-right corner. No
yaml capability.

- Without `actions`: informational, auto-dismisses after 5 s unless
  `timeoutMs`; closing resolves `{ status: 'canceled', reason: 'dismissed' }`.
- With `actions` (at most two): a gate without close control; resolves the
  chosen `action`, or `canceled` on timeout or replacement.
- The same `id` replaces (`reason: 'replaced'`); different ids queue.
- `background`: omitted/`system`, `light`, `dark`, or a `#RGB` / `#RRGGBB` /
  `#RRGGBBAA` fill. Width is 328 pt.

## Local notifications

`lx.host.notification` (Control app, needs `capabilities.notifications`):

- An immediate `show` while the product is frontmost resolves
  `status: 'suppressed'` and posts nothing; scheduled ones always present.
- `id` replaces on every path; the replaced tap target stops resolving.
- `status: 'posted'` means the OS accepted it, nothing more.
- `target` decides where a tap goes, resolved at tap time with no fallback:
  - omitted, or `{ kind: 'activate' }` — bring the product forward;
  - `{ kind: 'page', page, query }` — a page of this app, as `lx.navigateTo`;
  - `{ kind: 'app', appId, page, query }` — another lxapp, as `lx.navigateToApp`;
  - `{ kind: 'route', name, params }` — a host-registered location;
  - `{ kind: 'appLink', url }` — a URL on a configured
    [App Link](../app/applinks.md) host, delivered as `scene === 8003`.
- Android may fire schedules late and drops them on reboot; on HarmonyOS
  `silent` does nothing and `schedule` needs the agent-reminder privilege.

## Errors

A rejection means the operation failed, never that the user said no.
Dismissable APIs (`showActionSheet`, `showModal`, `chooseFile`, `pickFile`,
`pickFiles`, `chooseDirectory`, `chooseMedia`, `scanCode`, `lx.clipboard`
reads) resolve a result discriminated on `status`. `lx.share` reports
`outcome: 'completed' | 'dismissed' | 'unknown'`.

```ts
const scan = await lx.scanCode()
if (scan.status === 'canceled') return                   // the user backed out
lx.showToast({ title: scan.scanResult })    // narrowed: the payload is present
```

Branch on the numeric code through `@lingxia/types/error`, never on the
localized message:

```ts
import { parseLxApiError, formatLxApiError } from '@lingxia/types/error'

try {
  await lx.saveImageToPhotosAlbum({ filePath })
} catch (error) {
  const failure = parseLxApiError(error)
  if (!failure) throw error                 // not a runtime error; let it surface
  lx.showToast({ title: formatLxApiError(failure), icon: 'none' })
}
```

The module also exports `isLxApiError`, `requireLxApiError`,
`extractLxErrorCode`, `infoForLxErrorCode`, and `hostUpgradeRequired`; a parsed
error's `key` is an i18n key for your own copy.

## Logic and native APIs

`lx.*` belongs to Logic. `#[lingxia::native]` routes are called from the View
through `@lingxia/native`; a `lingxia::js` extension adds `lx.<namespace>.*` to
Logic. Both: [Native development](../native/development.md).
