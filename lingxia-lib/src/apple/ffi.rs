use super::app::App;
use crate::runtime::PlatformAppRuntime;
use lxapp::LxAppDelegate;
use lxapp::config::LxAppInfo as CoreLxAppInfo;
use lxapp::log::LogLevel;

/// Parses a color string (e.g., "#RRGGBB" or "transparent") into a u32 ARGB value.
fn parse_color_to_u32(color_str: &str, default_color: u32) -> u32 {
    if color_str.eq_ignore_ascii_case("transparent") {
        return 0x00000000;
    }

    if color_str.starts_with('#') {
        let hex_part = &color_str[1..];
        if hex_part.len() == 6 {
            if let Ok(rgb) = u32::from_str_radix(hex_part, 16) {
                return 0xFF000000 | rgb; // Add full alpha
            }
        }
    }
    default_color
}

#[swift_bridge::bridge]
mod bridge {
    // LxApp basic information for Swift
    #[swift_bridge(swift_repr = "struct")]
    pub struct LxAppInfo {
        pub initial_route: String,
        pub app_name: String,
        pub debug: bool,
    }

    // NavigationBar configuration for Swift
    #[swift_bridge(swift_repr = "struct")]
    pub struct NavigationBarConfig {
        pub background_color: u32,
        pub text_style: String,
        pub title_text: String,
        pub navigation_style: i32,
    }

    // TabBar configuration for Swift (without items array)
    #[swift_bridge(swift_repr = "struct")]
    pub struct TabBarConfig {
        pub color: u32,
        pub selected_color: u32,
        pub background_color: u32,
        pub border_style: u32,
        pub position: i32,
        pub dimension: i32,
        pub items_count: i32,
    }

    // TabBar item for Swift
    #[swift_bridge(swift_repr = "struct")]
    pub struct TabBarItem {
        pub page_path: String,
        pub text: String,
        pub icon_path: String,
        pub selected_icon_path: String,
        pub selected: bool,
        pub group: i32, // 0=middle/center (default), 1=start (top/left), 2=end (bottom/right)
    }

    extern "Rust" {
        #[swift_bridge(swift_name = "lxappInit")]
        fn lxapp_init(data_dir: &str, cache_dir: &str) -> Option<String>;

        #[swift_bridge(swift_name = "onPageShow")]
        fn on_page_show(appid: &str, path: &str);

        #[swift_bridge(swift_name = "onLxappClosed")]
        fn on_lxapp_closed(appid: &str) -> i32;

        #[swift_bridge(swift_name = "getLxAppInfo")]
        fn get_lxapp_info(appid: &str) -> LxAppInfo;

        #[swift_bridge(swift_name = "getNavigationBarConfig")]
        fn get_navigation_bar_config(appid: &str, path: &str) -> NavigationBarConfig;

        #[swift_bridge(swift_name = "getTabBarConfig")]
        fn get_tab_bar_config(appid: &str) -> Option<TabBarConfig>;

        #[swift_bridge(swift_name = "getTabBarItem")]
        fn get_tab_bar_item(appid: &str, index: i32) -> Option<TabBarItem>;

        #[swift_bridge(swift_name = "onBackPressed")]
        fn on_back_pressed(appid: &str) -> bool;

        #[swift_bridge(swift_name = "onLxappOpened")]
        fn on_lxapp_opened(appid: &str, path: &str) -> i32;

        #[swift_bridge(swift_name = "findWebView")]
        fn find_webview(appid: &str, path: &str) -> usize;

        #[swift_bridge(swift_name = "onApplinkReceived")]
        fn on_applink_received(applink_path: &str) -> i32;

        #[swift_bridge(swift_name = "onPushTokenReceived")]
        fn on_push_token_received(token: &str) -> i32;
    }

    extern "Swift" {
        // LxApp navigation functions
        #[swift_bridge(swift_name = "LxApp.openLxApp")]
        fn open_lxapp(appid: &str, path: &str) -> bool;

        #[swift_bridge(swift_name = "LxApp.closeLxApp")]
        fn close_lxapp(appid: &str) -> bool;

        #[swift_bridge(swift_name = "LxApp.switchPage")]
        fn switch_page(appid: &str, path: &str) -> bool;

        #[swift_bridge(swift_name = "LxApp.launchWithUrl")]
        fn launch_with_url(url: &str);

        #[swift_bridge(swift_name = "LxApp.isPushEnabled")]
        fn is_push_enabled() -> bool;
    }
}

// Re-export the bridge functions for use in other modules
pub use bridge::{close_lxapp, launch_with_url, open_lxapp, switch_page};

// Conversion from core LxAppInfo to FFI LxAppInfo
impl From<CoreLxAppInfo> for bridge::LxAppInfo {
    fn from(core_info: CoreLxAppInfo) -> Self {
        Self {
            initial_route: core_info.initial_route,
            app_name: core_info.app_name,
            debug: core_info.debug,
        }
    }
}

