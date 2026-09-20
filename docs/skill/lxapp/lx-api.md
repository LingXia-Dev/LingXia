# Logic runtime and typings

Every lxapp Logic file (`pages/*/index.ts`) runs against the global `lx`,
`Page`, and `App` objects.

**`@lingxia/types` is the API reference.** Its declarations and JSDoc are
generated from the runtime's Rust API definitions and are authoritative for
method signatures, option and result shapes, defaults, restrictions, platform
support, and task behavior. Do not maintain a second API catalog in Markdown.
This page only explains how to wire those declarations into a project and how
the Logic runtime differs from View.

For page mechanics (`data`, `setData`, lifecycle), see [`./guide.md`](./guide.md).
For stream and channel behavior, see [`./bridge.md`](./bridge.md).

---

## Install typing

The LingXia scaffold configures this automatically. For an existing lxapp,
install the package at the same version as the `lingxia` CLI:

```bash
npm install --save-dev @lingxia/types@<lingxia-version>
```

Logic needs both the LingXia globals and the generated portable Web API profile:

```json
{
  "compilerOptions": {
    "lib": ["ES2020"],
    "types": ["@lingxia/types", "@lingxia/types/logic-globals"]
  }
}
```

Keep that configuration in `tsconfig.logic.json`; View uses a separate config
with the DOM library. The scaffold's root `tsconfig.json` references both so the
editor applies the correct environment to each file.

---

## Find a method or type

- Navigation takes a configured page name, and after a build completion offers
  only this lxapp's own. A name computed at runtime needs
  `page as ConfiguredPageName`; another lxapp's page
  (`lx.navigateToApp`, `lx.shell.openApp`) stays a plain string.
- Type `lx.` in the editor and hover a member to read its generated JSDoc.
- Import reusable shapes from the package root, for example
  `import type { ScanCodeResult } from '@lingxia/types'`.
  Automation types come from `@lingxia/types/automation`.
- For the complete declaration, inspect
  `node_modules/@lingxia/types/dist/generated/logic.d.ts`.

Most methods are flat on `lx`. Related capabilities use typed namespaces such
as `lx.env`, `lx.host`, `lx.clipboard`, `lx.navigationBar`, `lx.tabBar`,
`lx.tray`, and `lx.shell`; editor completion is the authoritative namespace
map. Page Chrome geometry is a View concern exposed through the framework
page-chrome helpers and the low-level `window.lxPageChrome` snapshot.

---

## Standard Web APIs (built-in globals)

Logic runs in Rong rather than a browser. Its portable Web globals are declared
by `@lingxia/types/logic-globals`; this includes APIs such as `fetch`, timers,
`URL`, streams, abort signals, and `console`, but excludes browser DOM and Node
globals. If a global is absent from that profile, application Logic must not
assume it exists.

