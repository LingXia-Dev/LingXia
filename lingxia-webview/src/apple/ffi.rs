use super::app::App;
use crate::controller::Controller;
use miniapp::AppUiDelegate;
use miniapp::log::LogLevel;
use std::sync::mpsc;

#[swift_bridge::bridge]
mod bridge {
    extern "Rust" {
        #[swift_bridge(swift_name = "miniappInit")]
        fn miniapp_init(data_dir: &str, cache_dir: &str) -> Option<String>;

        #[swift_bridge(swift_name = "onWebviewAttached")]
        fn on_webview_attached(appid: &str, path: &str) -> i32;

        #[swift_bridge(swift_name = "onPageShow")]
        fn on_page_show(appid: &str, path: &str);

        #[swift_bridge(swift_name = "onMiniappClosed")]
        fn on_miniapp_closed(appid: &str) -> i32;

        #[swift_bridge(swift_name = "getPageConfig")]
        fn get_page_config(appid: &str, path: &str) -> Option<String>;

        #[swift_bridge(swift_name = "onBackPressed")]
        fn on_back_pressed(appid: &str) -> bool;

        #[swift_bridge(swift_name = "onMiniappOpened")]
        fn on_miniapp_opened(appid: &str, path: &str) -> i32;

        #[swift_bridge(swift_name = "getTabBarConfig")]
        fn get_tab_bar_config(appid: &str) -> Option<String>;

        #[swift_bridge(swift_name = "findWebView")]
        fn find_webview(appid: &str, path: &str) -> usize;
    }

    extern "Swift" {
        // MiniApp navigation functions
        #[swift_bridge(swift_name = "MiniApp.openMiniApp")]
        fn open_miniapp(appid: &str, path: &str) -> bool;

        #[swift_bridge(swift_name = "MiniApp.closeMiniApp")]
        fn close_miniapp(appid: &str) -> bool;

        #[swift_bridge(swift_name = "MiniApp.switchPage")]
        fn switch_page(appid: &str, path: &str) -> bool;
    }
}

// Re-export the bridge functions for use in other modules
pub use bridge::{close_miniapp, open_miniapp, switch_page};

/// Initialize the MiniApp system for iOS/macOS
pub fn miniapp_init(data_dir: &str, cache_dir: &str) -> Option<String> {
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
        "Initializing MiniApp with data_dir: {}, cache_dir: {}",
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

    // Create a channel to receive the result from the closure
    let (tx, rx) = mpsc::channel::<Option<(String, String)>>();

    if !Controller::run(
        move |controller| -> bool {
            let result_option = miniapp::init(controller);

            // Send the result back to the main thread
            if tx.send(result_option).is_err() {
                log::error!("Failed to send init result: Receiver dropped?");
            }

            true
        },
        app,
    ) {
        log::error!("Controller::run reported failure (returned false).");
        let _ = rx.recv();
        return None;
    }

    let final_init_details = match rx.recv() {
        Ok(details_option) => details_option,
        Err(e) => {
            log::error!("Failed to receive result from channel: {}", e);
            None
        }
    };

    // Format and return the result
    match final_init_details {
        Some((home_app_id, initial_route)) => {
            let combined_details = format!("{}:{}", home_app_id, initial_route);
            log::info!("MiniApp initialization successful: {}", combined_details);
            Some(combined_details)
        }
        None => {
            log::error!("Failed to obtain MiniApp home app details during initialization.");
            None
        }
    }
}

/// Notify that a WebView has been attached to the window
pub fn on_webview_attached(appid: &str, path: &str) -> i32 {
    let miniapp = miniapp::get(appid.to_string());
    miniapp.on_webview_attached(path.to_string());
    0
}

/// Notify that a page is being shown
pub fn on_page_show(appid: &str, path: &str) {
    let miniapp = miniapp::get(appid.to_string());
    miniapp.on_page_show(path.to_string());
}

/// Notify that MiniApp was closed
pub fn on_miniapp_closed(appid: &str) -> i32 {
    let miniapp = miniapp::get(appid.to_string());
    miniapp.on_miniapp_closed();
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

/// Notify that MiniApp was opened
pub fn on_miniapp_opened(appid: &str, path: &str) -> i32 {
    let miniapp = miniapp::get(appid.to_string());
    miniapp.on_miniapp_opened(path.to_string());
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
    // Get the controller and try to find the WebView
    if let Some(controller) = Controller::get() {
        if let Some(webview) = controller.get_webview(appid, path) {
            // WebView exists, return its pointer
            webview.inner().get_swift_webview_ptr()
        } else {
            // No WebView found
            0
        }
    } else {
        log::error!("Controller not initialized");
        0
    }
}
