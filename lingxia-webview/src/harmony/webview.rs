use crate::harmony::schemehandler::set_webview_scheme_handler;
use crate::harmony::tsfn::{call_arkts, call_arkts_with_callback};
use crate::webview::{WebTag, find_webview_by_tag};
use lxapp::log::LogLevel;
use lxapp::{LxAppDelegate, LxAppError, WebViewController};
use ohos_web_sys::*;

use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char, c_void};
use std::sync::OnceLock;

/// Wrapper for API pointers to make them Send + Sync
#[derive(Debug, Clone, Copy)]
struct ApiPtr<T>(*const T);
unsafe impl<T> Send for ApiPtr<T> {}
unsafe impl<T> Sync for ApiPtr<T> {}

/// Global cached APIs - initialized once and reused
static PORT_API: OnceLock<ApiPtr<ArkWeb_WebMessagePortAPI>> = OnceLock::new();
static MESSAGE_API: OnceLock<ApiPtr<ArkWeb_WebMessageAPI>> = OnceLock::new();

/// Get cached WebMessagePort API
fn get_port_api() -> Result<&'static ArkWeb_WebMessagePortAPI, LxAppError> {
    let api_ptr = PORT_API.get_or_init(|| unsafe {
        ApiPtr(
            OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_WEB_MESSAGE_PORT)
                as *const ArkWeb_WebMessagePortAPI,
        )
    });

    if api_ptr.0.is_null() {
        Err(LxAppError::WebView(
            "Failed to get WebMessagePort API".to_string(),
        ))
    } else {
        Ok(unsafe { &*api_ptr.0 })
    }
}

/// Get cached WebMessage API
fn get_message_api() -> Result<&'static ArkWeb_WebMessageAPI, LxAppError> {
    let api_ptr = MESSAGE_API.get_or_init(|| unsafe {
        ApiPtr(
            OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_WEB_MESSAGE)
                as *const ArkWeb_WebMessageAPI,
        )
    });

    if api_ptr.0.is_null() {
        Err(LxAppError::WebView(
            "Failed to get WebMessage API".to_string(),
        ))
    } else {
        Ok(unsafe { &*api_ptr.0 })
    }
}

#[derive(Debug)]
pub struct WebViewInner {
    webtag: WebTag,
    native_port: RefCell<Option<*mut ArkWeb_WebMessagePort>>,
    console_port: RefCell<Option<*mut ArkWeb_WebMessagePort>>,
    webview_native_port: RefCell<Option<*mut ArkWeb_WebMessagePort>>,
    webview_console_port: RefCell<Option<*mut ArkWeb_WebMessagePort>>,
    // Store user_data pointers for cleanup
    user_data_ptrs: RefCell<Vec<*mut c_void>>,
    // Store scheme handlers for cleanup
    scheme_handlers: RefCell<Vec<*mut ohos_web_sys::ArkWeb_SchemeHandler>>,
}

unsafe impl Send for WebViewInner {}
unsafe impl Sync for WebViewInner {}

/// WebMessage port types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortType {
    Console,
    Message,
}

impl std::fmt::Display for PortType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PortType::Console => write!(f, "ConsolePort"),
            PortType::Message => write!(f, "MessagePort"),
        }
    }
}

