# Dev sessions

For contributors changing `lingxia dev`, the session broker, or `lxdev`
transport. The usage contract is
[`docs/skill/cli/lingxia.md`](../skill/cli/lingxia.md#lingxia-dev),
[`docs/skill/cli/lxdev.md`](../skill/cli/lxdev.md), and
[auto-reload](../skill/cli/dev-reload.md) (`lingxia dev listen`). How mocks and
scenario functions reach a companion is [companion-protocol.md](companion-protocol.md).

A `lingxia dev` session builds a project, launches a host or Runner, and serves
it bundles and commands over a dev WebSocket. `lxdev` finds sessions through a
per-user broker. Invariants:

- A session is registered exactly as long as its process lives.
- A session builds and serves only from its own directory; `dist/` belongs to
  `lingxia build`.
- Shared build products are replaced by rename and read under a lock, so a
  session never reads half of one.
- Dev WebSockets are loopback-only, except the authenticated LAN listener for
  physical iOS devices. They are not a remote machine-management API.
- Bundle sync never runs on a UI thread, and every sync has an overall deadline.
- Dev Logic waits for the connection's mocks, but a dev server that never
  answered never stops an app from launching.

## Registration

Code: `lingxia-control-protocol/src/dev_session/broker.rs`.

Each session registers with a per-user local broker and keeps that connection
open; the broker drops the session when the connection closes. `lxdev` queries
the broker, which is why it works from any directory and why crashed sessions
need no pruning. A broker from a different CLI build is replaced before a new
session registers (its sessions re-register), and a session that cannot
register under its `--name` fails to start instead of running unnamed.

The broker is a Unix socket (`$LINGXIA_HOME/broker.sock`, default
`~/.lingxia`, mode 0600) or a per-user named pipe on Windows whose name carries
a hash of `LINGXIA_HOME`. A socket path past the platform limit (103 bytes on
macOS) is an error naming it. `lxdev` must run with the same `LINGXIA_HOME` to
find those sessions.

`LINGXIA_RUNNER_INSTANCE` gives a CLI build its own broker
(`broker-<instance>.sock`, or a suffixed pipe): sessions of an export or a
second checkout register there, the build check replaces only that broker,
and `lxdev` reaches them with the same variable or `--instance <name>`.
Without it every build shares the default broker, and a different build
replaces it.

Printed hints (a failed spec's `Rerun:` line, the stop command) never carry a
session id: they omit `--session` when not needed and otherwise use the name or
target.

## Session files

All under the project's `.lingxia/`.

| Path | Holds | Lifetime |
| --- | --- | --- |
| `dev/sessions/<session id>/` | `build/<appId>`, `build/<appId>.gpu-inputs.json`, and the manifests, snapshots and blobs under `lxapp/` | Removed when the session ends; at start, a directory whose owner (pid and start time) is proven gone is removed, one with no readable owner is reported and left. |
| `dev/embed/<profile>/<appId>/entry` | An embedded lxapp's build with its GPU inputs record and an input hash | Shared across sessions; replaced by rename under an exclusive lock, copied under a shared one. |
| `dev/runner[-<instance>].pid`, `.owner` | The macOS Runner slot: pid, and owner session pid and start time | Per project and instance. |
| `i18n/dev`, `i18n/build` | Compiled i18n, one copy per mode: content-named directories and a `current` file naming one | Replaced by rename. |

A host session builds each embedded lxapp into its own `build/<appId>` and
copies that into the host's assets (a prebuilt bundle is copied from its
`dist/`). The embed cache key hashes the bundle tree, the linked `@lingxia/*`
packages (source exports, compiler entry points and `dist`, excluding nested
`node_modules`, `.git` and `.lingxia`), the CLI build, and the files the GPU
inputs record names outside the bundle; a session whose inputs match copies
the entry instead of building. The GPU inputs record is what the watcher plans
hot swaps from.

A new session ends only a leftover Runner whose owner is gone; while the owner
lives it refuses before building, naming that session. Takeover within one
instance tells the old session who replaced it. Runner data lives under
`<project id>[-<instance>]`, so two projects under one instance stay separate.

## Transport

Desktop and Runner dev WebSockets are loopback-only. A physical iOS device
connects to an authenticated LAN listener using the token in
`~/.lingxia/apple/dev-device-token`. The LAN address skips VPN and container
ranges.

Android and HarmonyOS runtimes send an `app_state` event
(`{"foreground": bool}`) on connect and on each change. While it reports
background, the server fails a relayed call with `app_in_background` once it
has waited 5 s from the call's start or the move to the background, whichever
is later. A test cancel is answered at once and held until the app reports
foreground; a disconnect releases its run. Other hosts send none and keep the
plain timeout. Without this a call to a backgrounded app would wait out the
120 s command timeout; 5 s is long enough for a spec that leaves the app and
comes back on purpose.

## Bundle sync

Code: `crates/lingxia/src/devtool.rs`, `devtool/sync.rs`.

A dev host pulls a changed lxapp bundle as one zstd archive,
`lxapp/<id>/bundle/<distHash>.tar.zst`: pages repeat the UI runtime, so 6.5 MB
of pages travel as about 0.3 MB. A delta of up to 4 files (a hot swap), or a
server that answers 404, goes file by file.

| Where | Deadline |
| --- | --- |
| Pre-init pull off the UI thread (`prepare_dev_bundles`, called by the Android and Apple entry points before init) | 60 s |
| Sync inside init without a pre-init pull, or a dev restart | 10 s |
| An open waiting for its app's sync | 2 s, then the installed bundle; the sync finishes in the background |

Init adopts the pre-init outcome without fetching again. Syncing on the main
thread can hold it past Android's 5 s input timeout, and the ANR dialog then
takes focus from every spec.

A dev restart (`lxdev lxapp restart`, a rebuild) is in place: Logic and the
manifest restart and every page reloads its module in a fresh page context,
while the app session, its page instances and the View worker's isolate are
reused.

## Mock readiness

Code: `lingxia-network-dev/src/dev.rs`; the gate is `__lxWaitForDevMocks`,
installed by automation.

The host awaits the context-bound gate under session liveness before
evaluating the Logic bundle. A failed gate tears the context down before any
user module runs; a module that throws while the bundle evaluates is logged
and the context is kept.

The gate belongs to the dev connection, not to whether an app has handlers. A
configured dev endpoint starts it pending (home Logic may start before
HostAddon services dial the bridge), and the bridge marks it pending before its
first connection attempt. A failed attempt or disconnect releases it so the
app can launch offline; a successful connection starts a fresh gate. After
every app's mock load has settled, the dev server sends
`session.network.mock.ready`, also when there are no mocks or a load failed
(that app runs without mocks). The dev server loads a selection record even
for apps with zero handlers, and removing handlers loads an empty one, so
`--mock all` can never silently fall through to real I/O.

The 30 s startup deadline (`MOCK_BOOTSTRAP_IDLE`) is an idle window: the
connection and each `mock.load` re-arm it, so it bounds a stalled server, not
the number of apps. It refuses Logic only once the server has answered this
process. A first attempt can block without failing (iOS resolves a `.local`
dev host only after the local-network prompt is answered), and an app must
not die on that, so a never-reached server lets Logic start offline with a
warning. When that server later connects and reports ready, every open
lxapp's Logic cold-restarts once under the session's mocks.

## Targets, versions and Runners

`dev` picks native targets from what it launches: the device's reported
Android ABI, the host macOS architecture. Explicit ABI/arch overrides belong
to `build` and `package`.

The version-skew check also compares the CLI's commit with a locally built
package that records one, so a CLI built from a checkout needs a Runner built
from the same commit (`tools/lingxia-runner/macos/install-local-runner.sh`, or
`.ps1` on Windows).

Runners install under `$LINGXIA_HOME/runner/<version>`. Installing a release
removes only release downloads older than the newest two that no process runs
from; local builds and suffixed names are never touched. A CI job or scripted
test must set its own `LINGXIA_HOME` (and `LINGXIA_RUNNER_INSTANCE`), or it
installs into the user's Runners.

## Platform notes

- Windows over SSH: `lingxia dev` starts the host or Runner through a
  temporary interactive-token task so its window opens on the signed-in
  desktop; without a signed-in session of the same account, startup fails
  with an actionable error.
- `dev -p harmony` from a fresh shell needs `OHOS_NDK_HOME`
  (command-line-tools `sdk/default/openharmony`), its `toolchains` and
  command-line-tools `bin` on `PATH`, and a Java runtime on `PATH` (DevEco's
  `jbr/Contents/Home/bin`), or hvigor's PackageHap fails with "Unable to
  locate a Java Runtime".
- `lxdev lxapp info` carries `logic_features`: a read-only snapshot keyed by
  Logic context id. Desktop/handheld preset changes recreate Logic contexts;
  rotation and resizing keep the feature set.
- Windows `desktop` input uses foreground-only `SendInput`; `--pid` needs
  exactly one visible window. Window screenshots are occlusion-independent,
  but separate native popups may need their own capture. Owner-drawn Win32
  controls may expose no accessibility nodes.