`fetch` follows the host's network grant — unrestricted unless the app registry
returns one; see [Security Policy](./guide.md#security-policy). The Logic Web
profile does not include `WebSocket`.

OS process APIs are a separate host capability with opt-in declarations at
`@lingxia/types/process`; see
[`capabilities.process`](../app/project.md#capabilities-section).

---

## Runtime convention

### The product owns its settings

A setting the user recognises as belonging to the whole product — its language,
its light/dark scheme — has exactly one value and exactly one writer, the
product's Settings surface. No lxapp, panel, or built-in screen keeps a second
one, and none offers the user a picker of its own.

Narrowing what the product hands you is a different thing, and is invisible to
the user: shipping catalogs for two languages and falling back for the rest, or
declaring in `lxapp.json` that this lxapp's UI only works in dark. Those are
static properties of your code, not preferences someone chose.

So each of these reads the same way: a pair on `lx.host` that every lxapp
follows, and a writer behind `lx.host.control` that only the Control app has.

```ts
lx.host.displayLanguage.get();        lx.host.displayLanguage.watch(cb);
lx.host.appearance.get();             lx.host.appearance.watch(cb);

lx.host.control?.displayLanguage.setPreference('zh-CN');
lx.host.control?.appearance.setPreference('dark');
```

`get`/`watch` answer what is in effect. `getPreference`/`setPreference`/
`watchPreference` answer what the user chose — a system change under `'auto'`
moves the first pair and leaves the second quiet.

### Everything else

Unsupported cosmetic capabilities with no meaningful result, such as desktop
tray presentation on mobile, are silent no-ops. Result-bearing operations and
invalid usage reject or throw. Each generated method's JSDoc is authoritative
for its exact behavior.

Use `lx.supports(feature)` for optional feature contracts:

```ts
if (lx.supports('surface.window.fullChrome')) {
  // offer a window with full chrome
}
lx.surface.watchContext(({ aside }) => {
  // aside is live host docking availability, independent of viewport sizeClass
});
```

The supported set is frozen per Logic context. Unknown strings return false;
non-strings throw TypeError. `LxFeature` is generated from the runtime registry.
Required features need an appropriate `lxapp.json` `minRuntime`; optional ones
use supports and a fallback. Permissions, grants and resource failures are checked at the
operation, so true is not permission or a promise of success.

`process` is where that gap shows: `lx.supports('process')` is true and
`lx.process` exists in a Control app that declared `capabilities.process`, but
every call throws until the native host has granted the process resource.

Optional namespaces and their base feature share the frozen set: `terminal`,
`app.autostart`, `app.notification`, `app.banner`. For Control app identity
and its product-wide cache API use `lx.host.control !== undefined`, not a
feature key. `main` and `float` are baseline surface placements in ordinary
lxapp Logic; use them directly without a supports query. Focused Terminal
Settings contexts still omit the general app and surface APIs.

`lx.host.control` holds the product-wide settings and their single writer. It is
injected only into the app the host sealed as its Control app at build time, so
the same lxapp opened as a guest elsewhere simply does not have it. Write
`lx.host.control?.…`.

`lx.terminal.settings`, `colorSchemes`, `fonts`, and Windows terminal control
are additionally restricted to the host-bundled Terminal Settings session the
native host assigned as a control surface; not even the Control app reaches
them. A matching app id or bundled source does not grant this authority.

Which session is which, the full list of Control-app-only calls, and what a
refusal reads like: [The Control app](../app/control-app.md).

---

## Badges

`lx.host.setBadge(value, options?)` — Control-app only, resolves whether
anything was painted. Signatures are in `@lingxia/types`; what they do
not say:

- One call covers every product-owned surface. `surface: 'auto'` (the default)
  marks the dock *and* the menu-bar item on macOS, the taskbar *and* the
  notification-area item on Windows, the home-screen icon on iOS and HarmonyOS.
  There is no separate tray badge call.
- A surface with nothing to paint on resolves `false`, never a rejection —
  no such chrome on this platform, or a macOS tray the product has not shown.
- The method is always present. Call `await lx.host.setBadge(count)` directly;
  use its boolean result if the product needs to know whether it painted.
  There is no separate badge capability query.
- **Android returns `false`.** There is no cross-vendor launcher badge; what a
  launcher shows comes from active notifications, not from a standalone count.
- **Both Apple platforms tie the badge to notification permission**, in
  different ways. On macOS the label always reaches the system, but the Dock
  refuses to draw it for an app that is registered with Notification Center
  and not allowed — so a host that declares `capabilities.notifications` and
  whose user dismissed or denied the prompt gets `false` and no badge, while
  a host that never asks is unaffected. Call
  `lx.host.notification.requestPermission()` before you rely on a count.
- **iOS needs notification permission** and only accepts a number. The
  home-screen badge is drawn by the notification system, so a build that never
  asked cannot paint one, and a non-numeric value is a parameter error rather
  than a silent clear. That is the OS's rule; a badge is otherwise independent
  of `lx.host.notification`, which never changes it.
- A badge is decoration: it never prompts, never interrupts, and posting a
  notification does not set one.

## Desktop banner

`lx.host.banner` — Control-app only, desktop only (macOS / Windows). Not an OS
notification and not bound to App Link. Presence and
`lx.supports('app.banner')` always agree; guests and mobile builds
do not have the member. No yaml capability: this is product-drawn chrome.

- No `actions`: informational card, auto-dismisses in 5s unless `timeoutMs` is
  set. The close control resolves `{ status: 'canceled', reason: 'dismissed' }`.
- With `actions` (at most two): a gate. No close control. Resolves the chosen
  `action` id, or `canceled` on timeout / replace. Failures to present reject.
- Same `id` replaces the current card (`reason: 'replaced'`). Different ids
  queue. Host Rust uses the same primitive: `lingxia::app::banner::show`. That
  call blocks until the card resolves — from an async Rust task use a blocking
  worker. Do not call a no-timeout prompt on the macOS main thread: the panel
  is presented on main, and a click cannot run while `show` holds it.
- `background` is optional: omit/`system` follows the OS (vibrancy on macOS);
  `light`/`dark` force chrome; `#RGB` / `#RRGGBB` / `#RRGGBBAA` paints a solid
  fill and picks title contrast from luminance. Width is fixed at 328 pt.

## Local notifications

`lx.host.notification` — Control-app only, absent without
`capabilities.notifications`. Signatures are in `@lingxia/types`; what they do
not say:

- An immediate `show` while the product is frontmost resolves
  `status: 'suppressed'` and posts nothing. A scheduled one is presented when
  it fires, frontmost or not.
- `id` replaces on every path, `'suppressed'` included, and the replaced
  notification's tap target stops resolving at the same moment.
- `status: 'posted'` means the OS accepted it for display. Nothing reports that
  anyone saw, read, or acted on it.
- `target` decides where a tap goes:
  - omitted, or `{ kind: 'activate' }` — bring the product forward, nothing else;
  - `{ kind: 'page', page, query }` — a page of this Control app, same contract
    as `lx.navigateTo`. Ordinary scene, not an App Link.
  - `{ kind: 'app', appId, page, query }` — another lxapp, same contract as
    `lx.navigateToApp`.
  - `{ kind: 'route', name, params }` — a location the host registered at
    startup that is not a page. Ask the host which names and parameters exist;
    an unknown name or an undeclared parameter rejects at `show`.
  - `{ kind: 'appLink', url }` — an `https://` URL on a configured
    [App Link](../app/applinks.md) host, delivered as `scene === 8003`. Use this
    only for a real inbound product URL, not as a stand-in for `page` / `app`.
- A tap resolves the target again when it happens. A page or route the build no
  longer has, a cancelled or replaced notification, or cleared app data brings
  the product forward and reports that it is unavailable — it never falls back
  to some other target.
- Limits: Android battery saver can fire a schedule minutes late, and a reboot
  drops it. HarmonyOS banners are a user-only system toggle, `silent` does
  nothing there, and `schedule` rejects unless Huawei granted the app the
  agent-reminder privilege.

---

## Handling errors

A rejection means the operation failed. It never means the user said no. The
dismissable APIs — `showActionSheet`, `showModal`, `chooseFile`,
`chooseDirectory`, `chooseMedia`, `scanCode`, and the `lx.clipboard` reads
`readText` / `read` (iOS 16+ and macOS 15.4+ may show a paste prompt) — resolve
a result discriminated on `canceled`, so dismissal is a branch, not an error
path.

`lx.share` is the exception: some platforms only observe that the system sheet
opened and closed, so it reports a three-state `outcome` —
`'completed' | 'dismissed' | 'unknown'` — rather than claiming a certainty it
does not have.

```ts
const scan = await lx.scanCode()
if ((scan.status === 'canceled')) return                   // the user backed out
lx.showToast({ title: scan.scanResult })    // narrowed: the payload is present
```

A rejection carries a numeric code from the runtime's error registry, which is
generated from the same Rust definitions as the typings. Read that code through
`@lingxia/types/error`; never branch on the message text, which is localized and
not a contract:

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

`parseLxApiError` returns `null` for anything that is not a recognized runtime
error, so a genuine bug stays distinguishable from a known failure. The
module also exports `isLxApiError` as a type guard, `requireLxApiError` when an
unrecognized error should escalate, and `extractLxErrorCode` /
`infoForLxErrorCode` for direct registry access. A parsed error's `key` is an
i18n key, so a product with its own copy can look up wording instead of showing
the runtime's message.

---

## Logic and native APIs

`lx.*` belongs to Logic. Host routes declared with `#[lingxia::native(...)]` are
called from View through the generated `@lingxia/native` client. To expose a
host Rust helper to Logic as `lx.<namespace>.*`, define a `lingxia::js`
extension. See [Native development](../native/development.md) for both models.
