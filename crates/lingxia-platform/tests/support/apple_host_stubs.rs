//! Link-only Swift host substitutes for standalone Rust unit-test binaries.
//! Route metadata retains handlers that reference the host even when tests do
//! not invoke them. Unmodeled host calls fail immediately. Include this module
//! only under `cfg(all(test, target_vendor = "apple"))`.

use swift_bridge::string::{RustStr, RustString};

macro_rules! host_stub {
    ($name:ident($($arg:ident: $ty:ty),* $(,)?) $(-> $ret:ty)?) => {
        #[unsafe(export_name = concat!("__swift_bridge__$", stringify!($name)))]
        #[allow(clippy::too_many_arguments)]
        extern "C" fn $name($($arg: $ty),*) $(-> $ret)? {
            $(let _ = $arg;)*
            // Bypass libtest capture so the diagnostic survives abort().
            let _ = std::io::Write::write_all(
                &mut std::io::stderr(),
                concat!("unexpected Swift host call in Rust unit test: ", stringify!($name), "\n").as_bytes(),
            );
            std::process::abort();
        }
    };
}

// Signatures mirror lingxia-platform/src/apple/ffi.rs after swift-bridge lowering.
// Lifecycle tests apply the resolved appearance when registering a headless
// app. There is no native view to update; Swift UI behavior is tested by the SDK.
#[unsafe(export_name = "__swift_bridge__$apply_appearance")]
extern "C" fn apply_appearance(_appid: RustStr, _dark: bool) -> bool {
    true
}
host_stub!(host_appearance_dark() -> bool);
host_stub!(set_host_color_mode(mode: i32));
host_stub!(get_capsule_rect(appid: RustStr, callback_id: u64));
host_stub!(update_navbar_ui(appid: RustStr) -> bool);
host_stub!(update_tabbar_ui_async(appid: RustStr, callback_id: u64));
host_stub!(open_url(owner_appid: RustStr, owner_session_id: u64, url: RustStr, target: i32) -> bool);
host_stub!(take_opened_url_tab_id() -> *mut RustString);
host_stub!(review_document(file_path: RustStr, mime_type: RustStr, show_menu: bool) -> bool);
host_stub!(open_document_external(file_path: RustStr, mime_type: RustStr, show_menu: bool) -> bool);
host_stub!(reveal_in_file_manager(path: RustStr) -> bool);
host_stub!(on_home_first_ready());
host_stub!(show_splash_campaign(image_path: RustStr, duration_ms: u32));
host_stub!(update_tabbar_ui(appid: RustStr) -> bool);
host_stub!(present_layout(window_id: RustStr, layout_json: RustStr) -> bool);
host_stub!(close_surface(id: RustStr, appid: RustStr, reason: RustStr) -> bool);
host_stub!(request_lxapp_main_activation(appid: RustStr));
host_stub!(open_lxapp(appid: RustStr, path: RustStr, session_id: u64, presentation: i32, panel_id: RustStr) -> bool);
host_stub!(close_lxapp(appid: RustStr, session_id: u64) -> bool);
host_stub!(present_surface(
    id: RustStr, appid: RustStr, path: RustStr, session_id: u64,
    page_instance_id: RustStr, content: i32, kind: i32, width: f64, height: f64,
    width_ratio: f64, height_ratio: f64, position: i32, role: i32,
    close_button: bool, dismiss_on_outside: bool, modal: bool,
    ephemeral_web_data: bool, url_callback: bool, chrome: i32,
) -> bool);
