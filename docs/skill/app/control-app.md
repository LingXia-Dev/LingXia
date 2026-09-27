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

Network and privilege grants (`process`, `downloads`, `automation`, `host`) are
separate from class: see [Permissions](../native/permissions.md).

## Am I the Control app?

```ts
const control = lx.host.control;
if (!control) return;          // a guest — offer nothing that needs it
await control.appearance.setPreference('dark');
```

Bind `lx.host.control` once at the top of a Settings screen rather than writing
`lx.host.control!` at every call.

## Control-app-only calls

These act on the product, not on the calling lxapp:

- `lx.host.exit()`
- `lx.host.setBadge(...)` and `lx.tray.*` ([Badges](../lxapp/lx-api.md#badges))
- `lx.host.cache` ([Host cache](../lxapp/files.md#host-cache)); guests do not have the member
- `lx.host.checkUpdate()`, `lx.host.claimCustomUpdate()`, `lx.host.screenshot()`
  — the host app, not your bundle (that is `lx.getUpdateManager()`)
- `lx.host.autostart.*`
- `lx.host.notification.*` ([Notifications](../lxapp/lx-api.md#local-notifications))
- `lx.host.banner.*`
- `lx.host.control.displayLanguage` / `.appearance` — the setting writers
  ([Product settings](../lxapp/lx-api.md#product-settings))
- `lx.shell.*` mutations (sidebar actions, opening declared surfaces)
- `Rong.spawn` / `Rong.$` under `capabilities.process`

Reading the resolved settings (`lx.host.displayLanguage.get()`,
`lx.host.appearance.get()`) is open to every lxapp.

## Control-surface-only calls

`lx.terminal.settings`, `lx.terminal.colorSchemes`, `lx.terminal.fonts`, and
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
