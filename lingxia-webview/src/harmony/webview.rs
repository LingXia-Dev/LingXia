use crate::harmony::schemehandler::set_webview_scheme_handler;
use crate::harmony::tsfn::call_arkts;
use crate::webview::{WebTag, find_webview, get_webview_delegate, register_webview};
use crate::{LogLevel, WebViewController, WebViewError};
use ohos_web_sys::*;

use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char, c_void};
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::oneshot::Sender;

// Static C strings for proxy object and method names
static LINGXIA_PROXY_NAME: &[u8] = b"LingXiaProxy\0";
static LINGXIA_PROXY_GET_PORT: &[u8] = b"getPort\0";

// Keep proxy method array alive for WebView lifetime
#[repr(C)]
struct ProxyStorage {
    method: Box<[ArkWeb_ProxyMethod; 1]>,
}

/// Wrapper for API pointers to make them Send + Sync
#[derive(Debug, Clone, Copy)]
struct ApiPtr<T>(*const T);
unsafe impl<T> Send for ApiPtr<T> {}
unsafe impl<T> Sync for ApiPtr<T> {}

/// Global cached APIs - initialized once and reused
static PORT_API: OnceLock<ApiPtr<ArkWeb_WebMessagePortAPI>> = OnceLock::new();
static MESSAGE_API: OnceLock<ApiPtr<ArkWeb_WebMessageAPI>> = OnceLock::new();

/// Get cached WebMessagePort API
fn get_port_api() -> Result<&'static ArkWeb_WebMessagePortAPI, WebViewError> {
    let api_ptr = PORT_API.get_or_init(|| unsafe {
        ApiPtr(
            OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_WEB_MESSAGE_PORT)
                as *const ArkWeb_WebMessagePortAPI,
        )
    });

    if api_ptr.0.is_null() {
        Err(WebViewError::WebView(
            "Failed to get WebMessagePort API".to_string(),
        ))
    } else {
        Ok(unsafe { &*api_ptr.0 })
    }
}

/// Get cached WebMessage API
fn get_message_api() -> Result<&'static ArkWeb_WebMessageAPI, WebViewError> {
    let api_ptr = MESSAGE_API.get_or_init(|| unsafe {
        ApiPtr(
            OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_WEB_MESSAGE)
                as *const ArkWeb_WebMessageAPI,
        )
    });

    if api_ptr.0.is_null() {
        Err(WebViewError::WebView(
            "Failed to get WebMessage API".to_string(),
        ))
    } else {
        Ok(unsafe { &*api_ptr.0 })
    }
}

pub struct WebViewInner {
    pub(crate) webtag: WebTag,
    native_port: RefCell<Option<*mut ArkWeb_WebMessagePort>>,
    console_port: RefCell<Option<*mut ArkWeb_WebMessagePort>>,
    webview_native_port: RefCell<Option<*mut ArkWeb_WebMessagePort>>,
    webview_console_port: RefCell<Option<*mut ArkWeb_WebMessagePort>>,
    creation_sender: Mutex<Option<Sender<Result<Arc<crate::WebView>, WebViewError>>>>,
    // Store user_data pointers for cleanup
    user_data_ptrs: RefCell<Vec<*mut c_void>>,
    // Keep proxy allocations alive for lifetime
    proxy_allocs: RefCell<Vec<*mut c_void>>,
    // Whether lifecycle callbacks have been registered with ArkWeb
    callbacks_registered: RefCell<bool>,
    // Store scheme handlers for cleanup
    scheme_handlers: RefCell<Vec<*mut ohos_web_sys::ArkWeb_SchemeHandler>>,
}

impl std::fmt::Debug for WebViewInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebViewInner")
            .field("webtag", &self.webtag)
            .finish()
    }
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

