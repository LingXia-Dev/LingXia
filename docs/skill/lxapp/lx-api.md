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

- Type `lx.` in the editor and hover a member to read its generated JSDoc.
- Import reusable shapes from the package root, for example
  `import type { ScanCodeResult } from '@lingxia/types'`.
- For the complete declaration, inspect
  `node_modules/@lingxia/types/dist/generated/logic.d.ts`.

Most methods are flat on `lx`. Related capabilities use typed namespaces such
as `lx.env`, `lx.app`, `lx.appearance`, `lx.navigationBar`, `lx.tabBar`,
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

`fetch` and `WebSocket` are still constrained by
`security.network.trustedDomains`; see [Security Policy](./guide.md#security-policy).
OS process APIs are a separate host capability with opt-in declarations at
`@lingxia/types/process`; see
[`capabilities.process`](../app/project.md#capabilities-section).

---

## Runtime convention

Unsupported cosmetic capabilities with no meaningful result, such as desktop
tray presentation on mobile, are silent no-ops. Result-bearing operations and
invalid usage reject or throw. Each generated method's JSDoc is authoritative
for its exact behavior.

Ask before you offer, with `lx.supports(query)`:

```ts
if (lx.supports({ capability: 'surface', value: 'window' })) {
  // render "Open in new window"
}
```

The catalog is a closed union, so completion enumerates it and a typo is a
compile error. The answer is live — `{ capability: 'surface', value: 'aside' }` changes when a
desktop window crosses the compact breakpoint, so pair it with
`lx.onSurfaceContext` rather than caching it. It is an affordance for deciding
what to render and never replaces handling a rejection: the answer can be stale
by the time you act on it, and every gated operation still rejects.

A whole namespace that a host may not carry at all stays an optional member —
`lx.terminal`, `lx.app.autostart`. Presence and `lx.supports()` are answered
from one registry, so `('terminal' in lx)` and `lx.supports({ capability: 'terminal' })`
can never disagree.

---

## Handling errors

A rejection means the operation failed. It never means the user said no: the
six dismissable APIs — `showActionSheet`, `showModal`, `chooseFile`,
`chooseDirectory`, `chooseMedia`, `scanCode` — resolve a result discriminated
on `canceled`, so dismissal is a branch rather than an error path. (`lx.share` stands apart: some
platforms only observe that the system sheet opened and closed, so it reports a
three-state `outcome` — `'completed' | 'dismissed' | 'unknown'` — rather than
claiming a certainty it does not have.)

```ts
const scan = await lx.scanCode()
if (scan.canceled) return                   // the user backed out
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
