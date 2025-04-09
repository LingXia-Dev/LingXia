#![allow(non_snake_case)]

use super::asset::{ASSET_MANAGER, AssetManager};
use super::webview::WebView;
use super::webview::WebViewManager;
use android_logger::Config;
use http;
use http::header::{HeaderMap, HeaderName, HeaderValue};
use http::{Method, Request, Response, StatusCode};
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
    asset_manager: JObject,
    cache_dir: JString,
    data_dir: JString,
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
    mut _env: JNIEnv,
    _class: JClass,
    _app_id: JString,
    url: JString,
) -> jint {
    let url_str: String = match _env.get_string(&url) {
        Ok(s) => s.into(),
        Err(e) => {
            error!("Failed to get url string: {:?}", e);
            return 0;
        }
    };

    // Extract scheme from URL
    let scheme = if let Some(scheme_end) = url_str.find("://") {
        &url_str[..scheme_end]
    } else {
        ""
    };

    info!("Checking URL override: {} (scheme: {})", url_str, scheme);

    // Define allowed and intercepted schemes
    let allowed_schemes = vec!["http", "https", "file"];
    let intercepted_schemes = vec!["miniapp", "lingxia"];

    if intercepted_schemes.contains(&scheme) {
        // Handle custom scheme
        info!("Intercepting custom scheme: {}", scheme);
        return 1;
    }

    if !allowed_schemes.contains(&scheme) {
        // Block disallowed schemes
        error!("Blocking disallowed scheme: {}", scheme);
        return 1;
    }

    // Allow standard http/https/file requests to proceed
    0
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeGetExistingWebView<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    app_id: JString<'a>,
    path: JString<'a>,
) -> JObject<'a> {
    let app_id: String = match env.get_string(&app_id) {
        Ok(s) => s.into(),
        Err(e) => {
            error!("Failed to get app_id string: {:?}", e);
            return JObject::null();
        }
    };

    let path: String = match env.get_string(&path) {
        Ok(s) => s.into(),
        Err(e) => {
            error!("Failed to get path string: {:?}", e);
            return JObject::null();
        }
    };

    match WebViewManager::get_existing_webview(&app_id, &path) {
        Ok(Some(webview)) => webview,
        Ok(None) => JObject::null(),
        Err(e) => {
            error!("Failed to get existing WebView: {:?}", e);
            JObject::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_miniapp_WebView_nativeHandleRequest<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    _app_id: JString<'a>,
    url: JString<'a>,
    method: JString<'a>,
    headers: JString<'a>,
) -> JObject<'a> {
    // Convert Java strings to Rust strings
    let url_str: String = match env.get_string(&url) {
        Ok(s) => s.into(),
        Err(e) => {
            error!("Failed to get url string: {:?}", e);
            return JObject::null();
        }
    };

    let method_str: String = match env.get_string(&method) {
        Ok(s) => s.into(),
        Err(e) => {
            error!("Failed to get method string: {:?}", e);
            return JObject::null();
        }
    };

    let headers_str: String = match env.get_string(&headers) {
        Ok(s) => s.into(),
        Err(e) => {
            error!("Failed to get headers string: {:?}", e);
            return JObject::null();
        }
    };

    // Parse headers from JSON
    let headers_map: serde_json::Map<String, serde_json::Value> =
        match serde_json::from_str(&headers_str) {
            Ok(map) => map,
            Err(e) => {
                error!("Failed to parse headers JSON: {:?}", e);
                return JObject::null();
            }
        };

    // Convert to http::HeaderMap
    let mut http_headers = HeaderMap::new();
    for (key, value) in headers_map {
        if let Some(value_str) = value.as_str() {
            if let (Ok(header_name), Ok(header_value)) = (
                HeaderName::from_bytes(key.as_bytes()),
                HeaderValue::from_str(value_str),
            ) {
                http_headers.insert(header_name, header_value);
            }
        }
    }

    // Parse method
    let http_method = match method_str.parse::<Method>() {
        Ok(m) => m,
        Err(e) => {
            error!("Failed to parse HTTP method: {:?}", e);
            return JObject::null();
        }
    };

    // Create http::Request
    let request = match Request::builder()
        .method(http_method)
        .uri(&url_str)
        .body(())
    {
        Ok(mut req) => {
            *req.headers_mut() = http_headers;
            req
        }
        Err(e) => {
            error!("Failed to create request: {:?}", e);
            return JObject::null();
        }
    };

    // Handle request based on scheme
    let response = handle_request(request);

    // Convert response to Java WebResourceResponseData
    match response {
        Ok(response) => create_java_response(&mut env, response),
        Err(e) => {
            error!("Error handling request: {:?}", e);
            JObject::null()
        }
    }
}

