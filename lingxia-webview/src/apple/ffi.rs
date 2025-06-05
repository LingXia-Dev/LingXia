use super::app::App;
use crate::controller::Controller;
use miniapp::AppUiDelegate;
use miniapp::log::LogLevel;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::sync::{OnceLock, mpsc};

/// Global reference to the native app delegate for callbacks
/// Using usize to make it thread-safe
pub(crate) static APP_DELEGATE: OnceLock<usize> = OnceLock::new();

/// Initialize the MiniApp system for iOS/macOS
#[unsafe(no_mangle)]
pub extern "C" fn rust_miniapp_init(
    data_dir: *const c_char,
    cache_dir: *const c_char,
    app_delegate: *mut c_void,
) -> *mut c_char {
    oslog::OsLogger::new("com.lingxia.miniapp")
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
    let _ = APP_DELEGATE.set(app_delegate as usize);

    let data_dir_str = unsafe { CStr::from_ptr(data_dir).to_string_lossy().into_owned() };
    let cache_dir_str = unsafe { CStr::from_ptr(cache_dir).to_string_lossy().into_owned() };

    log::info!("Initializing MiniApp with data_dir: {}, cache_dir: {}", data_dir_str, cache_dir_str);

    let app = match App::new(data_dir_str, cache_dir_str) {
        Ok(app) => app,
        Err(e) => {
            log::error!("Failed to create App: {}", e);
            return std::ptr::null_mut();
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
        return std::ptr::null_mut();
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
            match CString::new(combined_details) {
                Ok(c_string) => c_string.into_raw(),
                Err(_) => std::ptr::null_mut(),
            }
        }
        None => {
            log::error!("Failed to obtain MiniApp home app details during initialization.");
            std::ptr::null_mut()
        }
    }
}

/// Notify that a WebView has been attached to the window
#[unsafe(no_mangle)]
pub extern "C" fn rust_webview_attached(appid: *const c_char, path: *const c_char) -> c_int {
    let appid = unsafe { CStr::from_ptr(appid).to_string_lossy().into_owned() };
    let path = unsafe { CStr::from_ptr(path).to_string_lossy().into_owned() };

    let miniapp = miniapp::get(appid);
    miniapp.on_page_created(path);
    0
}

/// Handle post message from WebView - improved interface
#[unsafe(no_mangle)]
pub extern "C" fn rust_handle_post_message(
    appid: *const c_char,
    path: *const c_char,
    message: *const c_char,
    message_len: usize,
) -> c_int {
    let appid = unsafe { CStr::from_ptr(appid).to_string_lossy().into_owned() };
    let path = unsafe { CStr::from_ptr(path).to_string_lossy().into_owned() };
    
    // Handle message with explicit length to support binary data
    let message = if message_len > 0 {
        let message_bytes = unsafe { std::slice::from_raw_parts(message as *const u8, message_len) };
        String::from_utf8_lossy(message_bytes).into_owned()
    } else {
        unsafe { CStr::from_ptr(message).to_string_lossy().into_owned() }
    };

    let miniapp = miniapp::get(appid);
    miniapp.handle_post_message(path, message);
    0
}

/// Notify that a page has started loading
#[unsafe(no_mangle)]
pub extern "C" fn rust_page_started(appid: *const c_char, path: *const c_char) -> c_int {
    let appid = unsafe { CStr::from_ptr(appid).to_string_lossy().into_owned() };
    let path = unsafe { CStr::from_ptr(path).to_string_lossy().into_owned() };

    let miniapp = miniapp::get(appid);
    miniapp.on_page_started(path);
    0
}

/// Notify that a page has finished loading
#[unsafe(no_mangle)]
pub extern "C" fn rust_page_finished(appid: *const c_char, path: *const c_char) -> c_int {
    let appid = unsafe { CStr::from_ptr(appid).to_string_lossy().into_owned() };
    let path = unsafe { CStr::from_ptr(path).to_string_lossy().into_owned() };

    let miniapp = miniapp::get(appid);
    miniapp.on_page_finished(path);
    0
}

