#![allow(non_snake_case)]

use super::asset::{ASSET_MANAGER, AssetManager};
use super::webview::WebView;
use super::webview::WebViewManager;
use android_logger::Config;
use http;
use http::header::{HeaderMap, HeaderName, HeaderValue};
use http::{Method, Request, Response};
use jni::objects::{JClass, JObject, JString};
use jni::sys::jint;
use jni::{JNIEnv, JavaVM};
use log::{error, info};
use serde_json;
use std::sync::{Arc, Mutex, OnceLock};

pub static JAVA_VM: OnceLock<Arc<JavaVM>> = OnceLock::new();

// Store the main thread's JNIEnv
thread_local! {
    static MAIN_THREAD_ID: OnceLock<std::thread::ThreadId> = OnceLock::new();
}

#[unsafe(no_mangle)]
pub extern "system" fn JNI_OnLoad(vm: JavaVM, _: *mut std::os::raw::c_void) -> jint {
    android_logger::init_once(
        Config::default()
            .with_max_level(log::LevelFilter::Debug)
            .with_tag("RustNative"),
    );

    // Store JavaVM globally
    let _ = JAVA_VM.set(Arc::new(vm));

    // Store the main thread ID
    MAIN_THREAD_ID.with(|id| {
        let _ = id.set(std::thread::current().id());
    });

    info!("Rust library loaded successfully");
    jni::sys::JNI_VERSION_1_6
}

// Helper function to get JNIEnv for current thread
pub(crate) fn get_env() -> Result<JNIEnv<'static>, Box<dyn std::error::Error>> {
    let vm = JAVA_VM.get().ok_or("JavaVM not initialized")?.clone();

    // Check if we're on the main thread
    let is_main_thread = MAIN_THREAD_ID.with(|id| {
        id.get()
            .map(|main_id| main_id == &std::thread::current().id())
            .unwrap_or(false)
    });

    if is_main_thread {
        // If we're on the main thread, get the env
        unsafe { Ok(JNIEnv::from_raw(vm.get_env()?.get_raw())?) }
    } else {
        // If we're not on the main thread, attach to get a new env
        unsafe { Ok(JNIEnv::from_raw(vm.attach_current_thread()?.get_raw())?) }
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
    let asset_manager_ptr = match AssetManager::from_java(
        env.get_native_interface() as *mut jni::sys::JNIEnv,
        asset_manager.as_raw(),
    ) {
        Ok(manager) => manager,
        Err(e) => {
            error!("Failed to create AssetManager: {}", e);
            return -1;
        }
    };

    // These paths always exist in Android
    let cache_dir = env.get_string(&cache_dir).unwrap().into();
    let data_dir = env.get_string(&data_dir).unwrap().into();

    // Initialize the global ASSET_MANAGER
    let _ = ASSET_MANAGER.set(Arc::new(Mutex::new(asset_manager_ptr.clone())));

    // Initialize MiniApp
    miniapp::init(Box::new(asset_manager_ptr), cache_dir, data_dir);

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
    let webview = WebView::from_java(java_webview);

    // Notify miniapp about page creation with the WebView controller
    if let Ok(mut miniapp) = miniapp::get().lock() {
        miniapp.on_page_created(app_id, path, Arc::new(webview));
    }
    0
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeOnMiniAppDestroy(
    _env: JNIEnv,
    _class: JClass,
) -> jint {
    match WebViewManager::destroy_all_webviews() {
        Ok(_) => 0,
        Err(e) => {
            log::error!("Failed to destroy WebViews: {:?}", e);
            -1
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeHandlePostMessage(
    mut env: JNIEnv,
    _class: JClass,
    app_id: JString,
    path: JString,
    message: JString,
) -> jint {
    let app_id: String = env
        .get_string(&app_id)
        .expect("Couldn't get app_id string")
        .into();
    let path: String = env
        .get_string(&path)
        .expect("Couldn't get path string")
        .into();
    let message: String = env
        .get_string(&message)
        .expect("Couldn't get message string")
        .into();

    match WebViewManager::handle_post_message(app_id, path, message) {
        Ok(_) => 0,
        Err(e) => {
            log::error!("Failed to handle post message: {:?}", e);
            -1
        }
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
    path: JString,
    url: JString,
) -> jint {
    let app_id: String = env.get_string(&app_id).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();
    let url: String = env.get_string(&url).unwrap().into();

    // Get the miniapp instance and check if we should override the URL
    miniapp::get()
        .lock()
        .map(|miniapp| {
            if miniapp.should_override_url_loading(app_id, path, url) {
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

    // Get the miniapp instance and find the page controller
    match miniapp::get().lock() {
        Ok(miniapp) => {
            if let Some(controller) = miniapp.find_page_controller(&app_id, &path) {
                // Since we're in Android crate, we know this must be a WebView
                let controller_ref = controller.as_ref();
                if let Some(webview) = controller_ref.as_any().downcast_ref::<WebView>() {
                    // Create a new local reference to the Java WebView object
                    match env.new_local_ref(webview.get_java_webview()) {
                        Ok(local_ref) => unsafe { JObject::from_raw(local_ref.into_raw()) },
                        Err(e) => {
                            error!("Failed to create local reference to WebView: {:?}", e);
                            JObject::null()
                        }
                    }
                } else {
                    error!("PageController is not a WebView");
                    JObject::null()
                }
            } else {
                // No page controller found
                JObject::null()
            }
        }
        Err(e) => {
            error!("Failed to get miniapp instance: {:?}", e);
            JObject::null()
        }
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

// Function for MiniAppActivity class to handle the mini app hidden event
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_miniapp_MiniAppActivity_nativeOnMiniAppHidden(
    mut env: JNIEnv,
    _class: JClass,
    app_id: JString,
    path: JString,
) -> jint {
    let app_id: String = env.get_string(&app_id).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();

    if let Ok(mut miniapp) = miniapp::get().lock() {
        info!(
            "Mini app hidden from MiniAppActivity: app_id={}, path={}",
            app_id, path
        );

        miniapp.on_miniapp_hidden(app_id);
    };
    0
}