/// Initialize the LxApp system for iOS/macOS
pub fn lxapp_init(data_dir: &str, cache_dir: &str) -> Option<String> {
    oslog::OsLogger::new("LingXia.Rust")
        .level_filter(log::LevelFilter::Info)
        .init()
        .unwrap();

    // Initialize the new logging system
    lxapp::log::LogManager::init(|log_msg| {
        let formatted_message = format!(
            "[{}{}{}] {}",
            log_msg.tag.as_str(),
            log_msg
                .appid
                .as_ref()
                .map(|id| format!(":{}", id))
                .unwrap_or_default(),
            log_msg
                .path
                .as_ref()
                .map(|p| format!(":{}", p))
                .unwrap_or_default(),
            log_msg.message
        );

        // Use log macros directly now that we have set up the global logger
        match log_msg.level {
            LogLevel::Verbose | LogLevel::Debug => {
                log::debug!("{}", formatted_message);
            }
            LogLevel::Info => {
                log::info!("{}", formatted_message);
            }
            LogLevel::Warn => {
                log::warn!("{}", formatted_message);
            }
            LogLevel::Error => {
                log::error!("{}", formatted_message);
            }
        }
    });

    log::info!(
        "Initializing LxApp with data_dir: {}, cache_dir: {}",
        data_dir,
        cache_dir
    );

    let app = match App::new(data_dir.to_string(), cache_dir.to_string()) {
        Ok(app) => app,
        Err(e) => {
            log::error!("Failed to create App: {}", e);
            return None;
        }
    };

    // Initialize PlatformAppRuntime and lxapp
    let runtime = PlatformAppRuntime::init(app);
    lxapp::init(runtime)
}

/// Notify that a page is being shown
pub fn on_page_show(appid: &str, path: &str) {
    let lxapp = lxapp::get(appid.to_string());
    lxapp.on_page_show(path.to_string());
}

/// Notify that LxApp was closed
pub fn on_lxapp_closed(appid: &str) -> i32 {
    let lxapp = lxapp::get(appid.to_string());
    lxapp.on_lxapp_closed();
    0
}

/// Handle back button press
pub fn on_back_pressed(appid: &str) -> bool {
    let lxapp = lxapp::get(appid.to_string());
    lxapp.on_back_pressed()
}

/// Notify that LxApp was opened
pub fn on_lxapp_opened(appid: &str, path: &str) -> i32 {
    let lxapp = lxapp::get(appid.to_string());
    lxapp.on_lxapp_opened(path.to_string());
    0
}

/// Find a WebView for the specified app and path
/// This is called from Swift to get a WebView instance pointer managed by Rust
/// Returns the usize pointer to the WebView, or 0 if not found
pub fn find_webview(appid: &str, path: &str) -> usize {
    // Use lingxia-webview's find_webview function
    if let Some(webview) = lingxia_webview::find_webview(appid, path) {
        // WebView exists, return its pointer
        webview.get_swift_webview_ptr()
    } else {
        log::error!("💥 WebView not found for appid: {}, path: {}", appid, path);
        0
    }
}

/// Get LxApp information
pub fn get_lxapp_info(appid: &str) -> bridge::LxAppInfo {
    let lxapp = lxapp::get(appid.to_string());
    let lxapp_info = lxapp.get_config().get_lxapp_info();

    // Convert from core LxAppInfo to FFI LxAppInfo
    lxapp_info.into()
}

/// Get NavigationBar configuration
pub fn get_navigation_bar_config(appid: &str, path: &str) -> bridge::NavigationBarConfig {
    let lxapp = lxapp::get(appid.to_string());
    let nav_config = lxapp.get_config().get_nav_bar_config(&lxapp, path);

    // Convert to FFI struct
    bridge::NavigationBarConfig {
        background_color: parse_color_to_u32(&nav_config.navigationBarBackgroundColor, 0xFFFFFFFF),
        text_style: nav_config.navigationBarTextStyle,
        title_text: nav_config.navigationBarTitleText,
        navigation_style: nav_config.navigationStyle.to_i32(),
    }
}

/// Get TabBar configuration
pub fn get_tab_bar_config(appid: &str) -> Option<bridge::TabBarConfig> {
    let lxapp = lxapp::get(appid.to_string());

    lxapp
        .get_config()
        .get_tab_bar_config(&lxapp)
        .map(|config| bridge::TabBarConfig {
            color: parse_color_to_u32(&config.color, 0xFF666666),
            selected_color: parse_color_to_u32(&config.selectedColor, 0xFF1677FF),
            background_color: parse_color_to_u32(&config.backgroundColor, 0xFFFFFFFF),
            border_style: parse_color_to_u32(&config.borderStyle, 0xFFF0F0F0),
            position: config.position.to_i32(),
            dimension: config.dimension,
            items_count: config.list.len() as i32,
        })
}

/// Get TabBar item by index
pub fn get_tab_bar_item(appid: &str, index: i32) -> Option<bridge::TabBarItem> {
    let lxapp = lxapp::get(appid.to_string());

    lxapp
        .get_config()
        .get_tab_bar_config(&lxapp)
        .and_then(|config| {
            config
                .list
                .get(index as usize)
                .map(|item| bridge::TabBarItem {
                    page_path: item.pagePath.clone(),
                    text: item.text.clone().unwrap_or_default(),
                    icon_path: item.iconPath.clone().unwrap_or_default(),
                    selected_icon_path: item.selectedIconPath.clone().unwrap_or_default(),
                    selected: item.selected,
                    group: match &item.group {
                        Some(lxapp::config::TabItemGroup::Start) => 1i32,
                        Some(lxapp::config::TabItemGroup::End) => 2i32,
                        None => 0i32,
                    },
                })
        })
}

/// Handle AppLink URL by processing the path
pub fn on_applink_received(url: &str) -> i32 {
    log::info!("[Apple] AppLink received: {}", url);
    0
}

/// Handle push notification device token
pub fn on_push_token_received(token: &str) -> i32 {
    log::info!("[Apple] Push token received: {}", token);
    0
}
