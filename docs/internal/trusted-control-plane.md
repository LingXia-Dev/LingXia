# Trusted Control Plane

> **Audience**: contributors working on in-process lxapp session classes, route authorization,
> the bridge ingress seam, or anything that decides whether a caller may reach
> a host route. For **external** CLI/IPC control of a host process, see
> [`host-control-ipc.md`](host-control-ipc.md). If you're **building an app on LingXia**, the
> author-facing version is [`docs/skill/app/control-app.md`](../skill/app/control-app.md);
> this document explains why the mechanism is shaped the way it is, and which
> invariants a change must not break.

## The problem

Every lxapp runs the same runtime, in the same process, over the same bridge.
The product's own home lxapp, a Settings screen the host bundles, and a
downloaded guest are indistinguishable by app id, by manifest, by bundle
source, and by anything they can put in a payload — a guest can ship the home
app's id, declare the home app's privileges, and send the home app's frames.

So authorization cannot be derived from anything the caller controls. It is
derived from **which session the native host created**, decided once at
creation and sealed before that session's Logic starts.

## Three ideas

1. **Identity is native, and it is a constructor argument.** `AppSessionClass`
   is assigned when `LxApp` is built and is immutable afterwards. `ControlApp`
   has exactly one constructor, reachable only for the home app id sealed from
   `lingxia.yaml` at bootstrap. `ControlSurface` has its own, gated
   on a host-bundled bundle source and explicitly refused for the home id.
   There is no setter, no promotion, and no route that returns one.

2. **The classes are disjoint, not ordered.** `ControlApp` is not "a
   ControlSurface with more". Neither reaches the other's `*Only` routes.
   `RouteAudience` therefore enumerates caller *sets*. The one shared set is
   `ControlAppOrSurface` (appearance, display language, sidebar Pins): the
   `ControlApp` and every `ControlSurface`. Every `ControlSurface` (Terminal
   Settings, the browser system pages `app.lingxia.browser`) reaches every
   `ControlSurfaceOnly` route; no route is scoped to one surface app.

3. **One authorization seam.** A single authorize check is the only place a
   caller class is compared to a route's audience. Route
   audiences are immutable registration metadata resolved before dispatch, not
   a runtime parameter; `#[lingxia::native]` records `AppSessionOnly` unless an
   explicit `audience` is given, so forgetting the attribute fails closed.
   The doc-hidden `#[lingxia::framework_native(...)]` shares the syntax for
   framework-owned routes but requires an explicit `audience`. Registration
   seals each route's kind and audience into the effective route inventory;
   the Ready schema and every admission path read that caller-filtered
   inventory, and a duplicate route name is rejected.

## Where each class comes from

| Class | Constructor | Gate |
|---|---|---|
| `ControlApp` | home constructor | The home app id equals the app id. Set once from `lingxia.yaml` at bootstrap. |
| `ControlSurface` | control-surface constructor | Host-bundled bundle source, **not** the home id, and the live native control authority. |
| `StandardApp` | default constructor | Default. |

**The class follows the identity, not the live map.** Session-class lookup
derives `ControlApp` from the sealed home app id rather than from whatever session
happens to be in the live lxapp map. This matters because the home session *is* removed
from that map — by LRU eviction, by uninstall, and by the 30-minute delayed
destroy — and reading the class off the map would rebuild the product's own
home as a guest, with no way back short of a process restart. `ControlSurface` is not
an identity (a host-bundled lxapp is a surface only while native code holds it
open as one), so it stays inherited from the live session.

Only native code opens a surface: the browser runtime opens `app.lingxia.browser`
as its tab owner and for host chrome's system pages, and the Control app's
`lx.surface.open({ app })` reaches Terminal Settings or the browser through
`ControlAppOnly` shell open-host routes. Any other `open({ app })` of those ids is
refused, so a guest cannot start one as a `StandardApp` session.

## Which class reaches which audience

Every authenticated caller is an lxapp session; there is no other caller
source. Browser tab documents are web content with no LingXia bridge, so they
reach no route at all.

| Audience | `ControlApp` | `ControlSurface` | `StandardApp` |
|---|---|---|---|
| `AppSessionOnly` | yes | yes | yes |
| `ControlAppOnly` | yes | no | no |
| `ControlSurfaceOnly` | no | yes | no |
| `ControlAppOrSurface` | yes | yes | no |

A `StandardApp` (any guest, and a host-bundled lxapp the native host did not
open as a surface) reaches only `AppSessionOnly` routes, so giving a route that audience opens it
to every downloaded lxapp.

## Grants are separate from class

A manifest privilege (`downloads`, `automation`, `host`) is a
*request*. `NativeHostRuntimeAuthority` is handed to
`HostAddon::issue_app_resource_grants` at session creation and sealed when it
returns; the devtools build gets the narrower `NativeDevtoolsAuthority`, which
can issue automation grants only, and only for privileges the manifest
requested.

Loading a privileged namespace is not the grant: each use rechecks the
session's live grant, so closing or replacing a session revokes it.

`issue_app_resource_grants` runs exactly once per session — the seal is claimed
with a compare-exchange, because both the creation path and every waiter on the
permission snapshot reach it. It runs under that session's creation lock when
the grant is already decided, and otherwise on the task that lands the
snapshot, which can be after the user closed the lxapp. So a resolver decides
from the authority alone: no prompt, no blocking on a person, and no opening,
restarting or closing an lxapp — that deadlocks. Sealing a closed session
grants nothing; the grant check refuses a session that is not live.

## Permission snapshot resolution

