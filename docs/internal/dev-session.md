# Dev sessions

For contributors changing `lingxia dev`, the session broker, or `lxdev`
transport. The usage contract is
[`docs/skill/cli/lingxia.md`](../skill/cli/lingxia.md#lingxia-dev) and
[`docs/skill/cli/lxdev.md`](../skill/cli/lxdev.md).

## Registration

Each `lingxia dev` session registers with a per-user local broker for exactly
as long as its process lives; `lxdev` queries the broker, which is why it works
from any directory and why crashed sessions need no pruning. A broker left by
an older `lingxia` is replaced before a new session registers (its sessions
re-register), and a session that cannot register under its `--name` fails to
start instead of running unnamed.

Printed hints (a failed spec's `Rerun:` line, the stop command) never carry a
session id: they omit `--session` when not needed and otherwise use the name
or target.

## Transport

Desktop and Runner dev websockets are loopback-only. A physical iOS device
connects to an authenticated LAN listener using the token in
`~/.lingxia/apple/dev-device-token`. The LAN address skips VPN and container
ranges. The dev websocket is not a remote machine-management API.

On Windows over SSH, `lingxia dev` starts the host or Runner through a
temporary interactive-token task so its window opens on the signed-in desktop;
without a signed-in session of the same account, startup fails with an
actionable error.

## Targets and versions

`dev` picks native targets from what it launches: the device's reported
Android ABI, the host macOS architecture. Explicit ABI/arch overrides belong to
`build` and `package`.

The version-skew check also compares the CLI's commit with a locally built
package that records one. A CLI built from a checkout needs a Runner built from
the same commit (`tools/lingxia-runner/macos/install-local-runner.sh`, or
`.ps1` on Windows).

Template providers refresh in the background; the one being scaffolded from is
refreshed first.

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
notification.
