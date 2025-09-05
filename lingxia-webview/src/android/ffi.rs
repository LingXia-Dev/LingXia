use crate::webview::{WebTag, get_webview_delegate, register_webview};
use crate::{LogLevel, WebViewError};
use http;
use http::header::{HeaderMap, HeaderName, HeaderValue};
use http::{Method, Request, Response};
use jni::JNIEnv;
use jni::objects::{JObject, JObjectArray, JString};
use jni::sys::jint;
use std::sync::Arc;

// Import from webview.rs
use crate::android::webview::{WEBVIEW_SENDERS, WebViewInner};

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

    let webtag = WebTag::new(&appid, &path);
    if let Some(delegate) = get_webview_delegate(&webtag) {
        delegate.handle_post_message(path, message);
    }
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

    let webtag = WebTag::new(&appid, &path);
    if let Some(delegate) = get_webview_delegate(&webtag) {
        delegate.on_page_started(path);
    }
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

    let webtag = WebTag::new(&appid, &path);
    if let Some(delegate) = get_webview_delegate(&webtag) {
        delegate.on_page_finished(path);
    }
    0
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaWebView_handleRequest<'a>(
    mut env: JNIEnv<'a>,
    _this: JObject<'a>,
    appid: JString<'a>,
    path: JString<'a>,
    url: JString<'a>,
    method: JString<'a>,
    headers_array: jni::sys::jobjectArray,
) -> JObject<'a> {
    // Convert Java strings to Rust strings
    let appid: String = env.get_string(&appid).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();
    let url_str: String = env.get_string(&url).unwrap().into();
    let method_str: String = env.get_string(&method).unwrap().into();

    // Parse headers from array: [key1, value1, key2, value2, ...]
    let mut http_headers = HeaderMap::new();

    if !headers_array.is_null() {
        // Convert raw pointer to JObjectArray
        let headers_array = unsafe { JObjectArray::from_raw(headers_array) };

        match env.get_array_length(&headers_array) {
            Ok(array_len) => {
                // Process pairs of key-value
                for i in (0..array_len).step_by(2) {
                    if i + 1 < array_len {
                        // Get key and value from array
                        if let (Ok(key_obj), Ok(value_obj)) = (
                            env.get_object_array_element(&headers_array, i),
                            env.get_object_array_element(&headers_array, i + 1),
                        ) {
                            let key_jstring = JString::try_from(key_obj);
                            let value_jstring = JString::try_from(value_obj);

                            let (key_jstring, value_jstring) =
                                (key_jstring.unwrap(), value_jstring.unwrap());
                            if let (Ok(key), Ok(value)) =
                                (env.get_string(&key_jstring), env.get_string(&value_jstring))
                            {
                                let key_str: String = key.into();
                                let value_str: String = value.into();

                                if let (Ok(name), Ok(val)) = (
                                    HeaderName::from_bytes(key_str.as_bytes()),
                                    HeaderValue::from_str(&value_str),
                                ) {
                                    http_headers.insert(name, val);
                                }
                            }
                        }
                    }
                }
            }
            Err(_) => {
                // If we can't get array length, continue with empty headers
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
    let webtag = WebTag::new(&appid, &path);
    let response = if let Some(delegate) = get_webview_delegate(&webtag) {
        delegate.handle_request(request)
    } else {
        None
    };
    if let Some(response) = response {
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

    let webtag = WebTag::new(&appid, &path);
    let log_level = match level {
        2 => LogLevel::Verbose, // VERBOSE
        3 => LogLevel::Debug,   // DEBUG
        4 => LogLevel::Info,    // INFO
        5 => LogLevel::Warn,    // WARN
        6 => LogLevel::Error,   // ERROR
        _ => LogLevel::Info,    // Default to INFO
    };

    if let Some(delegate) = get_webview_delegate(&webtag) {
        delegate.log(&path, log_level, &message);
    }
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

    let webtag = WebTag::new(&appid, &path);
    if let Some(delegate) = get_webview_delegate(&webtag) {
        delegate.on_page_scroll_changed(path, scroll_x, scroll_y, max_scroll_x, max_scroll_y);
    }
    0
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaWebView_notifyWebViewReady(
    mut env: JNIEnv,
    _class: JObject,
    appid: JString,
    path: JString,
    webview_obj: JObject,
) {
    let appid: String = env.get_string(&appid).unwrap().into();
    let path: String = env.get_string(&path).unwrap().into();

    // Retrieve the sender from our global map and send the WebView instance
    if let Some(senders) = WEBVIEW_SENDERS.get() {
        let webtag = WebTag::new(&appid, &path);

        if let Ok(mut senders_map) = senders.lock() {
            if let Some(sender) = senders_map.remove(&webtag.to_string()) {
                // Create global reference to the passed WebView object
                match env.new_global_ref(webview_obj) {
                    Ok(global_ref) => {
                        // Create WebViewInner from the Java object
                        let webview_inner = WebViewInner::from_java_object(global_ref);

                        // Create WebView wrapper
                        let webview = Arc::new(crate::WebView::new(
                            webview_inner,
                            appid.clone(),
                            path.clone(),
                        ));

                        // Register the WebView instance for future lookups
                        register_webview(webview.clone());

                        // Send the WebView instance through the channel
                        let _ = sender.send(Ok(webview));
                    }
                    Err(e) => {
                        let _ = sender.send(Err(WebViewError::WebView(format!(
                            "Failed to create global ref: {:?}",
                            e
                        ))));
                    }
                }
            }
        }
    }
}
