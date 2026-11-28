# Auto-reload

`lingxia dev` watches lxapp sources. A save rebuilds that lxapp and reloads
it by app id. A saved `.rs` in the route crate's `src/` regenerates the native
clients and type-checks the pages against them; the running host keeps the old
routes until you run `lingxia dev` again. `lingxia.yaml` and platform projects
are not watched.

## What is watched

- A standalone lxapp: the directory that contains `lxapp.json`.
- A host app: each `resources.bundles[].path` that is a local lxapp, home or
  a bundled guest. A package-only bundle is not watched.
- Not watched: `node_modules`, `dist`, `target`, `.git`, `.lingxia`,
  `test-results`, `.vite`.

A test run pauses the watcher. Saves made during the run rebuild once, when
the run ends. Keep files a test generates outside the watched tree.
[Testing](../lxapp/testing.md).

Saves under `mocks/` reload that lxapp's mocks only.
[Mocks](../lxapp/mock.md).

A saved `theme.json` (the lxapp's) or `themes/<id>.json` (the host's) reaches
the open pages and bars without a rebuild; page state stays. A theme that fails
its contrast check keeps the last good one, and the terminal lists each failing
pair.

## What a save does

An edit that touches only GPU page View code recompiles those pages and swaps
them into the running app, keeping page and Logic state; a page that fails to
compile keeps running. Anything else follows the rest of this section.

The session rebuilds that lxapp with its `--release` into its own directory
(`.lingxia/dev/sessions/<id>/`, printed at start); `dist/` is written only by
`lingxia build`. Then it restarts that app id. The instance has to still be in the runtime. It does
not have to be the one on screen.

- A guest that is not open stays closed. The build loads the next time it
  opens.
- Home that is not active still reopens.
- An app id the dev server does not publish keeps the source it already
  opened from. Home with no manifest is an error.

What restarts: [lifecycle](../lxapp/lifecycle.md) (a dev reload keeps the
window and starts Logic again).

## Develop on a device

`lingxia dev listen` runs in an lxapp directory (`lxapp.json`, no
`lingxia.yaml` beside it). It builds that lxapp and opens it once in a host
app, then reloads it on save until Ctrl+C. Nothing is built, installed, or
launched for the host. In a host project it is an error; use `lingxia dev`.

With the port on the host app's dev tab, this command is the session that
app dials:

```
lingxia dev listen -p ios --port 39231
```

- `-p` says how the app gets here: android and harmony over USB (the port is
  forwarded, `--device` picks the device), ios over Wi-Fi with this
  machine's device token, macos and windows on loopback. The iOS host has
  to be one built on this machine.
- A busy port fails; listen does not pick another, and names the session
  holding it.
- It works in a bundled lxapp's directory too, home included.
- The iOS host dials this Mac by its Bonjour name (`<LocalHostName>.local`),
  so an address change needs no rebuild. The address it had at build time is
  the fallback where the network drops multicast.

A relaunched app opens the lxapp again. A foregrounded one keeps its page,
and one you closed stays closed. A save the app missed while away reloads
when it reconnects.

`--no-open` builds and watches the lxapp but never opens it or brings the app
forward; open it from the app. Without it, an app in the background (screen
locked, another app in front) opens the lxapp when it comes back.

Without `--port`, it attaches to the one running `lingxia dev` host session.
Only sessions with an app connected count; several require `--session` (a
name, a target, `target@dir`, an ordinal, or an id prefix). A Runner session
is not a host. It refuses an app id, or the same directory, already in that
host's `resources.bundles`, or one that session already develops; saving
those already reloads them. The attach runs `lingxia:prepare` but no
companion, and build output appears in the host session's terminal. Only
`lingxia dev listen` run as the same user can attach: it reads the session's
attach token from a file only that user can read.

The session builds with this command's `--release`.
Without `--port`, an omitted one uses the host session's. Ctrl+C drops this
lxapp's watch only. The open lxapp stays, and so does a host session this
command did not start. Do not pass `--background`.
