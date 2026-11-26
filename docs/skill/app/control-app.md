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
await lx.host.appearancePreference.set('dark');
```

`role` is `'control'`, `'control_surface'` or `'guest'`. Control-only members
are present for every lxapp and refuse a guest at the call.

## Control-app-only calls

Calls that act on the product rather than the calling lxapp are the Control app's: most of
`lx.host` (exit, badge, tray, cache, service environment, updates, autostart, the product's
language and appearance preferences), `lx.shell`, and opening or finding other apps' surfaces
(`lx.surface.openApp`). Each member's declaration says `Control app only`; `lingxia api <member>`
shows it. `lx.host.update` is the host app's update; your bundle's is `lx.app.update.onReady`.

Open to every lxapp: reading the resolved settings (`lx.app.language.get()`,
`lx.app.appearance.get()`), and `lx.host.notification` / `lx.host.banner`, scoped to the caller
for guests.

## Control-surface-only calls

`lx.host.terminal.settings`, `lx.host.terminal.colorSchemes`, `lx.host.terminal.fonts`, and
the Windows terminal controls. The Control app cannot call them; it opens the
Settings screen instead.

## Refusal

A call from the wrong class rejects with `E_PERMISSION_DENIED`, naming the
admitted class:

```
lx.host.exit is only available in the Control app
```

Do not retry or impersonate; ask the Control app to act. It is never the user
declining (that resolves `status: 'canceled'`) and never a missing capability
(`lx.supports()` answers that).

Native routes express the same classes as their
[`audience`](../native/development.md#route-audience).
