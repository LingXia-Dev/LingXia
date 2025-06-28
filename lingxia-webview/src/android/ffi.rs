use super::app::App;
use crate::runtime::SimpleAppRuntime;
use android_logger::Config;
use http;
use http::header::{HeaderMap, HeaderName, HeaderValue};
use http::{Method, Request, Response};
use jni::objects::{GlobalRef, JClass, JObject, JString};
use jni::sys::jint;
use jni::{JNIEnv, JavaVM};
use log::{error, info};
use miniapp::AppUiDelegate;
use miniapp::log::LogLevel;
use serde_json;
use std::sync::{Arc, OnceLock};

pub static JAVA_VM: OnceLock<Arc<JavaVM>> = OnceLock::new();
static MAIN_THREAD_ID: OnceLock<std::thread::ThreadId> = OnceLock::new();

/// Global reference to MiniApp class for worker threads
pub(crate) static MINIAPP_CLASS: OnceLock<GlobalRef> = OnceLock::new();

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

    // Create global reference to MiniApp class for worker threads
    if let Some(jvm) = JAVA_VM.get() {
        if let Ok(mut env) = jvm.attach_current_thread() {
            if let Ok(local_class) = env.find_class("com/lingxia/miniapp/MiniApp") {
                if let Ok(global_class) = env.new_global_ref(local_class) {
                    let _ = MINIAPP_CLASS.set(global_class);
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
pub extern "system" fn Java_com_lingxia_miniapp_MiniApp_nativeOnMiniAppInited(
    mut env: JNIEnv,
    _class: JClass,
    data_dir: JString,
    cache_dir: JString,
    asset_manager: JObject,
) -> jni::sys::jstring {
    let data_dir = env.get_string(&data_dir).unwrap().into();
    let cache_dir = env.get_string(&cache_dir).unwrap().into();

    log::info!(
        "Initializing MiniApp with data_dir: {}, cache_dir: {}",
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
    let final_init_details = miniapp::init(runtime);

    // Format and return the result
    match final_init_details {
        Some((home_app_id, initial_route)) => {
            let combined_details = format!("{}:{}", home_app_id, initial_route);
            match env.new_string(&combined_details) {
                Ok(java_string) => java_string.into_raw(),
                Err(_) => JObject::null().into_raw(),
            }
        }
        None => {
            error!("Failed to obtain MiniApp home app details during initialization.");
            JObject::null().into_raw()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeHandlePostMessage(
    mut env: JNIEnv,
    _class: JClass,
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
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeOnPageStarted(
    mut env: JNIEnv,
    _class: JClass,
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
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeOnPageFinished(
    mut env: JNIEnv,
    _class: JClass,
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
pub extern "system" fn Java_com_lingxia_miniapp_MiniAppActivity_nativeOnPageShow(
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
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeShouldOverrideUrlLoading(
    mut env: JNIEnv,
    _class: JClass,
    appid: JString,
    url: JString,
) -> jint {
    let appid: String = env.get_string(&appid).unwrap().into();
    let url: String = env.get_string(&url).unwrap().into();

    // Get the miniapp instance and check if we should override the URL
    let miniapp = miniapp::get(appid.clone());
    if miniapp.should_override_url_loading(url) {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeFindWebView<'a>(
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
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeHandleRequest<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
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
    // Try to find the WebResourceResponseData class
    let response_class = match env.find_class("com/lingxia/miniapp/WebResourceResponseData") {
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

// Function for MiniAppActivity class to handle the mini app close event
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_miniapp_MiniAppActivity_nativeOnMiniAppClosed(
    mut env: JNIEnv,
    _class: JClass,
    appid: JString,
) -> jint {
    let appid: String = env.get_string(&appid).unwrap().into();

    let miniapp = miniapp::get(appid.clone());
    miniapp.on_miniapp_closed();
    0
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeOnConsoleMessage(
    mut env: JNIEnv,
    _class: JClass,
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

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_miniapp_MiniApp_nativeGetPageConfig<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    appid: JString<'a>,
    path: JString<'a>,
) -> JObject<'a> {
    let appid: String = env.get_string(&appid).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();

    // Get the miniapp instance and get page config
    let miniapp = miniapp::get(appid);
    if let Ok(json) = miniapp.get_page_config(&path) {
        // Create Java string from JSON
        if let Ok(java_string) = env.new_string(&json) {
            return java_string.into();
        }
    }
    JObject::null()
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_com_lingxia_miniapp_MiniAppActivity_nativeOnBackPressed(
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
pub extern "system" fn Java_com_lingxia_miniapp_MiniApp_nativeOnMiniAppOpened(
    mut env: JNIEnv,
    _class: JClass,
    appid: JString,
    path: JString,
) -> jint {
    let appid: String = env.get_string(&appid).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();

    let miniapp = miniapp::get(appid.clone());
    miniapp.on_miniapp_opened(path);
    0
}

// New function to get app configuration
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_miniapp_MiniApp_nativeGetTabBarConfig(
    mut env: JNIEnv,
    _class: JClass,
    appid: JString,
) -> jni::sys::jobject {
    let appid: String = env.get_string(&appid).unwrap().into();

    let miniapp = miniapp::get(appid);
    if let Ok(config) = miniapp.get_tab_bar_config() {
        if let Ok(result) = env.new_string(&config) {
            return result.into_raw();
        }
    }

    JObject::null().into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_com_lingxia_miniapp_WebView_nativeOnScrollChanged(
    mut env: JNIEnv,
    _class: JClass,
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
