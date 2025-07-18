use super::app::App;
use crate::runtime::SimpleAppRuntime;
use miniapp::AppUiDelegate;
use miniapp::log::LogLevel;

#[swift_bridge::bridge]
mod bridge {
    extern "Rust" {
        #[swift_bridge(swift_name = "lxappInit")]
        fn lxapp_init(data_dir: &str, cache_dir: &str) -> Option<String>;

        #[swift_bridge(swift_name = "onPageShow")]
        fn on_page_show(appid: &str, path: &str);

        #[swift_bridge(swift_name = "onLxappClosed")]
        fn on_lxapp_closed(appid: &str) -> i32;

        #[swift_bridge(swift_name = "getPageConfig")]
        fn get_page_config(appid: &str, path: &str) -> Option<String>;

        #[swift_bridge(swift_name = "onBackPressed")]
        fn on_back_pressed(appid: &str) -> bool;

        #[swift_bridge(swift_name = "onLxappOpened")]
        fn on_lxapp_opened(appid: &str, path: &str) -> i32;

        #[swift_bridge(swift_name = "getTabBarConfig")]
        fn get_tab_bar_config(appid: &str) -> Option<String>;

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
    let final_init_details = miniapp::init(runtime);

    // Format and return the result
    match final_init_details {
        Some((home_app_id, initial_route)) => {
            let combined_details = format!("{}:{}", home_app_id, initial_route);
            log::info!("LxApp initialization successful: {}", combined_details);
            Some(combined_details)
        }
        None => {
            log::error!("Failed to obtain LxApp home app details during initialization.");
            None
        }
    }
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

/// Get page configuration
pub fn get_page_config(appid: &str, path: &str) -> Option<String> {
    let miniapp = miniapp::get(appid.to_string());
    match miniapp.get_page_config(path) {
        Ok(config) => Some(config),
        Err(_) => None,
    }
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

/// Get tab bar configuration
pub fn get_tab_bar_config(appid: &str) -> Option<String> {
    let miniapp = miniapp::get(appid.to_string());
    match miniapp.get_tab_bar_config() {
        Ok(config) => Some(config),
        Err(_) => None,
    }
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