/// Register LingXiaProxy for a specific webtag
fn register_proxy_for_webtag(webtag: &WebTag) -> Result<(), LxAppError> {
    unsafe {
        let webtag_cstr = CString::new(webtag.as_str()).unwrap();

        let controller_api =
            OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_CONTROLLER);
        if controller_api.is_null() {
            return Err(LxAppError::WebView(
                "Failed to get Controller API".to_string(),
            ));
        }
        let controller = &*(controller_api as *const ArkWeb_ControllerAPI);

        // Create proxy data pointing to webtag string
        let webtag_string = webtag.as_str().to_string();
        let proxy_data = Box::new(webtag_string);
        let proxy_data_ptr = Box::into_raw(proxy_data) as *mut std::ffi::c_void;

        // Track this allocation for cleanup in the WebView
        if let Some(webview) = find_webview_by_tag(webtag) {
            webview.track_user_data(proxy_data_ptr);
        }

        let object_name_cstr = CString::new("LingXiaProxy").unwrap();
        let get_port_cstr = CString::new("getPort").unwrap();

        let method_list = [ArkWeb_ProxyMethod {
            methodName: get_port_cstr.as_ptr(),
            callback: Some(get_port_callback),
            userData: proxy_data_ptr,
        }];

        let proxy_object = ArkWeb_ProxyObject {
            objName: object_name_cstr.as_ptr(),
            methodList: method_list.as_ptr(),
            size: method_list.len(),
        };

        if let Some(register_proxy) = controller.registerJavaScriptProxy {
            register_proxy(webtag_cstr.as_ptr(), &proxy_object);
            log::info!("Registered LingXiaProxy for {}", webtag.as_str());
            Ok(())
        } else {
            // Cleanup manually since registration failed
            let _ = Box::from_raw(proxy_data_ptr as *mut String);
            Err(LxAppError::WebView(
                "registerJavaScriptProxy not available".to_string(),
            ))
        }
    }
}

/// Get port callback - handles LingXiaProxy.getPort(type) calls
unsafe extern "C" fn get_port_callback(
    _web_tag: *const std::ffi::c_char,
    bridge_data: *const ArkWeb_JavaScriptBridgeData,
    data_count: usize,
    user_data: *mut std::ffi::c_void,
) {
    if user_data.is_null() || data_count < 1 || bridge_data.is_null() {
        return;
    }

    unsafe {
        let webtag_string = &*(user_data as *const String);
        let webtag = WebTag::from(webtag_string.as_str());
        let type_data = &*bridge_data.offset(0);

        if let Some(port_type_str) = extract_string_from_bridge_data(type_data) {
            // Trigger sending the appropriate port
            match port_type_str.as_str() {
                "ConsolePort" => {
                    if let Err(e) = send_port_to_webview_for_webtag(&webtag, PortType::Console) {
                        log::error!("Failed to send console port: {}", e);
                    }
                }
                "LingXiaPort" => {
                    if let Err(e) = send_port_to_webview_for_webtag(&webtag, PortType::Message) {
                        log::error!("Failed to send message port: {}", e);
                    }
                }
                _ => {
                    log::warn!("Unknown port type: {}", port_type_str);
                }
            }
        }
    }
}

/// Extract string from bridge data
fn extract_string_from_bridge_data(data: &ArkWeb_JavaScriptBridgeData) -> Option<String> {
    unsafe {
        if !data.buffer.is_null() && data.size > 0 {
            let bytes = std::slice::from_raw_parts(data.buffer as *const u8, data.size);
            std::str::from_utf8(bytes).ok().map(|s| s.to_string())
        } else {
            None
        }
    }
}

/// Send port to WebView for webtag (unified function)
pub fn send_port_to_webview_for_webtag(
    webtag: &WebTag,
    port_type: PortType,
) -> Result<(), LxAppError> {
    let webview = find_webview_by_tag(webtag).ok_or_else(|| {
        LxAppError::WebView(format!("WebView not found for webtag: {}", webtag.as_str()))
    })?;

    webview.send_port(port_type)
}

