use super::app::App;
use crate::controller::Controller;
use miniapp::AppUiDelegate;
use miniapp::log::LogLevel;
use std::sync::{OnceLock, mpsc};

/// Global reference to the native app delegate for callbacks
/// Using usize to make it thread-safe
pub(crate) static APP_DELEGATE: OnceLock<usize> = OnceLock::new();

#[swift_bridge::bridge]
mod bridge {
    extern "Rust" {
        #[swift_bridge(swift_name = "miniappInit")]
        fn miniapp_init(data_dir: &str, cache_dir: &str, app_delegate: usize) -> Option<String>;

        #[swift_bridge(swift_name = "onWebviewAttached")]
        fn on_webview_attached(appid: &str, path: &str) -> i32;

        #[swift_bridge(swift_name = "handlePostMessage")]
        fn handle_post_message(appid: &str, path: &str, message: &str) -> i32;

        #[swift_bridge(swift_name = "onPageStarted")]
        fn on_page_started(appid: &str, path: &str) -> i32;

        #[swift_bridge(swift_name = "onPageFinished")]
        fn on_page_finished(appid: &str, path: &str) -> i32;

        #[swift_bridge(swift_name = "onPageShow")]
        fn on_page_show(appid: &str, path: &str);

        #[swift_bridge(swift_name = "shouldOverrideUrlLoading")]
        fn should_override_url_loading(appid: &str, url: &str) -> bool;

        #[swift_bridge(swift_name = "handleRequest")]
        fn handle_request(
            appid: &str,
            url: &str,
            method: &str,
            headers: &str,
            body: Vec<u8>,
        ) -> Option<String>;

        #[swift_bridge(swift_name = "onMiniappClosed")]
        fn on_miniapp_closed(appid: &str) -> i32;

        #[swift_bridge(swift_name = "consoleMessage")]
        fn console_message(appid: &str, path: &str, level: i32, message: &str) -> i32;

        #[swift_bridge(swift_name = "getPageConfig")]
        fn get_page_config(appid: &str, path: &str) -> Option<String>;

        #[swift_bridge(swift_name = "onBackPressed")]
        fn on_back_pressed(appid: &str) -> bool;

        #[swift_bridge(swift_name = "onMiniappOpened")]
        fn on_miniapp_opened(appid: &str, path: &str) -> i32;

        #[swift_bridge(swift_name = "getTabBarConfig")]
        fn get_tab_bar_config(appid: &str) -> Option<String>;

        #[swift_bridge(swift_name = "onScrollChanged")]
        fn on_scroll_changed(
            appid: &str,
            path: &str,
            scroll_x: i32,
            scroll_y: i32,
            max_scroll_x: i32,
            max_scroll_y: i32,
        ) -> i32;
    }

    extern "Swift" {
        // Resource access functions implemented in Swift
        #[swift_bridge(swift_name = "read_asset_data")]
        fn read_asset_data(path: &str) -> Vec<u8>;

        #[swift_bridge(swift_name = "list_asset_directory")]
        fn list_asset_directory(dir_path: &str) -> Vec<String>;
    }
}

// Re-export the bridge functions for use in other modules
pub use bridge::{list_asset_directory, read_asset_data};

/// Initialize the MiniApp system for iOS/macOS
pub fn miniapp_init(data_dir: &str, cache_dir: &str, app_delegate: usize) -> Option<String> {
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

    // Store app delegate globally as usize
    let _ = APP_DELEGATE.set(app_delegate);

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

/// Handle post message from WebView
pub fn handle_post_message(appid: &str, path: &str, message: &str) -> i32 {
    let miniapp = miniapp::get(appid.to_string());
    miniapp.handle_post_message(path.to_string(), message.to_string());
    0
}

/// Notify that a page has started loading
pub fn on_page_started(appid: &str, path: &str) -> i32 {
    let miniapp = miniapp::get(appid.to_string());
    miniapp.on_page_started(path.to_string());
    0
}

/// Notify that a page has finished loading
pub fn on_page_finished(appid: &str, path: &str) -> i32 {
    let miniapp = miniapp::get(appid.to_string());
    miniapp.on_page_finished(path.to_string());
    0
}

/// Notify that a page is being shown
pub fn on_page_show(appid: &str, path: &str) {
    let miniapp = miniapp::get(appid.to_string());
    miniapp.on_page_show(path.to_string());
}

/// Check if URL loading should be overridden
pub fn should_override_url_loading(appid: &str, url: &str) -> bool {
    let miniapp = miniapp::get(appid.to_string());
    miniapp.should_override_url_loading(url.to_string())
}

/// Handle HTTP request
pub fn handle_request(
    appid: &str,
    url: &str,
    method: &str,
    headers: &str,
    body: Vec<u8>,
) -> Option<String> {
    // Parse headers from JSON string
    let header_map: std::collections::HashMap<String, String> =
        serde_json::from_str(headers).unwrap_or_default();

    // Convert to HTTP types
    let http_method = match method {
        "GET" => http::Method::GET,
        "POST" => http::Method::POST,
        "PUT" => http::Method::PUT,
        "DELETE" => http::Method::DELETE,
        "PATCH" => http::Method::PATCH,
        "HEAD" => http::Method::HEAD,
        "OPTIONS" => http::Method::OPTIONS,
        _ => http::Method::GET,
    };

    let mut request_builder = http::Request::builder().method(http_method).uri(url);

    for (key, value) in header_map {
        request_builder = request_builder.header(&key, &value);
    }

    let request = match request_builder.body(body) {
        Ok(req) => req,
        Err(_) => return None,
    };

    let miniapp = miniapp::get(appid.to_string());
    match miniapp.handle_request(request) {
        Some(response) => {
            // Convert response to JSON string
            let response_data = serde_json::json!({
                "status": response.status().as_u16(),
                "headers": response.headers().iter().map(|(k, v)| {
                    (k.as_str(), v.to_str().unwrap_or(""))
                }).collect::<std::collections::HashMap<_, _>>(),
                "body": response.body()
            });

            Some(response_data.to_string())
        }
        None => None,
    }
}

/// Notify that MiniApp was closed
pub fn on_miniapp_closed(appid: &str) -> i32 {
    let miniapp = miniapp::get(appid.to_string());
    miniapp.on_miniapp_closed();
    0
}

/// Handle console message
pub fn console_message(appid: &str, path: &str, level: i32, message: &str) -> i32 {
    let log_level = match level {
        2 => LogLevel::Verbose, // VERBOSE
        3 => LogLevel::Debug,   // DEBUG
        4 => LogLevel::Info,    // INFO
        5 => LogLevel::Warn,    // WARN
        6 => LogLevel::Error,   // ERROR
        _ => LogLevel::Info,    // Default to INFO
    };

    let miniapp = miniapp::get(appid.to_string());
    miniapp.log(path, log_level, message);
    1
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

/// Handle scroll change event
pub fn on_scroll_changed(
    appid: &str,
    path: &str,
    scroll_x: i32,
    scroll_y: i32,
    max_scroll_x: i32,
    max_scroll_y: i32,
) -> i32 {
    let miniapp = miniapp::get(appid.to_string());
    miniapp.on_page_scroll_changed(
        path.to_string(),
        scroll_x,
        scroll_y,
        max_scroll_x,
        max_scroll_y,
    );
    0
}
