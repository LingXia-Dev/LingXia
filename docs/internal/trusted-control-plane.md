# Trusted Control Plane

> **Audience**: contributors working on session classes, route authorization,
> the bridge ingress seam, or anything that decides whether a caller may reach
> a host route. If you're **building an app on LingXia**, the author-facing
> version is [`docs/skill/app/control-app.md`](../skill/app/control-app.md);
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
   has exactly one constructor, reachable only for the app id sealed into
   `APP_CONFIG.home_app_id` at bootstrap. `ControlSurface` has its own, gated
   on a host-bundled bundle source and explicitly refused for the home id.
   There is no setter, no promotion, and no route that returns one.

2. **The classes are disjoint, not ordered.** `ControlApp` is not "a
   ControlSurface with more". Neither reaches the other's routes.
   `RouteAudience` therefore enumerates caller *sets*, and a variant named for
   one class never silently includes another — which is why
   `ControlAppOrBrowserOnly` spells out both rather than being called
   "ControlOnly".

3. **One authorization seam.** `lxapp::host::authorize(caller, audience)` is
   the only place a caller class is compared to a route's audience. Route
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
| `ControlApp` | `LxApp::new_as_home` | `home_app_id()` equals the app id. Set once from `lingxia.yaml` into a `OnceLock`. |
| `ControlSurface` | `LxApp::new_control_surface` | Host-bundled bundle source, and **not** the home id. |
| `StandardApp` | `LxApp::new` | Default. |

**The class follows the identity, not the live map.** `LxApps::session_class_for`
derives `ControlApp` from `home_app_id()` rather than from whatever session
happens to be in `lxapps`. This matters because the home session *is* removed
from that map — by LRU eviction, by uninstall, and by the 30-minute delayed
destroy — and reading the class off the map rebuilt the product's own home as
a guest, with no way back short of a process restart. `ControlSurface` is not
an identity (a host-bundled lxapp is a surface only while the Control app keeps
one open), so it stays inherited from the live session.

## Callers that are not lxapp sessions

`AuthenticatedCaller` has a second source: `BrowserDocument`, the ingress scope
a browser control document holds. It carries no `AppScope`, so it reaches no
lxapp's storage namespace or resource grants.

Its scope is **per frame**, held by the browser registry, and never a durable
property of a bridge connection. A bound-V3 connection lives on the owning
lxapp's page, so it must not lend that lxapp's identity to the document's
frames: `predecode_inbound` drops the connection's caller for a bound-V3 frame,
and `handle_incoming` refuses one outright — browser ingress
(`prepare_required_v3_incoming`, which installs the browser caller) is the only
entry.

## Grants are separate from class

A manifest privilege (`process`, `downloads`, `automation`, `host`) is a
*request*. `NativeHostRuntimeAuthority` is handed to
`HostAddon::issue_app_resource_grants` at session creation and sealed when it
returns; the devtools build gets the narrower `NativeDevtoolsAuthority`, which
can issue automation grants only, and only for privileges the manifest
requested.

Two independent gates therefore apply, and both are rechecked live:

- `AppResourceGrant::Process` is refused to any class but `ControlApp`, at the
  authority, regardless of what an addon tries to grant.
- Loading a privileged namespace is not the grant. `spawn`, shell commands,
  and retained child handles recheck the session's live grant, so closing or
  replacing a session revokes its handles.

`issue_app_resource_grants` runs exactly once per session — the seal is claimed
with a compare-exchange, because both the creation path and every waiter on the
permission snapshot reach it. It runs under that session's creation lock when
the grant is already decided, and otherwise on the task that lands the
snapshot, which can be after the user closed the lxapp. So a resolver decides
from the authority alone: no prompt, no blocking on a person, and no opening,
restarting or closing an lxapp — that deadlocks. Sealing a closed session
grants nothing; `has_resource_grant` refuses a session that is not live.

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

Host-owned `AutomationRuntime` programs carry a native runtime authority and do
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

`normalizer::with_current_document_binding` holds the process-wide normalizer
registry lock **and** the per-WebView state lock across its action, so a
navigation start cannot land between the generation check and the effect.
Neither lock is reentrant, and the normalizer is a global.

**The rule: that form is only for a short native effect that cannot re-enter
the normalizer.** Outbound posts and port setup qualify. Anything that runs
host code does not, and must use `document_binding_is_current`, which answers
and releases:

- Inbound delivery to a delegate. The delegate is the whole bridge and Logic
  runtime, and it may post back synchronously — on Harmony it does, so holding
  the lock deadlocked the ArkWeb callback thread on every document's first
  frame. Holding it also serialized all four message workers behind one
  WebView's dispatch and blocked navigation callbacks on every platform.
- Enqueueing. `enqueue_web_message` samples the binding through the same
  registry lock, so wrapping it self-deadlocks. A per-document port instead
  goes through `enqueue_document_web_message`, which proves the generation in
  one acquisition.
- The console log delegate.

Releasing loses nothing that was enforceable: a message carries the generation
snapshotted at enqueue time, that is what downstream authorization reads, and
every outbound path re-verifies the generation under its own document gate
(`DocumentOutboundGate`, `HarmonyDocumentAuthority::with_current_generation`,
Android's `postDocumentMessageNow`).

The same rule applies to `HarmonyDocumentAuthority`: take one generation lock
at a time. Ingress checks the authority, releases, then enqueues; nesting the
two put ingress and outbound in opposite lock orders.

## Invariants to preserve

- No route audience defaults by omission. `LogicRoute::audience` is an
  exhaustive match on purpose — a catch-all makes the next route
  ControlApp-only, which breaks every guest that expected to call it.
- Every control-plane refusal reaches JS as `E_PERMISSION_DENIED` (business
  code 3000 maps to it), and names the class that would have been admitted.
  Not "unsupported" — that points the author at the wrong fix.
- The `audience = "..."` strings are SDK-facing and part of the published
  contract. Renaming one is a breaking change for extension authors.
- `authority-escape-gate.sh` and `crates/lingxia/tests/facade_boundary.rs`
  are the compile-time regression line for reaching an authority from outside
  bootstrap. Keep new authority types behind them.
