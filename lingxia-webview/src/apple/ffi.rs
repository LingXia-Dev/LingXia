use super::app::App;
use crate::runtime::SimpleAppRuntime;
use miniapp::AppUiDelegate;
use miniapp::config::LxAppInfo as CoreLxAppInfo;
use miniapp::log::LogLevel;

// Constants for TabBarPosition
pub const TAB_BAR_POSITION_BOTTOM: i32 = 0;
pub const TAB_BAR_POSITION_TOP: i32 = 1;
pub const TAB_BAR_POSITION_LEFT: i32 = 2;
pub const TAB_BAR_POSITION_RIGHT: i32 = 3;

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
        pub background_color: String,
        pub text_style: String,
        pub title_text: String,
        pub navigation_style: i32,
    }

    // TabBar configuration for Swift (without items array)
    #[swift_bridge(swift_repr = "struct")]
    pub struct TabBarConfig {
        pub color: String,
        pub selected_color: String,
        pub background_color: String,
        pub border_style: String,
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
    }

    extern "Swift" {
        // LxApp navigation functions
        #[swift_bridge(swift_name = "LxApp.openLxApp")]
        fn open_lxapp(appid: &str, path: &str) -> bool;

        #[swift_bridge(swift_name = "LxApp.closeLxApp")]
        fn close_lxapp(appid: &str) -> bool;

        #[swift_bridge(swift_name = "LxApp.switchPage")]
        fn switch_page(appid: &str, path: &str) -> bool;
    }
}

// Re-export the bridge functions for use in other modules
pub use bridge::{close_lxapp, open_lxapp, switch_page};

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
    miniapp::log::LogManager::init(|log_msg| {
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

    // Initialize SimpleAppRuntime and miniapp
    let runtime = SimpleAppRuntime::init(app);
    miniapp::init(runtime)
}

/// Notify that a page is being shown
pub fn on_page_show(appid: &str, path: &str) {
    let miniapp = miniapp::get(appid.to_string());
    miniapp.on_page_show(path.to_string());
}

/// Notify that LxApp was closed
pub fn on_lxapp_closed(appid: &str) -> i32 {
    let miniapp = miniapp::get(appid.to_string());
    miniapp.on_lxapp_closed();
    0
}

/// Handle back button press
pub fn on_back_pressed(appid: &str) -> bool {
    let miniapp = miniapp::get(appid.to_string());
    miniapp.on_back_pressed()
}

/// Notify that LxApp was opened
pub fn on_lxapp_opened(appid: &str, path: &str) -> i32 {
    let miniapp = miniapp::get(appid.to_string());
    miniapp.on_lxapp_opened(path.to_string());
    0
}

/// Find a WebView for the specified app and path
/// This is called from Swift to get a WebView instance pointer managed by Rust
/// Returns the usize pointer to the WebView, or 0 if not found
pub fn find_webview(appid: &str, path: &str) -> usize {
    // Get the runtime and try to find the WebView
    if let Some(runtime) = SimpleAppRuntime::get() {
        if let Some(webview) = runtime.get_webview(appid, path) {
            // WebView exists, return its pointer
            webview.get_swift_webview_ptr()
        } else {
            log::error!(
                "💥 WebView NOT FOUND in runtime for appid={}, path={}",
                appid,
                path
            );
            // No WebView found
            0
        }
    } else {
        log::error!("Runtime not initialized");
        0
    }
}

/// Get LxApp information
pub fn get_lxapp_info(appid: &str) -> bridge::LxAppInfo {
    let miniapp = miniapp::get(appid.to_string());
    let lxapp_info = miniapp.get_config().get_lxapp_info();

    // Convert from core LxAppInfo to FFI LxAppInfo
    lxapp_info.into()
}

/// Get NavigationBar configuration
pub fn get_navigation_bar_config(appid: &str, path: &str) -> bridge::NavigationBarConfig {
    let miniapp = miniapp::get(appid.to_string());
    let nav_config = miniapp.get_config().get_nav_bar_config(&miniapp, path);

    // Convert to FFI struct
    bridge::NavigationBarConfig {
        background_color: nav_config.navigationBarBackgroundColor,
        text_style: nav_config.navigationBarTextStyle,
        title_text: nav_config.navigationBarTitleText,
        navigation_style: nav_config.navigationStyle.to_i32(),
    }
}

/// Get TabBar configuration
pub fn get_tab_bar_config(appid: &str) -> Option<bridge::TabBarConfig> {
    let miniapp = miniapp::get(appid.to_string());

    miniapp
        .get_config()
        .get_tab_bar_config(&miniapp)
        .map(|config| bridge::TabBarConfig {
            color: config.color,
            selected_color: config.selectedColor,
            background_color: config.backgroundColor,
            border_style: config.borderStyle,
            position: match config.position {
                miniapp::config::TabBarPosition::Bottom => TAB_BAR_POSITION_BOTTOM,
                miniapp::config::TabBarPosition::Top => TAB_BAR_POSITION_TOP,
                miniapp::config::TabBarPosition::Left => TAB_BAR_POSITION_LEFT,
                miniapp::config::TabBarPosition::Right => TAB_BAR_POSITION_RIGHT,
            },
            dimension: config.dimension,
            items_count: config.list.len() as i32,
        })
}

/// Get TabBar item by index
pub fn get_tab_bar_item(appid: &str, index: i32) -> Option<bridge::TabBarItem> {
    let miniapp = miniapp::get(appid.to_string());

    miniapp
        .get_config()
        .get_tab_bar_config(&miniapp)
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
                })
        })
}
