use super::app::App;
use crate::runtime::SimpleAppRuntime;
use android_logger::Config;
use http;
use http::header::{HeaderMap, HeaderName, HeaderValue};
use http::{Method, Request, Response};
use jni::objects::{GlobalRef, JClass, JObject, JString};
use jni::sys::{jboolean, jint};
use jni::{JNIEnv, JavaVM};
use log::{error, info};
use miniapp::AppUiDelegate;
use miniapp::log::LogLevel;
use serde_json;
use std::sync::{Arc, OnceLock};

pub static JAVA_VM: OnceLock<Arc<JavaVM>> = OnceLock::new();
static MAIN_THREAD_ID: OnceLock<std::thread::ThreadId> = OnceLock::new();

/// Global reference to LxApp class for worker threads
pub(crate) static LXAPP_CLASS: OnceLock<GlobalRef> = OnceLock::new();

#[unsafe(no_mangle)]
pub extern "system" fn JNI_OnLoad(vm: JavaVM, _: *mut std::os::raw::c_void) -> jint {
    android_logger::init_once(
        Config::default()
            .with_max_level(log::LevelFilter::Debug)
            .with_tag("Rust"),
    );

    // Initialize the new logging system
    miniapp::log::LogManager::init(|log_message| {
        let formatted_message = format!(
            "[{}{}{}] {}",
            log_message.tag.as_str(),
            log_message
                .appid
                .as_ref()
                .map(|id| format!(":{}", id))
                .unwrap_or_default(),
            log_message
                .path
                .as_ref()
                .map(|p| format!(":{}", p))
                .unwrap_or_default(),
            log_message.message
        );

        match log_message.level {
            LogLevel::Verbose => log::trace!("{}", formatted_message),
            LogLevel::Debug => log::debug!("{}", formatted_message),
            LogLevel::Info => log::info!("{}", formatted_message),
            LogLevel::Warn => log::warn!("{}", formatted_message),
            LogLevel::Error => log::error!("{}", formatted_message),
        }
    });

    // Store JavaVM globally
    let _ = JAVA_VM.set(Arc::new(vm));

    // Store the main thread ID
    let _ = MAIN_THREAD_ID.set(std::thread::current().id());

    // Create global reference to LxApp class for worker threads
    if let Some(jvm) = JAVA_VM.get() {
        if let Ok(mut env) = jvm.attach_current_thread() {
            if let Ok(local_class) = env.find_class("com/lingxia/lxapp/LxApp") {
                if let Ok(global_class) = env.new_global_ref(local_class) {
                    let _ = LXAPP_CLASS.set(global_class);
                }
            }
        }
    }

    info!("Rust library loaded successfully");
    jni::sys::JNI_VERSION_1_6
}

