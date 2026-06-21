use crate::webview::{WebTag, find_webview};
use crate::{WebResourceBody, WebResourceResponse};
use napi_ohos::Result as NapiResult;
use ohos_web_sys::*;
use std::ffi::{CStr, CString};
use std::fs::File;
use std::io::Read;
use std::os::raw::{c_char, c_int};
use std::ptr;

/// Callback function for handling lx:// and https:// scheme requests
pub unsafe extern "C" fn on_lx_request_start(
    scheme_handler: *const ArkWeb_SchemeHandler,
    resource_request: *mut ArkWeb_ResourceRequest,
    resource_handler: *const ArkWeb_ResourceHandler,
    intercept: *mut bool,
) {
    if scheme_handler.is_null() || resource_request.is_null() || resource_handler.is_null() {
        log::error!("Invalid parameters in scheme handler callback");
        return;
    }

    // Get webtag from user data
    let user_data = unsafe { OH_ArkWebSchemeHandler_GetUserData(scheme_handler) };
    if user_data.is_null() {
        log::error!("No user data found in scheme handler");
        return;
    }

    let webtag_cstr = unsafe { CStr::from_ptr(user_data as *const c_char) };
    let webtag_str = match webtag_cstr.to_str() {
        Ok(s) => s,
        Err(_) => {
            log::error!("Invalid webtag in user data");
            return;
        }
    };
    let webtag = WebTag::from(webtag_str);

    // Get request URL
    let mut url_ptr: *mut c_char = ptr::null_mut();
    unsafe { OH_ArkWebResourceRequest_GetUrl(resource_request, &mut url_ptr) };
    if url_ptr.is_null() {
        log::error!("Failed to get request URL");
        return;
    }

    let url = match unsafe { CStr::from_ptr(url_ptr).to_str() } {
        Ok(s) => s.to_string(),
        Err(_) => {
            log::error!("Invalid UTF-8 in request URL");
            unsafe { OH_ArkWeb_ReleaseString(url_ptr) };
            return;
        }
    };
    unsafe { OH_ArkWeb_ReleaseString(url_ptr) };

    // Get request method
    let mut method_ptr: *mut c_char = ptr::null_mut();
    unsafe { OH_ArkWebResourceRequest_GetMethod(resource_request, &mut method_ptr) };
    let method = if method_ptr.is_null() {
        "GET".to_string()
    } else {
        let method_str = unsafe {
            CStr::from_ptr(method_ptr)
                .to_str()
                .unwrap_or("GET")
                .to_string()
        };
        unsafe { OH_ArkWeb_ReleaseString(method_ptr) };
        method_str
    };

    log::info!(
        "Processing request: {} {} for webtag: {}",
        method,
        url,
        webtag.as_str()
    );

    // Build HTTP request to check if lxapp wants to handle it
    let http_request = match http::Request::builder()
        .method(method.as_str())
        .uri(&url)
        .body(Vec::new()) // TODO
    {
        Ok(req) => req,
        Err(e) => {
            log::error!("Failed to build HTTP request: {}", e);
            return; // Don't intercept if we can't build request
        }
    };

    // Extract scheme from URL
    let scheme = url.split("://").next().unwrap_or("").to_string();

    // Dispatch to closure-based scheme handler
    let http_response = if let Some(webview) = find_webview(&webtag) {
        webview.handle_scheme_request(&scheme, http_request)
    } else {
        None
    };
    if let Some(http_response) = http_response {
        unsafe {
            *intercept = true;
            send_response(resource_handler, http_response);
        }
    }
}

