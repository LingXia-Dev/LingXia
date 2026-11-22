# Control app

Which lxapp session is trusted, and what each class may call. A session's class
is fixed by the host when it creates the session; nothing in a manifest,
payload, or app id can claim one.

## Session classes

| Class | Which session | How it is designated |
|---|---|---|
| **Control app** | The product's own home lxapp (at most one). | `app.homeAppId` in `lingxia.yaml`. |
| **Control surface** | A Settings screen the host bundles (Terminal Settings). | Opened by the Control app; must be host-bundled. |
| **Standard app** | Guests, downloaded lxapps, anything the user opens. | Default. |

The classes are disjoint: a control surface reaches none of the Control app's
calls, and vice versa. The home lxapp opened as a guest elsewhere is a Standard
app, with the same id and code.

Network and privilege grants (`downloads`, `automation`, `host`) are
separate from class: see [Permissions](../native/permissions.md).

## Am I the Control app?

```ts
if (lx.app.info.role !== 'control') return; // offer nothing that needs it
await lx.host.appearance.preference.set('dark');
```

`role` is `'control'`, `'control_surface'` or `'guest'`. Control-only members
are present for every lxapp and refuse a guest at the call.

## Control-app-only calls

These act on the product, not on the calling lxapp:

- `lx.host.exit()`
- `lx.host.setBadge(...)` and `lx.host.tray.*`
- `lx.host.cache` ([Host cache](../lxapp/files.md#host-cache)); guests do not have the member
- `lx.host.update.*`, `lx.host.screenshot()`
  — the host app, not your bundle (that is `lx.app.update.onReady`)
- `lx.host.autostart.*`
- `lx.host.displayLanguage.preference` / `lx.host.appearance.preference`
  ([Product settings](../lxapp/core.md#product-settings))
- `lx.shell.*`: sidebar actions, `setAppDisplay`, `activate()`
- `lx.surface.app(appId)` / `lx.surface.declared(id)` to find a live app or declared surface
- `lx.surface.openApp`, and `lx.surface.openDeclared` with `key`, `as` or
  `edge`: other lxapps are refused at run time

Reading the resolved settings (`lx.host.displayLanguage.get()`,
`lx.host.appearance.get()`) is open to every lxapp.

`lx.host.notification` and `lx.host.banner` are open to every lxapp, scoped to the caller for guests;
their declarations list a guest's limits.

## Control-surface-only calls

`lx.host.terminal.settings`, `lx.host.terminal.colorSchemes`, `lx.host.terminal.fonts`, and
the Windows terminal controls. The Control app cannot call them; it opens the
Settings screen instead.

## Refusal

A call from the wrong class rejects with `E_PERMISSION_DENIED`, naming the
admitted class:

```
lx.host.setBadge is only available in the Control app
```

Do not retry or impersonate; ask the Control app to act. It is never the user
declining (that resolves `status: 'canceled'`) and never a missing capability
(`lx.supports()` answers that).

Native routes express the same classes as their
[`audience`](../native/development.md#route-audience).
