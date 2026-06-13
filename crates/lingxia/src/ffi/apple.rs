use lingxia_messaging::invoke_callback;
use lxapp::{
    CloseReason, CreatePageInstanceRequest, LxAppDelegate, LxAppUiEventType, OrientationConfig,
    PageInstanceEvent, PageInstanceId, PageOrientation, PageOwner, PageQueryInput, PageTarget,
    PresentationKind, SceneId,
};
#[cfg(all(target_os = "macos", feature = "shell-runtime"))]
use std::sync::Arc;

/// Parses a color string (e.g., "#RRGGBB" or "transparent") into a u32 ARGB value.
fn parse_color_to_u32(color_str: &str, default_color: u32) -> u32 {
    if color_str.eq_ignore_ascii_case("transparent") {
        return 0x00000000;
    }

    if let Some(hex_part) = color_str.strip_prefix('#')
        && hex_part.len() == 6
        && let Ok(rgb) = u32::from_str_radix(hex_part, 16)
    {
        return 0xFF000000 | rgb; // Add full alpha
    }
    default_color
}

#[swift_bridge::bridge]
mod bridge {
    // LxApp basic information for Swift
    #[swift_bridge(swift_repr = "struct")]
    pub struct LxAppInfo {
        pub app_name: String,
        pub version: String,
        pub release_type: String,
        pub cache_dir: String,
    }

    // NavigationBar state for Swift
    #[swift_bridge(swift_repr = "struct")]
    pub struct NavigationBarState {
        pub background_color: u32,
        pub text_style: String,
        pub title_text: String,
        pub show_navbar: bool,
        pub show_back_button: bool,
        pub show_home_button: bool,
    }

    // TabBar state for Swift (without items array)
    #[swift_bridge(swift_repr = "struct")]
    pub struct TabBar {
        pub color: u32,
        pub selected_color: u32,
        pub background_color: u32,
        pub border_style: u32,
        pub position: i32,
        pub dimension: i32,
        pub items_count: i32,
        pub is_visible: bool,
        pub selected_index: i32,
    }

    // Group alignment types
    pub enum GroupAlignment {
        Center, // 0=middle/center (default)
        Start,  // 1=start (top/left)
        End,    // 2=end (bottom/right)
    }

    // TabBar item for Swift
    #[swift_bridge(swift_repr = "struct")]
    pub struct TabBarItem {
        pub page_path: String,
        pub text: String,
        pub icon_path: String,
        pub selected_icon_path: String,
        pub selected: bool,
        pub group: GroupAlignment,
        pub badge: String,
        pub has_red_dot: bool,
    }

    // Push notification trigger types
    pub enum PushTrigger {
        Background,
        Tap,
        Launch,
    }

    // lxapp-scoped UI event types.
    pub enum LxAppUiEventType {
        TabBarClick,
        CapsuleClick,
        NavigationClick,
        BackPress,
        PullDownRefresh,
    }

    // host-app scoped UI events.
    pub enum AppUiEventType {
        /// Panel icon clicked in the host app UI
        PanelIconClick,
        /// "Click to restart" callout clicked: apply the staged host-app update
        UpdateRestartClick,
        /// "Click to install" callout clicked: re-open the host-app update flow
        UpdateInstallClick,
    }

    // Current LxApp info from Rust stack
    #[swift_bridge(swift_repr = "struct")]
    pub struct CurrentLxApp {
        pub appid: String,
        pub path: String,
        pub session_id: u64,
    }

    #[swift_bridge(swift_repr = "struct")]
    pub struct CreatePageInstanceResult {
        pub ok: bool,
        pub page_instance_id: String,
        pub appid: String,
        pub resolved_path: String,
        pub query: String,
        pub error: String,
    }

    #[swift_bridge(swift_repr = "struct")]
    pub struct PageBindingResult {
        pub page_instance_id: String,
        pub webview_ptr: usize,
    }

