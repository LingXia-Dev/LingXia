use crate::webview::{WebTag, find_webview_by_tag};
use miniapp::{self, LxAppDelegate};
use napi_ohos::Result as NapiResult;
use ohos_web_sys::*;
use std::ffi::{CStr, CString};
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
        "Processing request: {} {} for app_id: {}",
        method,
        url,
        app_id
    );

    // Build HTTP request to check if miniapp wants to handle it
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

    // Ask miniapp if it wants to handle this request
    let miniapp = miniapp::get(app_id.to_string());
    if let Some(http_response) = miniapp.handle_request(http_request) {
        unsafe {
            *intercept = true;
            send_response(resource_handler, http_response);
        }
    }
}

/// Send a successful response
unsafe fn send_response(
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
        // Get and free the user data (app_id CString)
        let user_data = OH_ArkWebSchemeHandler_GetUserData(scheme_handler);
        if !user_data.is_null() {
            // Reconstruct CString from raw pointer to properly free it
            let _app_id_cstr = CString::from_raw(user_data as *mut c_char);
            // CString will be automatically dropped here, freeing the memory
        }

        // Destroy the scheme handler
        OH_ArkWeb_DestroySchemeHandler(scheme_handler);
    }
}

/// Register custom schemes globally (called once during lxapp_init)
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
    // Extract app_id from webtag
    let app_id = webtag.extract_appid();

    // Get the WebView instance to track scheme handlers
    let webview = find_webview_by_tag(webtag).ok_or_else(|| {
        napi_ohos::Error::new(
            napi_ohos::Status::GenericFailure,
            format!("WebView not found for tag: {}", webtag),
        )
    })?;

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

        if !lx_success {
            log::error!("Failed to set lx:// scheme handler for web_tag: {}", webtag);
            cleanup_scheme_handler(lx_scheme_handler);
            return Err(napi_ohos::Error::new(
                napi_ohos::Status::GenericFailure,
                format!("Failed to set lx:// scheme handler for web_tag: {}", webtag),
            ));
        }

        // Track the lx scheme handler for cleanup
        webview.track_scheme_handler(lx_scheme_handler);

        // Create scheme handler for https://
        let mut https_scheme_handler: *mut ArkWeb_SchemeHandler = std::ptr::null_mut();
        OH_ArkWeb_CreateSchemeHandler(&mut https_scheme_handler);

        let app_id_cstr2 = CString::new(app_id.clone()).unwrap();
        let app_id_ptr2 = app_id_cstr2.into_raw(); // Transfer ownership to raw pointer
        OH_ArkWebSchemeHandler_SetUserData(
            https_scheme_handler,
            app_id_ptr2 as *mut std::ffi::c_void,
        );

        // Set callbacks (same callbacks as lx://)
        OH_ArkWebSchemeHandler_SetOnRequestStart(https_scheme_handler, Some(on_lx_request_start));
        OH_ArkWebSchemeHandler_SetOnRequestStop(https_scheme_handler, Some(on_lx_request_stop));

        // Register https:// handler for this WebView specifically
        let https_scheme_cstr = CString::new("https").unwrap();
        let https_success = OH_ArkWeb_SetSchemeHandler(
            https_scheme_cstr.as_ptr(),
            webtag_cstr.as_ptr(),
            https_scheme_handler,
        );

        if !https_success {
            log::error!(
                "Failed to set https:// scheme handler for web_tag: {}",
                webtag
            );
            cleanup_scheme_handler(https_scheme_handler);
            // Don't fail completely if https handler fails, lx:// is more critical
            log::warn!("Continuing without https:// scheme handler");
        } else {
            // Track the https scheme handler for cleanup only if successful
            webview.track_scheme_handler(https_scheme_handler);
        }

        log::info!(
            "Successfully set scheme handlers for web_tag: {} (lx: {}, https: {})",
            webtag,
            lx_success,
            https_success
        );
        Ok(())
    }
}