Opening a guest checks its registry record first, so a new instance usually
starts already decided. When the record is missing or its status aged out,
resolution runs asynchronously: Logic waits without blocking the UI thread,
page HTML paints, and every network check denies until the decision lands.
`AppResourceGrant`s are sealed once from that snapshot, never from the pending
deny (which would stick). Transfers, Worker networking, native media, and
privileged APIs all read the same instance snapshot.

The snapshot is fixed until the instance is replaced; navigation and a
Logic-only restart do not refresh it. Shutdown cancels pending Logic startup
and document loads. A lookup is abandoned after five seconds and its future is
dropped, so providers must be cancel-safe at every await; a late response
cannot reach a replacement instance.

An answer the client cannot get is never a restriction: a `404`, an error, or a
timeout keeps the last known grant, or the default allow. Offline devices stay
usable, and revocation lands when the record next refreshes (grants and
statuses share one freshness window), not instantly. An unreadable or invalid
grant denies the instance: the registry asserted a policy, and guessing would
widen it. Malformed `LINGXIA_RUNNER_LXAPP_PERMISSIONS` denies every app for the
same reason. Both Runner entrypoints register their identity before SDK
initialization, including direct launches without the CLI environment marker.

## Host automation authority and surface handles

Host-owned automation runtime programs carry a native runtime authority and do
not impersonate an lxapp. A manifest `automation` or `host` entry is only a
request: cross-lxapp, browser, shell, device, terminal, and desktop process
operations also need a native grant sealed to that session. Retained drivers
are revalidated on each call and expire when their owner session begins
teardown.

Native terminal snapshots and commands go through an owner-bound surface
handle, never a global surface-id string. Platform UI publishes under
native-host authority; lxapp automation binds a handle only after its sealed
`AutomationHost` grant is checked. The handle is revalidated for snapshot,
dispatch, and completion, so a restart or takeover cannot reuse it.

## Ingress lock discipline

This is the least obvious invariant here and the easiest to break.

The current-document-binding scope holds the process-wide normalizer
registry lock **and** the per-WebView state lock across its action, so a
navigation start cannot land between the generation check and the effect.
Neither lock is reentrant, and the normalizer is a global.

**The rule: that form is only for a short native effect that cannot re-enter
the normalizer.** Outbound posts and port setup qualify. Anything that runs
host code does not, and must use the binding-is-current query, which answers
and releases:

- Inbound delivery to a delegate. The delegate is the whole bridge and Logic
  runtime, and it may post back synchronously — on Harmony it does, so holding
  the lock would deadlock the ArkWeb callback thread on every document's first
  frame. Holding it would also serialize all four message workers behind one
  WebView's dispatch and block navigation callbacks on every platform.
- Enqueueing. The web-message enqueue samples the binding through the same
  registry lock, so wrapping it self-deadlocks. A per-document port instead
  goes through the per-document enqueue, which proves the generation in
  one acquisition.
- The console log delegate.

Releasing loses nothing that was enforceable: a message carries the generation
snapshotted at enqueue time, that is what downstream authorization reads, and
every outbound path re-verifies the generation under its own document gate
(the shared outbound gate, the Harmony document authority, and Android's
per-document post).

The same rule applies to the Harmony document authority: take one generation lock
at a time. Ingress checks the authority, releases, then enqueues; nesting the
two would put ingress and outbound in opposite lock orders.

## Attention routes

`lx.host.notification.*`, `lx.host.banner.*`, `lx.device.permission.get` and
`lx.device.permission.request` are the direct routes a guest reaches. They are `AppSessionOnly`, and the
handler then scopes the effect by an attention scope derived from the session
class, never from a payload:

- A guest's ids are a digest of its app id and its own id, so it cannot name
  another app's notification or banner. The product's ids are unchanged.
- A guest's notification target, and each of its banner actions' targets, can
  only be itself. Bringing the product forward on its own (`lx.shell.activate`)
  is the Control app's.
- A guest may hold 32 notifications and 3 banners; a banner is up 60 s at
  most and waits behind the product's, so a guest cannot hold the one slot.
- A guest spends the product's notification permission at most once per
  session, only while open; a show the platform suppressed without asking
  returns the claim.
- A guest's banner carries its name and icon, so it cannot pass as the
  product's.

Widening that list widens what every downloaded lxapp can do; a unit test
pins it to exactly these routes.

## Invariants to preserve

- GPU View ingress is native-owned: the attached endpoint and runtime-issued
  connection generation must match before protocol decoding. Page-profile caller
  identity comes from the owning LxApp. Never synthesize a WebView message
  context for a GPU page. Capture its endpoint and generation
  for outbound work and recheck the generation at native worker delivery.
  GPU Views may call only the page-owned `navigation.setLeaveGuard`,
  `navigation.leave`, and `navigation.setTitle` host routes directly; all three
  still use normal audience and calling-page admission. Other host APIs go
  through Logic. Native leave
  requests capture the endpoint generation and recheck it on the View worker.

- No route audience defaults by omission. The Logic route audience table is an
  exhaustive match on purpose — a catch-all makes the next route
  ControlApp-only, which breaks every guest that expected to call it.
- Every control-plane refusal reaches JS as `E_PERMISSION_DENIED` (business
  code 3000 maps to it), and names the class that would have been admitted.
  Not "unsupported" — that points the author at the wrong fix.
- The `audience = "..."` strings are SDK-facing and part of the published
  contract. Renaming one is a breaking change for extension authors.
- The authority-escape gate and `lingxia`'s facade-boundary tests
  are the compile-time regression line for reaching an authority from outside
  bootstrap. Keep new authority types behind them.
