use crate::harmony::ffi::CALLBACK_TSFN;
use crate::harmony::schemehandler::set_webview_scheme_handler;
use miniapp::log::LogLevel;
use miniapp::{AppUiDelegate, MiniAppError, WebViewController};
use napi_ohos::{Status, threadsafe_function::ThreadsafeFunctionCallMode};
use ohos_web_sys::*;

use std::ffi::{CStr, CString, c_char, c_void};
use std::sync::{Arc, Mutex, OnceLock};

/// Wrapper for API pointers to make them Send + Sync
#[derive(Debug, Clone, Copy)]
struct ApiPtr<T>(*const T);
unsafe impl<T> Send for ApiPtr<T> {}
unsafe impl<T> Sync for ApiPtr<T> {}

/// Global cached APIs - initialized once and reused
static PORT_API: OnceLock<ApiPtr<ArkWeb_WebMessagePortAPI>> = OnceLock::new();
static MESSAGE_API: OnceLock<ApiPtr<ArkWeb_WebMessageAPI>> = OnceLock::new();

/// Get cached WebMessagePort API
fn get_port_api() -> Result<&'static ArkWeb_WebMessagePortAPI, MiniAppError> {
    let api_ptr = PORT_API.get_or_init(|| unsafe {
        ApiPtr(
            OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_WEB_MESSAGE_PORT)
                as *const ArkWeb_WebMessagePortAPI,
        )
    });

    if api_ptr.0.is_null() {
        Err(MiniAppError::WebView(
            "Failed to get WebMessagePort API".to_string(),
        ))
    } else {
        Ok(unsafe { &*api_ptr.0 })
    }
}

/// Get cached WebMessage API
fn get_message_api() -> Result<&'static ArkWeb_WebMessageAPI, MiniAppError> {
    let api_ptr = MESSAGE_API.get_or_init(|| unsafe {
        ApiPtr(
            OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_WEB_MESSAGE)
                as *const ArkWeb_WebMessageAPI,
        )
    });

    if api_ptr.0.is_null() {
        Err(MiniAppError::WebView(
            "Failed to get WebMessage API".to_string(),
        ))
    } else {
        Ok(unsafe { &*api_ptr.0 })
    }
}

/// WebTag newtype for better type safety
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebTag(String);

impl WebTag {
    pub fn new(appid: &str, path: &str) -> Self {
        Self(format!("{}-{}", appid, path))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn extract_appid(&self) -> Option<String> {
        self.0.split_once('-').map(|(appid, _)| appid.to_string())
    }

    pub fn extract_parts(&self) -> Option<(String, String)> {
        self.0
            .split_once('-')
            .map(|(appid, path)| (appid.to_string(), path.to_string()))
    }
}

impl From<(String, String)> for WebTag {
    fn from((appid, path): (String, String)) -> Self {
        Self::new(&appid, &path)
    }
}

impl From<(&str, &str)> for WebTag {
    fn from((appid, path): (&str, &str)) -> Self {
        Self::new(appid, path)
    }
}

impl std::fmt::Display for WebTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug)]
pub struct WebViewInner {
    webtag: WebTag,
    user_data: Arc<WebViewUserData>,
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

/// User data structure for WebView callbacks
/// Contains the ports that need to be shared across callbacks
#[derive(Debug)]
pub struct WebViewUserData {
    pub webtag: WebTag,
    pub native_port: Arc<Mutex<Option<*mut ArkWeb_WebMessagePort>>>,
    pub console_port: Arc<Mutex<Option<*mut ArkWeb_WebMessagePort>>>,
    pub webview_native_port: Arc<Mutex<Option<*mut ArkWeb_WebMessagePort>>>,
    pub webview_console_port: Arc<Mutex<Option<*mut ArkWeb_WebMessagePort>>>,
}

unsafe impl Send for WebViewUserData {}
unsafe impl Sync for WebViewUserData {}

impl WebViewUserData {
    pub fn new(webtag: WebTag) -> Self {
        Self {
            webtag,
            native_port: Arc::new(Mutex::new(None)),
            console_port: Arc::new(Mutex::new(None)),
            webview_native_port: Arc::new(Mutex::new(None)),
            webview_console_port: Arc::new(Mutex::new(None)),
        }
    }

