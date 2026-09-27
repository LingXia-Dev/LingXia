# Permissions

What an lxapp may reach (network hosts) and use (privilege classes). Neither
`lxapp.json` nor `lingxia.yaml` declares them; the app registry does, on the
same record as the app's name and status. OS permissions (camera, location)
stay on platform flows. Session classes (Control app vs guest) are separate:
[Control app](../app/control-app.md).

## Model

- **Default allow.** Without a grant an lxapp has the public network and every
  privilege class (`downloads`, `process`, `automation`, `host`, …). That
  covers no provider, an unknown app, an unreachable registry, and a record
  without `permissions`.
- **A grant restricts.** Each half (`network`, `privileges`) is independent:
  unconstrained keeps the default, `[]` denies it, `["*"]` allows it all.
- **The home lxapp** is never looked up; the host vouched for it by loading it.
  A package `appId` cannot claim home trust. The Runner's project is a guest,
  even in the home slot.
- **Some privileges need more.** `destination: "downloads"` also needs the
  native `AppResourceGrant::Downloads`
  ([`issue_app_resource_grants`](./development.md#host-addon)); app-owned
  downloads need only the network. `process` also needs the Control app,
  `capabilities.process`, and a native Process grant.
- Non-public addresses stay blocked either way (local fixtures need an active
  dev session).

## Implement the registry

Register once before SDK initialization, for example in
`HostAddon::install_logic_extensions` (not `start_services`). The pre-open
status check already fetches the record, so a grant costs no extra request.

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

- Answer per app **and per channel**: a draft is not the release the grant was
  written for.
- Build answers with `LxAppPermissions::all()`, `network([...])`,
  `privileges([...])`, and chain `with_network` / `with_privileges`.
- Hosts are lowercase, exact or `*.example.com`, with no scheme, path, or port.
  `["*"]` cannot be mixed with named hosts. One invalid entry invalidates the
  grant, and an invalid grant denies the app.
- Scope lookups to the host account or tenant; an app id is not a credential.
  Keep the registry independent of update checks.
- Lookups time out after five seconds and must be safe to cancel. A failed
  lookup (error, `404`, timeout) keeps the last grant, or the default.
- The grant is fixed per instance. Recreate the instance to apply a change;
  native code that serves its own content can await
  `LxApp::wait_permissions_ready()` (it also completes on denial).

## Runner grants

Without a registry provider, constrain named apps in the Runner's environment:

```sh
LINGXIA_RUNNER_LXAPP_PERMISSIONS='{"com.example.reader":{"domains":["api.example.com"]}}' lingxia dev
```

The JSON maps app ids to `{"domains": [...], "privileges": [...]}`. An omitted
field is unconstrained, `[]` denies, and an unlisted app stays unrestricted
(list it with empty arrays to test a denial). Malformed JSON denies every app.
Restart the Runner to apply changes. A host has one provider: leave the
variable unset when a real registry is linked in.
