use crate::traits::NavigationPolicy;
use crate::webview::{
    WebTag, WebViewCreateStage, find_webview, get_webview_delegate, register_webview,
};
use crate::{LogLevel, WebResourceBody, WebResourceResponse, WebViewError};
use http::header::{HeaderMap, HeaderName, HeaderValue};
use http::{Method, Request};
use jni::objects::{JByteArray, JObject, JObjectArray, JString};
use jni::sys::{jint, jlong};
use jni::{Env, EnvUnowned, errors::ThrowRuntimeExAndDefault, jni_sig, jni_str};
use std::fs;
use std::sync::Arc;

// Import from webview.rs
use crate::android::webview::{WEBVIEW_SENDERS, WebViewInner};

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaWebView_handlePostMessage(
    mut env: EnvUnowned,
    _this: JObject,
    appid: JString,
    path: JString,
    session_id: jlong,
    message: JString,
) -> jint {
    env.with_env(|env| -> Result<jint, jni::errors::Error> {
        let appid: String = appid.try_to_string(env)?;
        let path: String = path.try_to_string(env)?;
        let message: String = message.try_to_string(env)?;
        let session_id = if session_id > 0 {
            Some(session_id as u64)
        } else {
            None
        };

        let webtag = WebTag::new(&appid, &path, session_id);
        if let Some(delegate) = get_webview_delegate(&webtag) {
            delegate.handle_post_message(message);
        }
        Ok(0)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaWebView_onPageStarted(
    mut env: EnvUnowned,
    _this: JObject,
    appid: JString,
    path: JString,
    session_id: jlong,
) -> jint {
    env.with_env(|env| -> Result<jint, jni::errors::Error> {
        let appid: String = appid.try_to_string(env)?;
        let path: String = path.try_to_string(env)?;
        let session_id = if session_id > 0 {
            Some(session_id as u64)
        } else {
            None
        };

        let webtag = WebTag::new(&appid, &path, session_id);
        if let Some(delegate) = get_webview_delegate(&webtag) {
            delegate.on_page_started();
        }
        Ok(0)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaWebView_onPageFinished(
    mut env: EnvUnowned,
    _this: JObject,
    appid: JString,
    path: JString,
    session_id: jlong,
) -> jint {
    env.with_env(|env| -> Result<jint, jni::errors::Error> {
        let appid: String = appid.try_to_string(env)?;
        let path: String = path.try_to_string(env)?;
        let session_id = if session_id > 0 {
            Some(session_id as u64)
        } else {
            None
        };

        let webtag = WebTag::new(&appid, &path, session_id);
        if let Some(delegate) = get_webview_delegate(&webtag) {
            delegate.on_page_finished();
        }
        Ok(0)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaWebView_handleRequest<'a>(
    mut env: EnvUnowned<'a>,
    _this: JObject<'a>,
    appid: JString<'a>,
    path: JString<'a>,
    session_id: jlong,
    url: JString<'a>,
    method: JString<'a>,
    headers_array: jni::sys::jobjectArray,
) -> JObject<'a> {
    env.with_env(|env| -> Result<JObject<'a>, jni::errors::Error> {
        // Convert Java strings to Rust strings
        let appid: String = appid.try_to_string(env)?;
        let path: String = path.try_to_string(env)?;
        let url_str: String = url.try_to_string(env)?;
        let method_str: String = method.try_to_string(env)?;
        let session_id = if session_id > 0 {
            Some(session_id as u64)
        } else {
            None
        };

        // Parse headers from array: [key1, value1, key2, value2, ...]
        let mut http_headers = HeaderMap::new();

        if !headers_array.is_null() {
            // Convert raw pointer to JObjectArray
            let headers_array = unsafe { JObjectArray::<JString>::from_raw(env, headers_array) };

            match headers_array.len(env) {
                Ok(array_len) => {
                    // Process pairs of key-value
                    for i in (0..array_len).step_by(2) {
                        if i + 1 < array_len {
                            // Get key and value from array
                            if let (Ok(key_obj), Ok(value_obj)) = (
                                headers_array.get_element(env, i as usize),
                                headers_array.get_element(env, (i + 1) as usize),
                            ) {
                                let key_jstring = key_obj;
                                let value_jstring = value_obj;

                                if let (Ok(key_str), Ok(value_str)) = (
                                    key_jstring.try_to_string(env),
                                    value_jstring.try_to_string(env),
                                ) {
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
            Err(_) => return Ok(JObject::null()),
        };

        // Dispatch to closure-based scheme handler
        let webtag = WebTag::new(&appid, &path, session_id);
        let scheme = request.uri().scheme_str().unwrap_or("").to_string();
        let response = if let Some(webview) = find_webview(&webtag) {
            webview.handle_scheme_request(&scheme, request)
        } else {
            None
        };
        if let Some(response) = response {
            Ok(create_java_response(env, response)?)
        } else {
            Ok(JObject::null())
        }
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

fn create_java_response<'a>(
    env: &mut Env<'a>,
    response: WebResourceResponse,
) -> jni::errors::Result<JObject<'a>> {
    // Try to find the WebResourceResponseData inner class with package
    let response_class = env.find_class(jni_str!(
        "com/lingxia/webview/LingXiaWebView$WebResourceResponseData"
    ))?;

    let (parts, body) = response.into_parts();

    // Extract response components
    let status = parts.status.as_u16() as i32;
    let reason = parts.status.canonical_reason().unwrap_or("Unknown");
    let headers = &parts.headers;

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
    let map = env.new_object(jni_str!("java/util/HashMap"), jni_sig!("()V"), &[])?;

    // Convert headers to Java HashMap
    for (key, value) in headers.iter() {
        if let Ok(v) = value.to_str() {
            let key_str = env.new_string(key.as_str())?;
            let value_str = env.new_string(v)?;

            let _ = env.call_method(
                &map,
                jni_str!("put"),
                jni_sig!("(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;"),
                &[(&key_str).into(), (&value_str).into()],
            );
        }
    }

    // Create Java strings and byte array
    let mime_type_str = env.new_string(mime_type)?;
    let encoding_str = env.new_string(encoding)?;
    let reason_str = env.new_string(reason)?;

    // Prepare file path, pipe fd, or bytes
    let (file_path_str, pipe_fd_jint, data_array, content_length): (JString, jint, JObject, i64) =
        match body {
            WebResourceBody::Path(path) => {
                let file_path_str = env.new_string(path.to_string_lossy())?;
                let content_length = headers
                    .get(http::header::CONTENT_LENGTH)
                    .and_then(|h| h.to_str().ok())
                    .and_then(|v| v.parse::<i64>().ok())
                    .or_else(|| fs::metadata(&path).ok().map(|meta| meta.len() as i64))
                    .unwrap_or(-1);
                (file_path_str, 0, JObject::null(), content_length)
            }
            WebResourceBody::Pipe(reader) => {
                let empty_path = env.new_string("")?;
                let fd = reader.into_raw_fd();
                (empty_path, fd as jint, JObject::null(), -1i64)
            }
            WebResourceBody::Bytes(data) => {
                let empty_path = env.new_string("")?;
                let data_array: JByteArray = env.byte_array_from_slice(&data)?;
                (empty_path, 0, JObject::from(data_array), data.len() as i64)
            }
        };

    // Create the WebResourceResponseData object
    env.new_object(
        response_class,
        jni_sig!("(Ljava/lang/String;Ljava/lang/String;ILjava/lang/String;Ljava/util/Map;Ljava/lang/String;I[BJ)V"),
        &[
            (&mime_type_str).into(),
            (&encoding_str).into(),
            status.into(),
            (&reason_str).into(),
            (&map).into(),
            (&file_path_str).into(),
            pipe_fd_jint.into(),
            (&data_array).into(),
            content_length.into(),
        ],
    )
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaWebView_handleNavigationPolicy(
    mut env: EnvUnowned,
    _this: JObject,
    appid: JString,
    path: JString,
    session_id: jlong,
    url: JString,
) -> bool {
    env.with_env(|env| -> Result<bool, jni::errors::Error> {
        let appid: String = appid.try_to_string(env)?;
        let path: String = path.try_to_string(env)?;
        let url: String = url.try_to_string(env)?;
        let session_id = if session_id > 0 {
            Some(session_id as u64)
        } else {
            None
        };

        let webtag = WebTag::new(&appid, &path, session_id);
        if let Some(webview) = find_webview(&webtag) {
            return Ok(matches!(
                webview.handle_navigation(&url),
                NavigationPolicy::Cancel
            ));
        }

        Ok(false)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaWebView_onConsoleMessage(
    mut env: EnvUnowned,
    _this: JObject,
    appid: JString,
    path: JString,
    session_id: jlong,
    level: jint,
    message: JString,
) -> jint {
    env.with_env(|env| -> Result<jint, jni::errors::Error> {
        let appid: String = appid.try_to_string(env)?;
        let path: String = path.try_to_string(env)?;
        let message: String = message.try_to_string(env)?;
        let session_id = if session_id > 0 {
            Some(session_id as u64)
        } else {
            None
        };

        let webtag = WebTag::new(&appid, &path, session_id);
        let log_level = match level {
            2 => LogLevel::Verbose, // VERBOSE
            3 => LogLevel::Debug,   // DEBUG
            4 => LogLevel::Info,    // INFO
            5 => LogLevel::Warn,    // WARN
            6 => LogLevel::Error,   // ERROR
            _ => LogLevel::Info,    // Default to INFO
        };

        if let Some(delegate) = get_webview_delegate(&webtag) {
            delegate.log(log_level, &message);
        }
        Ok(1)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_lingxia_webview_LingXiaWebView_notifyWebViewReady(
    mut env: EnvUnowned,
    _class: JObject,
    appid: JString,
    path: JString,
    session_id: jlong,
    webview_obj: JObject,
) {
    env.with_env(|env| -> Result<(), jni::errors::Error> {
        let appid: String = appid.try_to_string(env)?;
        let path: String = path.try_to_string(env)?;
        let session_id = if session_id > 0 {
            Some(session_id as u64)
        } else {
            None
        };

        // Retrieve the sender from our global map and send the WebView instance
        if let Some(senders) = WEBVIEW_SENDERS.get() {
            let webtag = WebTag::new(&appid, &path, session_id);
            let mut matched_pending = false;

            if let Ok(mut senders_map) = senders.lock()
                && let Some(pending) = senders_map.remove(&webtag.to_string())
            {
                matched_pending = true;
                // Create global reference to the passed WebView object
                match env.new_global_ref(webview_obj) {
                    Ok(global_ref) => {
                        // Create WebViewInner from the Java object
                        let webview_inner =
                            WebViewInner::from_java_object(global_ref, webtag.clone());

                        // Create WebView wrapper
                        let webview = Arc::new(crate::WebView::new(
                            webview_inner,
                            pending.effective_options.clone(),
                        ));

                        // Register the WebView instance for future lookups
                        register_webview(webview.clone());

                        // Send the WebView instance through the channel
                        pending.sender.succeed(webview);
                    }
                    Err(e) => {
                        pending.sender.fail(
                            WebViewCreateStage::Requested,
                            WebViewError::WebView(format!("Failed to create global ref: {:?}", e)),
                        );
                    }
                }
            }

            if !matched_pending {
                log::warn!(
                    "notifyWebViewReady without pending sender for {}",
                    webtag.as_str()
                );
            }
        } else {
            log::warn!(
                "notifyWebViewReady called before sender map initialization for {}:{}",
                appid,
                path
            );
        }
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}