/// Send a successful response
unsafe fn send_response(
    resource_handler: *const ArkWeb_ResourceHandler,
    http_response: WebResourceResponse,
) {
    let (parts, body) = http_response.into_parts();
    let mut headers_map = parts.headers.clone();

    // Create ArkWeb response
    let mut response: *mut ArkWeb_Response = ptr::null_mut();
    unsafe {
        OH_ArkWeb_CreateResponse(&mut response);
    }

    // Set status code
    unsafe {
        OH_ArkWebResponse_SetStatus(response, parts.status.as_u16() as c_int);
    }

    if !headers_map.contains_key(http::header::CONTENT_LENGTH) {
        let content_len = match &body {
            WebResourceBody::Path(path) => std::fs::metadata(path).ok().map(|m| m.len()),
            WebResourceBody::Bytes(data) => Some(data.len() as u64),
            WebResourceBody::Pipe(_) => None,
        };
        if let Some(len) = content_len
            && let Ok(value) = http::HeaderValue::from_str(&len.to_string())
        {
            headers_map.insert(http::header::CONTENT_LENGTH, value);
        }
    }

    // Set headers
    for (key, value) in headers_map.iter() {
        if let Ok(value_str) = value.to_str()
            && let (Ok(key_cstr), Ok(value_cstr)) =
                (CString::new(key.as_str()), CString::new(value_str))
        {
            unsafe {
                OH_ArkWebResponse_SetHeaderByName(
                    response,
                    key_cstr.as_ptr(),
                    value_cstr.as_ptr(),
                    true, // overwrite
                );
            }
        }
    }

    // Send response headers
    unsafe {
        OH_ArkWebResourceHandler_DidReceiveResponse(resource_handler, response);
    }

    match body {
        WebResourceBody::Path(path) => {
            let mut file = match File::open(&path) {
                Ok(f) => f,
                Err(e) => {
                    log::error!(
                        "Failed to open response file for Harmony webview: {} ({})",
                        path.display(),
                        e
                    );
                    unsafe {
                        OH_ArkWebResponse_SetStatus(response, 500);
                    }
                    let message = CString::new("Internal Server Error").unwrap();
                    unsafe {
                        OH_ArkWebResourceHandler_DidReceiveResponse(resource_handler, response);
                        OH_ArkWebResourceHandler_DidReceiveData(
                            resource_handler,
                            message.as_ptr(),
                            message.as_bytes().len() as i64,
                        );
                        OH_ArkWebResourceHandler_DidFinish(resource_handler);
                        OH_ArkWeb_DestroyResponse(response);
                    }
                    return;
                }
            };

            // Send response body
            let mut buffer = [0u8; 64 * 1024];
            loop {
                match file.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(read_bytes) => unsafe {
                        OH_ArkWebResourceHandler_DidReceiveData(
                            resource_handler,
                            buffer.as_ptr(),
                            read_bytes as i64,
                        );
                    },
                    Err(e) => {
                        log::error!(
                            "Failed while streaming response data for Harmony webview: {}",
                            e
                        );
                        break;
                    }
                }
            }
        }
        WebResourceBody::Pipe(reader) => {
            let mut file = reader.into_file();

            let mut buffer = [0u8; 64 * 1024];
            loop {
                match file.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(read_bytes) => unsafe {
                        OH_ArkWebResourceHandler_DidReceiveData(
                            resource_handler,
                            buffer.as_ptr(),
                            read_bytes as i64,
                        );
                    },
                    Err(e) => {
                        log::error!(
                            "Failed while streaming response data for Harmony webview: {}",
                            e
                        );
                        break;
                    }
                }
            }
        }
        WebResourceBody::Bytes(data) => {
            if !data.is_empty() {
                unsafe {
                    OH_ArkWebResourceHandler_DidReceiveData(
                        resource_handler,
                        data.as_ptr(),
                        data.len() as i64,
                    );
                }
            }
        }
    }

    // Finish the response
    unsafe {
        OH_ArkWebResourceHandler_DidFinish(resource_handler);
    }

    // Clean up
    unsafe {
        OH_ArkWeb_DestroyResponse(response);
    }
}

/// Callback function for stopping scheme requests
pub unsafe extern "C" fn on_lx_request_stop(
    _scheme_handler: *const ArkWeb_SchemeHandler,
    resource_request: *const ArkWeb_ResourceRequest,
) {
    log::debug!("Stopped request");

    // Clean up the resource request
    if !resource_request.is_null() {
        unsafe {
            OH_ArkWebResourceRequest_Destroy(resource_request);
        }
    }
}

/// Clean up scheme handler and its user data
pub unsafe fn cleanup_scheme_handler(scheme_handler: *mut ArkWeb_SchemeHandler) {
    if scheme_handler.is_null() {
        return;
    }

    unsafe {
        // Get and free the user data (webtag CString)
        let user_data = OH_ArkWebSchemeHandler_GetUserData(scheme_handler);
        if !user_data.is_null() {
            // Reconstruct CString from raw pointer to properly free it
            let _webtag_cstr = CString::from_raw(user_data as *mut c_char);
            // CString will be automatically dropped here, freeing the memory
        }

        // Destroy the scheme handler
        OH_ArkWeb_DestroySchemeHandler(scheme_handler);
    }
}