pub fn webview_controller_created(webtag_str: &str) -> Result<(), WebViewError> {
    let webtag = WebTag::from(webtag_str);
    let webview = find_webview(&webtag)
        .ok_or_else(|| WebViewError::WebView(format!("WebView not found: {}", webtag_str)))?;

    // Register lifecycle callbacks now that controller is created
    if !*webview.inner.callbacks_registered.borrow() {
        if let Err(e) = register_webview_callbacks(&webtag) {
            log::error!(
                "WebView callback registration failed for {}: {:?}",
                webtag_str,
                e
            );
        } else {
            *webview.inner.callbacks_registered.borrow_mut() = true;
            log::info!("Registered ArkWeb lifecycle callbacks for {}", webtag_str);
        }
    }

    // Register JS proxy when ArkTS notifies controller created (page UI attached)
    // This binds the proxy into the actual page JS world.
    if let Err(e) = register_proxy_for_webtag(&webtag) {
        log::warn!(
            "Failed to register LingXiaProxy at created-ack for {}: {}",
            webtag_str,
            e
        );
    } else {
        log::info!("Registered LingXiaProxy at created-ack for {}", webtag_str);
    }

    if let Ok(mut sender_opt) = webview.inner.creation_sender.lock() {
        if let Some(sender) = sender_opt.take() {
            let _ = sender.send(Ok(webview.clone()));
            log::info!("WebView creation acknowledged for {}", webtag_str);
        }
    }

    Ok(())
}

/// Called when ArkTS reports that a WebView controller was destroyed.
/// Cleanup is centralized here via the NAPI bridge (on_webview_controller_destroyed)
/// rather than the low-level ArkWeb onDestroy callback to avoid double free.
pub fn webview_controller_destroyed(webtag_str: &str) {
    let webtag = WebTag::from(webtag_str);
    if let Some(webview) = find_webview(&webtag) {
        // Allow callbacks to be re-registered if a new controller is later created
        *webview.inner.callbacks_registered.borrow_mut() = false;

        // Idempotent cleanup of native resources tied to the old controller.
        webview.inner.cleanup_webmessage_ports();
        webview.inner.cleanup_user_data();
        webview.inner.cleanup_scheme_handlers();
    }
}