    /// Register LingXiaProxy for this WebView
    pub fn register_proxy(&self) -> Result<(), MiniAppError> {
        unsafe {
            let webtag_cstr = CString::new(self.webtag.as_str()).unwrap();

            let controller_api =
                OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_CONTROLLER);
            if controller_api.is_null() {
                return Err(MiniAppError::WebView(
                    "Failed to get Controller API".to_string(),
                ));
            }
            let controller = &*(controller_api as *const ArkWeb_ControllerAPI);

            // Create proxy data pointing to this WebViewUserData
            let proxy_data = Box::new(self as *const WebViewUserData);
            let proxy_data_ptr = Box::into_raw(proxy_data) as *mut std::ffi::c_void;

            let object_name_cstr = CString::new("LingXiaProxy").unwrap();
            let get_port_cstr = CString::new("getPort").unwrap();

            let get_port_method = ArkWeb_ProxyMethod {
                methodName: get_port_cstr.as_ptr(),
                callback: Some(Self::get_port_callback),
                userData: proxy_data_ptr,
            };

            let method_list = vec![get_port_method];
            let proxy_object = ArkWeb_ProxyObject {
                objName: object_name_cstr.as_ptr(),
                methodList: method_list.as_ptr(),
                size: method_list.len(),
            };

            if let Some(register_proxy) = controller.registerJavaScriptProxy {
                register_proxy(webtag_cstr.as_ptr(), &proxy_object);
                log::info!("Registered LingXiaProxy for {}", self.webtag.as_str());
                Ok(())
            } else {
                let _ = Box::from_raw(proxy_data_ptr as *mut *const WebViewUserData);
                Err(MiniAppError::WebView(
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
            let user_data_ptr = *(user_data as *const *const WebViewUserData);
            let user_data = &*user_data_ptr;
            let type_data = &*bridge_data.offset(0);

            if let Some(port_type_str) = Self::extract_string(type_data) {
                // Trigger sending the appropriate port
                match port_type_str.as_str() {
                    "ConsolePort" => {
                        if let Err(e) = user_data.send_console_port_to_webview() {
                            log::error!("Failed to send console port: {}", e);
                        }
                    }
                    "LingXiaPort" => {
                        if let Err(e) = user_data.send_message_port_to_webview() {
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
    unsafe fn extract_string(data: &ArkWeb_JavaScriptBridgeData) -> Option<String> {
        if !data.buffer.is_null() && data.size > 0 {
            unsafe {
                let bytes = std::slice::from_raw_parts(data.buffer as *const u8, data.size);
                std::str::from_utf8(bytes).ok().map(|s| s.to_string())
            }
        } else {
            None
        }
    }

    /// Send port to WebView (unified function)
    fn send_port_to_webview(&self, port_type: PortType) -> Result<(), MiniAppError> {
        unsafe {
            let webtag_cstr = CString::new(self.webtag.as_str()).unwrap();
            let controller_api =
                OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_CONTROLLER);
            if controller_api.is_null() {
                return Err(MiniAppError::WebView(
                    "Failed to get Controller API".to_string(),
                ));
            }
            let controller = &*(controller_api as *const ArkWeb_ControllerAPI);

            let (port, message, port_name) = match port_type {
                PortType::Console => (
                    self.webview_console_port.lock().unwrap().clone(),
                    "LingXia-console-init",
                    "console",
                ),
                PortType::Message => (
                    self.webview_native_port.lock().unwrap().clone(),
                    "LingXia-port-init",
                    "message",
                ),
            };

            if let Some(webview_port) = port {
                let result = controller.postWebMessage.ok_or_else(|| {
                    MiniAppError::WebView("postWebMessage not available".to_string())
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
                    Err(MiniAppError::WebView(format!(
                        "Failed to send {} port: error {}",
                        port_name, result
                    )))
                }
            } else {
                Err(MiniAppError::WebView(format!(
                    "{} port not available",
                    port_name
                )))
            }
        }
    }

    /// Send console port to WebView
    fn send_console_port_to_webview(&self) -> Result<(), MiniAppError> {
        self.send_port_to_webview(PortType::Console)
    }

    /// Send message port to WebView
    fn send_message_port_to_webview(&self) -> Result<(), MiniAppError> {
        self.send_port_to_webview(PortType::Message)
    }
}

impl WebViewInner {
    /// Create a new WebView instance for HarmonyOS
    pub fn create(appid: &str, path: &str) -> Result<Self, MiniAppError> {
        let webtag = WebTag::new(appid, path);

        // Create shared user data
        let user_data = Arc::new(WebViewUserData::new(webtag.clone()));

        let webview_inner = WebViewInner {
            webtag: webtag.clone(),
            user_data: user_data.clone(),
        };

        // Call ArkTS to create WebView controller with immediate callback registration
        let webtag_for_callback = webtag.clone();
        match call_arkts_with_callback("createWebViewController", &[webtag.as_str()], move || {
            // Set scheme handler after WebView is created
            if let Err(e) = set_webview_scheme_handler(&webtag_for_callback) {
                log::error!(
                    "💥 Failed to set scheme handler for {}: {}",
                    webtag_for_callback,
                    e
                );
                return;
            }

            // Use the shared user data for callback registration
            let user_data_clone = user_data.clone();

            if let Err(e) = register_webview_callbacks(user_data_clone.clone()) {
                log::error!(
                    "💥 Webview callback registration failed for {}: {:?}",
                    webtag_for_callback,
                    e
                );
            }

            // Register LingXiaProxy for home page immediately after WebView creation
            if webtag_for_callback.as_str().contains("home") {
                if let Err(e) = WebViewUserData::register_proxy(&user_data_clone) {
                    log::error!(
                        "Failed to register LingXiaProxy for home page {}: {}",
                        webtag_for_callback,
                        e
                    );
                } else {
                    log::info!(
                        "Registered LingXiaProxy for home page in createWebViewController callback: {}",
                        webtag_for_callback
                    );
                }
            }

            log::info!(
                "Scheme handler and callbacks setup completed for {}",
                webtag_for_callback
            );
        }) {
            Ok(_) => {
                // Return the WebView after successful creation and callback registration
                Ok(webview_inner)
            }
            Err(e) => Err(MiniAppError::WebView(format!(
                "Failed to create WebView: {}",
                e
            ))),
        }
    }
}

/// Helper function for TSFN calls
fn call_arkts(name: &str, args: &[&str]) -> Result<(), MiniAppError> {
    let tsfn = CALLBACK_TSFN
        .get()
        .ok_or_else(|| MiniAppError::WebView("No callback".to_string()))?;
    let data = format!("{}|{}", name, args.join("|"));
    match tsfn.call(data, ThreadsafeFunctionCallMode::Blocking) {
        Status::Ok => Ok(()),
        _ => Err(MiniAppError::WebView("TSFN call failed".to_string())),
    }
}

/// Helper function for TSFN calls with callback
fn call_arkts_with_callback<F>(name: &str, args: &[&str], callback: F) -> Result<(), MiniAppError>
where
    F: FnOnce() + Send + 'static,
{
    let tsfn = CALLBACK_TSFN
        .get()
        .ok_or_else(|| MiniAppError::WebView("No callback".to_string()))?;
    let data = format!("{}|{}", name, args.join("|"));

    // Call ArkTS with return value and wait for completion
    match tsfn.call_with_return_value(
        data,
        ThreadsafeFunctionCallMode::Blocking,
        |_env, _result| {
            callback();
            Ok(())
        },
    ) {
        Status::Ok => Ok(()),
        _ => Err(MiniAppError::WebView(
            "TSFN call_with_return_value failed".to_string(),
        )),
    }
}

impl WebViewController for WebViewInner {
    fn load_url(&self, url: String) -> Result<(), MiniAppError> {
        call_arkts("loadUrl", &[self.webtag.as_str(), &url])
    }

    fn evaluate_javascript(&self, js: String) -> Result<(), MiniAppError> {
        unsafe {
            let web_tag_cstr = CString::new(self.webtag.as_str()).unwrap();

            // Get Controller API
            let controller_api =
                OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_CONTROLLER);
            if controller_api.is_null() {
                return Err(MiniAppError::WebView(
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
                Err(MiniAppError::WebView(
                    "runJavaScript function not available".to_string(),
                ))
            }
        }
    }

    fn set_devtools(&self, enabled: bool) -> Result<(), MiniAppError> {
        call_arkts("setDevtools", &[self.webtag.as_str(), &enabled.to_string()])
    }

    fn clear_browsing_data(&self) -> Result<(), MiniAppError> {
        call_arkts("clearBrowsingData", &[self.webtag.as_str()])
    }

    fn set_user_agent(&self, ua: String) -> Result<(), MiniAppError> {
        call_arkts("setUserAgent", &[self.webtag.as_str(), &ua])
    }

    fn set_scroll_listener_enabled(
        &self,
        enabled: bool,
        _throttle_ms: Option<u64>,
    ) -> Result<(), MiniAppError> {
        call_arkts(
            "setScrollListenerEnabled",
            &[self.webtag.as_str(), &enabled.to_string()],
        )
    }

    fn post_message(&self, message: String) -> Result<(), MiniAppError> {
        // Use the native port from WebViewUserData
        let port = if let Ok(port_guard) = self.user_data.native_port.lock() {
            *port_guard
        } else {
            None
        };

        if let Some(port) = port {
            unsafe {
                let webtag_cstr = CString::new(self.webtag.as_str()).unwrap();
                let message_cstr = CString::new(message.clone()).unwrap();

                // Get cached APIs
                let port_api = get_port_api()?;
                let message_api = get_message_api()?;

                // Create WebMessage
                let create_fn = message_api.createWebMessage.ok_or_else(|| {
                    MiniAppError::WebView("createWebMessage not available".to_string())
                })?;
                let web_message = create_fn();

                if web_message.is_null() {
                    return Err(MiniAppError::WebView(
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
                    MiniAppError::WebView("postMessage not available".to_string())
                })?(port, webtag_cstr.as_ptr(), web_message);

                // Clean up the created message
                if let Some(destroy_message) = message_api.destroyWebMessage {
                    let mut msg_ptr = web_message;
                    destroy_message(&mut msg_ptr as *mut *mut ArkWeb_WebMessage);
                }

                if result != ArkWeb_ErrorCode_ARKWEB_SUCCESS {
                    return Err(MiniAppError::WebView(format!(
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
            return Err(MiniAppError::WebView(
                "No native port available for post_message".to_string(),
            ));
        }
    }
}

impl Drop for WebViewInner {
    fn drop(&mut self) {
        if let Err(e) = call_arkts("destroyWebViewController", &[self.webtag.as_str()]) {
            log::error!("Failed to destroy WebView controller: {:?}", e);
        }
    }
}

/// Register WebView lifecycle callbacks with WebViewUserData as user_data
fn register_webview_callbacks(user_data_arc: Arc<WebViewUserData>) -> Result<(), MiniAppError> {
    unsafe {
        let webtag_cstr = CString::new(user_data_arc.webtag.as_str()).unwrap();

        // Convert Arc to raw pointer for user_data
        let user_data = Arc::into_raw(user_data_arc) as *mut c_void;

        // Get the ArkWeb_ComponentAPI using the correct API
        let component_api =
            OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_COMPONENT);
        if component_api.is_null() {
            return Err(MiniAppError::WebView(
                "Failed to get ArkWeb_ComponentAPI".to_string(),
            ));
        }

        let api = &*(component_api as *const ArkWeb_ComponentAPI);

        // Register onControllerAttached callback
        if let Some(on_controller_attached) = api.onControllerAttached {
            on_controller_attached(
                webtag_cstr.as_ptr(),
                Some(on_controller_attached_callback),
                user_data,
            );
        }

        // Register onPageBegin callback
        if let Some(on_page_begin) = api.onPageBegin {
            on_page_begin(
                webtag_cstr.as_ptr(),
                Some(on_page_begin_callback),
                user_data,
            );
        }

        // Register onPageEnd callback
        if let Some(on_page_end) = api.onPageEnd {
            on_page_end(webtag_cstr.as_ptr(), Some(on_page_end_callback), user_data);
        }

        // Register onDestroy callback
        if let Some(on_destroy) = api.onDestroy {
            on_destroy(webtag_cstr.as_ptr(), Some(on_destroy_callback), user_data);
        }

        Ok(())
    }
}

// WebView lifecycle callback functions
extern "C" fn on_controller_attached_callback(web_tag: *const c_char, user_data: *mut c_void) {
    if let Ok(webtag) = unsafe { CStr::from_ptr(web_tag).to_str() } {
        log::info!("WebView controller attached: {}", webtag);

        // Register LingXiaProxy for all pages in onAttach as test
        if !user_data.is_null() {
            let user_data_arc = unsafe { Arc::from_raw(user_data as *const WebViewUserData) };

            if let Err(e) = WebViewUserData::register_proxy(&user_data_arc) {
                log::error!(
                    "Failed to register LingXiaProxy in onAttach for {}: {}",
                    webtag,
                    e
                );
            } else {
                log::info!("Registered LingXiaProxy in onAttach callback: {}", webtag);
            }

            // Don't drop the Arc, convert back to raw pointer
            let _ = Arc::into_raw(user_data_arc);
        }
    }
}

extern "C" fn on_page_begin_callback(web_tag: *const c_char, user_data: *mut c_void) {
    if let Ok(webtag) = unsafe { CStr::from_ptr(web_tag).to_str() } {
        log::info!("Page begin loading: {}", webtag);

        // Setup ports when page begins loading - WebView is fully ready now
        if !user_data.is_null() {
            let user_data_arc = unsafe { Arc::from_raw(user_data as *const WebViewUserData) };

            // Setup console interception and WebMessage handlers
            if let Err(e) = setup_console_interception_with_userdata(&user_data_arc) {
                log::error!("Failed to setup console interception for {}: {}", webtag, e);
            }

            if let Err(e) = setup_webmessage_handlers_with_userdata(&user_data_arc) {
                log::error!("Failed to setup WebMessage handlers for {}: {}", webtag, e);
            }

            // Don't drop the Arc, convert back to raw pointer
            let _ = Arc::into_raw(user_data_arc);
        }
    }
}

extern "C" fn on_page_end_callback(web_tag: *const c_char, _user_data: *mut c_void) {
    if let Ok(webtag) = unsafe { CStr::from_ptr(web_tag).to_str() } {
        log::info!("Page end loading: {}", webtag);

        // Extract app_id and path from webtag (format: "appid-path")
        if let Some((appid, path)) = webtag.split_once('-') {
            let miniapp = miniapp::get(appid.to_string());
            miniapp.on_page_finished(path.to_string());
        }
    }
}

extern "C" fn on_destroy_callback(web_tag: *const c_char, user_data: *mut c_void) {
    if let Ok(webtag) = unsafe { CStr::from_ptr(web_tag).to_str() } {
        log::info!("WebView destroyed: {}", webtag);

        // Properly release the Box to avoid memory leak
        if !user_data.is_null() {
            unsafe {
                let _webview_box = Box::from_raw(user_data as *mut WebViewInner);
                // Box will be automatically dropped here, cleaning up the WebViewInner
                log::info!("WebViewInner cleaned up for {}", webtag);
            }
        }
    }
}

/// Generic WebMessage port setup function with direct injection
fn setup_webmessage_port(
    user_data: &Arc<WebViewUserData>,
    port_type: PortType,
    callback_fn: extern "C" fn(
        *const c_char,
        *mut ArkWeb_WebMessagePort,
        *mut ArkWeb_WebMessage,
        *mut c_void,
    ),
) -> Result<(), MiniAppError> {
    unsafe {
        let webtag_cstr = CString::new(user_data.webtag.as_str()).unwrap();

        // Get APIs
        let controller_api =
            OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_CONTROLLER);
        let port_api =
            OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_WEB_MESSAGE_PORT);

        let controller = &*(controller_api as *const ArkWeb_ControllerAPI);
        let port_api_struct = &*(port_api as *const ArkWeb_WebMessagePortAPI);

        let mut size: usize = 0;
        let ports = controller.createWebMessagePorts.ok_or_else(|| {
            MiniAppError::WebView(format!(
                "createWebMessagePorts not available for {:?}",
                port_type
            ))
        })?(webtag_cstr.as_ptr(), &mut size);

        if ports.is_null() || size < 2 {
            log::error!(
                "Failed to create {:?} WebMessage ports for {}: ports={:?}, size={}",
                port_type,
                user_data.webtag.as_str(),
                ports,
                size
            );
            return Err(MiniAppError::WebView(format!(
                "Failed to create {:?} WebMessage ports",
                port_type
            )));
        }

        let port1 = *ports.offset(0); // Native side port
        let port2 = *ports.offset(1); // WebView side port

        // Store both ports in WebViewUserData
        match port_type {
            PortType::Message => {
                *user_data.native_port.lock().unwrap() = Some(port1);
                *user_data.webview_native_port.lock().unwrap() = Some(port2);
            }
            PortType::Console => {
                *user_data.console_port.lock().unwrap() = Some(port1);
                *user_data.webview_console_port.lock().unwrap() = Some(port2);
            }
        }

        // Set message event handler
        let user_data_for_callback = Arc::clone(user_data);
        let user_data_ptr = Arc::into_raw(user_data_for_callback) as *mut c_void;

        if let Some(set_handler) = port_api_struct.setMessageEventHandler {
            set_handler(
                port1,
                webtag_cstr.as_ptr(),
                Some(callback_fn),
                user_data_ptr,
            );
        }

        log::info!("Setup {} port for {}", port_type, user_data.webtag);
        Ok(())
    }
}

/// Set up WebMessage handlers and store the native port in WebViewUserData
fn setup_webmessage_handlers_with_userdata(
    user_data: &Arc<WebViewUserData>,
) -> Result<(), MiniAppError> {
    setup_webmessage_port(user_data, PortType::Message, on_web_message_received)
}

/// Set up console interception with WebMessage port using WebViewUserData
fn setup_console_interception_with_userdata(
    user_data: &Arc<WebViewUserData>,
) -> Result<(), MiniAppError> {
    // First inject console script
    inject_console_script(user_data.webtag.as_str())?;

    // Then setup the WebMessage port
    setup_webmessage_port(user_data, PortType::Console, on_console_message_received)
}

/// Inject console interception script
fn inject_console_script(webtag: &str) -> Result<(), MiniAppError> {
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
        let webtag_cstr = CString::new(webtag).unwrap();

        // Get Controller API
        let controller_api =
            OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_CONTROLLER);
        if controller_api.is_null() {
            return Err(MiniAppError::WebView(
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
            Err(MiniAppError::WebView(
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

    // Reconstruct Arc from raw pointer to access WebViewUserData
    let user_data_arc = unsafe { Arc::from_raw(user_data as *const WebViewUserData) };
    let Some((appid, path)) = user_data_arc.webtag.extract_parts() else {
        // Release the Arc back to raw pointer to keep it alive
        let _ = Arc::into_raw(user_data_arc);
        return;
    };

    // Release the Arc back to raw pointer to keep it alive for future callbacks
    let _ = Arc::into_raw(user_data_arc);

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

        // Forward to miniapp logic layer
        let miniapp = miniapp::get(appid.to_string());
        miniapp.handle_post_message(path.to_string(), msg_str.to_string());
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
                let webtag = WebTag(webtag.to_string());
                if let Some((appid, path)) = webtag.extract_parts() {
                    // Convert log level for miniapp crate
                    let log_level = match level {
                        "error" => LogLevel::Error,
                        "warn" => LogLevel::Warn,
                        "info" => LogLevel::Info,
                        "debug" => LogLevel::Debug,
                        _ => LogLevel::Info,
                    };

                    // Forward to miniapp crate for logging only
                    let miniapp = miniapp::get(appid);
                    miniapp.log(&path, log_level, console_message);
                }
            }
        }
    }
}
