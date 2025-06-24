use crate::harmony::webview::WebTag;
use miniapp::{self, AppUiDelegate};
use napi_ohos::Result as NapiResult;
use ohos_web_sys::*;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::ptr;

/// Callback function for handling lx:// scheme requests
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

    // Get app_id from user data
    let user_data = unsafe { OH_ArkWebSchemeHandler_GetUserData(scheme_handler) };
    if user_data.is_null() {
        log::error!("No user data found in scheme handler");
        return;
    }

    let app_id_cstr = unsafe { CStr::from_ptr(user_data as *const c_char) };
    let app_id = match app_id_cstr.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => {
            log::error!("Invalid app_id in user data");
            return;
        }
    };

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
        "Processing lx:// request: {} {} for app_id: {}",
        method,
        url,
        app_id
    );

    // Set intercept to true to handle this request
    unsafe {
        *intercept = true;
    }

    // Handle the request directly
    unsafe {
        process_lx_request(&app_id, &url, &method, resource_handler);
    }
}

/// Process the actual lx:// request and send response
unsafe fn process_lx_request(
    app_id: &str,
    url: &str,
    method: &str,
    resource_handler: *const ArkWeb_ResourceHandler,
) {
    // Build HTTP request for miniapp
    let http_request = match http::Request::builder()
        .method(method)
        .uri(url)
        .body(Vec::new())
    {
        Ok(req) => req,
        Err(e) => {
            log::error!("Failed to build HTTP request: {}", e);
            unsafe {
                send_lx_error_response(resource_handler, 500, "Failed to build HTTP request");
            }
            return;
        }
    };

    // Forward request to miniapp
    let miniapp = miniapp::get(app_id.to_string());
    match miniapp.handle_request(http_request) {
        Some(http_response) => {
            log::info!(
                "Got response from handle_request, status: {}, body length: {}",
                http_response.status(),
                http_response.body().len()
            );
            unsafe {
                send_lx_response(resource_handler, http_response);
            }
        }
        None => {
            log::warn!("handle_request returned None, sending 404");
            unsafe {
                send_lx_error_response(resource_handler, 404, "Not Found");
            }
        }
    }
}