impl WebViewInner {
    /// Create a WebView instance
    pub fn create(appid: &str, path: &str) -> Result<Self, LxAppError> {
        let webtag = WebTag::new(appid, path);

        // Create WebView instance
        let webview_inner = WebViewInner {
            webtag: webtag.clone(),
            native_port: RefCell::new(None),
            console_port: RefCell::new(None),
            webview_native_port: RefCell::new(None),
            webview_console_port: RefCell::new(None),
            user_data_ptrs: RefCell::new(Vec::new()),
            scheme_handlers: RefCell::new(Vec::new()),
        };

        // Call ArkTS to create WebView controller with callback for proper timing
        let webtag_for_callback = webtag.clone();
        call_arkts_with_callback("createWebViewController", &[webtag.as_str()], move || {
            // Set scheme handler after WebView is created
            if let Err(e) = set_webview_scheme_handler(&webtag_for_callback) {
                log::error!(
                    "Failed to set scheme handler for {}: {}",
                    webtag_for_callback.as_str(),
                    e
                );
            }

            // Register WebView callbacks after WebView is fully created
            if let Err(e) = register_webview_callbacks(&webtag_for_callback) {
                log::error!(
                    "WebView callback registration failed for {}: {:?}",
                    webtag_for_callback.as_str(),
                    e
                );
            } else {
                log::info!(
                    "WebView callbacks registered for {}",
                    webtag_for_callback.as_str()
                );
            }

            // why call here ?
            // for init route page of home lxapp, it has no change to trigger onControllerAttached
            // and it only be workable for init route page.
            if let Err(e) = register_proxy_for_webtag(&webtag_for_callback) {
                log::error!(
                    "Failed to register LingXiaProxy for home page {}: {}",
                    webtag_for_callback.as_str(),
                    e
                );
            }

            log::info!(
                "Scheme handler and callbacks setup completed for {}",
                webtag_for_callback.as_str()
            );
        })?;

        Ok(webview_inner)
    }

    /// Add a user_data pointer for cleanup
    fn track_user_data(&self, ptr: *mut c_void) {
        self.user_data_ptrs.borrow_mut().push(ptr);
        log::debug!("Tracked user_data for {}: {:?}", self.webtag.as_str(), ptr);
    }

    /// Track scheme handler for cleanup
    pub fn track_scheme_handler(&self, handler: *mut ohos_web_sys::ArkWeb_SchemeHandler) {
        self.scheme_handlers.borrow_mut().push(handler);
    }

    /// Cleanup all tracked user_data
    fn cleanup_user_data(&self) {
        let ptrs = self
            .user_data_ptrs
            .borrow_mut()
            .drain(..)
            .collect::<Vec<_>>();
        let count = ptrs.len();
        for ptr in ptrs {
            unsafe {
                let _cleanup = Box::from_raw(ptr as *mut String);
            }
        }
        if count > 0 {
            log::info!(
                "Cleaned up {} user_data pointers for {}",
                count,
                self.webtag.as_str()
            );
        }
    }

    /// Cleanup all tracked scheme handlers
    fn cleanup_scheme_handlers(&self) {
        let handlers = self
            .scheme_handlers
            .borrow_mut()
            .drain(..)
            .collect::<Vec<_>>();
        let count = handlers.len();
        for handler in handlers {
            unsafe {
                super::schemehandler::cleanup_scheme_handler(handler);
            }
        }
        if count > 0 {
            log::info!(
                "Cleaned up {} scheme handlers for {}",
                count,
                self.webtag.as_str()
            );
        }
    }

    /// Send port to WebView
    pub fn send_port(&self, port_type: PortType) -> Result<(), LxAppError> {
        unsafe {
            let webtag_cstr = CString::new(self.webtag.as_str()).unwrap();
            let controller_api =
                OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_CONTROLLER);
            if controller_api.is_null() {
                return Err(LxAppError::WebView(
                    "Failed to get Controller API".to_string(),
                ));
            }
            let controller = &*(controller_api as *const ArkWeb_ControllerAPI);

            let (port, message, port_name) = match port_type {
                PortType::Console => (
                    self.webview_console_port.borrow_mut().take(),
                    "LingXia-console-init",
                    "console",
                ),
                PortType::Message => (
                    self.webview_native_port.borrow_mut().take(),
                    "LingXia-port-init",
                    "message",
                ),
            };

            if let Some(webview_port) = port {
                let result = controller.postWebMessage.ok_or_else(|| {
                    LxAppError::WebView("postWebMessage not available".to_string())
                })?(
                    webtag_cstr.as_ptr(),
                    CString::new(message).unwrap().as_ptr(),
                    [webview_port].as_mut_ptr(),
                    1,
                    CString::new("*").unwrap().as_ptr(),
                );

                if result == 0 {
                    log::info!(
                        "Successfully sent {} port to WebView for {}",
                        port_name,
                        self.webtag.as_str()
                    );
                    Ok(())
                } else {
                    Err(LxAppError::WebView(format!(
                        "Failed to send {} port: error {}",
                        port_name, result
                    )))
                }
            } else {
                Err(LxAppError::WebView(format!(
                    "{} port not available",
                    port_name
                )))
            }
        }
    }
}