/// Notify that a page is being shown
#[unsafe(no_mangle)]
pub extern "C" fn rust_page_show(appid: *const c_char, path: *const c_char) {
    let appid = unsafe { CStr::from_ptr(appid).to_string_lossy().into_owned() };
    let path = unsafe { CStr::from_ptr(path).to_string_lossy().into_owned() };

    let miniapp = miniapp::get(appid);
    miniapp.on_page_show(path);
}

/// Check if URL loading should be overridden - fixed to match Android interface
#[unsafe(no_mangle)]
pub extern "C" fn rust_should_override_url_loading(
    appid: *const c_char,
    url: *const c_char,
) -> c_int {
    let appid = unsafe { CStr::from_ptr(appid).to_string_lossy().into_owned() };
    let url = unsafe { CStr::from_ptr(url).to_string_lossy().into_owned() };

    let miniapp = miniapp::get(appid);
    if miniapp.should_override_url_loading(url) { 1 } else { 0 }
}

/// Find WebView instance - simplified implementation
#[unsafe(no_mangle)]
pub extern "C" fn rust_find_webview(appid: *const c_char, path: *const c_char) -> *mut c_void {
    let _appid = unsafe { CStr::from_ptr(appid).to_string_lossy().into_owned() };
    let _path = unsafe { CStr::from_ptr(path).to_string_lossy().into_owned() };

    // For now, return null as we don't have a webview registry in miniapp crate
    // This would need to be implemented based on the actual webview management system
    std::ptr::null_mut()
}

/// Handle HTTP request - improved with body support
#[unsafe(no_mangle)]
pub extern "C" fn rust_handle_request(
    appid: *const c_char,
    url: *const c_char,
    method: *const c_char,
    headers: *const c_char,
    body: *const u8,
    body_len: usize,
) -> *mut c_char {
    let appid = unsafe { CStr::from_ptr(appid).to_string_lossy().into_owned() };
    let url = unsafe { CStr::from_ptr(url).to_string_lossy().into_owned() };
    let method = unsafe { CStr::from_ptr(method).to_string_lossy().into_owned() };
    let headers = unsafe { CStr::from_ptr(headers).to_string_lossy().into_owned() };

    // Parse headers from JSON string
    let header_map: std::collections::HashMap<String, String> = 
        serde_json::from_str(&headers).unwrap_or_default();

    // Convert to HTTP types
    let http_method = match method.as_str() {
        "GET" => http::Method::GET,
        "POST" => http::Method::POST,
        "PUT" => http::Method::PUT,
        "DELETE" => http::Method::DELETE,
        "PATCH" => http::Method::PATCH,
        "HEAD" => http::Method::HEAD,
        "OPTIONS" => http::Method::OPTIONS,
        _ => http::Method::GET,
    };

    let mut request_builder = http::Request::builder().method(http_method).uri(&url);

    for (key, value) in header_map {
        request_builder = request_builder.header(&key, &value);
    }

    // Handle request body
    let request_body = if body_len > 0 && !body.is_null() {
        unsafe { std::slice::from_raw_parts(body, body_len).to_vec() }
    } else {
        Vec::new()
    };

    let request = match request_builder.body(request_body) {
        Ok(req) => req,
        Err(_) => return std::ptr::null_mut(),
    };

    let miniapp = miniapp::get(appid);
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

            match CString::new(response_data.to_string()) {
                Ok(c_string) => c_string.into_raw(),
                Err(_) => std::ptr::null_mut(),
            }
        }
        None => std::ptr::null_mut(),
    }
}

/// Notify that MiniApp was closed
#[unsafe(no_mangle)]
pub extern "C" fn rust_miniapp_closed(appid: *const c_char) -> c_int {
    let appid = unsafe { CStr::from_ptr(appid).to_string_lossy().into_owned() };
    
    let miniapp = miniapp::get(appid);
    miniapp.on_miniapp_closed();
    0
}

