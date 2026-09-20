---
title: Control app
description: Which lxapp is trusted, what only that session may call, and why an app id is not a grant.
sidebar:
  order: 7
---

A product is one app to its user, but several lxapps to the runtime: the home face the host ships, host-bundled Settings screens, and guests the user opens later. They share the same `lx.*` surface and can even share an app id. **Whether a call is allowed is answered from the session class the host created**, not from the code that is running.

## Three session classes

| Class | Which session | How it is designated |
|---|---|---|
| **Control app** | The product's own home lxapp — at most one, and only if the product has one | `app.homeAppId` in `lingxia.yaml`, sealed into the build |
| **Control surface** | A Settings screen the host itself bundles (for example Terminal Settings) | Opened as a surface by the Control app; must be a host-bundled lxapp |
| **Standard app** | Guests, downloaded lxapps, anything the user opens | The default |

The classes are disjoint. Re-opening the home lxapp as a guest somewhere else yields a Standard app with the same app id and the same code. A desktop host with `--control native` and no home lxapp has no Control app session.

## Check before offering product chrome

```ts
const control = lx.host.control
if (!control) return
await control.appearance.setPreference('dark')
```

`lx.host.control` exists only in the Control app, so `lx.host.control !== undefined` identifies it. Bind the handle once; do not write `lx.host.control!` at every call.

## What only the Control app may call

These act on the product, not on the calling lxapp:

- `lx.host.exit()`, `lx.host.setBadge()`, `lx.host.cache`, `lx.host.checkUpdate()`, `lx.host.claimCustomUpdate()`, `lx.host.screenshot()`, `lx.host.autostart.*`
- `lx.host.control.displayLanguage` / `lx.host.control.appearance` (writers)
- `lx.shell.*` mutations — `sidebarActions`, opening or reconfiguring declared surfaces

Every lxapp may still **read** `lx.host.displayLanguage.get()` and `lx.host.appearance.get()`, and should follow those values. Bundle updates for this lxapp stay on `lx.getUpdateManager()`.

A refusal is `E_PERMISSION_DENIED` and names the class that would have been admitted. Treat it as a design signal: ask the Control app to do the product work; do not impersonate it. It is not the user dismissing a dialog, and it is not a missing capability (`lx.supports()` answers that).

Privileges such as `process`, `downloads`, and `automation` are host grants sealed to that exact session. `capabilities.process` is granted only to the Control app, and every spawn rechecks the live grant.

See [Native host apps](../native-host-apps/) for how `homeAppId` is sealed, and [Adaptive surfaces](../adaptive-surfaces/) for `lx.shell.sidebarActions`.
