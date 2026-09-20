# The Control app — which lxapp is trusted, and what that means

A product is one app to its user, but several lxapps to the runtime: the one
the host ships as its own face, the host's own Settings screens, and whatever
guests the user opens afterwards. They run the same runtime and can share an
app id, so "is this call allowed?" cannot be answered from the code that is
running. It is answered from **which session the host created**.

Read this before you reach for an API that acts on the product rather than on
your own lxapp — quitting it, badging its icon, changing its language or
scheme, or rearranging its shell.

## The three session classes

A session's class is fixed when the host creates it. Nothing an lxapp does at
runtime changes it, and nothing in a manifest, a payload, or an app id can
claim one.

| Class | Which session | How it is designated |
|---|---|---|
| **Control app** | The product's own home lxapp — exactly one per product, and only if the product has one. | `app.homeAppId` in `lingxia.yaml`, sealed into the build. |
| **Control surface** | A Settings screen the host itself bundles (the Terminal Settings screen). | Opened as a surface by the Control app; must be a host-bundled lxapp. |
| **Standard app** | Everything else: guests, downloaded lxapps, anything the user opens. | The default. |

The classes are **disjoint**. A control surface is not a restricted Control
app: it reaches its own routes and none of the Control app's, and the Control
app does not reach the surface's. Re-opening the home lxapp as a guest
somewhere else gives you a Standard app, with the same app id and the same
code.

## Am I the Control app?

```ts
const control = lx.host.control;
if (!control) return;          // a guest — offer nothing that needs it
await control.appearance.setPreference('dark');
```

`lx.host.control !== undefined` identifies the Control app. Bind the member once
at the top of a Settings screen rather than writing `lx.host.control!` at every call.

## What only the Control app may call

These act on the product, not on the lxapp that called them, which is why a
guest cannot reach them:

- `lx.host.exit()` — quits the product.
- `lx.host.setBadge(value, options?)` — a count on the product's own chrome,
  wherever this platform shows one. `lx.tray.*` is Control-app only for the
  same reason: the status item belongs to the product.
- `lx.host.cache` — every lxapp the host has ever run. Injected only into the
  Control app, same presence as `lx.host.control`; guests do not have the member.
- `lx.host.checkUpdate()`, `lx.host.claimCustomUpdate()`, and
  `lx.host.screenshot()` — the native host app, not your bundle. A check is a
  query; claiming (or `update.apply()`) takes over the built-in auto-flow for
  the rest of the process. (Your own bundle's updates are
  `lx.getUpdateManager()`, which every lxapp has.)
- `lx.host.autostart.*` — launch at login.
- `lx.host.notification.*` — local banners that resume the product. Tap
  target is `page` / `app` / `route` / `appLink` / `activate`.
- `lx.host.banner.*` — product-drawn top-right desktop card (inform or confirm).
- `lx.host.control.displayLanguage` / `lx.host.control.appearance` — the writers
  behind the product's language and light/dark setting.
- `lx.shell.*` mutations — sidebar actions, opening declared surfaces,
  reconfiguring the shell.

Everything else on `lx.*` is available to any lxapp. Reading what those
settings resolved to is not restricted either: `lx.host.displayLanguage.get()`
and `lx.host.appearance.get()` are for everyone, and every lxapp should follow
them rather than keeping a preference of its own.

## What only a control surface may call

`lx.terminal.settings`, `lx.terminal.colorSchemes`, `lx.terminal.fonts`, and
the Windows terminal controls belong to the host-bundled Settings screen. The
Control app cannot call them either — it *opens* that screen instead. Matching
the app id or shipping the same bundle does not grant this.

## What a refusal looks like

A call you are not the right class for rejects with `E_PERMISSION_DENIED`, and
the message names the class that would have been admitted:

```
lx.host.setBadge is only available in the Control app
```

Treat that as a design signal, not something to retry or route around. If a
guest screen needs the product to do something, the Control app is what does
it — ask it, don't impersonate it.

Two things it never means: it is not the user declining (dismissable APIs
resolve a `status: 'canceled'` result instead), and it is not a missing capability
(`lx.supports()` answers that, and an absent namespace is simply absent).

## Privileges are grants, never claims

A privilege — `process`, `downloads`, `automation`, `host` — is decided by the
host, per session: the app registry says which classes this app id may use on
this channel, and the native host seals the resulting grant before that
session's Logic starts. An lxapp asks for nothing in its manifest, and two
sessions with identical app ids and identical packages can end up with
different grants.

`capabilities.process` is granted only to the Control app. Loading the
namespace is not the grant: every `spawn`, shell command, and retained child
handle rechecks the live session's grant, and closing or replacing that session
terminates its process trees.

## For host and extension authors

If you are writing the native side, the same distinction appears as the
`audience` on a route — see the native development reference for the exact
strings and which caller class each admits. Pick it from who should be able to
call the route, not from how sensitive it feels: the classes are disjoint, so
`control-app-only` genuinely excludes the control surface.
