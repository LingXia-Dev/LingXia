# Lxapp Permissions

`lxapp.json` does not declare network hosts or privilege classes. The app
registry does, on the same record that carries the app's name, icon and status.

- **Default** — nothing restricts an lxapp unless the registry returns a grant
  for it. No registry provider, an app the registry does not know, an
  unreachable registry, and a record with no grant all leave public network and
  every privilege class (`downloads`, `process`, `automation`, `host`, …).
- **A grant** — the only path that restricts. Fill in
  [`LxAppRegistryInfo::permissions`](#implement-the-registry) with an
  allowlist.
- **Each half** — independent. Leave `network` or `privileges` unconstrained to
  keep the default for that half. `Some([])` denies that half. `["*"]` allows
  it all.

OS permissions (camera, location) stay on host/platform flows.
`lingxia.yaml` carries no permission settings.

## Defaults

The host-selected home lxapp never consults the registry for permissions: the
host already vouched for it by loading it. It gets public network and every
privilege class. Package `appId` is not a way to claim home trust — the host
selects the identity and the loading path, and the manifest must match that
identity.

A guest the registry has no policy for is the same default. The development
Runner's project is a guest even when the Runner presents it in its home slot,
so with no `LINGXIA_RUNNER_LXAPP_PERMISSIONS` and no registry provider it is
unrestricted. Both Runner entrypoints register their identity before SDK
initialization, including direct launches without the CLI environment marker.

`destination: "downloads"` still needs a native session grant
(`AppResourceGrant::Downloads`) after the privilege is allowed. App-owned
`downloadFile` only needs the network grant. `process` still needs the Control
app, `lingxia.yaml` `capabilities.process`, and a native Process grant.

## Implement the registry

Register once before SDK initialization, for example in
`HostAddon::install_logic_extensions` (not `start_services`). One record
answers what an app *is* and what it *may do*; there is no separate permission
RPC, and the pre-open status check already fetches it, so a grant costs no
request of its own.

```rust
use lingxia::provider::{
    BoxFuture, LxAppChannel, LxAppPermissions, LxAppRegistryInfo, LxAppRegistryProvider,
    LxAppRegistryRequest, ProviderError, register_lxapp_registry_provider,
};

struct ProductRegistry;

impl LxAppRegistryProvider for ProductRegistry {
    fn fetch_registry_info<'a>(
        &'a self,
        app: LxAppRegistryRequest<'a>,
    ) -> BoxFuture<'a, Result<Option<LxAppRegistryInfo>, ProviderError>> {
        Box::pin(async move {
            if app.appid != "com.example.reader" || app.channel != LxAppChannel::Release {
                return Ok(None);
            }
            Ok(Some(LxAppRegistryInfo {
                name: Some("Reader".to_string()),
                // Restrict hosts; privileges stay at the default (unrestricted).
                permissions: Some(LxAppPermissions::network(["api.example.com"])),
                ..Default::default()
            }))
        })
    }
}

struct AppHostAddon;

impl lingxia::HostAddon for AppHostAddon {
    fn install_logic_extensions(&self) {
        register_lxapp_registry_provider(Box::new(ProductRegistry));
    }
}
```

Answer per app **and per channel**: a draft of an app id is not the
app the release grant was written for. The record does not name the app — the
request already did.

`LxAppPermissions::all()` is unconstrained (the default).
`network([...])` / `privileges([...])` constrain one half and leave the other
unrestricted. Chain `with_network` / `with_privileges` to constrain both.
An empty list denies that half. Build the answer with the constructors rather
than a struct literal.

Grant domains use lowercase, exact hosts or `*.example.com`, no scheme/path/port.
`["*"]` allows every public host and cannot be combined with named hosts. An
invalid entry invalidates the whole grant, and an unreadable grant denies the
instance — the registry asserted a policy, and guessing at it would widen the
app past what it asked for. Privilege ids are `downloads`, `process`,
`automation`, `host`, and any later class; `["*"]` allows every class.

## Lifecycle

Opening a guest checks its registry record first, so a new instance usually
starts already decided, with no lookup of its own. When the record is missing or
its status has aged out, resolution runs asynchronously: Logic waits for the
decision without blocking the UI thread, page HTML paints meanwhile, and every
network check denies until the decision lands. Native resource grants
(`AppResourceGrant`) are sealed once, from that same snapshot — never from the
pending deny, which would stick. All managed consumers read the same instance
snapshot — transfers, Worker networking, native media, and privileged APIs.

Custom native integrations can await `LxApp::wait_permissions_ready()` before
starting network-dependent work or serving custom HTML outside the normal page
loader. The wait completes for denial as well as approval; it does not mean a
request is allowed. Continue using the runtime URL/domain checks.

The snapshot stays fixed until the instance is replaced. Page navigation and
Logic-only restart do not refresh it. Instance shutdown cancels pending Logic
startup and document loads. A five-second timeout discards the lookup and falls
back to the app's standing grant; a late response cannot reach a replacement
instance.

## A server-backed registry

Keep the registry independent of the update contract: do not gate a lookup on
whether `check_update` finds a new package. Scope lookups to the host
account/tenant; an app id alone is not an authorization credential.

If no provider is registered, or it returns no `permissions`, apps stay on the
default allow. A half left unconstrained keeps that half's default.

An answer the client cannot get is never treated as a restriction: a `404`, an
error and a timeout all leave the app on its last known grant, or unrestricted
if it never had one. That keeps an offline device usable, and it means
revocation reaches an app when its record next refreshes — grants and statuses
share the same short freshness window — rather than instantly on a live
instance.

The runtime abandons a lookup after five seconds and drops the future, so the
implementation must be safe to cancel at any await point.

## Runner development grants

For a standalone project with no registry provider, optionally constrain named
apps in the Runner process environment. Unset keeps the default allow:

```sh
LINGXIA_RUNNER_LXAPP_PERMISSIONS='{"lingxia-chat":{"domains":["www.deepseek.com"]}}' lingxia dev
```

The value is a JSON object of app ids to `{"domains": [...], "privileges": [...]}`.
An omitted field is unconstrained. An empty list denies that half. An unlisted
app is an app this registry has no policy for, so it stays unrestricted — list
it with empty arrays to test a denial. Malformed JSON denies every app: the
developer asked for a policy, and a lookup that merely *failed* would read as no
answer and restrict nothing.
Local fixtures also require an active dev session to reach non-public addresses.

When this variable is set, the Runner registers its local registry before SDK
initialization. Leave it unset when a real registry provider is linked in: a
host may register only one, so do not compose local and server grants or fall
back to local authorization when the server errors. Restart the Runner to pick
up changes to its process environment.