impl WebViewController for WebViewInner {
    fn load_url(&self, url: String) -> Result<(), LxAppError> {
        call_arkts("loadUrl", &[self.webtag.as_str(), &url])
    }

    fn load_data(
        &self,
        data: String,
        base_url: String,
        history_url: Option<String>,
    ) -> Result<(), LxAppError> {
        unsafe {
            let webtag_cstr = CString::new(self.webtag.as_str()).unwrap();
            let data_cstr = CString::new(data.clone()).unwrap();
            let base_url_cstr = CString::new(base_url.clone()).unwrap();

            // Use history_url if provided, otherwise use base_url
            let history_url_str = history_url.unwrap_or(base_url.clone());
            let history_url_cstr = CString::new(history_url_str).unwrap();

            // Use the native HarmonyOS OH_NativeArkWeb_LoadData function
            let result = OH_NativeArkWeb_LoadData(
                webtag_cstr.as_ptr(),
                data_cstr.as_ptr(),
                CString::new("text/html").unwrap().as_ptr(), // MIME type: text/html
                CString::new("UTF-8").unwrap().as_ptr(),     // Encoding: UTF-8
                base_url_cstr.as_ptr(),
                history_url_cstr.as_ptr(),
            );

            if result == ArkWeb_ErrorCode_ARKWEB_SUCCESS {
                log::info!(
                    "Successfully loaded data into WebView {} with base URL: {}",
                    self.webtag.as_str(),
                    base_url
                );
                Ok(())
            } else {
                Err(LxAppError::WebView(format!(
                    "Failed to load data into WebView: error code {:?}",
                    result
                )))
            }
        }
    }

