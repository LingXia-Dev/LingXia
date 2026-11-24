//! Native library for the example app (builds to .so/.a): its host addon, registered
//! with every platform SDK by `lingxia::host!`.

#[cfg(feature = "standard")]
mod extension;
mod routes;

lingxia::host!(ExampleHostAddon);

struct ExampleHostAddon;

/// Stands in for the server: constrains network for the showcase's bundled
/// guest lxapps. Privileges stay unconstrained, and so does any app this
/// registry has no entry for.
#[cfg(not(feature = "cloud"))]
struct ShowcaseGuestRegistry;

#[cfg(not(feature = "cloud"))]
impl lingxia::provider::LxAppRegistryProvider for ShowcaseGuestRegistry {
    fn fetch_registry_info<'a>(
        &'a self,
        app: lingxia::provider::LxAppRegistryRequest<'a>,
    ) -> lingxia::provider::BoxFuture<
        'a,
        Result<Option<lingxia::provider::LxAppRegistryInfo>, lingxia::provider::ProviderError>,
    > {
        Box::pin(async move {
            // Loopback serves the specs' local fixtures; it is honoured only in a
            // dev session.
            let domains: &[&str] = match app.appid {
                "lingxia-showcase-web" => &["www.deepseek.com", "127.0.0.1"],
                "app.lingxia.terminal-settings" => &["api.nuget.org"],
                _ => return Ok(None),
            };
            Ok(Some(lingxia::provider::LxAppRegistryInfo {
                permissions: Some(lingxia::provider::LxAppPermissions::network(
                    domains.iter().copied(),
                )),
                ..Default::default()
            }))
        })
    }
}

impl lingxia::HostAddon for ExampleHostAddon {
    #[cfg(feature = "devtools")]
    fn issue_devtools_app_resource_grants(
        &self,
        authority: &mut lingxia::NativeDevtoolsAuthority<'_>,
    ) {
        authority.grant_automation();
    }

    #[cfg(feature = "standard")]
    fn install_logic_extensions(&self) {
        lingxia::js::register_logic_extension(Box::new(extension::HelloExtension));
        // Cloud provider (lx.cloud/auth + update/fingerprint/push). Must register in
        // this hook — the logic context is built before `start_services`. Injected via
        // `--with-provider cloud`.
        #[cfg(feature = "cloud")]
        if let Err(err) = lingxia_cloud_client::init(lingxia_cloud_client::CloudOptions::default())
        {
            log::error!("[cloud] provider init failed: {err}");
        }
        // Stands in for the server so the bundled guests run under a real
        // grant. An injected provider owns the registry when one is linked in —
        // only one may register.
        #[cfg(not(feature = "cloud"))]
        lingxia::provider::register_lxapp_registry_provider(Box::new(ShowcaseGuestRegistry));
    }

    /// A native route, so a notification can open something that is not a
    /// page — no lxapp, no product URL.
    fn install_navigation_routes(
        &self,
        routes: &mut lingxia::navigation::NavigationRoutes,
    ) -> Result<(), String> {
        use lingxia::navigation::{NavigationRoute, RouteParam};
        // Native: no lxapp, no home Logic, no configured App Link host.
        routes.add(
            NavigationRoute::new("showcase.native.note", |request| {
                let note = request.param_str("text").unwrap_or_default();
                log::info!("[showcase] native navigation route opened with: {note}");
                Ok(())
            })
            .param(RouteParam::string("text")),
        )
    }

    fn start_services(&self) {
        #[cfg(feature = "devtools")]
        lingxia_control_runtime::start_dev_session_bridge_from_env();
        #[cfg(feature = "control")]
        if let Err(error) = lingxia_control_runtime::local_control::install(true) {
            log::warn!("local control unavailable: {error}");
        }
    }
}
