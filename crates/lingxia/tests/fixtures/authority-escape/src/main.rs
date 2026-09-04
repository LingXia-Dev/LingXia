//! This crate must not compile, including with every lxapp feature enabled.

struct ForgedProcessAuthority;

impl rong_command::ProcessAuthority for ForgedProcessAuthority {
    fn authorize(&self) -> Result<(), String> {
        Ok(())
    }
}

fn main() {
    let _ = lxapp::__init_with_native_authority;
    let _ = lxapp::terminal_automation::NativeHostRuntimeToken::for_test;
    let _ = lxapp::NativeControlPlaneAuthority::for_test;
    let _ = lxapp::NativeControlPlaneAuthority::for_native_runtime;
    let _ = lxapp::host::__install_app_resource_grant_resolver;
    let _ = lxapp::host::__install_devtools_resource_grant_resolver;
    let _ = lxapp::host::AuthenticatedCaller::LxAppSession;
    let _ = lxapp::host::AuthenticatedCaller::BrowserDocument;
    let _ = lxapp::add_global_page_script;
    let _ = lxapp::LxApp::add_page_script;
    let _ = lingxia::__init_with_native_authority;
    let _ = lingxia::resolve_settings_destination;
    #[cfg(any(target_os = "ios", target_os = "macos"))]
    let _ = lingxia::apple::resolve_settings_destination_for_host;
    let _ = rong_command::init;
    let _ = rong_command::init_with_authority;
}