/// Handle console message - fixed to match Android implementation
#[unsafe(no_mangle)]
pub extern "C" fn rust_console_message(
    appid: *const c_char,
    path: *const c_char,
    level: c_int,
    message: *const c_char,
) -> c_int {
    let appid = unsafe { CStr::from_ptr(appid).to_string_lossy().into_owned() };
    let path = unsafe { CStr::from_ptr(path).to_string_lossy().into_owned() };
    let message = unsafe { CStr::from_ptr(message).to_string_lossy().into_owned() };

    let log_level = match level {
        2 => LogLevel::Verbose, // VERBOSE
        3 => LogLevel::Debug,   // DEBUG
        4 => LogLevel::Info,    // INFO
        5 => LogLevel::Warn,    // WARN
        6 => LogLevel::Error,   // ERROR
        _ => LogLevel::Info,    // Default to INFO
    };

    let miniapp = miniapp::get(appid);
    miniapp.log(&path, log_level, &message);
    1
}

/// Get page configuration
#[unsafe(no_mangle)]
pub extern "C" fn rust_get_page_config(appid: *const c_char, path: *const c_char) -> *mut c_char {
    let appid = unsafe { CStr::from_ptr(appid).to_string_lossy().into_owned() };
    let path = unsafe { CStr::from_ptr(path).to_string_lossy().into_owned() };

    let miniapp = miniapp::get(appid);
    match miniapp.get_page_config(&path) {
        Ok(config) => {
            match CString::new(config) {
                Ok(c_string) => c_string.into_raw(),
                Err(_) => std::ptr::null_mut(),
            }
        }
        Err(_) => std::ptr::null_mut(),
    }
}

/// Handle back button press
#[unsafe(no_mangle)]
pub extern "C" fn rust_back_pressed(appid: *const c_char) -> c_int {
    let appid = unsafe { CStr::from_ptr(appid).to_string_lossy().into_owned() };
    
    let miniapp = miniapp::get(appid);
    if miniapp.on_back_pressed() { 1 } else { 0 }
}

/// Notify that MiniApp was opened
#[unsafe(no_mangle)]
pub extern "C" fn rust_miniapp_opened(appid: *const c_char, path: *const c_char) -> c_int {
    let appid = unsafe { CStr::from_ptr(appid).to_string_lossy().into_owned() };
    let path = unsafe { CStr::from_ptr(path).to_string_lossy().into_owned() };

    let miniapp = miniapp::get(appid);
    miniapp.on_miniapp_opened(path);
    0
}

/// Get tab bar configuration
#[unsafe(no_mangle)]
pub extern "C" fn rust_get_tab_bar_config(appid: *const c_char) -> *mut c_char {
    let appid = unsafe { CStr::from_ptr(appid).to_string_lossy().into_owned() };

    let miniapp = miniapp::get(appid);
    match miniapp.get_tab_bar_config() {
        Ok(config) => {
            match CString::new(config) {
                Ok(c_string) => c_string.into_raw(),
                Err(_) => std::ptr::null_mut(),
            }
        }
        Err(_) => std::ptr::null_mut(),
    }
}

/// Handle scroll change event
#[unsafe(no_mangle)]
pub extern "C" fn rust_scroll_changed(
    appid: *const c_char,
    path: *const c_char,
    scroll_x: c_int,
    scroll_y: c_int,
    max_scroll_x: c_int,
    max_scroll_y: c_int,
) -> c_int {
    let appid = unsafe { CStr::from_ptr(appid).to_string_lossy().into_owned() };
    let path = unsafe { CStr::from_ptr(path).to_string_lossy().into_owned() };

    let miniapp = miniapp::get(appid);
    miniapp.on_page_scroll_changed(
        path,
        scroll_x as i32,
        scroll_y as i32,
        max_scroll_x as i32,
        max_scroll_y as i32,
    );
    0
}

/// Free a C string allocated by Rust
#[unsafe(no_mangle)]
pub extern "C" fn rust_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}
