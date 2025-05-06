use super::platform::Platform;
use super::webview::WebView;
use crate::controller::Controller;
use android_logger::Config;
use http;
use http::header::{HeaderMap, HeaderName, HeaderValue};
use http::{Method, Request, Response};
use jni::objects::{JClass, JObject, JString};
use jni::sys::jint;
use jni::{JNIEnv, JavaVM};
use log::{error, info};
use miniapp::log::{LogLevel, LogTag, Logging};
use serde_json;
use std::sync::{Arc, OnceLock};

pub static JAVA_VM: OnceLock<Arc<JavaVM>> = OnceLock::new();

/// Java class name for MiniApp
pub(crate) const CLASS_MINIAPP: &str = "com/lingxia/miniapp/MiniApp";

#[unsafe(no_mangle)]
pub extern "system" fn JNI_OnLoad(vm: JavaVM, _: *mut std::os::raw::c_void) -> jint {
    android_logger::init_once(
        Config::default()
            .with_max_level(log::LevelFilter::Debug)
            .with_tag("RustNative"),
    );

    // Store JavaVM globally and keep a copy for UI thread initialization
    let jvm = Arc::new(vm);
    let jvm_clone = jvm.clone();
    let _ = JAVA_VM.set(jvm);

    // Initialize and start the controller
    if !Controller::run(move || -> bool {
        let _ = jvm_clone.attach_current_thread().map_err(|e| {
            error!("Failed to attach UI thread to JVM: {:?}", e);
            return false;
        });

        true
    }) {
        error!("Failed to start controller");
    }

    info!("Rust library loaded successfully");
    jni::sys::JNI_VERSION_1_6
}