    extern "Rust" {
        #[swift_bridge(swift_name = "lingxiaInit")]
        fn lingxia_init(data_dir: &str, cache_dir: &str, locale: &str) -> Option<String>;

        #[swift_bridge(swift_name = "emitSdkLog")]
        fn emit_sdk_log(level: i32, category: &str, appid: &str, path: &str, message: &str)
        -> bool;

        #[swift_bridge(swift_name = "onPageShow")]
        fn on_page_show(appid: &str, path: &str);

        #[swift_bridge(swift_name = "onLxappClosed")]
        fn on_lxapp_closed(appid: &str, session_id: u64) -> bool;

        #[swift_bridge(swift_name = "onDeviceOrientationChanged")]
        fn on_device_orientation_changed(appid: &str, session_id: u64, value: &str) -> bool;

        #[swift_bridge(swift_name = "getLxAppInfo")]
        fn get_lxapp_info(appid: &str) -> LxAppInfo;

        #[swift_bridge(swift_name = "getNavigationBarState")]
        fn get_navigation_bar_state(appid: &str, path: &str) -> NavigationBarState;

        #[swift_bridge(swift_name = "getPageOrientation")]
        fn get_page_orientation(appid: &str, path: &str) -> i32;

        #[swift_bridge(swift_name = "getTabBar")]
        fn get_tab_bar(appid: &str) -> Option<TabBar>;

        #[swift_bridge(swift_name = "getTabBarItem")]
        fn get_tab_bar_item(appid: &str, index: i32) -> Option<TabBarItem>;

        // lxapp-scoped event (appid must be a real lxapp id)
        #[swift_bridge(swift_name = "onLxappEvent")]
        fn on_lxapp_event(appid: &str, event_type: LxAppUiEventType, data: &str) -> bool;

        // Host-app scoped UI event (no lxapp context; e.g. panel icon click)
        #[swift_bridge(swift_name = "onAppEvent")]
        fn on_app_event(event_type: AppUiEventType, data: &str) -> bool;

        #[swift_bridge(swift_name = "findWebViewByPageInstanceId")]
        fn find_webview_by_page_instance_id(page_instance_id: &str) -> usize;

        #[swift_bridge(swift_name = "resolvePageBinding")]
        fn resolve_page_binding(appid: &str, path: &str, session_id: u64) -> PageBindingResult;

        #[swift_bridge(swift_name = "createPageInstance")]
        fn create_page_instance_for_open(
            appid: &str,
            path: &str,
            session_id: u64,
            surface: i32,
            owner_page_instance_id: &str,
        ) -> CreatePageInstanceResult;

        #[swift_bridge(swift_name = "notifyPageInstanceMounted")]
        fn notify_page_instance_mounted(page_instance_id: &str) -> bool;

        #[swift_bridge(swift_name = "notifyPageInstanceVisible")]
        fn notify_page_instance_visible(page_instance_id: &str) -> bool;

        #[swift_bridge(swift_name = "notifyPageInstanceHidden")]
        fn notify_page_instance_hidden(page_instance_id: &str, reason: &str) -> bool;

        #[swift_bridge(swift_name = "disposePageInstance")]
        fn dispose_page_instance(page_instance_id: &str, reason: &str) -> bool;

        #[swift_bridge(swift_name = "onSurfaceClosed")]
        fn on_surface_closed(appid: &str, id: &str, reason: &str) -> bool;

        #[swift_bridge(swift_name = "openBrowserTab")]
        fn open_browser_tab(appid: &str, session_id: u64, url: &str) -> Option<String>;

        #[swift_bridge(swift_name = "openBrowserTabWithId")]
        fn open_browser_tab_with_id(
            appid: &str,
            session_id: u64,
            url: &str,
            tab_id: &str,
        ) -> Option<String>;

        #[swift_bridge(swift_name = "browserTabClose")]
        fn browser_tab_close(tab_id: &str) -> bool;

        #[swift_bridge(swift_name = "getBuiltinBrowserAppId")]
        fn get_builtin_browser_app_id() -> String;

        #[swift_bridge(swift_name = "browserTabPathForId")]
        fn browser_tab_path_for_id(tab_id: &str) -> String;

        #[swift_bridge(swift_name = "updateBrowserTabInfo")]
        fn update_browser_tab_info(tab_id: &str, current_url: &str, title: &str) -> bool;

        #[swift_bridge(swift_name = "startBrowserTabDownload")]
        fn start_browser_tab_download(
            tab_id: &str,
            url: &str,
            user_agent: &str,
            suggested_filename: &str,
            source_page_url: &str,
            cookie: &str,
        ) -> bool;

        #[swift_bridge(swift_name = "toggleWebViewDevtoolsByPtr")]
        fn toggle_webview_devtools_by_ptr(webview_ptr: usize, detached: bool) -> bool;

        #[swift_bridge(swift_name = "onApplinkReceived")]
        fn on_applink_received(applink_path: &str) -> i32;

        #[swift_bridge(swift_name = "getCurrentLxApp")]
        fn get_current_lxapp() -> CurrentLxApp;

        #[swift_bridge(swift_name = "getLxAppSessionId")]
        fn get_lxapp_session_id(appid: &str) -> u64;

        #[swift_bridge(swift_name = "onPushlinkReceived")]
        fn on_pushlink_received(url: &str, trigger: PushTrigger) -> i32;

        #[swift_bridge(swift_name = "onPushTokenReceived")]
        fn on_push_token_received(token: &str) -> i32;

        #[swift_bridge(swift_name = "onCallback")]
        fn on_callback(id: u64, success: bool, data: &str) -> bool;

        #[swift_bridge(swift_name = "onNativeComponentEvent")]
        fn on_native_component_event(
            appid: &str,
            path: &str,
            component_id: &str,
            event_name: &str,
            payload_json: &str,
            bindings_json: &str,
        ) -> bool;

        #[swift_bridge(swift_name = "isPullDownRefreshEnabled")]
        fn is_pull_down_refresh_enabled(appid: &str, path: &str) -> bool;

        #[swift_bridge(swift_name = "resolveLxUri")]
        fn resolve_lx_uri(appid: &str, input: &str) -> Option<String>;

        #[swift_bridge(swift_name = "handleBrowserAddressInput")]
        fn handle_browser_address_input(request_json: &str) -> Option<String>;

        #[swift_bridge(swift_name = "browserUrlIsHidden")]
        fn browser_url_is_hidden(raw: &str) -> bool;

        #[swift_bridge(swift_name = "onAppShow")]
        fn on_app_show(lxappid: &str);

        #[swift_bridge(swift_name = "onAppHide")]
        fn on_app_hide(lxappid: &str);

        #[swift_bridge(swift_name = "onUserCaptureScreen")]
        fn on_user_capture_screen(lxappid: &str);

        // Returns a bitmask of enabled SDK capabilities (compile-time constant).
        // Bit 0 (0x1) = shell (browser, downloads, settings, panels).
        #[swift_bridge(swift_name = "getAppCapabilities")]
        fn get_app_capabilities() -> u32;

        // Get panels config as JSON string (returns None if no panels configured)
        #[swift_bridge(swift_name = "getPanelsConfigJson")]
        fn get_panels_config_json() -> Option<String>;

        // Open a lxapp for a panel (triggers download + JS init if needed).
        // panel_id is forwarded so Swift can route the openLxApp callback to the right panel.
        #[swift_bridge(swift_name = "openPanelLxapp")]
        fn open_panel_lxapp(panel_id: &str, appid: &str, path: &str);

        #[swift_bridge(swift_name = "getTerminalBackendStatusJson")]
        fn get_terminal_backend_status_json() -> String;

        #[swift_bridge(swift_name = "terminalSessionCreate")]
        fn terminal_session_create(cols: u16, rows: u16) -> u64;

        #[swift_bridge(swift_name = "terminalSessionWrite")]
        fn terminal_session_write(id: u64, input: &str) -> bool;

        #[swift_bridge(swift_name = "terminalSessionRead")]
        fn terminal_session_read(id: u64) -> String;

        #[swift_bridge(swift_name = "terminalSessionSnapshot")]
        fn terminal_session_snapshot(id: u64) -> String;

        #[swift_bridge(swift_name = "terminalSessionExited")]
        fn terminal_session_exited(id: u64) -> bool;

        #[swift_bridge(swift_name = "terminalSessionResize")]
        fn terminal_session_resize(id: u64, cols: u16, rows: u16) -> bool;

        #[swift_bridge(swift_name = "terminalSessionClose")]
        fn terminal_session_close(id: u64);
    }