/// Register LingXiaProxy for a specific webtag
fn register_proxy_for_webtag(webtag: &WebTag) -> Result<(), WebViewError> {
    unsafe {
        let webtag_cstr = CString::new(webtag.as_str()).unwrap();

        let controller_api =
            OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_CONTROLLER);
        if controller_api.is_null() {
            return Err(WebViewError::WebView(
                "Failed to get Controller API".to_string(),
            ));
        }
        let controller = &*(controller_api as *const ArkWeb_ControllerAPI);

        if let Some(register_proxy) = controller.registerJavaScriptProxy {
            // If storage already exists, reuse it to rebind into current page JS world
            if let Some(wv) = find_webview(webtag) {
                let allocs = wv.inner.proxy_allocs.borrow_mut();
                if let Some(p) = allocs.get(0) {
                    let storage = *p as *mut ProxyStorage;
                    let proxy_object = ArkWeb_ProxyObject {
                        objName: LINGXIA_PROXY_NAME.as_ptr() as *const c_char,
                        methodList: (*storage).method.as_ptr(),
                        size: (*storage).method.len(),
                    };
                    register_proxy(webtag_cstr.as_ptr(), &proxy_object);
                    log::info!(
                        "Re-registered LingXiaProxy for {} (page context)",
                        webtag.as_str()
                    );
                    return Ok(());
                }
            }

            // First-time allocation path
            let webtag_string = webtag.as_str().to_string();
            let proxy_data = Box::new(webtag_string);
            let proxy_data_ptr = Box::into_raw(proxy_data) as *mut std::ffi::c_void;

            let storage = Box::new(ProxyStorage {
                method: Box::new([ArkWeb_ProxyMethod {
                    methodName: LINGXIA_PROXY_GET_PORT.as_ptr() as *const c_char,
                    callback: Some(get_port_callback),
                    userData: proxy_data_ptr,
                }]),
            });
            let storage = Box::into_raw(storage);

            let proxy_object = ArkWeb_ProxyObject {
                objName: LINGXIA_PROXY_NAME.as_ptr() as *const c_char,
                methodList: (*storage).method.as_ptr(),
                size: (*storage).method.len(),
            };
            register_proxy(webtag_cstr.as_ptr(), &proxy_object);

            // Keep allocations alive for WebView lifetime
            if let Some(webview) = find_webview(webtag) {
                webview
                    .inner
                    .proxy_allocs
                    .borrow_mut()
                    .push(storage as *mut c_void);
                webview
                    .inner
                    .user_data_ptrs
                    .borrow_mut()
                    .push(proxy_data_ptr);
            }
            log::info!("Registered LingXiaProxy for {}", webtag.as_str());
            Ok(())
        } else {
            Err(WebViewError::WebView(
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
        log::warn!("get_port_callback missing user_data or args");
        return;
    }

    unsafe {
        let webtag_string = &*(user_data as *const String);
        let webtag = WebTag::from(webtag_string.as_str());
        let type_data = &*bridge_data.offset(0);

        if let Some(port_type_str) = extract_string_from_bridge_data(type_data) {
            // Ensure ports exist; create on-demand if onPageBegin hasn't run yet
            match port_type_str.as_str() {
                "ConsolePort" => {
                    let need_setup = find_webview(&webtag)
                        .map(|wv| wv.inner.webview_console_port.borrow().is_none())
                        .unwrap_or(true);
                    if need_setup {
                        if let Err(e) = setup_webmessage_port_for_webtag(
                            &webtag,
                            PortType::Console,
                            on_console_message_received,
                        ) {
                            log::error!(
                                "On-demand console port setup failed for {}: {}",
                                webtag.as_str(),
                                e
                            );
                        }
                    }
                    if let Err(e) = send_port_to_webview_for_webtag(&webtag, PortType::Console) {
                        log::error!("Failed to send console port: {}", e);
                    }
                }
                "LingXiaPort" => {
                    let need_setup = find_webview(&webtag)
                        .map(|wv| wv.inner.webview_native_port.borrow().is_none())
                        .unwrap_or(true);

                    if need_setup {
                        if let Err(e) = setup_webmessage_port_for_webtag(
                            &webtag,
                            PortType::Message,
                            on_web_message_received,
                        ) {
                            log::error!(
                                "On-demand message port setup failed for {}: {}",
                                webtag.as_str(),
                                e
                            );
                        }
                    }
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
) -> Result<(), WebViewError> {
    let webview = find_webview(webtag).ok_or_else(|| {
        WebViewError::WebView(format!("WebView not found for webtag: {}", webtag.as_str()))
    })?;

    webview.inner.send_port(port_type)
}

impl WebViewInner {
    /// Create a WebView instance
    pub fn create(
        appid: &str,
        path: &str,
        session_id: Option<u64>,
        sender: Sender<Result<Arc<crate::WebView>, WebViewError>>,
    ) {
        if session_id.is_none() {
            log::warn!(
                "Creating Harmony WebView without session id for {}-{}",
                appid,
                path
            );
        }
        let webtag = WebTag::new(appid, path, session_id);

        // Create WebView instance, storing the sender
        let webview_inner = WebViewInner {
            webtag: webtag.clone(),
            native_port: RefCell::new(None),
            console_port: RefCell::new(None),
            webview_native_port: RefCell::new(None),
            webview_console_port: RefCell::new(None),
            creation_sender: Mutex::new(Some(sender)),
            user_data_ptrs: RefCell::new(Vec::new()),
            proxy_allocs: RefCell::new(Vec::new()),
            callbacks_registered: RefCell::new(false),
            scheme_handlers: RefCell::new(Vec::new()),
        };

        // Create WebView wrapper and register it
        let webview = Arc::new(crate::WebView::new(webview_inner));
        register_webview(webview.clone());

        // Call ArkTS to create the WebView controller via TSFN (no callback path).
        // ArkTS will notify native through onWebviewControllerCreated(webtag)
        // once the ArkUI Web component is actually attached (onAppear).
        if let Err(e) = call_arkts("createWebViewController", &[webtag.as_str()]) {
            log::error!("Failed to call createWebViewController: {}", e);
            if let Some(webview) = find_webview(&webtag) {
                if let Ok(mut sender_opt) = webview.inner.creation_sender.lock() {
                    if let Some(s) = sender_opt.take() {
                        let _ = s.send(Err(e));
                    }
                }
            }
            return;
        }

        // Register scheme handler immediately using the Ark-facing tag (webtag)
        if let Err(e) = set_webview_scheme_handler(&webtag) {
            log::error!(
                "Failed to set scheme handler for {}: {}",
                webtag.as_str(),
                e
            );
        }
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

    /// Cleanup WebMessage ports
    fn cleanup_webmessage_ports(&self) {
        unsafe {
            // Get port API if available
            if let Ok(port_api) = get_port_api() {
                let mut cleanup_count = 0;
                let webtag_cstr = CString::new(self.webtag.as_str()).unwrap();

                // Cleanup native message port
                if let Some(port) = self.native_port.borrow_mut().take() {
                    if let Some(close_fn) = port_api.close {
                        close_fn(port, webtag_cstr.as_ptr() as *const u8);
                        cleanup_count += 1;
                    }
                }

                // Cleanup webview message port
                if let Some(port) = self.webview_native_port.borrow_mut().take() {
                    if let Some(close_fn) = port_api.close {
                        close_fn(port, webtag_cstr.as_ptr() as *const u8);
                        cleanup_count += 1;
                    }
                }

                // Cleanup console port
                if let Some(port) = self.console_port.borrow_mut().take() {
                    if let Some(close_fn) = port_api.close {
                        close_fn(port, webtag_cstr.as_ptr() as *const u8);
                        cleanup_count += 1;
                    }
                }

                // Cleanup webview console port
                if let Some(port) = self.webview_console_port.borrow_mut().take() {
                    if let Some(close_fn) = port_api.close {
                        close_fn(port, webtag_cstr.as_ptr() as *const u8);
                        cleanup_count += 1;
                    }
                }

                if cleanup_count > 0 {
                    log::info!(
                        "Cleaned up {} WebMessage ports for {}",
                        cleanup_count,
                        self.webtag.as_str()
                    );
                }
            }
        }
    }

    /// Send port to WebView
    pub fn send_port(&self, port_type: PortType) -> Result<(), WebViewError> {
        unsafe {
            // Use the Ark-facing tag when talking to ArkWeb
            let webtag_cstr = CString::new(self.webtag.as_str()).unwrap();
            let controller_api =
                OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_CONTROLLER);
            if controller_api.is_null() {
                return Err(WebViewError::WebView(
                    "Failed to get Controller API".to_string(),
                ));
            }
            let controller = &*(controller_api as *const ArkWeb_ControllerAPI);

            // Use borrow() instead of take() - we need to keep the port reference
            let (port_opt, message, port_name) = match port_type {
                PortType::Console => (
                    *self.webview_console_port.borrow(),
                    "LingXia-console-init",
                    "console",
                ),
                PortType::Message => (
                    *self.webview_native_port.borrow(),
                    "LingXia-port-init",
                    "message",
                ),
            };

            if let Some(webview_port) = port_opt {
                // Prepare stable CStrings for the call duration
                let msg_cstr = CString::new(message).unwrap();
                let target_cstr = CString::new("*").unwrap();

                // Create a mutable copy of the port pointer for the API call
                let mut port_array = [webview_port];

                let result = controller.postWebMessage.ok_or_else(|| {
                    WebViewError::WebView("postWebMessage not available".to_string())
                })?(
                    webtag_cstr.as_ptr(),
                    msg_cstr.as_ptr(),
                    port_array.as_mut_ptr(),
                    1,
                    target_cstr.as_ptr(),
                );

                if result == 0 {
                    log::info!(
                        "Successfully sent {} port to WebView for {}",
                        port_name,
                        self.webtag.as_str()
                    );
                    Ok(())
                } else {
                    Err(WebViewError::WebView(format!(
                        "Failed to send {} port: error {}",
                        port_name, result
                    )))
                }
            } else {
                Err(WebViewError::WebView(format!(
                    "{} port not available",
                    port_name
                )))
            }
        }
    }
}

impl WebViewController for WebViewInner {
    fn load_url(&self, url: String) -> Result<(), WebViewError> {
        call_arkts("loadUrl", &[self.webtag.as_str(), &url])
    }

    fn load_data(
        &self,
        data: String,
        base_url: String,
        history_url: Option<String>,
    ) -> Result<(), WebViewError> {
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
                Err(WebViewError::WebView(format!(
                    "Failed to load data into WebView: error code {:?}",
                    result
                )))
            }
        }
    }

    fn evaluate_javascript(&self, js: String) -> Result<(), WebViewError> {
        unsafe {
            let web_tag_cstr = CString::new(self.webtag.as_str()).unwrap();

            // Get Controller API
            let controller_api =
                OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_CONTROLLER);
            if controller_api.is_null() {
                return Err(WebViewError::WebView(
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
                Err(WebViewError::WebView(
                    "runJavaScript function not available".to_string(),
                ))
            }
        }
    }

    fn clear_browsing_data(&self) -> Result<(), WebViewError> {
        call_arkts("clearBrowsingData", &[self.webtag.as_str()])
    }

    fn set_user_agent(&self, ua: String) -> Result<(), WebViewError> {
        call_arkts("setUserAgent", &[self.webtag.as_str(), &ua])
    }

    fn set_scroll_listener_enabled(
        &self,
        enabled: bool,
        _throttle_ms: Option<u64>,
    ) -> Result<(), WebViewError> {
        call_arkts(
            "setScrollListenerEnabled",
            &[self.webtag.as_str(), &enabled.to_string()],
        )
    }

    fn post_message(&self, message: String) -> Result<(), WebViewError> {
        // Use an internal helper for message posting
        self.post_message_internal(&message, true)
    }
}

impl WebViewInner {
    /// Internal helper for post_message.
    fn post_message_internal(&self, message: &str, _allow_retry: bool) -> Result<(), WebViewError> {
        // Access native_port directly through RefCell
        let port = *self.native_port.borrow();

        if let Some(port) = port {
            // Use Ark-facing tag when posting messages to ArkWeb
            let webtag_cstr = CString::new(self.webtag.as_str()).unwrap();

            let port_api = get_port_api()?;
            let message_api = get_message_api()?;

            // Create WebMessage
            let create_fn = message_api.createWebMessage.ok_or_else(|| {
                WebViewError::WebView("createWebMessage not available".to_string())
            })?;
            let web_message = unsafe { create_fn() };

            if web_message.is_null() {
                log::error!(
                    "post_message: createWebMessage returned null for {}",
                    self.webtag.as_str()
                );
                return Err(WebViewError::WebView(
                    "Failed to create WebMessage".to_string(),
                ));
            }

            // Set message type to string
            if let Some(set_type) = message_api.setType {
                unsafe {
                    set_type(web_message, ArkWeb_WebMessageType_ARKWEB_STRING);
                }
            } else {
                log::warn!("setType not available in WebMessage API");
            }

            // Build a C string buffer that includes a trailing NUL byte.
            // Length passed to ArkWeb follows the official C example: len + 1.
            let c_string = match CString::new(message) {
                Ok(s) => s,
                Err(e) => {
                    log::error!(
                        "post_message failed to build CString for {}: {:?}",
                        self.webtag.as_str(),
                        e
                    );
                    return Err(WebViewError::WebView(
                        "Failed to build CString for message".to_string(),
                    ));
                }
            };
            let bytes_with_nul = c_string.as_bytes_with_nul();
            let byte_len_with_nul = bytes_with_nul.len();

            // Set message data with length INCLUDING trailing NUL to match C sample
            if let Some(set_data) = message_api.setData {
                unsafe {
                    set_data(
                        web_message,
                        c_string.as_ptr() as *mut std::ffi::c_void,
                        byte_len_with_nul,
                    );
                }
            } else {
                log::error!("setData not available in WebMessage API");
                return Err(WebViewError::WebView("setData not available".to_string()));
            }

            // Post message using the existing port
            let result = if let Some(post) = port_api.postMessage {
                unsafe { post(port, webtag_cstr.as_ptr(), web_message) }
            } else {
                return Err(WebViewError::WebView(
                    "postMessage not available".to_string(),
                ));
            };

            // Destroy WebMessage after post, mirroring C example
            if let Some(destroy_message) = message_api.destroyWebMessage {
                let mut msg_ptr = web_message;
                unsafe {
                    destroy_message(&mut msg_ptr as *mut *mut ArkWeb_WebMessage);
                }
            }

            if result != 0 {
                log::error!(
                    "post_message: postMessage failed for {} with error {}",
                    self.webtag.as_str(),
                    result
                );
                return Err(WebViewError::WebView(format!(
                    "postMessage failed with error {}",
                    result
                )));
            }

            Ok(())
        } else {
            log::warn!("post_message: no native_port for {}", self.webtag.as_str());
            return Err(WebViewError::WebView(
                "No native port available for post_message".to_string(),
            ));
        }
    }
}

impl Drop for WebViewInner {
    fn drop(&mut self) {
        // Cleanup all tracked scheme handlers first
        self.cleanup_scheme_handlers();

        // Cleanup WebMessage ports
        self.cleanup_webmessage_ports();

        // Cleanup all tracked user_data
        self.cleanup_user_data();

        // Free proxy allocations
        let proxies = self.proxy_allocs.borrow_mut().drain(..).collect::<Vec<_>>();
        for p in proxies {
            unsafe {
                // Recreate to drop method array (ProxyStorage)
                let _ = Box::from_raw(p as *mut ProxyStorage);
            }
        }

        // Ask ArkTS to destroy the controller; ArkTS will notify native via onWebviewControllerDestroyed
        if let Err(e) = call_arkts("destroyWebViewController", &[self.webtag.as_str()]) {
            log::error!("Failed to destroy WebView controller: {:?}", e);
        }
        log::info!(
            "[WebViewInner] Harmony WebViewInner dropped and destroyed ({})",
            self.webtag.as_str()
        );
    }
}

/// Register WebView lifecycle callbacks with shared user_data pointer
fn register_webview_callbacks(webtag: &WebTag) -> Result<(), WebViewError> {
    unsafe {
        let webtag_cstr = CString::new(webtag.as_str()).unwrap();

        // Create a single shared user_data for all callbacks (like the original implementation)
        let webtag_string = webtag.as_str().to_string();
        let user_data = Box::into_raw(Box::new(webtag_string)) as *mut c_void;

        // Track this user_data for cleanup (but don't double-cleanup in on_destroy_callback)
        if let Some(webview) = find_webview(webtag) {
            webview.inner.track_user_data(user_data);
        }

        // Get the ArkWeb_ComponentAPI using the correct API
        let component_api =
            OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_COMPONENT);
        if component_api.is_null() {
            return Err(WebViewError::WebView(
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
extern "C" fn on_controller_attached_callback(web_tag: *const c_char, _user_data: *mut c_void) {
    if let Ok(webtag_str) = unsafe { CStr::from_ptr(web_tag).to_str() } {
        log::info!("WebView controller attached: {}", webtag_str);
    }
}

extern "C" fn on_page_begin_callback(web_tag: *const c_char, user_data: *mut c_void) {
    if let Ok(webtag_str) = unsafe { CStr::from_ptr(web_tag).to_str() } {
        log::info!("Page begin loading: {}", webtag_str);

        let webtag_string = unsafe { &*(user_data as *const String) };
        let webtag = WebTag::from(webtag_string.as_str());

        // Only inject console interception script; port setup is deferred to get_port_callback
        if !user_data.is_null() {
            if let Err(e) = inject_console_script(&webtag) {
                log::error!("Failed to inject console script for {}: {}", webtag_str, e);
            }
        }

        if let Some(delegate) = get_webview_delegate(&webtag) {
            delegate.on_page_started();
        }
    }
}

extern "C" fn on_page_end_callback(web_tag: *const c_char, _user_data: *mut c_void) {
    if let Ok(webtag) = unsafe { CStr::from_ptr(web_tag).to_str() } {
        log::info!("Page end loading: {}", webtag);

        // Extract app_id and path from webtag
        let webtag = WebTag::from(webtag);

        if let Some(delegate) = get_webview_delegate(&webtag) {
            delegate.on_page_finished();
        }
    }
}

extern "C" fn on_destroy_callback(web_tag: *const c_char, _user_data: *mut c_void) {
    if let Ok(webtag_str) = unsafe { CStr::from_ptr(web_tag).to_str() } {
        // ArkWeb component level reports WebView is destroyed; only log here.
        // Resource cleanup is unified through ArkTS -> onWebviewControllerDestroyed(NAPI) -> webview_controller_destroyed,
        // to avoid double-free caused by duplicate calls.
        log::info!("WebView destroyed (ArkWeb onDestroy): {}", webtag_str);
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
) -> Result<(), WebViewError> {
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
            WebViewError::WebView(format!(
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
            return Err(WebViewError::WebView(format!(
                "Failed to create {:?} WebMessage ports",
                port_type
            )));
        }

        let port1 = *ports.offset(0); // Native side port
        let port2 = *ports.offset(1); // WebView side port

        // Store both ports in WebViewInner
        let webview = crate::find_webview(webtag)
            .ok_or_else(|| WebViewError::WebView("WebView not found".to_string()))?;

        // Access the inner WebViewInner to store ports
        let webview_inner = &webview.inner;
        match port_type {
            PortType::Message => {
                *webview_inner.native_port.borrow_mut() = Some(port1);
                *webview_inner.webview_native_port.borrow_mut() = Some(port2);
            }
            PortType::Console => {
                *webview_inner.console_port.borrow_mut() = Some(port1);
                *webview_inner.webview_console_port.borrow_mut() = Some(port2);
            }
        }

        // Set message event handler
        let webtag_string = webtag.as_str().to_string();
        let user_data_ptr = Box::into_raw(Box::new(webtag_string)) as *mut c_void;

        // Track this allocation for cleanup in the WebViewInner
        webview_inner.track_user_data(user_data_ptr);

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
fn inject_console_script(webtag: &WebTag) -> Result<(), WebViewError> {
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
            return Err(WebViewError::WebView(
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
            Err(WebViewError::WebView(
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
    let full_webtag = WebTag::from(webtag_string.as_str());
    let (appid, path) = full_webtag.extract_parts();

    // Extract message data
    unsafe {
        let message_api =
            OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_WEB_MESSAGE);
        if message_api.is_null() {
            log::error!("Failed to get WebMessage API in on_web_message_received");
            return;
        }

        let api = &*(message_api as *const ArkWeb_WebMessageAPI);

        // Check message type first
        if let Some(get_type) = api.getType {
            let msg_type = get_type(message);
            if msg_type != ArkWeb_WebMessageType_ARKWEB_STRING {
                log::warn!(
                    "Received non-string message type {:?} for {}",
                    msg_type,
                    webtag
                );
            }
        }

        let Some(get_data) = api.getData else {
            log::error!("getData not available in WebMessage API");
            return;
        };

        let mut data_length: usize = 0;
        let data_ptr = get_data(message, &mut data_length);
        if data_ptr.is_null() || data_length == 0 {
            log::warn!(
                "Received empty or null message for {} (ptr={:?}, len={})",
                webtag,
                data_ptr,
                data_length
            );
            return;
        }

        let data_slice = std::slice::from_raw_parts(data_ptr as *const u8, data_length);
        let Ok(msg_str) = std::str::from_utf8(data_slice) else {
            log::error!(
                "Failed to parse UTF-8 message for {} (len={})",
                webtag,
                data_length
            );
            return;
        };

        // Forward to delegate
        if let Some(delegate) = get_webview_delegate(&full_webtag) {
            delegate.handle_post_message(msg_str.to_string());
        } else {
            log::warn!(
                "on_web_message_received: no delegate for {} (appid={}, path={})",
                full_webtag.as_str(),
                appid,
                path
            );
        }
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
                let full_webtag = WebTag::from(webtag);
                // Convert log level for lxapp crate
                let log_level = match level {
                    "error" => LogLevel::Error,
                    "warn" => LogLevel::Warn,
                    "info" => LogLevel::Info,
                    "debug" => LogLevel::Debug,
                    _ => LogLevel::Info,
                };

                // Forward to delegate for logging
                if let Some(delegate) = get_webview_delegate(&full_webtag) {
                    delegate.log(log_level, console_message);
                }
            }
        }
    }
}