/// Register custom schemes globally (called once during lxapp_init)
pub fn register_custom_schemes() -> NapiResult<()> {
    unsafe {
        // STANDARD | SECURE | CORS_ENABLED | CSP_BYPASSING | FETCH_API
        let options = 1 | 2 | 16 | 32 | 64;

        // ArkWeb only recognizes a non-standard scheme as navigable once it has
        // been registered globally here, before any Web component is created; the
        // per-webview handler in set_webview_scheme_handler can only intercept a
        // scheme that is already registered. Register every custom scheme the
        // webviews serve: `lx` for lxapp bundles and `lingxia` for the browser's
        // own start/settings/downloads pages.
        for scheme in ["lx", "lingxia"] {
            let scheme_cstr = CString::new(scheme).unwrap();
            OH_ArkWeb_RegisterCustomSchemes(scheme_cstr.as_ptr(), options);
            log::info!(
                "Successfully registered custom scheme: {} with extended options",
                scheme
            );
        }
        Ok(())
    }
}

/// Set native ArkWeb scheme handlers for a specific WebView.
///
/// Reads `registered_schemes` from the WebView's effective options to determine
/// which schemes need native ArkWeb handlers. Skips if no schemes are registered
/// (e.g. browser-relaxed mode).
pub fn set_webview_scheme_handler(webtag: &WebTag) -> NapiResult<()> {
    let webview = find_webview(webtag).ok_or_else(|| {
        napi_ohos::Error::new(
            napi_ohos::Status::GenericFailure,
            format!("WebView not found for tag: {}", webtag),
        )
    })?;

    let schemes = &webview.effective_options().registered_schemes;
    if schemes.is_empty() {
        log::info!(
            "No registered schemes for web_tag={}, skipping native handler setup",
            webtag
        );
        return Ok(());
    }

    let webtag_cstr_for_set = CString::new(webtag.as_str()).unwrap();
    let mut results: Vec<(String, bool)> = Vec::new();

    for scheme_name in schemes {
        unsafe {
            let mut handler: *mut ArkWeb_SchemeHandler = std::ptr::null_mut();
            OH_ArkWeb_CreateSchemeHandler(&mut handler);

            // Store webtag as user data
            let webtag_cstr = CString::new(webtag.as_str()).unwrap();
            let webtag_ptr = webtag_cstr.into_raw();
            OH_ArkWebSchemeHandler_SetUserData(handler, webtag_ptr as *mut std::ffi::c_void);

            // Set callbacks
            OH_ArkWebSchemeHandler_SetOnRequestStart(handler, Some(on_lx_request_start));
            OH_ArkWebSchemeHandler_SetOnRequestStop(handler, Some(on_lx_request_stop));

            // Register handler for this scheme on this WebView
            let scheme_cstr = CString::new(scheme_name.as_str()).unwrap();
            let success = OH_ArkWeb_SetSchemeHandler(
                scheme_cstr.as_ptr(),
                webtag_cstr_for_set.as_ptr(),
                handler,
            );

            if success {
                webview.inner.track_scheme_handler(handler);
            } else {
                log::error!(
                    "Failed to set {}:// scheme handler for web_tag: {}",
                    scheme_name,
                    webtag
                );
                cleanup_scheme_handler(handler);
                // Fail hard only for custom schemes (lx); HTTPS failure is non-fatal
                if scheme_name != "https" {
                    return Err(napi_ohos::Error::new(
                        napi_ohos::Status::GenericFailure,
                        format!(
                            "Failed to set {}:// scheme handler for web_tag: {}",
                            scheme_name, webtag
                        ),
                    ));
                }
            }
            results.push((scheme_name.clone(), success));
        }
    }

    let summary: Vec<String> = results
        .iter()
        .map(|(s, ok)| format!("{}: {}", s, ok))
        .collect();
    log::info!(
        "Scheme handlers for web_tag={}: [{}]",
        webtag,
        summary.join(", ")
    );
    Ok(())
}