    extern "Swift" {
        #[swift_bridge(swift_name = "LxApp.presentInternalBrowserTab")]
        fn present_internal_browser_tab(tab_id: &str) -> bool;

        #[swift_bridge(swift_name = "LxApp.prepareInternalBrowserTabForInput")]
        fn prepare_internal_browser_tab_for_input(tab_id: &str) -> bool;
    }
}

#[cfg(all(target_os = "macos", feature = "shell-runtime"))]
struct AppleBrowserNativeInputHost;

#[cfg(all(target_os = "macos", feature = "shell-runtime"))]
impl lingxia_browser::BrowserNativeInputHost for AppleBrowserNativeInputHost {
    fn prepare_for_input(&self, tab_id: &str) -> Result<(), String> {
        if self::bridge::prepare_internal_browser_tab_for_input(tab_id) {
            Ok(())
        } else {
            Err(format!("failed to prepare browser tab for input: {tab_id}"))
        }
    }
}

#[cfg(all(target_os = "macos", feature = "shell-runtime"))]
fn install_browser_native_input_host() {
    let _ = lingxia_browser::register_native_input_host(Arc::new(AppleBrowserNativeInputHost));
}

#[cfg(not(all(target_os = "macos", feature = "shell-runtime")))]
fn install_browser_native_input_host() {}

/// Initialize the Lingxia SDK for iOS/macOS
pub fn lingxia_init(data_dir: &str, cache_dir: &str, locale: &str) -> Option<String> {
    crate::logging::init();
    install_browser_native_input_host();

    log::info!(
        "Initializing Lingxia SDK with data_dir: {}, cache_dir: {}",
        data_dir,
        cache_dir
    );

    let platform = match lingxia_platform::Platform::new(
        data_dir.to_string(),
        cache_dir.to_string(),
        locale.to_string(),
    ) {
        Ok(platform) => platform,
        Err(e) => {
            log::error!("Failed to create Platform: {}", e);
            return None;
        }
    };

    crate::init_with_platform(platform)
}

pub fn emit_sdk_log(level: i32, category: &str, appid: &str, path: &str, message: &str) -> bool {
    crate::logging::emit_sdk_log(level, category, appid, path, message)
}

/// Notify that a page is being shown
pub fn on_page_show(appid: &str, path: &str) {
    if let Some(lxapp) = lxapp::try_get(appid) {
        lxapp.on_page_show(path.to_string());
    }
}

