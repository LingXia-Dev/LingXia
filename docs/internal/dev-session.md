# Dev sessions

For contributors changing `lingxia dev`, the session broker, or `lxdev`
transport. The usage contract is
[`docs/skill/cli/lingxia.md`](../skill/cli/lingxia.md#lingxia-dev),
[`docs/skill/cli/lxdev.md`](../skill/cli/lxdev.md), and
[auto-reload](../skill/cli/dev-reload.md).

## Reconnect

[Auto-reload](../skill/cli/dev-reload.md) — `lingxia dev listen`: developing
one lxapp against an installed host, on the port its dev tab shows, or
attached to a running host session.

## Registration

Each `lingxia dev` session registers with a per-user local broker for exactly
as long as its process lives; `lxdev` queries the broker, which is why it works
from any directory and why crashed sessions need no pruning. A broker left by
an older `lingxia` is replaced before a new session registers (its sessions
re-register), and a session that cannot register under its `--name` fails to
start instead of running unnamed.

`LINGXIA_RUNNER_INSTANCE` gives a CLI build its own broker (`~/.lingxia/broker-<instance>.sock`, or a
suffixed pipe on Windows): sessions of an export or second checkout register there, the build check
replaces only that broker, and `lxdev` reaches them with the same variable or `--instance <name>`.
Without it every build shares the default broker, and a different build replaces it.
`LINGXIA_HOME` moves the broker with the rest of the state: the socket is `$LINGXIA_HOME/broker[-<instance>].sock`
(a socket path past the platform limit, 103 bytes on macOS, is an error naming it), and on Windows the
pipe name carries a hash of it. `lxdev` must run with the same `LINGXIA_HOME` to find those sessions.

A macOS Runner session owns one slot per project and instance: `.lingxia/dev/runner[-<instance>].pid`, an
`.owner` record (session pid and start time), and Runner data under `<project id>[-<instance>]`. A new
session ends only a leftover Runner whose owner is gone; while the owner lives it refuses before building,
naming that session. Takeover within one instance tells the old session who replaced it.

Each session builds and serves from `.lingxia/dev/sessions/<session id>/` (`build/<appId>`, and the
manifests, snapshots and blobs under `lxapp/`), never another session's; `dist/` is written only by
`lingxia build`: a host session builds each embedded lxapp into its own `build/<appId>` and copies that
into the host's assets (a prebuilt bundle is copied in from its `dist/`). That build is kept per profile
in `.lingxia/dev/embed/<profile>/<appId>/entry` with a hash of its inputs (the bundle tree, linked
`@lingxia/*` packages, the CLI build, and the files `gpu-inputs.json` names outside the bundle); a
session whose inputs match copies it instead of building. An entry is replaced by rename under an
exclusive lock and copied under a shared one, so concurrent sessions never read half of one. The directory is removed
when the session ends; at start, one whose `owner` (pid and start time) is proven gone is removed, and one
with no readable owner is reported and left. Sessions of one project still share the generated inputs
beside the sources (`.lingxia/i18n`, `.lingxia/gpu-inputs.json`).

Printed hints (a failed spec's `Rerun:` line, the stop command) never carry a
session id: they omit `--session` when not needed and otherwise use the name
or target.

## Transport

Desktop and Runner dev websockets are loopback-only. A physical iOS device
connects to an authenticated LAN listener using the token in
`~/.lingxia/apple/dev-device-token`. The LAN address skips VPN and container
ranges. The dev websocket is not a remote machine-management API.

Android and HarmonyOS runtimes send an `app_state` event (`{"foreground": bool}`) on connect and on each
change; while it reports background, the server fails a relayed call with `app_in_background` once it has
waited 5 s from the call's start or the move to the background, whichever is later. A test cancel is answered at
once and held until the app reports foreground; a disconnect releases its run. Other hosts send none and keep the
plain timeout.

On Windows over SSH, `lingxia dev` starts the host or Runner through a
temporary interactive-token task so its window opens on the signed-in desktop;
without a signed-in session of the same account, startup fails with an
actionable error.

## Bundle sync

A dev host pulls a changed lxapp bundle as one zstd archive,
`lxapp/<id>/bundle/<distHash>.tar.zst` (pages repeat the UI runtime, so 6.5 MB
of pages travel as about 0.3 MB); a delta of up to 4 files (a hot swap) and an
older server (404) go file by file, one connection each. Never on a UI thread: Android runs
`prepare_dev_bundles` on a background thread before `lingxiaInit`, and init
adopts that outcome without fetching again (a fetch there held the main thread
past Android's 5 s input timeout, and the ANR dialog then took focus from
every spec). A host without that call still syncs inside init, bounded to 10 s.
An open waits at most 2 s for its app's sync; a slower one finishes in the
background for later opens. Every sync has an overall deadline, not only a
per-request timeout.

A dev restart (`lxdev lxapp restart`, a rebuild) is in place: Logic and the manifest restart and every page reloads its module in a fresh page context, while the app session, its page instances and the View worker's isolate are reused.

## Targets and versions

`dev` picks native targets from what it launches: the device's reported
Android ABI, the host macOS architecture. Explicit ABI/arch overrides belong to
`build` and `package`.

The version-skew check also compares the CLI's commit with a locally built
package that records one. A CLI built from a checkout needs a Runner built from
the same commit (`tools/lingxia-runner/macos/install-local-runner.sh`, or
`.ps1` on Windows).

Runners install under `$LINGXIA_HOME/runner/<version>` (default `~/.lingxia`). Installing a release
removes only older release downloads beyond the newest two that no process runs from; local builds
and suffixed names are never touched. An agent lane, CI job or scripted test must set its own
`LINGXIA_HOME` (and `LINGXIA_RUNNER_INSTANCE`), or it installs into the user's Runners.

`dev -p harmony` from a fresh shell needs `OHOS_NDK_HOME` (command-line-tools
`sdk/default/openharmony`), its `toolchains` and command-line-tools `bin` on
`PATH`, and a Java runtime on `PATH` (DevEco's `jbr/Contents/Home/bin`), or
hvigor's PackageHap fails with "Unable to locate a Java Runtime".

Template providers refresh in the background; the one being scaffolded from is
refreshed first.

Host bundle cache keys include the contents of reachable `@lingxia/*` packages,
including source exports, compiler entrypoints and `dist`. Exclude nested
`node_modules`, `.git` and `.lingxia` state; dependency discovery hashes reachable
LingXia packages separately. Re-run the host build after editing a linked package.

## `lxdev` details

- `lxdev lxapp info` carries `logic_features`: a read-only snapshot keyed by
  Logic context id. Desktop/handheld preset changes recreate Logic contexts;
  rotation and resizing keep the feature set.
- Windows `desktop` input uses foreground-only `SendInput`; `--pid` needs
  exactly one visible window, and true background input is not implemented.
  Window screenshots are occlusion-independent, but separate native popups may
  need their own capture. Owner-drawn Win32 controls may expose no
  accessibility nodes.

## Mock readiness

The host awaits the context-bound `__lxWaitForDevMocks` gate (installed by
automation) under session liveness before evaluating the Logic bundle; a
failed gate tears the context down before any user module runs. A module that
throws while the bundle evaluates is logged and the context is kept.
The dev server loads a selection record even for apps with zero handlers,
so `--mock all` cannot silently fall through to real I/O. Removing handlers
loads an empty record, preserving the selection and readiness contract.

## Logic mock initialization

A dev Logic bundle waits on the connection's mock bootstrap signal, not on the
existence of app handlers. A configured dev endpoint initializes the gate as
pending (home Logic may start before HostAddon services); the bridge also marks
startup pending before its first connection attempt; a failed attempt or disconnect releases the wait so the app
can launch offline. A successful connection starts a fresh gate. After every
app's mock load has settled, the dev server sends `session.network.mock.ready`,
including when there are no mocks or a load failed (that app runs without
mocks). The 30-second startup deadline is an idle window: the connection and
each `mock.load` re-arm it, so it bounds a stalled dev server, not the number
of apps; each load is capped well below it. Readiness uses a watch
notification. The deadline refuses Logic only once the server has answered this
process: a first attempt can block without failing (iOS resolves a `.local` dev
host only after the local-network prompt is answered), and an app must not die on
that, so a never-reached server lets Logic start offline with a warning. When that
server then connects and reports `session.network.mock.ready`, every open lxapp's
Logic cold-restarts (the dev rebuild path) under the session's mocks, once.