fn handle_request(request: Request<()>) -> Result<Response<Vec<u8>>, Box<dyn std::error::Error>> {
    let uri = request.uri();
    let scheme = uri.scheme_str().unwrap_or("");

    // Don't intercept http/https requests
    if scheme == "http" || scheme == "https" {
        info!("Not intercepting {}: {}", scheme, uri);
        return Err("Not intercepting http/https requests".into());
    }

    // Handle lingxia:// scheme
    if scheme == "lingxia" {
        info!("Processing lingxia scheme request: {}", uri);

        // Get the path part after lingxia://
        let path = uri.path().trim_start_matches('/');
        info!("Original path: {}", path);

        // Handle demo path
        let asset_path = if path.starts_with("demo/") {
            path.to_string()
        } else {
            format!("demo/{}", path)
        };
        info!("Looking for asset at path: {}", asset_path);

        // Get the stored asset manager
        let asset_manager = match ASSET_MANAGER.get() {
            Some(manager_arc) => match manager_arc.lock() {
                Ok(manager) => manager.0,
                Err(e) => {
                    error!("Failed to lock asset manager: {:?}", e);
                    return Ok(Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .header("Content-Type", "text/plain")
                        .body(b"Failed to lock asset manager".to_vec())?);
                }
            },
            None => {
                error!("Asset manager not initialized");
                return Ok(Response::builder()
                    .status(StatusCode::SERVICE_UNAVAILABLE)
                    .header("Content-Type", "text/plain")
                    .body(b"Asset manager not initialized".to_vec())?);
            }
        };

        let result = unsafe {
            info!("Attempting to open asset: {}", asset_path);
            let asset = ndk_sys::AAssetManager_open(
                asset_manager,
                format!("{}\0", asset_path).as_bytes().as_ptr() as *const _,
                ndk_sys::AASSET_MODE_BUFFER as i32,
            );

            if !asset.is_null() {
                let length = ndk_sys::AAsset_getLength64(asset) as usize;
                let mut buffer = vec![0u8; length];
                let bytes_read = ndk_sys::AAsset_read(asset, buffer.as_mut_ptr() as *mut _, length);

                ndk_sys::AAsset_close(asset);

                if bytes_read > 0 {
                    // Determine MIME type based on file extension
                    let mime_type = if asset_path.ends_with(".html") {
                        "text/html"
                    } else if asset_path.ends_with(".js") {
                        "application/javascript"
                    } else if asset_path.ends_with(".css") {
                        "text/css"
                    } else {
                        "application/octet-stream"
                    };

                    info!(
                        "Successfully loaded asset: {} ({} bytes)",
                        asset_path, bytes_read
                    );
                    return Ok(Response::builder()
                        .status(StatusCode::OK)
                        .header("Content-Type", mime_type)
                        .header("Content-Length", bytes_read.to_string())
                        .body(buffer)?);
                }
            }
            error!("Failed to load asset: {}", asset_path);
            Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header("Content-Type", "text/plain")
                .body(format!("Asset not found: {}", asset_path).into_bytes())?)
        };

        result
    } else {
        // Return error for unknown schemes
        error!("Unknown scheme: {}", scheme);
        Err("Unknown scheme".into())
    }
}

fn create_java_response<'a>(env: &mut JNIEnv<'a>, response: Response<Vec<u8>>) -> JObject<'a> {
    let response_class = match env.find_class("com/lingxia/miniapp/WebResourceResponseData") {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to find WebResourceResponseData class: {:?}", e);
            return JObject::null();
        }
    };

    // Extract response components
    let status = response.status().as_u16() as i32;
    let reason = response.status().canonical_reason().unwrap_or("Unknown");
    let headers = response.headers();
    let body = response.body();

    // Get content type from headers or default
    let content_type = headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("application/octet-stream");

    // Split content type into mime type and encoding
    let (mime_type, encoding) = match content_type.split_once(';') {
        Some((mime, enc)) => (mime.trim(), enc.trim().trim_start_matches("charset=")),
        None => (content_type, "UTF-8"),
    };

    // Create response headers Map
    let map_class = match env.find_class("java/util/HashMap") {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to find HashMap class: {:?}", e);
            return JObject::null();
        }
    };

    let map = match env.new_object(map_class, "()V", &[]) {
        Ok(m) => m,
        Err(e) => {
            error!("Failed to create HashMap: {:?}", e);
            return JObject::null();
        }
    };

    // Add headers to map
    for (key, value) in headers.iter() {
        if let Ok(v) = value.to_str() {
            let key_str = match env.new_string(key.as_str()) {
                Ok(s) => s,
                Err(e) => {
                    error!("Failed to create header key string: {:?}", e);
                    continue;
                }
            };
            let value_str = match env.new_string(v) {
                Ok(s) => s,
                Err(e) => {
                    error!("Failed to create header value string: {:?}", e);
                    continue;
                }
            };
            if let Err(e) = env.call_method(
                &map,
                "put",
                "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                &[(&key_str).into(), (&value_str).into()],
            ) {
                error!("Failed to put header to map: {:?}", e);
            }
        }
    }

    // Create byte array for response data
    let byte_array = match env.byte_array_from_slice(body) {
        Ok(arr) => arr,
        Err(e) => {
            error!("Failed to create byte array: {:?}", e);
            return JObject::null();
        }
    };

    // Create strings for constructor
    let mime_type_str = match env.new_string(mime_type) {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to create mime type string: {:?}", e);
            return JObject::null();
        }
    };
    let encoding_str = match env.new_string(encoding) {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to create encoding string: {:?}", e);
            return JObject::null();
        }
    };
    let reason_str = match env.new_string(reason) {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to create reason string: {:?}", e);
            return JObject::null();
        }
    };

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
        Ok(obj) => {
            info!(
                "Successfully created Java response object: status={}, mime={}, encoding={}, bodySize={}",
                status,
                mime_type,
                encoding,
                body.len()
            );
            obj
        }
        Err(e) => {
            error!("Failed to create Java response object: {:?}", e);
            JObject::null()
        }
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
    let app_id: String = match env.get_string(&app_id) {
        Ok(s) => s.into(),
        Err(e) => {
            error!("Failed to get app_id string: {:?}", e);
            return -1;
        }
    };

    let path: String = match env.get_string(&path) {
        Ok(s) => s.into(),
        Err(e) => {
            error!("Failed to get path string: {:?}", e);
            return -1;
        }
    };

    info!(
        "Mini app hidden from MiniAppActivity: app_id={}, path={}",
        app_id, path
    );
    0
}