    fn evaluate_javascript(&self, js: String) -> Result<(), LxAppError> {
        unsafe {
            let web_tag_cstr = CString::new(self.webtag.as_str()).unwrap();

            // Get Controller API
            let controller_api =
                OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_CONTROLLER);
            if controller_api.is_null() {
                return Err(LxAppError::WebView(
                    "Failed to get Controller API".to_string(),
                ));
            }
            let controller = &*(controller_api as *const ArkWeb_ControllerAPI);

            // Execute the actual JavaScript
            let js_cstr = CString::new(js.clone()).unwrap();
            let js_object = ArkWeb_JavaScriptObject {
                buffer: js_cstr.as_ptr() as *mut u8,
                size: js.len(),
                callback: None,
                userData: std::ptr::null_mut(),
            };

            if let Some(run_js) = controller.runJavaScript {
                run_js(web_tag_cstr.as_ptr(), &js_object);
                log::info!(
                    "Successfully submitted JavaScript for evaluation in WebView {}",
                    self.webtag
                );
                Ok(())
            } else {
                Err(LxAppError::WebView(
                    "runJavaScript function not available".to_string(),
                ))
            }
        }
    }

    fn clear_browsing_data(&self) -> Result<(), LxAppError> {
        call_arkts("clearBrowsingData", &[self.webtag.as_str()])
    }

    fn set_user_agent(&self, ua: String) -> Result<(), LxAppError> {
        call_arkts("setUserAgent", &[self.webtag.as_str(), &ua])
    }

    fn set_scroll_listener_enabled(
        &self,
        enabled: bool,
        _throttle_ms: Option<u64>,
    ) -> Result<(), LxAppError> {
        call_arkts(
            "setScrollListenerEnabled",
            &[self.webtag.as_str(), &enabled.to_string()],
        )
    }

    fn post_message(&self, message: String) -> Result<(), LxAppError> {
        // Access native_port directly through RefCell
        let port = *self.native_port.borrow();

        if let Some(port) = port {
            unsafe {
                let webtag_cstr = CString::new(self.webtag.as_str()).unwrap();
                let message_cstr = CString::new(message.clone()).unwrap();

                // Get cached APIs
                let port_api = get_port_api()?;
                let message_api = get_message_api()?;

                // Create WebMessage
                let create_fn = message_api.createWebMessage.ok_or_else(|| {
                    LxAppError::WebView("createWebMessage not available".to_string())
                })?;
                let web_message = create_fn();

                if web_message.is_null() {
                    return Err(LxAppError::WebView(
                        "Failed to create WebMessage".to_string(),
                    ));
                }

                // Set message type to string
                if let Some(set_type) = message_api.setType {
                    set_type(web_message, ArkWeb_WebMessageType_ARKWEB_STRING);
                }

                // Set message data
                if let Some(set_data) = message_api.setData {
                    set_data(
                        web_message,
                        message_cstr.as_ptr() as *mut std::ffi::c_void,
                        message.len() + 1,
                    );
                }

                // Post message using the existing port
                let result = port_api.postMessage.ok_or_else(|| {
                    LxAppError::WebView("postMessage not available".to_string())
                })?(port, webtag_cstr.as_ptr(), web_message);

                // Clean up the created message
                if let Some(destroy_message) = message_api.destroyWebMessage {
                    let mut msg_ptr = web_message;
                    destroy_message(&mut msg_ptr as *mut *mut ArkWeb_WebMessage);
                }

                if result != ArkWeb_ErrorCode_ARKWEB_SUCCESS {
                    return Err(LxAppError::WebView(format!(
                        "Failed to post message via port: error {:?}",
                        result
                    )));
                }

                // log::info!(
                //     "Successfully posted message via native WebMessagePort for {}",
                //     self.webtag
                // );
                Ok(())
            }
        } else {
            return Err(LxAppError::WebView(
                "No native port available for post_message".to_string(),
            ));
        }
    }
}

impl Drop for WebViewInner {
    fn drop(&mut self) {
        // Cleanup all tracked scheme handlers first
        self.cleanup_scheme_handlers();

        // Cleanup all tracked user_data
        self.cleanup_user_data();

        if let Err(e) = call_arkts("destroyWebViewController", &[self.webtag.as_str()]) {
            log::error!("Failed to destroy WebView controller: {:?}", e);
        }
    }
}

/// Register WebView lifecycle callbacks with shared user_data pointer
fn register_webview_callbacks(webtag: &WebTag) -> Result<(), LxAppError> {
    unsafe {
        let webtag_cstr = CString::new(webtag.as_str()).unwrap();

        // Create a single shared user_data for all callbacks (like the original implementation)
        let webtag_string = webtag.as_str().to_string();
        let user_data = Box::into_raw(Box::new(webtag_string)) as *mut c_void;

        // Track this user_data for cleanup (but don't double-cleanup in on_destroy_callback)
        if let Some(webview) = find_webview_by_tag(webtag) {
            webview.track_user_data(user_data);
        }

        // Get the ArkWeb_ComponentAPI using the correct API
        let component_api =
            OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_COMPONENT);
        if component_api.is_null() {
            return Err(LxAppError::WebView(
                "Failed to get ArkWeb_ComponentAPI".to_string(),
            ));
        }

        let api = &*(component_api as *const ArkWeb_ComponentAPI);

        // Register all callbacks with the same user_data pointer (critical for HarmonyOS)
        if let Some(on_controller_attached) = api.onControllerAttached {
            on_controller_attached(
                webtag_cstr.as_ptr(),
                Some(on_controller_attached_callback),
                user_data,
            );
        }

        if let Some(on_page_begin) = api.onPageBegin {
            on_page_begin(
                webtag_cstr.as_ptr(),
                Some(on_page_begin_callback),
                user_data,
            );
        }

        if let Some(on_page_end) = api.onPageEnd {
            on_page_end(webtag_cstr.as_ptr(), Some(on_page_end_callback), user_data);
        }

        if let Some(on_destroy) = api.onDestroy {
            on_destroy(webtag_cstr.as_ptr(), Some(on_destroy_callback), user_data);
        }

        Ok(())
    }
}