pub fn resolve_lx_uri(appid: &str, input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Some(trimmed.to_string());
    }

    let lxapp = lxapp::try_get(appid)?;

    if let Some(path) = trimmed.strip_prefix("file://") {
        let resolved = lxapp.resolve_accessible_path(path).ok()?;
        return Some(format!("file://{}", resolved.to_string_lossy()));
    }

    let resolved = lxapp.resolve_accessible_path(trimmed).ok()?;
    Some(format!("file://{}", resolved.to_string_lossy()))
}

pub fn handle_browser_address_input(request_json: &str) -> Option<String> {
    crate::browser::resolve_input_json(request_json)
}

pub fn browser_url_is_hidden(raw: &str) -> bool {
    crate::browser::should_hide_url(raw)
}

/// Catch panics at FFI boundary and return a default value on failure.
macro_rules! ffi_catch_unwind {
    ($name:expr, $default:expr, $body:expr) => {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe($body)) {
            Ok(v) => v,
            Err(_) => {
                log::error!("panic in {}", $name);
                $default
            }
        }
    };
}

/// Notify that LxApp was closed
pub fn on_lxapp_closed(appid: &str, session_id: u64) -> bool {
    ffi_catch_unwind!("on_lxapp_closed", false, || {
        if let Some(lxapp) = lxapp::try_get(appid) {
            if session_id == 0 || session_id != lxapp.session_id() {
                return false;
            }
            lxapp.on_lxapp_closed(session_id);
            return true;
        }
        false
    })
}

