use http;
use http::header::{HeaderMap, HeaderName, HeaderValue};
use http::{Method, Request, Response};
use jni::JNIEnv;
use jni::objects::{JObject, JString};
use jni::sys::jint;
use lxapp::LxAppDelegate;
use lxapp::log::LogLevel;
use serde_json;

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

    let lxapp = lxapp::get(appid.clone());
    lxapp.handle_post_message(path, message);
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

    let lxapp = lxapp::get(appid);
    lxapp.on_page_started(path);
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

    let lxapp = lxapp::get(appid);
    lxapp.on_page_finished(path);
    0
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
    let lxapp = lxapp::get(appid.clone());
    if let Some(response) = lxapp.handle_request(request) {
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

    let lxapp = lxapp::get(appid.clone());
    let log_level = match level {
        2 => LogLevel::Verbose, // VERBOSE
        3 => LogLevel::Debug,   // DEBUG
        4 => LogLevel::Info,    // INFO
        5 => LogLevel::Warn,    // WARN
        6 => LogLevel::Error,   // ERROR
        _ => LogLevel::Info,    // Default to INFO
    };

    lxapp.log(&path, log_level, &message);
    1
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

    let lxapp = lxapp::get(appid.clone());
    lxapp.on_page_scroll_changed(path, scroll_x, scroll_y, max_scroll_x, max_scroll_y);
    0
}