// WebView lifecycle callback functions
extern "C" fn on_controller_attached_callback(web_tag: *const c_char, user_data: *mut c_void) {
    if let Ok(webtag_str) = unsafe { CStr::from_ptr(web_tag).to_str() } {
        log::info!("WebView controller attached: {}", webtag_str);

        // Register LingXiaProxy for all pages in onAttach as test
        if !user_data.is_null() {
            let webtag_string = unsafe { &*(user_data as *const String) };
            let webtag = WebTag::from(webtag_string.as_str());
            if let Err(e) = register_proxy_for_webtag(&webtag) {
                log::error!(
                    "Failed to register LingXiaProxy in onAttach for {}: {}",
                    webtag_str,
                    e
                );
            } else {
                log::info!(
                    "Registered LingXiaProxy in onAttach callback: {}",
                    webtag_str
                );
            }
        }
    }
}

extern "C" fn on_page_begin_callback(web_tag: *const c_char, user_data: *mut c_void) {
    if let Ok(webtag_str) = unsafe { CStr::from_ptr(web_tag).to_str() } {
        log::info!("Page begin loading: {}", webtag_str);

        let webtag_string = unsafe { &*(user_data as *const String) };
        let webtag = WebTag::from(webtag_string.as_str());

        // Setup ports when page begins loading - WebView is fully ready now
        if !user_data.is_null() {
            // Setup console and message ports
            if let Err(e) = inject_console_script(&webtag) {
                log::error!("Failed to inject console script for {}: {}", webtag_str, e);
            }
            if let Err(e) = setup_webmessage_port_for_webtag(
                &webtag,
                PortType::Console,
                on_console_message_received,
            ) {
                log::error!("Failed to setup console port for {}: {}", webtag_str, e);
            }
            if let Err(e) = setup_webmessage_port_for_webtag(
                &webtag,
                PortType::Message,
                on_web_message_received,
            ) {
                log::error!("Failed to setup message port for {}: {}", webtag_str, e);
            }
        }

        let (appid, path) = webtag.extract_parts();
        let lxapp = lxapp::get(appid);
        lxapp.on_page_started(path);
    }
}

extern "C" fn on_page_end_callback(web_tag: *const c_char, _user_data: *mut c_void) {
    if let Ok(webtag) = unsafe { CStr::from_ptr(web_tag).to_str() } {
        log::info!("Page end loading: {}", webtag);

        // Extract app_id and path from webtag
        let webtag = WebTag::from(webtag);
        let (appid, path) = webtag.extract_parts();
        let lxapp = lxapp::get(appid);
        lxapp.on_page_finished(path);
    }
}

extern "C" fn on_destroy_callback(web_tag: *const c_char, _user_data: *mut c_void) {
    if let Ok(webtag_str) = unsafe { CStr::from_ptr(web_tag).to_str() } {
        log::info!("WebView destroyed: {}", webtag_str);
    }
}