// Helper function to get JNIEnv for current thread
pub(crate) fn get_env() -> Result<JNIEnv<'static>, Box<dyn std::error::Error>> {
    let vm = JAVA_VM.get().ok_or("JavaVM not initialized")?;

    // Check if we're on the main thread
    let current_thread = std::thread::current().id();
    let is_main_thread = MAIN_THREAD_ID
        .get()
        .map(|main_id| *main_id == current_thread)
        .unwrap_or(false);

    if is_main_thread {
        // If we're on the main thread, get the env
        match vm.get_env() {
            Ok(env) => unsafe {
                JNIEnv::from_raw(env.get_raw()).map_err(|e| {
                    error!("JNI error: {:?}", e);
                    e.into()
                })
            },
            Err(e) => {
                error!("Failed to get JNI env for main thread: {:?}", e);
                Err(e.into())
            }
        }
    } else {
        // If we're not on the main thread, attach as daemon to avoid lifecycle issues
        match vm.attach_current_thread_as_daemon() {
            Ok(env) => Ok(env),
            Err(e) => {
                error!("Failed to attach thread as daemon: {:?}", e);
                Err(e.into())
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_onLxAppInited(
    mut env: JNIEnv,
    _class: JClass,
    data_dir: JString,
    cache_dir: JString,
    asset_manager: JObject,
) -> jni::sys::jstring {
    let data_dir = env.get_string(&data_dir).unwrap().into();
    let cache_dir = env.get_string(&cache_dir).unwrap().into();

    log::info!(
        "Initializing LxApp with data_dir: {}, cache_dir: {}",
        data_dir,
        cache_dir,
    );

    let app = match App::from_java(&mut env, asset_manager.as_raw(), data_dir, cache_dir) {
        Ok(app) => app,
        Err(_) => {
            return JObject::null().into_raw();
        }
    };

    // Initialize SimpleAppRuntime and miniapp
    let runtime = SimpleAppRuntime::init(app);
    let home_app_id = miniapp::init(runtime);

    // Return the home appid
    match home_app_id {
        Some(appid) => match env.new_string(&appid) {
            Ok(java_string) => java_string.into_raw(),
            Err(_) => JObject::null().into_raw(),
        },
        None => {
            error!("Failed to obtain LxApp home app details during initialization.");
            JObject::null().into_raw()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaWebView_handlePostMessage(
    mut env: JNIEnv,
    _this: JObject,
    appid: JString,
    path: JString,
    message: JString,
) -> jint {
    let appid: String = env.get_string(&appid).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();
    let message: String = env.get_string(&message).unwrap().into();

    let miniapp = miniapp::get(appid.clone());
    miniapp.handle_post_message(path, message);
    0
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaWebView_onPageStarted(
    mut env: JNIEnv,
    _this: JObject,
    appid: JString,
    path: JString,
) -> jint {
    let appid: String = env.get_string(&appid).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();

    let miniapp = miniapp::get(appid);
    miniapp.on_page_started(path);
    0
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaWebView_onPageFinished(
    mut env: JNIEnv,
    _this: JObject,
    appid: JString,
    path: JString,
) -> jint {
    let appid: String = env.get_string(&appid).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();

    let miniapp = miniapp::get(appid);
    miniapp.on_page_finished(path);
    0
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_onPageShow(
    mut env: JNIEnv,
    _class: JClass,
    appid: JString,
    path: JString,
) {
    let appid: String = env.get_string(&appid).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();

    let miniapp = miniapp::get(appid);
    miniapp.on_page_show(path);
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_findWebView<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
    path: JString<'a>,
) -> JObject<'a> {
    let appid: String = env.get_string(&appid).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();

    // Get the runtime and try to find the WebView
    if let Some(runtime) = SimpleAppRuntime::get() {
        if let Some(webview) = runtime.get_webview(&appid, &path) {
            // Get direct access to the WebView and create a new local reference to the Java WebView object
            match env.new_local_ref(webview.get_java_webview()) {
                Ok(local_ref) => unsafe { JObject::from_raw(local_ref.into_raw()) },
                Err(e) => {
                    error!("Failed to create local reference to WebView: {:?}", e);
                    JObject::null()
                }
            }
        } else {
            // No WebView found for this appid/path
            error!("💥 Not found webview for {}-{}", appid, path);
            JObject::null()
        }
    } else {
        error!("Runtime not initialized");
        JObject::null()
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaWebView_handleRequest<'a>(
    mut env: JNIEnv<'a>,
    _this: JObject<'a>,
    appid: JString<'a>,
    url: JString<'a>,
    method: JString<'a>,
    headers: JString<'a>,
) -> JObject<'a> {
    // Convert Java strings to Rust strings
    let appid: String = env.get_string(&appid).unwrap().into();
    let url_str: String = env.get_string(&url).unwrap().into();
    let method_str: String = env.get_string(&method).unwrap().into();
    let headers_str: String = env.get_string(&headers).unwrap().into();

    // Parse headers JSON
    let headers_map: serde_json::Map<String, serde_json::Value> =
        match serde_json::from_str(&headers_str) {
            Ok(map) => map,
            Err(_) => return JObject::null(),
        };

    // Build headers
    let mut http_headers = HeaderMap::new();
    for (key, value) in headers_map {
        if let Some(value_str) = value.as_str() {
            if let (Ok(name), Ok(val)) = (
                HeaderName::from_bytes(key.as_bytes()),
                HeaderValue::from_str(value_str),
            ) {
                http_headers.insert(name, val);
            }
        }
    }

    // Parse HTTP method with fallback to GET
    let http_method = method_str.parse::<Method>().unwrap_or(Method::GET);

    // Build request with proper error handling
    let request = match Request::builder()
        .method(http_method)
        .uri(url_str)
        .body(Vec::new())
    {
        Ok(mut req) => {
            *req.headers_mut() = http_headers;
            req
        }
        Err(_) => return JObject::null(),
    };

    // Handle request and convert response
    let miniapp = miniapp::get(appid.clone());
    if let Some(response) = miniapp.handle_request(request) {
        create_java_response(&mut env, response)
    } else {
        JObject::null()
    }
}

fn create_java_response<'a>(env: &mut JNIEnv<'a>, response: Response<Vec<u8>>) -> JObject<'a> {
    // Try to find the WebResourceResponseData inner class with package
    let response_class =
        match env.find_class("com/lingxia/webview/LingXiaWebView$WebResourceResponseData") {
            Ok(c) => c,
            Err(_) => return JObject::null(),
        };

    // Extract response components
    let status = response.status().as_u16() as i32;
    let reason = response.status().canonical_reason().unwrap_or("Unknown");
    let headers = response.headers();
    let body = response.body();

    // Get content type and parse it
    let (mime_type, encoding) = headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .map(|content_type| {
            let parts: Vec<&str> = content_type.split(';').map(str::trim).collect();
            let mime = parts[0];
            let enc = parts
                .iter()
                .find(|p| p.starts_with("charset="))
                .map(|p| p.trim_start_matches("charset="))
                .unwrap_or("UTF-8");
            (mime, enc)
        })
        .unwrap_or(("application/octet-stream", "UTF-8"));

    // Create HashMap for headers
    let map = match env.new_object("java/util/HashMap", "()V", &[]) {
        Ok(map) => map,
        Err(_) => return JObject::null(),
    };

    // Convert headers to Java HashMap
    for (key, value) in headers.iter() {
        if let Ok(v) = value.to_str() {
            let key_str = env.new_string(key.as_str());
            let value_str = env.new_string(v);

            if let (Ok(k), Ok(v)) = (key_str, value_str) {
                let _ = env.call_method(
                    &map,
                    "put",
                    "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                    &[(&k).into(), (&v).into()],
                );
            }
        }
    }

    // Create Java strings and byte array
    let mime_type_str = match env.new_string(mime_type) {
        Ok(s) => s,
        Err(_) => return JObject::null(),
    };
    let encoding_str = match env.new_string(encoding) {
        Ok(s) => s,
        Err(_) => return JObject::null(),
    };
    let reason_str = match env.new_string(reason) {
        Ok(s) => s,
        Err(_) => return JObject::null(),
    };
    let byte_array = match env.byte_array_from_slice(body) {
        Ok(arr) => arr,
        Err(_) => return JObject::null(),
    };

    // Create the WebResourceResponseData object
    match env.new_object(
        response_class,
        "(Ljava/lang/String;Ljava/lang/String;ILjava/lang/String;Ljava/util/Map;[B)V",
        &[
            (&mime_type_str).into(),
            (&encoding_str).into(),
            status.into(),
            (&reason_str).into(),
            (&map).into(),
            (&byte_array).into(),
        ],
    ) {
        Ok(obj) => obj,
        Err(_) => JObject::null(),
    }
}

// Function for LxAppActivity class to handle the mini app close event
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_onLxAppClosed(
    mut env: JNIEnv,
    _class: JClass,
    appid: JString,
) -> jint {
    let appid: String = env.get_string(&appid).unwrap().into();

    let miniapp = miniapp::get(appid.clone());
    miniapp.on_lxapp_closed();
    0
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaWebView_onConsoleMessage(
    mut env: JNIEnv,
    _this: JObject,
    appid: JString,
    path: JString,
    level: jint,
    message: JString,
) -> jint {
    let appid: String = env.get_string(&appid).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();
    let message: String = env.get_string(&message).unwrap().into();

    let miniapp = miniapp::get(appid.clone());
    let log_level = match level {
        2 => LogLevel::Verbose, // VERBOSE
        3 => LogLevel::Debug,   // DEBUG
        4 => LogLevel::Info,    // INFO
        5 => LogLevel::Warn,    // WARN
        6 => LogLevel::Error,   // ERROR
        _ => LogLevel::Info,    // Default to INFO
    };

    miniapp.log(&path, log_level, &message);
    1
}

/// Get navigation bar configuration for a specific page
///
/// IMPORTANT: This function returns NavigationBarConfig directly to Kotlin.
/// Kotlin side should handle visibility logic based on:
/// - navigationStyle: 0=Default (show), 1=Custom (hide)
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_getNavigationBarConfig<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
    path: JString<'a>,
) -> JObject<'a> {
    let appid: String = env.get_string(&appid).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();

    // Get the miniapp instance
    let miniapp = miniapp::get(appid.clone());

    // Get navigation bar config using new API
    let nav_config = miniapp.get_config().get_nav_bar_config(&miniapp, &path);

    // Debug logging
    log::info!(
        "[Android] nativeGetPageConfig: appid={}, path={}, navigationStyle={:?}, bgColor={}, textStyle={}, title={}",
        appid,
        path,
        nav_config.navigationStyle,
        nav_config.navigationBarBackgroundColor,
        nav_config.navigationBarTextStyle,
        nav_config.navigationBarTitleText
    );

    // Find the NavigationBarConfig class
    let nav_bar_class = match env.find_class("com/lingxia/lxapp/NavigationBarConfig") {
        Ok(c) => c,
        Err(_) => return JObject::null(),
    };

    // Parse color values from hex strings
    let bg_color_int = i32::from_str_radix(
        &nav_config
            .navigationBarBackgroundColor
            .trim_start_matches('#'),
        16,
    )
    .unwrap_or(0xFFFFFF)
        | 0xFF000000u32 as i32; // Add alpha channel for Android

    log::info!(
        "[Android] Color parsing: original={}, parsed=0x{:08X}",
        nav_config.navigationBarBackgroundColor,
        bg_color_int as u32
    );

    // Create Java strings
    let title_text = match env.new_string(&nav_config.navigationBarTitleText) {
        Ok(s) => s,
        Err(_) => return JObject::null(),
    };
    let text_style = match env.new_string(&nav_config.navigationBarTextStyle) {
        Ok(s) => s,
        Err(_) => return JObject::null(),
    };
    // Use int for navigation style (0=Default, 1=Custom)
    let navigation_style_int = nav_config.navigationStyle.to_i32();

    // Create NavigationBarConfig object (using int for navigation style)
    match env.new_object(
        nav_bar_class,
        "(ILjava/lang/String;Ljava/lang/String;I)V",
        &[
            (bg_color_int as jint).into(),
            (&text_style).into(),
            (&title_text).into(),
            (navigation_style_int as jint).into(),
        ],
    ) {
        Ok(obj) => obj,
        Err(_) => JObject::null(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_com_lingxia_lxapp_NativeApi_onBackPressed(
    mut env: JNIEnv,
    _class: JClass,
    appid: JString,
) -> jint {
    let appid: String = env.get_string(&appid).unwrap().into();
    let miniapp = miniapp::get(appid);
    if miniapp.on_back_pressed() { 1 } else { 0 }
}

// Function to notify the Rust layer that a mini app has been opened
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_onLxAppOpened(
    mut env: JNIEnv,
    _class: JClass,
    appid: JString,
    path: JString,
) -> jint {
    let appid: String = env.get_string(&appid).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();

    let miniapp = miniapp::get(appid.clone());
    miniapp.on_lxapp_opened(path);
    0
}

/// Get LxApp information using new typed API
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_getLxAppInfo<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
) -> JObject<'a> {
    let appid: String = env.get_string(&appid).unwrap().into();
    let miniapp = miniapp::get(appid.clone());

    let lxapp_info = miniapp.get_config().get_lxapp_info();

    // Debug logging
    log::info!(
        "[Android] nativeGetLxAppInfo: appid={}, initial_route={}, app_name={}, debug={}",
        appid,
        lxapp_info.initial_route,
        lxapp_info.app_name,
        lxapp_info.debug
    );

    // Find the LxAppInfo class
    let lxapp_info_class = match env.find_class("com/lingxia/lxapp/LxAppInfo") {
        Ok(c) => c,
        Err(_) => return JObject::null(),
    };

    // Create Java strings
    let initial_route_str = match env.new_string(&lxapp_info.initial_route) {
        Ok(s) => s,
        Err(_) => return JObject::null(),
    };
    let app_name_str = match env.new_string(&lxapp_info.app_name) {
        Ok(s) => s,
        Err(_) => return JObject::null(),
    };

    // Create LxAppInfo object
    match env.new_object(
        lxapp_info_class,
        "(Ljava/lang/String;Ljava/lang/String;Z)V",
        &[
            (&initial_route_str).into(),
            (&app_name_str).into(),
            (lxapp_info.debug as jboolean).into(),
        ],
    ) {
        Ok(obj) => obj,
        Err(_) => JObject::null(),
    }
}

// Get TabBar configuration using new typed API
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_getTabBarConfig<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
) -> JObject<'a> {
    let appid: String = env.get_string(&appid).unwrap().into();
    let miniapp = miniapp::get(appid.clone());

    let tab_bar_config = match miniapp.get_config().get_tab_bar_config(&miniapp) {
        Some(config) => config,
        None => {
            log::info!(
                "[Android] nativeGetTabBarConfig: No TabBar config found for appid={}",
                appid
            );
            return JObject::null();
        }
    };

    // Debug logging
    log::info!(
        "[Android] nativeGetTabBarConfig: appid={}, items_count={}",
        appid,
        tab_bar_config.list.len()
    );

    // Find the TabBarConfig class
    let tab_bar_class = match env.find_class("com/lingxia/lxapp/TabBarConfig") {
        Ok(c) => c,
        Err(_) => return JObject::null(),
    };

    // Convert background color
    let background_color = match tab_bar_config.backgroundColor.as_str() {
        "transparent" => 0x00000000i32, // Transparent
        color_str => {
            if color_str.starts_with('#') && color_str.len() == 7 {
                i32::from_str_radix(&color_str[1..], 16).unwrap_or(0xFFFFFFFFu32 as i32)
            } else {
                0xFFFFFFFFu32 as i32 // Default white
            }
        }
    };

    // Convert selected color
    let selected_color = match tab_bar_config.selectedColor.as_str() {
        color_str if color_str.starts_with('#') && color_str.len() == 7 => {
            i32::from_str_radix(&color_str[1..], 16).unwrap_or(0xFF1677FFu32 as i32)
        }
        _ => 0xFF1677FFu32 as i32, // Default blue
    };

    // Convert unselected color
    let color = match tab_bar_config.color.as_str() {
        color_str if color_str.starts_with('#') && color_str.len() == 7 => {
            i32::from_str_radix(&color_str[1..], 16).unwrap_or(0xFF666666u32 as i32)
        }
        _ => 0xFF666666u32 as i32, // Default gray
    };

    // Convert border style (color)
    let border_style = match tab_bar_config.borderStyle.as_str() {
        color_str if color_str.starts_with('#') && color_str.len() == 7 => {
            i32::from_str_radix(&color_str[1..], 16).unwrap_or(0xFFF0F0F0u32 as i32)
        }
        _ => 0xFFF0F0F0u32 as i32, // Default light gray
    };

    // Convert dimension (height for top/bottom, width for left/right)
    let dimension = tab_bar_config.dimension;

    // Use int for position (0=Bottom, 1=Top, 2=Left, 3=Right)
    let position_int = tab_bar_config.position.to_i32();

    // Create TabBarItem list
    let array_list_class = match env.find_class("java/util/ArrayList") {
        Ok(c) => c,
        Err(_) => return JObject::null(),
    };

    let tab_items_list = match env.new_object(array_list_class, "()V", &[]) {
        Ok(list) => list,
        Err(_) => return JObject::null(),
    };

    // Add TabBarItems to the list
    for item in &tab_bar_config.list {
        if let Some(tab_item) = create_tab_bar_item(&mut env, item) {
            let _ = env.call_method(
                &tab_items_list,
                "add",
                "(Ljava/lang/Object;)Z",
                &[(&tab_item).into()],
            );
        }
    }

    // Create Integer objects for nullable fields
    let bg_color_obj = env
        .new_object("java/lang/Integer", "(I)V", &[background_color.into()])
        .unwrap_or(JObject::null());
    let selected_color_obj = env
        .new_object("java/lang/Integer", "(I)V", &[selected_color.into()])
        .unwrap_or(JObject::null());
    let color_obj = env
        .new_object("java/lang/Integer", "(I)V", &[color.into()])
        .unwrap_or(JObject::null());
    let border_style_obj = env
        .new_object("java/lang/Integer", "(I)V", &[border_style.into()])
        .unwrap_or(JObject::null());
    let dimension_obj = env
        .new_object("java/lang/Integer", "(I)V", &[dimension.into()])
        .unwrap_or(JObject::null());

    // Create Position enum
    let position_class = match env.find_class("com/lingxia/lxapp/TabBarConfig$Position") {
        Ok(c) => c,
        Err(_) => return JObject::null(),
    };

    let position_enum_value = match position_int {
        1 => "TOP",
        2 => "LEFT",
        3 => "RIGHT",
        _ => "BOTTOM", // default
    };

    let position_enum = match env.get_static_field(
        position_class,
        position_enum_value,
        "Lcom/lingxia/lxapp/TabBarConfig$Position;",
    ) {
        Ok(pos) => pos,
        Err(_) => return JObject::null(),
    };

    // Create TabBarConfig object (using Position enum for backward compatibility)
    match env.new_object(
        tab_bar_class,
        "(Ljava/lang/Integer;Ljava/lang/Integer;Ljava/lang/Integer;Ljava/lang/Integer;Ljava/lang/Integer;Lcom/lingxia/lxapp/TabBarConfig$Position;Ljava/util/List;Z)V",
        &[
            (&bg_color_obj).into(),
            (&selected_color_obj).into(),
            (&color_obj).into(),
            (&border_style_obj).into(),
            (&dimension_obj).into(),
            (&position_enum).into(),
            (&tab_items_list).into(),
            true.into(), // visible
        ],
    ) {
        Ok(obj) => obj,
        Err(_) => JObject::null(),
    }
}

// Helper function to create TabBarItem
fn create_tab_bar_item<'a>(
    env: &mut JNIEnv<'a>,
    item: &miniapp::config::TabItem,
) -> Option<JObject<'a>> {
    let tab_bar_item_class = env.find_class("com/lingxia/lxapp/TabBarItem").ok()?;

    let page_path = env.new_string(&item.pagePath).ok()?;
    let text = if let Some(ref text_str) = item.text {
        env.new_string(text_str).ok()
    } else {
        None
    };
    let icon_path = if let Some(ref icon_str) = item.iconPath {
        env.new_string(icon_str).ok()
    } else {
        env.new_string("").ok()
    }?;
    let selected_icon_path = if let Some(ref selected_icon_str) = item.selectedIconPath {
        env.new_string(selected_icon_str).ok()
    } else {
        env.new_string("").ok()
    }?;

    let text_obj = text.map(|t| t.into()).unwrap_or_else(|| JObject::null());

    env.new_object(
        tab_bar_item_class,
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ZZ)V",
        &[
            (&page_path).into(),
            (&text_obj).into(),
            (&icon_path).into(),
            (&selected_icon_path).into(),
            item.selected.into(),
            true.into(), // visible - TabItem doesn't have visible field, default to true
        ],
    )
    .ok()
}

/// Get TabBar item by index
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_lxapp_NativeApi_getTabBarItem<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
    index: jint,
) -> JObject<'a> {
    let appid: String = env.get_string(&appid).unwrap().into();
    let miniapp = miniapp::get(appid.clone());

    let tab_bar_config = match miniapp.get_config().get_tab_bar_config(&miniapp) {
        Some(config) => config,
        None => {
            log::info!(
                "[Android] nativeGetTabBarItem: No TabBar config found for appid={}",
                appid
            );
            return JObject::null();
        }
    };

    let item = match tab_bar_config.list.get(index as usize) {
        Some(item) => item,
        None => {
            log::info!(
                "[Android] nativeGetTabBarItem: Index {} out of bounds for appid={}",
                index,
                appid
            );
            return JObject::null();
        }
    };

    // Debug logging
    log::info!(
        "[Android] nativeGetTabBarItem: appid={}, index={}, page_path={}",
        appid,
        index,
        item.pagePath
    );

    create_tab_bar_item(&mut env, item).unwrap_or(JObject::null())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaWebView_onScrollChanged(
    mut env: JNIEnv,
    _this: JObject,
    appid: JString,
    path: JString,
    scroll_x: jint,
    scroll_y: jint,
    max_scroll_x: jint,
    max_scroll_y: jint,
) -> jint {
    let appid: String = env.get_string(&appid).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();

    let miniapp = miniapp::get(appid.clone());
    miniapp.on_page_scroll_changed(path, scroll_x, scroll_y, max_scroll_x, max_scroll_y);
    0
}
