# Logic runtime

What every Logic file (`pages/*/index.ts`, `lxapp.ts`, shared modules) runs
against. Logic imports the runtime from one place:
`import { definePage, lx } from '@lingxia/logic'`. `App` and the Web profile are
globals.

**`@lingxia/types` is the API reference.** Its declarations are generated from
the runtime: signatures, defaults, platforms, permissions, errors and examples
are on each member's hover. Look one up with `lingxia api lx.<namespace>` (or
`lingxia api <member>`). This page holds only the rules that span members.

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

Keep that in `tsconfig.logic.json`; the View has its own config, and the root
`tsconfig.json` references both. A GPU lxapp extends the CLI's bases instead
([GPU View](./gpu.md)).

## Finding your way

- Import shapes from the same place: `import type { PickCodeResult } from '@lingxia/logic'`.
- `lx` without the import is a type error that names it; a View never imports it
  (`UI_LX` at build), it calls an action. Automation types come from
  `@lingxia/types/automation`; Logic `lx.automation()` needs the `automation`
  (and, for other apps, `host`) [grant](../native/permissions.md).
- Navigation takes a configured **page name**, never a path. A name computed at
  runtime needs `page as ConfiguredPageName`; another lxapp's page is a plain string.
- Calls live in namespaces by subject: `lx.nav`, `lx.ui`, `lx.app`, `lx.host`,
  `lx.device`, `lx.media`, `lx.network`, `lx.pick`, `lx.fs`, `lx.clipboard`,
  `lx.storage()`, `lx.surface`, `lx.shell`.
- Page-addressed calls live on `page`, not on `lx`: its data, params, signal,
  result and channel ([Page API](./guide.md)).

## Subscriptions and cancellation

- `watch(cb)` delivers the current value, then each change; `on<Event>(cb)`
  delivers events and never replays one. The first `watch` callback can run
  before `watch` returns its unsubscribe.
- A value that changes (appearance, display language, orientation, network
  status) is a `Live<T>`: `get()` now, synchronous; `watch(cb)` to follow it.
- Every subscription returns its unsubscribe and takes `{ signal }`:
  `lx.network.status.watch(cb, { signal: page.signal })` ends with the page.
- A call that waits on the network, a radio or a person takes `signal` instead of
  a timeout (`AbortSignal.timeout(ms)`); abort rejects `E_ABORT`.

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

- What this lxapp renders in is on `lx.app`: `language` and `appearance`; every
  lxapp has them. Re-set the strings you hand to native chrome (modals, action
  sheets) when they change.
- `lx.host.displayLanguage` / `lx.host.appearance` are the user's choice
  (`'auto'` or a value) and can be set. Only the Control app may call them;
  elsewhere they reject `E_PERMISSION_DENIED`. A system change under `'auto'`
  moves only the first pair.

## Features and permission

- `lx.supports(key)` answers whether this host has a feature; `true` is not
  permission, which surfaces at the call. Capability members are always
  present: without the feature they reject `E_NOT_SUPPORTED`.
- Control-app members are not features: ask `lx.app.info.role === 'control'`
  ([Control app](../app/control-app.md)).

## Errors

A rejection means the operation failed, never that the user said no.
Dismissable calls (`lx.ui.actionSheet`, every `lx.pick.*`, `lx.clipboard`
reads) resolve a result discriminated on `status`; `lx.ui.confirm` resolves
`false`.

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