/// Generic WebMessage port setup function for webtag
fn setup_webmessage_port_for_webtag(
    webtag: &WebTag,
    port_type: PortType,
    callback_fn: extern "C" fn(
        *const c_char,
        *mut ArkWeb_WebMessagePort,
        *mut ArkWeb_WebMessage,
        *mut c_void,
    ),
) -> Result<(), LxAppError> {
    unsafe {
        let webtag_cstr = CString::new(webtag.as_str()).unwrap();

        // Get APIs
        let controller_api =
            OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_CONTROLLER);
        let port_api =
            OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_WEB_MESSAGE_PORT);

        let controller = &*(controller_api as *const ArkWeb_ControllerAPI);
        let port_api_struct = &*(port_api as *const ArkWeb_WebMessagePortAPI);

        let mut size = 0;
        let ports = controller.createWebMessagePorts.ok_or_else(|| {
            LxAppError::WebView(format!(
                "createWebMessagePorts not available for {:?}",
                port_type
            ))
        })?(webtag_cstr.as_ptr(), &mut size);

        if ports.is_null() || size < 2 {
            log::error!(
                "Failed to create {:?} WebMessage ports for {}: ports={:?}, size={}",
                port_type,
                webtag.as_str(),
                ports,
                size
            );
            return Err(LxAppError::WebView(format!(
                "Failed to create {:?} WebMessage ports",
                port_type
            )));
        }

        let port1 = *ports.offset(0); // Native side port
        let port2 = *ports.offset(1); // WebView side port

        // Store both ports in WebViewInner
        let webview = crate::find_webview_by_tag(webtag)
            .ok_or_else(|| LxAppError::WebView("WebView not found".to_string()))?;

        match port_type {
            PortType::Message => {
                *webview.native_port.borrow_mut() = Some(port1);
                *webview.webview_native_port.borrow_mut() = Some(port2);
            }
            PortType::Console => {
                *webview.console_port.borrow_mut() = Some(port1);
                *webview.webview_console_port.borrow_mut() = Some(port2);
            }
        }

        // Set message event handler
        let webtag_string = webtag.as_str().to_string();
        let user_data_ptr = Box::into_raw(Box::new(webtag_string)) as *mut c_void;

        // Track this allocation for cleanup in the WebView
        webview.track_user_data(user_data_ptr);

        if let Some(set_handler) = port_api_struct.setMessageEventHandler {
            set_handler(
                port1,
                webtag_cstr.as_ptr(),
                Some(callback_fn),
                user_data_ptr,
            );
        }

        log::info!("Setup {} port for {}", port_type, webtag);
        Ok(())
    }
}

/// Inject console interception script
fn inject_console_script(webtag: &WebTag) -> Result<(), LxAppError> {
    let console_script = r#"
        (function() {
            const orig = {
                log: console.log,
                error: console.error,
                warn: console.warn,
                info: console.info
            };
            let port = null;

            function getPort() {
                if (window.LingXiaProxy?.getPort) {
                    window.LingXiaProxy.getPort('ConsolePort');
                    window.addEventListener('message', (e) => {
                        if (e.data === 'LingXia-console-init') {
                            port = e.ports[0];
                        }
                    });
                } else {
                    setTimeout(getPort, 50);
                }
            }

            function send(level, args) {
                if (port) {
                    const msg = args.map(a => typeof a === 'object' ? JSON.stringify(a) : String(a)).join(' ');
                    port.postMessage(JSON.stringify({level, message: msg}));
                }
            }

            ['log', 'error', 'warn', 'info'].forEach(level => {
                console[level] = function(...args) {
                    send(level, args);
                    orig[level].apply(console, args);
                };
            });

            getPort();
        })();
    "#;

    unsafe {
        let webtag_cstr = CString::new(webtag.as_str()).unwrap();

        // Get Controller API
        let controller_api =
            OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_CONTROLLER);
        if controller_api.is_null() {
            return Err(LxAppError::WebView(
                "Failed to get Controller API for console script".to_string(),
            ));
        }
        let controller = &*(controller_api as *const ArkWeb_ControllerAPI);

        // Execute console script
        let script_cstr = CString::new(console_script).unwrap();
        let script_object = ArkWeb_JavaScriptObject {
            buffer: script_cstr.as_ptr() as *mut u8,
            size: console_script.len(),
            callback: None, // No callback needed for console script
            userData: std::ptr::null_mut(),
        };

        if let Some(run_js) = controller.runJavaScript {
            run_js(webtag_cstr.as_ptr(), &script_object);
            Ok(())
        } else {
            Err(LxAppError::WebView(
                "runJavaScript not available for console script".to_string(),
            ))
        }
    }
}

