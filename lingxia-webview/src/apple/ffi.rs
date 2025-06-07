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
    // High-efficiency HTTP request/response structures
    #[swift_bridge(swift_repr = "struct")]
    struct HttpRequest {
        url: String,
        method: String,
        header_keys: Vec<String>,
        header_values: Vec<String>,
        body: Vec<u8>,
    }

    #[swift_bridge(swift_repr = "struct")]
    struct HttpResponse {
        status_code: u16,
        header_keys: Vec<String>,
        header_values: Vec<String>,
        body: Vec<u8>,
        mime_type: String,
    }

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
        fn handle_request(appid: &str, request: HttpRequest) -> Option<HttpResponse>;

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
        #[swift_bridge(swift_name = "readAssetData")]
        fn read_asset_data(path: &str) -> Vec<u8>;

        #[swift_bridge(swift_name = "listAssetDirectory")]
        fn list_asset_directory(dir_path: &str) -> Vec<String>;
    }
}

// Re-export the bridge functions for use in other modules
pub use bridge::{HttpRequest, HttpResponse, list_asset_directory, read_asset_data};

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

/// Handle HTTP request using high-efficiency swift-bridge types
pub fn handle_request(appid: &str, request: HttpRequest) -> Option<HttpResponse> {
    // Build HTTP request directly from swift-bridge struct
    let mut request_builder = http::Request::builder()
        .method(request.method.as_str())
        .uri(&request.url);

    // Add headers efficiently using parallel arrays
    for (key, value) in request.header_keys.iter().zip(request.header_values.iter()) {
        if let (Ok(name), Ok(val)) = (
            http::HeaderName::from_bytes(key.as_bytes()),
            http::HeaderValue::from_str(value),
        ) {
            request_builder = request_builder.header(name, val);
        }
    }

    let http_request = match request_builder.body(request.body) {
        Ok(req) => req,
        Err(_) => return None,
    };

    // Call existing handle_request infrastructure
    let miniapp = miniapp::get(appid.to_string());
    match miniapp.handle_request(http_request) {
        Some(response) => {
            // Convert response headers efficiently using parallel arrays
            let mut header_keys = Vec::new();
            let mut header_values = Vec::new();
            for (key, value) in response.headers().iter() {
                if let Ok(value_str) = value.to_str() {
                    header_keys.push(key.as_str().to_string());
                    header_values.push(value_str.to_string());
                }
            }

            // Determine MIME type from Content-Type header
            let mime_type = response
                .headers()
                .get(http::header::CONTENT_TYPE)
                .and_then(|h| h.to_str().ok())
                .map(|content_type| {
                    content_type
                        .split(';')
                        .next()
                        .unwrap_or("application/octet-stream")
                        .trim()
                })
                .unwrap_or("application/octet-stream");

            // Return high-efficiency response struct
            Some(HttpResponse {
                status_code: response.status().as_u16(),
                header_keys,
                header_values,
                body: response.body().clone(),
                mime_type: mime_type.to_string(),
            })
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