/// Notify device orientation changes from host platform.
pub fn on_device_orientation_changed(appid: &str, session_id: u64, value: &str) -> bool {
    let Some(lxapp) = lxapp::try_get(appid) else {
        return false;
    };

    if session_id == 0 || session_id != lxapp.session_id() {
        return false;
    }

    let normalized = match value {
        "portrait" => "portrait",
        "landscape" => "landscape",
        _ => return false,
    };

    let payload = format!(r#"{{"value":"{}"}}"#, normalized);
    lxapp::publish_app_event(appid, "DeviceOrientationChange", Some(payload))
}

/// Handle lxapp-scoped UI events from Swift. `appid` must be a real lxapp id.
pub fn on_lxapp_event(appid: &str, event_type: self::bridge::LxAppUiEventType, data: &str) -> bool {
    let ui_event_type = match event_type {
        self::bridge::LxAppUiEventType::TabBarClick => LxAppUiEventType::TabBarClick,
        self::bridge::LxAppUiEventType::CapsuleClick => LxAppUiEventType::CapsuleClick,
        self::bridge::LxAppUiEventType::NavigationClick => LxAppUiEventType::NavigationClick,
        self::bridge::LxAppUiEventType::BackPress => LxAppUiEventType::BackPress,
        self::bridge::LxAppUiEventType::PullDownRefresh => LxAppUiEventType::PullDownRefresh,
    };

    lxapp::try_get(appid)
        .map(|lxapp| lxapp.on_lxapp_event(ui_event_type, data.to_string()))
        .unwrap_or(false)
}

/// Handle host-app scoped UI events from Swift (no lxapp context).
pub fn on_app_event(event_type: self::bridge::AppUiEventType, data: &str) -> bool {
    match event_type {
        self::bridge::AppUiEventType::PanelIconClick => {
            // data = panelId; look up config and ask Rust to load the lxapp if needed
            if let Some((app_id, path)) = crate::browser::panel_item_for_id(data) {
                crate::browser::open_panel_lxapp(data, &app_id, &path);
                true
            } else {
                false
            }
        }
        self::bridge::AppUiEventType::UpdateRestartClick => {
            // The sidebar "ready to update" callout was clicked: swap the
            // staged bundle and quit so it relaunches into the new version.
            lingxia_platform::apply_staged_macos_update()
        }
        self::bridge::AppUiEventType::UpdateInstallClick => {
            // The "update available" reminder was clicked: re-run the update
            // flow, which re-presents the card and downloads on confirm.
            crate::task::spawn(async {
                let _ = crate::update::host_app::check().await;
            });
            true
        }
    }
}

pub fn on_native_component_event(
    appid: &str,
    path: &str,
    component_id: &str,
    event_name: &str,
    payload_json: &str,
    bindings_json: &str,
) -> bool {
    lxapp::on_native_component_event(
        appid,
        path,
        component_id,
        event_name,
        payload_json,
        bindings_json,
    )
}

pub fn open_browser_tab(appid: &str, session_id: u64, url: &str) -> Option<String> {
    ffi_catch_unwind!("open_browser_tab", None, || {
        match crate::browser::open_for_app(appid, session_id, url, None) {
            Ok(tab_id) => Some(tab_id),
            Err(e) => {
                log::error!("open_browser_tab failed: {}", e);
                None
            }
        }
    })
}

pub fn open_browser_tab_with_id(
    appid: &str,
    session_id: u64,
    url: &str,
    tab_id: &str,
) -> Option<String> {
    ffi_catch_unwind!("open_browser_tab_with_id", None, || {
        match crate::browser::open_for_app(appid, session_id, url, Some(tab_id)) {
            Ok(tab_id) => Some(tab_id),
            Err(e) => {
                log::error!("open_browser_tab_with_id failed: {}", e);
                None
            }
        }
    })
}

pub fn browser_tab_close(tab_id: &str) -> bool {
    ffi_catch_unwind!("browser_tab_close", false, || {
        crate::browser::close(tab_id).is_ok()
    })
}

pub fn get_builtin_browser_app_id() -> String {
    crate::browser::APP_ID.to_string()
}

pub fn browser_tab_path_for_id(tab_id: &str) -> String {
    crate::browser::tab_path(tab_id)
}

pub fn present_internal_browser_tab(tab_id: &str) -> bool {
    ffi_catch_unwind!("present_internal_browser_tab", false, || {
        self::bridge::present_internal_browser_tab(tab_id)
    })
}

pub fn update_browser_tab_info(tab_id: &str, current_url: &str, title: &str) -> bool {
    ffi_catch_unwind!("update_browser_tab_info", false, || {
        let current_url = if current_url.trim().is_empty() {
            None
        } else {
            Some(current_url)
        };
        let title = if title.trim().is_empty() {
            None
        } else {
            Some(title)
        };
        crate::browser::update_tab(tab_id, current_url, title)
    })
}

pub fn start_browser_tab_download(
    tab_id: &str,
    url: &str,
    user_agent: &str,
    suggested_filename: &str,
    source_page_url: &str,
    cookie: &str,
) -> bool {
    ffi_catch_unwind!("start_browser_tab_download", false, || {
        match crate::browser::download(
            tab_id,
            url,
            Some(user_agent),
            Some(suggested_filename),
            Some(source_page_url),
            Some(cookie),
        ) {
            Ok(()) => {
                log::info!(
                    "start_browser_tab_download accepted tab_id={} url={}",
                    tab_id,
                    url
                );
                true
            }
            Err(err) => {
                log::warn!(
                    "start_browser_tab_download failed tab_id={} url={} error={}",
                    tab_id,
                    url,
                    err
                );
                false
            }
        }
    })
}

pub fn toggle_webview_devtools_by_ptr(webview_ptr: usize, detached: bool) -> bool {
    if webview_ptr == 0 {
        return false;
    }
    #[cfg(target_os = "macos")]
    {
        lingxia_webview::platform::apple::toggle_webview_devtools_by_swift_ptr(
            webview_ptr,
            detached,
        )
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = detached;
        false
    }
}

/// Get current active LxApp ID and path from Rust stack
pub fn get_current_lxapp() -> self::bridge::CurrentLxApp {
    let (current_appid, current_path, current_session_id) = lxapp::get_current_lxapp();
    self::bridge::CurrentLxApp {
        appid: current_appid,
        path: current_path,
        session_id: current_session_id,
    }
}

/// Get runtime session id for a specific lxapp.
pub fn get_lxapp_session_id(appid: &str) -> u64 {
    lxapp::try_get(appid)
        .map(|lxapp| lxapp.session_id())
        .unwrap_or(0)
}

pub fn find_webview_by_page_instance_id(page_instance_id: &str) -> usize {
    let page_instance_id = page_instance_id.trim();
    if page_instance_id.is_empty() {
        return 0;
    }
    let _ = lxapp::touch_page_instance_by_id(page_instance_id);

    lxapp::find_page_by_instance_id(page_instance_id)
        .and_then(|page| page.webview())
        .map(|webview| webview.get_swift_webview_ptr())
        .unwrap_or(0)
}

fn resolve_page_instance_id(appid: &str, path: &str, session_id: u64) -> String {
    if session_id == 0 {
        return String::new();
    }

    fn normalize_lookup_path(path: &str) -> &str {
        let path = path.split('?').next().unwrap_or(path);
        path.split('#').next().unwrap_or(path)
    }

    let Some(lxapp) = lxapp::try_get(appid) else {
        return String::new();
    };
    if lxapp.session_id() != session_id {
        return String::new();
    }

    let normalized_path = normalize_lookup_path(path);
    let resolved_path = lxapp
        .find_page_path(normalized_path)
        .unwrap_or_else(|| normalized_path.to_string());
    let page_instance_id = lxapp
        .page_instance_id_for_path(&resolved_path)
        .unwrap_or_default();
    if !page_instance_id.is_empty() {
        let _ = lxapp::touch_page_instance_by_id(&page_instance_id);
    }
    page_instance_id
}

pub fn resolve_page_binding(
    appid: &str,
    path: &str,
    session_id: u64,
) -> self::bridge::PageBindingResult {
    let page_instance_id = resolve_page_instance_id(appid, path, session_id);
    let webview_ptr = if page_instance_id.is_empty() {
        0
    } else {
        find_webview_by_page_instance_id(&page_instance_id)
    };
    self::bridge::PageBindingResult {
        page_instance_id,
        webview_ptr,
    }
}

fn parse_close_reason(reason: &str) -> CloseReason {
    match reason.trim().to_ascii_lowercase().as_str() {
        "user" => CloseReason::User,
        "owner_closed" => CloseReason::OwnerClosed,
        "app_closed" | "appclose" | "close" => CloseReason::AppClosed,
        "programmatic" => CloseReason::Programmatic,
        "failed" | "presentation_failed" => CloseReason::Unknown,
        _ => CloseReason::Unknown,
    }
}

fn map_presentation_kind(surface: i32) -> PresentationKind {
    match surface {
        1 => PresentationKind::Panel,
        3 => PresentationKind::Overlay,
        _ => PresentationKind::Window,
    }
}

pub fn create_page_instance_for_open(
    appid: &str,
    path: &str,
    session_id: u64,
    surface: i32,
    owner_page_instance_id: &str,
) -> self::bridge::CreatePageInstanceResult {
    if session_id == 0 {
        return self::bridge::CreatePageInstanceResult {
            ok: false,
            page_instance_id: String::new(),
            appid: appid.to_string(),
            resolved_path: String::new(),
            query: String::new(),
            error: "invalid session_id".to_string(),
        };
    }

    let Some(lxapp_instance) = lxapp::try_get(appid) else {
        return self::bridge::CreatePageInstanceResult {
            ok: false,
            page_instance_id: String::new(),
            appid: appid.to_string(),
            resolved_path: String::new(),
            query: String::new(),
            error: "lxapp not found".to_string(),
        };
    };

    if lxapp_instance.session_id() != session_id {
        return self::bridge::CreatePageInstanceResult {
            ok: false,
            page_instance_id: String::new(),
            appid: appid.to_string(),
            resolved_path: String::new(),
            query: String::new(),
            error: "stale session".to_string(),
        };
    }

    let owner = if let Some(owner_id) = PageInstanceId::parse(owner_page_instance_id.to_string()) {
        PageOwner::Page(owner_id)
    } else {
        PageOwner::Scene(SceneId("system".to_string()))
    };

    match lxapp::create_page_instance(CreatePageInstanceRequest {
        owner,
        appid: appid.to_string(),
        target: PageTarget::Path(path.to_string()),
        query: None::<PageQueryInput>,
        surface: map_presentation_kind(surface),
    }) {
        Ok(created) => self::bridge::CreatePageInstanceResult {
            ok: true,
            page_instance_id: created.page_instance_id.to_string(),
            appid: created.appid,
            resolved_path: created.resolved_path,
            query: created.query,
            error: String::new(),
        },
        Err(err) => self::bridge::CreatePageInstanceResult {
            ok: false,
            page_instance_id: String::new(),
            appid: appid.to_string(),
            resolved_path: String::new(),
            query: String::new(),
            error: err.to_string(),
        },
    }
}

pub fn notify_page_instance_mounted(page_instance_id: &str) -> bool {
    lxapp::notify_page_instance_by_id(page_instance_id, PageInstanceEvent::Mounted).is_ok()
}

pub fn notify_page_instance_visible(page_instance_id: &str) -> bool {
    lxapp::notify_page_instance_by_id(page_instance_id, PageInstanceEvent::Visible).is_ok()
}

pub fn notify_page_instance_hidden(page_instance_id: &str, reason: &str) -> bool {
    lxapp::notify_page_instance_by_id(
        page_instance_id,
        PageInstanceEvent::Hidden {
            reason: parse_close_reason(reason),
        },
    )
    .is_ok()
}

pub fn dispose_page_instance(page_instance_id: &str, reason: &str) -> bool {
    lxapp::dispose_page_instance_by_id(page_instance_id, parse_close_reason(reason)).is_ok()
}

pub fn on_surface_closed(appid: &str, id: &str, reason: &str) -> bool {
    if let Some(lxapp) = lxapp::try_get(appid) {
        let _ = lxapp.forget_surface(id);
    }
    #[cfg(feature = "standard")]
    {
        lingxia_logic::notify_surface_closed(id, reason)
    }
    #[cfg(not(feature = "standard"))]
    {
        let _ = reason;
        false
    }
}

/// Get LxApp information
/// Returns default empty values if app not found.
///
/// This works around a swift-bridge `Option<struct with String>` limitation.
pub fn get_lxapp_info(appid: &str) -> self::bridge::LxAppInfo {
    if let Some(lxapp) = lxapp::try_get(appid) {
        let lxapp_info = lxapp.get_lxapp_info();
        self::bridge::LxAppInfo {
            app_name: lxapp_info.app_name,
            version: lxapp_info.version,
            release_type: lxapp_info.release_type,
            cache_dir: lxapp.user_cache_dir.to_string_lossy().into_owned(),
        }
    } else {
        self::bridge::LxAppInfo {
            app_name: String::new(),
            version: String::new(),
            release_type: String::new(),
            cache_dir: String::new(),
        }
    }
}

/// Get NavigationBar state
/// Returns default values if app not found.
///
/// This works around a swift-bridge `Option<struct with String>` limitation.
pub fn get_navigation_bar_state(appid: &str, path: &str) -> self::bridge::NavigationBarState {
    if let Some(lxapp) = lxapp::try_get(appid) {
        let nav_state = lxapp.get_navbar_state(path);
        let bg_color = parse_color_to_u32(&nav_state.navigationBarBackgroundColor, 0xFFFFFFFF);

        self::bridge::NavigationBarState {
            background_color: bg_color,
            text_style: nav_state.navigationBarTextStyle,
            title_text: nav_state.navigationBarTitleText,
            show_navbar: nav_state.show_navbar,
            show_back_button: nav_state.show_back_button,
            show_home_button: nav_state.show_home_button,
        }
    } else {
        self::bridge::NavigationBarState {
            background_color: 0,
            text_style: String::new(),
            title_text: String::new(),
            show_navbar: false,
            show_back_button: false,
            show_home_button: false,
        }
    }
}

/// Get page orientation for a specific page path.
/// Returns: 0=auto, 1=portrait, 2=landscape, 3=reverse-portrait, 4=reverse-landscape
pub fn get_page_orientation(appid: &str, path: &str) -> i32 {
    let Some(lxapp_instance) = lxapp::try_get(appid) else {
        return 0;
    };

    let orientation = lxapp_instance.get_page_orientation(path);
    orientation_to_value(orientation)
}

fn orientation_to_value(orientation: OrientationConfig) -> i32 {
    match (orientation.mode, orientation.rotation) {
        (PageOrientation::Auto, _) => 0,
        (PageOrientation::Portrait, 180) => 3,
        (PageOrientation::Portrait, _) => 1,
        (PageOrientation::Landscape, 180) => 4,
        (PageOrientation::Landscape, _) => 2,
    }
}

/// Get TabBar state
pub fn get_tab_bar(appid: &str) -> Option<self::bridge::TabBar> {
    lxapp::try_get(appid).and_then(|lxapp| {
        lxapp.get_tabbar().map(|tabbar| self::bridge::TabBar {
            color: parse_color_to_u32(&tabbar.color, 0xFF666666),
            selected_color: parse_color_to_u32(&tabbar.selectedColor, 0xFF1677FF),
            background_color: parse_color_to_u32(&tabbar.backgroundColor, 0xFFFFFFFF),
            border_style: parse_color_to_u32(&tabbar.borderStyle, 0xFFF0F0F0),
            position: tabbar.position.to_i32(),
            dimension: tabbar.dimension,
            items_count: tabbar.list.len() as i32,
            is_visible: tabbar.is_visible,
            selected_index: tabbar.selected_index,
        })
    })
}

/// Get TabBar item by index
pub fn get_tab_bar_item(appid: &str, index: i32) -> Option<self::bridge::TabBarItem> {
    lxapp::try_get(appid)
        .and_then(|lxapp| lxapp.get_tabbar())
        .and_then(|tabbar| {
            tabbar.get_item(index).map(|item| self::bridge::TabBarItem {
                page_path: item.pagePath.clone(),
                text: item.text.clone().unwrap_or_default(),
                icon_path: item.iconPath.clone().unwrap_or_default(),
                selected_icon_path: item.selectedIconPath.clone().unwrap_or_default(),
                selected: item.selected,
                group: match &item.group {
                    Some(lxapp::tabbar::TabItemGroup::Start) => self::bridge::GroupAlignment::Start,
                    Some(lxapp::tabbar::TabItemGroup::End) => self::bridge::GroupAlignment::End,
                    None => self::bridge::GroupAlignment::Center,
                },
                badge: item.badge.clone().unwrap_or_default(),
                has_red_dot: item.has_red_dot,
            })
        })
}

/// Handle AppLink URL by processing the path (Universal Link)
pub fn on_applink_received(url: &str) -> i32 {
    lingxia_service::applink::handle(url)
}

/// Handle Push Notification Link with trigger context
pub fn on_pushlink_received(url: &str, trigger: self::bridge::PushTrigger) -> i32 {
    let trigger_name = match trigger {
        self::bridge::PushTrigger::Background => "Background",
        self::bridge::PushTrigger::Tap => "Tap",
        self::bridge::PushTrigger::Launch => "Launch",
    };

    log::info!(
        "[Apple] Push Link received: {} (trigger: {})",
        url,
        trigger_name
    );

    lingxia_service::applink::handle(url)
}

/// Handle push notification device token
pub fn on_push_token_received(token: &str) -> i32 {
    crate::push::bind_push_token_for_ffi(token.to_string())
}

/// Callback from platform (called from Swift/Objective-C)
///
/// # Parameters
/// - `id`: Callback ID for correlating with pending operation
/// - `success`: Whether the operation completed successfully
/// - `data`: When `success=true`, contains JSON payload; when `success=false`, contains error code string (see i18n/err_code)
pub fn on_callback(id: u64, success: bool, data: &str) -> bool {
    let result = if success {
        Ok(data.to_string())
    } else {
        // Parse data as u32 error code, default to 1000 (unknown error) if failed
        Err(data.parse::<u32>().unwrap_or(1000))
    };

    if invoke_callback(id, result) {
        true
    } else {
        log::warn!("[Apple] Callback not found for id={}", id);
        false
    }
}

/// Check if pull-down refresh is enabled for a specific page
pub fn is_pull_down_refresh_enabled(appid: &str, path: &str) -> bool {
    lxapp::is_pull_down_refresh_enabled(appid, path)
}

/// Notify that app entered foreground
/// Called from Swift when UIApplication receives willEnterForeground notification
pub fn on_app_show(lxappid: &str) {
    if let Some(lxapp) = lxapp::try_get(lxappid) {
        let args = lxapp::AppServiceEventArgs {
            source: lxapp::AppServiceEventSource::Host,
            reason: lxapp::AppServiceEventReason::Foreground,
        }
        .to_json_string();
        let _ = lxapp.appservice_notify(lxapp::AppServiceEvent::OnShow, Some(args));
    }
}

/// Notify that app entered background
/// Called from Swift when UIApplication receives didEnterBackground notification
pub fn on_app_hide(lxappid: &str) {
    if let Some(lxapp) = lxapp::try_get(lxappid) {
        let args = lxapp::AppServiceEventArgs {
            source: lxapp::AppServiceEventSource::Host,
            reason: lxapp::AppServiceEventReason::Background,
        }
        .to_json_string();
        let _ = lxapp.appservice_notify(lxapp::AppServiceEvent::OnHide, Some(args));
    }
}

/// Notify that user captured a screenshot
/// Called from Swift when UIApplication receives userDidTakeScreenshot notification
pub fn on_user_capture_screen(lxappid: &str) {
    if let Some(lxapp) = lxapp::try_get(lxappid) {
        let args = lxapp::AppServiceEventArgs {
            source: lxapp::AppServiceEventSource::Host,
            reason: lxapp::AppServiceEventReason::Screenshot,
        }
        .to_json_string();
        let _ = lxapp.appservice_notify(lxapp::AppServiceEvent::OnUserCaptureScreen, Some(args));
    }
}

pub fn get_app_capabilities() -> u32 {
    crate::capabilities::app_capabilities()
}

/// Get panels config as a JSON string.
/// Returns None if no panels are configured in app.json.
pub fn get_panels_config_json() -> Option<String> {
    crate::browser::panels_config_json()
}

/// Open a lxapp for a panel without pushing it to the navigation stack.
/// panel_id is used by Rust as the panel slot context for surface routing.
pub fn open_panel_lxapp(panel_id: &str, appid: &str, path: &str) {
    crate::browser::open_panel_lxapp(panel_id, appid, path);
}

pub fn get_terminal_backend_status_json() -> String {
    #[cfg(feature = "terminal-runtime")]
    {
        return crate::terminal::ghostty_status_json();
    }

    #[cfg(not(feature = "terminal-runtime"))]
    {
        r#"{"backend":"ghostty","available":false,"status":"terminal runtime disabled","sourceDir":null,"libDir":null}"#.to_string()
    }
}

pub fn terminal_session_create(cols: u16, rows: u16) -> u64 {
    #[cfg(feature = "terminal-runtime")]
    {
        return crate::terminal::terminal_create(cols, rows);
    }

    #[cfg(not(feature = "terminal-runtime"))]
    {
        let _ = (cols, rows);
        0
    }
}

pub fn terminal_session_write(id: u64, input: &str) -> bool {
    #[cfg(feature = "terminal-runtime")]
    {
        return crate::terminal::terminal_write(id, input);
    }

    #[cfg(not(feature = "terminal-runtime"))]
    {
        let _ = (id, input);
        false
    }
}

pub fn terminal_session_read(id: u64) -> String {
    #[cfg(feature = "terminal-runtime")]
    {
        return crate::terminal::terminal_read(id);
    }

    #[cfg(not(feature = "terminal-runtime"))]
    {
        let _ = id;
        String::new()
    }
}

pub fn terminal_session_snapshot(id: u64) -> String {
    #[cfg(feature = "terminal-runtime")]
    {
        return crate::terminal::terminal_snapshot(id);
    }

    #[cfg(not(feature = "terminal-runtime"))]
    {
        let _ = id;
        r#"{"cols":0,"rows":0,"lines":[],"cells":[],"cursor_row":0,"cursor_col":0,"cursor_visible":false,"application_cursor":false,"bracketed_paste":false,"alternate_screen":false,"title":null,"generation":0,"exited":true}"#.to_string()
    }
}

pub fn terminal_session_exited(id: u64) -> bool {
    #[cfg(feature = "terminal-runtime")]
    {
        return crate::terminal::terminal_exited(id);
    }

    #[cfg(not(feature = "terminal-runtime"))]
    {
        let _ = id;
        true
    }
}

pub fn terminal_session_resize(id: u64, cols: u16, rows: u16) -> bool {
    #[cfg(feature = "terminal-runtime")]
    {
        return crate::terminal::terminal_resize(id, cols, rows);
    }

    #[cfg(not(feature = "terminal-runtime"))]
    {
        let _ = (id, cols, rows);
        false
    }
}

pub fn terminal_session_close(id: u64) {
    #[cfg(feature = "terminal-runtime")]
    {
        crate::terminal::terminal_close(id);
    }

    #[cfg(not(feature = "terminal-runtime"))]
    {
        let _ = id;
    }
}