// Helper function to get JNIEnv for current thread
// IMPORTANT: This should only be called from the UI thread that was attached to the JVM by Controller::run
pub(crate) fn get_env() -> Option<JNIEnv<'static>> {
    let vm = match JAVA_VM.get() {
        Some(vm) => vm.clone(),
        None => {
            error!("JavaVM not initialized");
            return None;
        }
    };

    // Only get the environment if the thread is already attached
    // This ensures get_env() is only used by the UI thread that was attached in Controller::run
    match vm.get_env() {
        Ok(env) => unsafe { JNIEnv::from_raw(env.get_raw()).ok() },
        Err(_) => {
            error!("current thread requires JVM");
            None
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
) -> jint {
    // Get the native AAssetManager pointer from the passed Java object
    let data_dir = env.get_string(&data_dir).unwrap().into();
    let cache_dir = env.get_string(&cache_dir).unwrap().into();

    let platform = match Platform::from_java(
        env.get_native_interface() as *mut jni::sys::JNIEnv,
        asset_manager.as_raw(),
        data_dir,
        cache_dir,
    ) {
        Ok(platform) => platform,
        Err(e) => {
            error!("Failed to create Platform: {}", e);
            return -1;
        }
    };

    // Initialize MiniApp
    miniapp::init(Box::new(platform));

    0
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeOnWebViewCreated(
    mut env: JNIEnv,
    _class: JClass,
    app_id: JString,
    path: JString,
    java_webview: JObject,
) -> jint {
    let app_id: String = env.get_string(&app_id).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();

    // Create WebView
    let webview = WebView::from_java(java_webview, app_id.clone(), path.clone());

    // Add WebView to Controller
    if let Some(controller) = Controller::get() {
        if controller.put_webview(app_id.clone(), path.clone(), webview.clone()) {
            info!("WebView added to Controller for {}/{}", app_id, path);
        } else {
            error!("Failed to add WebView to Controller");
        }
    }

    // Notify miniapp about page creation with the WebView controller
    if let Ok(mut miniapp) = miniapp::get().lock() {
        miniapp.on_page_created(app_id, path, Arc::new(webview));
    }
    0
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeHandlePostMessage(
    mut env: JNIEnv,
    _class: JClass,
    app_id: JString,
    path: JString,
    message: JString,
) -> jint {
    let app_id: String = env.get_string(&app_id).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();
    let message: String = env.get_string(&message).unwrap().into();

    if let Ok(miniapp) = miniapp::get().lock() {
        miniapp.handle_post_message(app_id, path, message);
        0
    } else {
        -1
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeOnPageStarted(
    mut env: JNIEnv,
    _class: JClass,
    app_id: JString,
    path: JString,
) -> jint {
    let app_id: String = env.get_string(&app_id).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();

    if let Ok(miniapp) = miniapp::get().lock() {
        miniapp.on_page_started(app_id, path);
    }
    0
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeOnPageFinished(
    mut env: JNIEnv,
    _class: JClass,
    app_id: JString,
    path: JString,
) -> jint {
    let app_id: String = env.get_string(&app_id).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();

    if let Ok(miniapp) = miniapp::get().lock() {
        miniapp.on_page_finished(app_id, path);
    }
    0
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeOnPageShow(
    mut env: JNIEnv,
    _class: JClass,
    app_id: JString,
    path: JString,
) {
    let app_id: String = env.get_string(&app_id).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();

    if let Ok(miniapp) = miniapp::get().lock() {
        miniapp.on_page_show(app_id, path);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeShouldOverrideUrlLoading(
    mut env: JNIEnv,
    _class: JClass,
    app_id: JString,
    url: JString,
) -> jint {
    let app_id: String = env.get_string(&app_id).unwrap().into();
    let url: String = env.get_string(&url).unwrap().into();

    // Get the miniapp instance and check if we should override the URL
    miniapp::get()
        .lock()
        .map(|miniapp| {
            if miniapp.should_override_url_loading(app_id, url) {
                1
            } else {
                0
            }
        })
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeGetExistingWebView<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    app_id: JString<'a>,
    path: JString<'a>,
) -> JObject<'a> {
    let app_id: String = env.get_string(&app_id).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();

    // Get the controller and try to find the WebView
    if let Some(controller) = Controller::get() {
        if let Some(webview_rc) = controller.get_webview(&app_id, &path) {
            // Create a new local reference to the Java WebView object
            match env.new_local_ref(webview_rc.get_java_webview()) {
                Ok(local_ref) => unsafe { JObject::from_raw(local_ref.into_raw()) },
                Err(e) => {
                    error!("Failed to create local reference to WebView: {:?}", e);
                    JObject::null()
                }
            }
        } else {
            // No WebView found for this app_id/path
            JObject::null()
        }
    } else {
        error!("Controller not initialized");
        JObject::null()
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeHandleRequest<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    app_id: JString<'a>,
    url: JString<'a>,
    method: JString<'a>,
    headers: JString<'a>,
) -> JObject<'a> {
    // Convert Java strings to Rust strings
    let app_id: String = env.get_string(&app_id).unwrap().into();
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
    match miniapp::get().lock() {
        Ok(miniapp) => {
            if let Some(response) = miniapp.handle_request(app_id, request) {
                create_java_response(&mut env, response)
            } else {
                JObject::null()
            }
        }
        Err(_) => JObject::null(),
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
    app_id: JString,
) -> jint {
    let app_id: String = env.get_string(&app_id).unwrap().into();

    if let Ok(mut miniapp) = miniapp::get().lock() {
        miniapp.on_miniapp_closed(app_id);
    };
    0
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeOnConsoleMessage(
    mut env: JNIEnv,
    _class: JClass,
    app_id: JString,
    level: jint,
    message: JString,
) -> jint {
    let app_id: String = env.get_string(&app_id).unwrap().into();
    let message: String = env.get_string(&message).unwrap().into();

    if let Ok(miniapp) = miniapp::get().lock() {
        match level {
            2 => miniapp.log(LogLevel::Verbose, &app_id, LogTag::WebViewConsole, message), // VERBOSE
            3 => miniapp.log(LogLevel::Debug, &app_id, LogTag::WebViewConsole, message),   // DEBUG
            4 => miniapp.log(LogLevel::Info, &app_id, LogTag::WebViewConsole, message),    // INFO
            5 => miniapp.log(LogLevel::Warn, &app_id, LogTag::WebViewConsole, message),    // WARN
            6 => miniapp.log(LogLevel::Error, &app_id, LogTag::WebViewConsole, message),   // ERROR
            _ => miniapp.log(LogLevel::Info, &app_id, LogTag::WebViewConsole, message), // Default to INFO
        }
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeGetPageConfig<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    app_id: JString<'a>,
    path: JString<'a>,
) -> JObject<'a> {
    let app_id: String = env.get_string(&app_id).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();

    // Get the miniapp instance and get page config
    match miniapp::get().lock() {
        Ok(miniapp) => {
            if let Some(json) = miniapp.get_page_config(&app_id, &path) {
                // Create Java string from JSON
                if let Ok(java_string) = env.new_string(&json) {
                    return java_string.into();
                }
            }
            JObject::null()
        }
        Err(_) => JObject::null(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_com_lingxia_miniapp_MiniAppActivity_nativeOnBackPressed(
    mut env: JNIEnv,
    _class: JClass,
    app_id: JString,
) -> jint {
    let app_id: String = env.get_string(&app_id).unwrap().into();
    if let Ok(miniapp) = miniapp::get().lock() {
        if miniapp.on_back_pressed(&app_id) {
            1
        } else {
            0
        }
    } else {
        0
    }
}

// Function to notify the Rust layer that a mini app has been opened
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_miniapp_MiniApp_nativeOnMiniAppOpened(
    mut env: JNIEnv,
    _class: JClass,
    app_id: JString,
) -> jint {
    let app_id: String = env.get_string(&app_id).unwrap().into();

    if let Ok(mut miniapp) = miniapp::get().lock() {
        miniapp.on_miniapp_opened(app_id);
    };
    0
}

// New function to get app configuration
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_miniapp_MiniApp_nativeGetTabBarConfig(
    mut env: JNIEnv,
    _class: JClass,
    app_id: JString,
) -> jni::sys::jobject {
    let app_id: String = env.get_string(&app_id).unwrap().into();

    if let Ok(miniapp) = miniapp::get().lock() {
        if let Some(config) = miniapp.get_tab_bar_config(&app_id) {
            if let Ok(result) = env.new_string(config) {
                return result.into_raw();
            }
        }
    }

    JObject::null().into_raw()
}