/// Send a successful response
unsafe fn send_lx_response(
    resource_handler: *const ArkWeb_ResourceHandler,
    http_response: http::Response<Vec<u8>>,
) {
    let (parts, body) = http_response.into_parts();

    // Create ArkWeb response
    let mut response: *mut ArkWeb_Response = ptr::null_mut();
    unsafe {
        OH_ArkWeb_CreateResponse(&mut response);
    }

    // Set status code
    unsafe {
        OH_ArkWebResponse_SetStatus(response, parts.status.as_u16() as c_int);
    }

    // Set headers
    for (key, value) in parts.headers.iter() {
        if let Ok(value_str) = value.to_str() {
            if let (Ok(key_cstr), Ok(value_cstr)) =
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
    }

    // Send response headers
    unsafe {
        OH_ArkWebResourceHandler_DidReceiveResponse(resource_handler, response);
    }

    // Send response body
    if !body.is_empty() {
        unsafe {
            OH_ArkWebResourceHandler_DidReceiveData(
                resource_handler,
                body.as_ptr(),
                body.len() as i64,
            );
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

/// Send an error response
unsafe fn send_lx_error_response(
    resource_handler: *const ArkWeb_ResourceHandler,
    status_code: i32,
    error_message: &str,
) {
    // Create ArkWeb response
    let mut response: *mut ArkWeb_Response = ptr::null_mut();
    unsafe {
        OH_ArkWeb_CreateResponse(&mut response);
    }

    // Set status code
    unsafe {
        OH_ArkWebResponse_SetStatus(response, status_code);
    }

    // Set content type
    let content_type_key = CString::new("Content-Type").unwrap();
    let content_type_value = CString::new("text/plain").unwrap();
    unsafe {
        OH_ArkWebResponse_SetHeaderByName(
            response,
            content_type_key.as_ptr(),
            content_type_value.as_ptr(),
            true,
        );
    }

    // Send response headers
    unsafe {
        OH_ArkWebResourceHandler_DidReceiveResponse(resource_handler, response);
    }

    // Send error message as body
    let error_bytes = error_message.as_bytes();
    unsafe {
        OH_ArkWebResourceHandler_DidReceiveData(
            resource_handler,
            error_bytes.as_ptr(),
            error_bytes.len() as i64,
        );
    }

    // Finish the response
    unsafe {
        OH_ArkWebResourceHandler_DidFinish(resource_handler);
    }

    // Clean up
    unsafe {
        OH_ArkWeb_DestroyResponse(response);
    }

    log::error!("SchemeHandler error: {}", error_message);
}

/// Callback function for stopping scheme requests
pub unsafe extern "C" fn on_lx_request_stop(
    _scheme_handler: *const ArkWeb_SchemeHandler,
    resource_request: *const ArkWeb_ResourceRequest,
) {
    log::debug!("Stopped lx:// request");

    // Clean up the resource request
    if !resource_request.is_null() {
        unsafe {
            OH_ArkWebResourceRequest_Destroy(resource_request);
        }
    }
}

/// Register custom schemes globally (called once during miniapp_init)
pub fn register_custom_schemes() -> NapiResult<()> {
    log::info!("Registering custom schemes globally");

    unsafe {
        // Register lx:// scheme globally with more comprehensive options
        let lx_scheme_cstr = CString::new("lx").unwrap();
        // Try more flags: STANDARD | SECURE | CORS_ENABLED | CSP_BYPASSING | FETCH_API
        let options = 1 | 2 | 16 | 32 | 64;
        OH_ArkWeb_RegisterCustomSchemes(lx_scheme_cstr.as_ptr(), options);

        log::info!("Successfully registered custom scheme: lx with extended options");
        Ok(())
    }
}

/// Set scheme handler for a specific WebView (called in WebViewInner::create)
pub fn set_webview_scheme_handler(webtag: &WebTag) -> NapiResult<()> {
    log::info!("Setting scheme handler for WebView: {}", webtag);

    // Extract app_id from webtag
    let app_id = webtag.extract_appid().unwrap();

    unsafe {
        // Create scheme handler for lx://
        let mut lx_scheme_handler: *mut ArkWeb_SchemeHandler = std::ptr::null_mut();
        OH_ArkWeb_CreateSchemeHandler(&mut lx_scheme_handler);

        let app_id_cstr = CString::new(app_id.clone()).unwrap();
        let app_id_ptr = app_id_cstr.into_raw(); // Transfer ownership to raw pointer
        OH_ArkWebSchemeHandler_SetUserData(lx_scheme_handler, app_id_ptr as *mut std::ffi::c_void);

        // Set callbacks
        OH_ArkWebSchemeHandler_SetOnRequestStart(lx_scheme_handler, Some(on_lx_request_start));
        OH_ArkWebSchemeHandler_SetOnRequestStop(lx_scheme_handler, Some(on_lx_request_stop));

        // Register lx:// handler for this WebView specifically
        let lx_scheme_cstr = CString::new("lx").unwrap();
        let webtag_cstr = CString::new(webtag.as_str()).unwrap();
        let lx_success = OH_ArkWeb_SetSchemeHandler(
            lx_scheme_cstr.as_ptr(),
            webtag_cstr.as_ptr(),
            lx_scheme_handler,
        );

        if lx_success {
            log::info!(
                "Successfully set lx:// scheme handler for web_tag: {}",
                webtag
            );
            Ok(())
        } else {
            log::error!("Failed to set lx:// scheme handler for web_tag: {}", webtag);
            OH_ArkWeb_DestroySchemeHandler(lx_scheme_handler);
            Err(napi_ohos::Error::new(
                napi_ohos::Status::GenericFailure,
                format!("Failed to set lx:// scheme handler for web_tag: {}", webtag),
            ))
        }
    }
}
