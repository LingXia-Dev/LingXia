//! Native library for the example app (builds to .so/.a).
//!
//! This crate:
//! 1. Links the core `lingxia` runtime into the final static/shared library
//! 2. Exports host addon registration via platform FFI (JNI/NAPI/C)

#[cfg(feature = "standard")]
mod extension;

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
            let domains = match app.appid {
                "lingxia-chat" => "www.deepseek.com",
                "app.lingxia.terminal-settings" => "api.nuget.org",
                _ => return Ok(None),
            };
            Ok(Some(lingxia::provider::LxAppRegistryInfo {
                permissions: Some(lingxia::provider::LxAppPermissions::network([domains])),
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

    fn start_services(&self) {
        #[cfg(feature = "devtools")]
        lingxia_control_runtime::start_dev_session_bridge_from_env();
        #[cfg(feature = "control")]
        if let Err(error) = lingxia_control_runtime::local_control::install(true) {
            log::warn!("local control unavailable: {error}");
        }
    }
}

fn register_host_addon() {
    static REGISTER: std::sync::Once = std::sync::Once::new();
    REGISTER.call_once(|| lingxia::register_host_addon(Box::new(ExampleHostAddon)));
}

/// Answer as this product's command line if that is what this invocation is,
/// and return the exit code. `None` means carry on and be the app.
///
/// The Windows executable calls this as the first thing in `main`.
#[cfg(all(feature = "control", target_os = "windows"))]
pub fn run_cli_if_invoked() -> Option<i32> {
    register_host_addon();
    lingxia::product_cli::run_if_invoked()
}

// Android: JNI export
#[cfg(target_os = "android")]
mod android {
    use jni::EnvUnowned;
    use jni::objects::JClass;

    #[inline]
    fn register_host_addon() {
        super::register_host_addon();
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_lingxia_example_lxapp_MainActivity_nativeRegisterHostAddon<
        'local,
    >(
        _env: EnvUnowned<'local>,
        _class: JClass<'local>,
    ) {
        register_host_addon();
    }

    // Backward-compatible symbol for older example app package IDs.
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_julibits_lingxia_muke_MainActivity_nativeRegisterHostAddon<
        'local,
    >(
        _env: EnvUnowned<'local>,
        _class: JClass<'local>,
    ) {
        register_host_addon();
    }
}

// Harmony: NAPI export
#[cfg(target_env = "ohos")]
#[napi_derive_ohos::napi]
pub fn lingxia_register_host_addon() {
    register_host_addon();
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
#[unsafe(no_mangle)]
pub extern "C" fn lingxia_register_host_addon() {
    register_host_addon();
}

#[cfg(target_os = "windows")]
pub fn lingxia_register_host_addon() {
    register_host_addon();
}