/// WebMessage callback
extern "C" fn on_web_message_received(
    web_tag: *const c_char,
    _port: *mut ArkWeb_WebMessagePort,
    message: *mut ArkWeb_WebMessage,
    user_data: *mut c_void,
) {
    let Ok(webtag) = (unsafe { CStr::from_ptr(web_tag).to_str() }) else {
        log::error!("Failed to parse web_tag");
        return;
    };

    if user_data.is_null() || message.is_null() {
        log::error!("user_data or message is null for {}", webtag);
        return;
    }

    // Get webtag string from user_data
    let webtag_string = unsafe { &*(user_data as *const String) };
    let webtag = WebTag::from(webtag_string.as_str());
    let (appid, path) = webtag.extract_parts();

    // Extract message data
    unsafe {
        let message_api =
            OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_WEB_MESSAGE);
        if message_api.is_null() {
            return;
        }

        let api = &*(message_api as *const ArkWeb_WebMessageAPI);
        let Some(get_data) = api.getData else { return };

        let mut data_length: usize = 0;
        let data_ptr = get_data(message, &mut data_length);
        if data_ptr.is_null() || data_length == 0 {
            return;
        }

        let data_slice = std::slice::from_raw_parts(data_ptr as *const u8, data_length);
        let Ok(msg_str) = std::str::from_utf8(data_slice) else {
            return;
        };

        //log::info!("WebMessage received from {}: {}", webtag, msg_str);

        // Forward to lxapp logic layer
        let lxapp = lxapp::get(appid.to_string());
        lxapp.handle_post_message(path.to_string(), msg_str.to_string());
    }
}

/// Console WebMessage callback
extern "C" fn on_console_message_received(
    web_tag: *const c_char,
    _port: *mut ArkWeb_WebMessagePort,
    message: *mut ArkWeb_WebMessage,
    _user_data: *mut c_void,
) {
    let Ok(webtag) = (unsafe { CStr::from_ptr(web_tag).to_str() }) else {
        return;
    };
    if message.is_null() {
        return;
    }

    // Extract message data
    unsafe {
        let message_api =
            OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_WEB_MESSAGE);
        if message_api.is_null() {
            return;
        }

        let api = &*(message_api as *const ArkWeb_WebMessageAPI);
        let Some(get_data) = api.getData else { return };

        let mut data_length: usize = 0;
        let data_ptr = get_data(message, &mut data_length);
        if data_ptr.is_null() || data_length == 0 {
            return;
        }

        let data_slice = std::slice::from_raw_parts(data_ptr as *const u8, data_length);
        let Ok(msg_str) = std::str::from_utf8(data_slice) else {
            return;
        };

        // Parse console message
        if let Ok(console_msg) = serde_json::from_str::<serde_json::Value>(msg_str) {
            if let (Some(level), Some(console_message)) = (
                console_msg.get("level").and_then(|v| v.as_str()),
                console_msg.get("message").and_then(|v| v.as_str()),
            ) {
                // Extract appid and path from webtag
                let webtag = WebTag::from(webtag);
                let (appid, path) = webtag.extract_parts();
                // Convert log level for lxapp crate
                let log_level = match level {
                    "error" => LogLevel::Error,
                    "warn" => LogLevel::Warn,
                    "info" => LogLevel::Info,
                    "debug" => LogLevel::Debug,
                    _ => LogLevel::Info,
                };

                // Forward to lxapp crate for logging only
                let lxapp = lxapp::get(appid);
                lxapp.log(&path, log_level, console_message);
            }
        }
    }
}
