use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

use crate::harmony::schemehandler::set_webview_scheme_handler;
use crate::harmony::tsfn::call_arkts;
use crate::traits::{
    FileChooserRequest, FileChooserResponse, LoadError, LoadErrorKind, NavigationPolicy,
};
use crate::webview::{
    EffectiveWebViewCreateOptions, ProxyActivation, ProxyApplyReport, ProxyConfig, SecurityProfile,
    WebTag, WebViewCreateSender, WebViewCreateStage, find_webview, find_webview_delegate,
    register_webview,
};
use crate::{DownloadRequest, LoadDataRequest, LogLevel, WebViewController, WebViewError};
use ohos_web_sys::*;

use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char, c_void};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;

fn encode_options_token(options: &EffectiveWebViewCreateOptions) -> Result<String, WebViewError> {
    let json = serde_json::to_vec(options).map_err(|e| {
        WebViewError::InvalidCreateOptions(format!("Serialize options failed: {e}"))
    })?;
    Ok(URL_SAFE_NO_PAD.encode(json))
}

fn cstring_from_str(field: &str, value: &str) -> Result<CString, WebViewError> {
    CString::new(value).map_err(|_| {
        WebViewError::WebView(format!(
            "Failed to encode {} as CString: contains interior NUL byte",
            field
        ))
    })
}

fn lock_or_recover<'a, T>(mutex: &'a Mutex<T>, name: &str) -> std::sync::MutexGuard<'a, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            log::error!("Mutex poisoned at {}, recovering inner value", name);
            poisoned.into_inner()
        }
    }
}

const NETCONN_MAX_STR_LEN: usize = 256;
const NETCONN_MAX_EXCLUSION_SIZE: usize = 256;

#[repr(C)]
struct NetConnHttpProxy {
    host: [c_char; NETCONN_MAX_STR_LEN],
    exclusion_list: [[c_char; NETCONN_MAX_STR_LEN]; NETCONN_MAX_EXCLUSION_SIZE],
    exclusion_list_size: i32,
    port: u16,
}

#[link(name = "net_connection")]
unsafe extern "C" {
    fn OH_NetConn_SetAppHttpProxy(http_proxy: *mut NetConnHttpProxy) -> i32;
}

fn fill_c_buffer(dst: &mut [c_char], src: &str, field: &str) -> Result<(), WebViewError> {
    let trimmed = src.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    if trimmed.bytes().any(|b| b == 0) {
        return Err(WebViewError::WebView(format!(
            "{} contains interior NUL byte",
            field
        )));
    }
    if trimmed.len() >= dst.len() {
        return Err(WebViewError::WebView(format!(
            "{} exceeds max length {}",
            field,
            dst.len() - 1
        )));
    }
    for (idx, byte) in trimmed.bytes().enumerate() {
        dst[idx] = byte as c_char;
    }
    Ok(())
}

pub(crate) fn apply_http_proxy(
    config: Option<&ProxyConfig>,
) -> Result<ProxyApplyReport, WebViewError> {
    let mut raw: Box<NetConnHttpProxy> = Box::new(unsafe { std::mem::zeroed() });

    if let Some(proxy) = config {
        fill_c_buffer(&mut raw.host, &proxy.host, "proxy host")?;
        raw.port = proxy.port;

        let mut filled: i32 = 0;
        for rule in &proxy.bypass {
            if rule.trim().is_empty() {
                continue;
            }
            if (filled as usize) >= NETCONN_MAX_EXCLUSION_SIZE {
                break;
            }
            fill_c_buffer(
                &mut raw.exclusion_list[filled as usize],
                rule,
                "proxy bypass rule",
            )?;
            filled += 1;
        }
        raw.exclusion_list_size = filled;
    }

    let rc = unsafe { OH_NetConn_SetAppHttpProxy(raw.as_mut() as *mut NetConnHttpProxy) };
    if rc != 0 {
        return Err(WebViewError::WebView(format!(
            "OH_NetConn_SetAppHttpProxy failed with code {}",
            rc
        )));
    }

    let report = if config.is_some() {
        ProxyApplyReport::applied(ProxyActivation::EffectiveNow)
    } else {
        ProxyApplyReport::cleared(ProxyActivation::EffectiveNow)
    };
    Ok(report)
}

// Static C strings for proxy object and method names
static LINGXIA_PROXY_NAME: &[u8] = b"LingXiaProxy\0";
static LINGXIA_PROXY_GET_PORT: &[u8] = b"getPort\0";
static LINGXIA_PROXY_NATIVE_COMPONENT_UPDATE: &[u8] = b"nativeComponentUpdate\0";

// Keep proxy method array alive for WebView lifetime
#[repr(C)]
struct ProxyStorage {
    method: Box<[ArkWeb_ProxyMethod; 2]>,
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

type WebViewCreationSender = WebViewCreateSender;

#[derive(Debug, Default, Clone, Copy)]
struct WebMessagePorts {
    native_port: Option<*mut ArkWeb_WebMessagePort>,
    console_port: Option<*mut ArkWeb_WebMessagePort>,
    webview_native_port: Option<*mut ArkWeb_WebMessagePort>,
    webview_console_port: Option<*mut ArkWeb_WebMessagePort>,
}

pub struct WebViewInner {
    pub(crate) webtag: WebTag,
    /// ArkWeb-facing tag for controller operations (may include `#session` suffix).
    ark_webtag: Mutex<String>,
    ports: Mutex<WebMessagePorts>,
    /// Condition variable for message port readiness (avoids busy-wait)
    port_ready_signal: (Mutex<bool>, Condvar),
    creation_sender: Mutex<Option<WebViewCreationSender>>,
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

    // Sync the ArkWeb-facing tag to the latest controller tag and drop any cached ports that
    // belonged to the previous controller instance.
    {
        if let Ok(mut tag) = webview.inner.ark_webtag.lock()
            && tag.as_str() != webtag_str
        {
            log::debug!(
                "WebView controller created: updating ark_webtag {} -> {}",
                tag.as_str(),
                webtag_str
            );
            *tag = webtag_str.to_string();
        }
        // Old ports can silently drop messages after controller recreation; reset them here so
        // `getPort()` triggers a fresh port setup.
        webview.inner.cleanup_webmessage_ports();
        // Proxy callback storage from previous controller must be dropped before re-registering.
        webview.inner.cleanup_proxy_allocs();
    }

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
            log::debug!("Registered ArkWeb lifecycle callbacks for {}", webtag_str);
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

    if let Ok(mut sender_opt) = webview.inner.creation_sender.lock()
        && let Some(sender) = sender_opt.take()
    {
        sender.succeed(webview.clone());
        log::info!("WebView creation acknowledged for {}", webtag_str);
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
        webview.inner.cleanup_proxy_allocs();
        webview.inner.cleanup_scheme_handlers();
    }
}

/// Register LingXiaProxy for a specific webtag
fn register_proxy_for_webtag(webtag: &WebTag) -> Result<(), WebViewError> {
    unsafe {
        let webtag_cstr = cstring_from_str("webtag", webtag.as_str())?;

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
                if let Some(p) = allocs.first() {
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
            let storage = Box::new(ProxyStorage {
                method: Box::new([
                    ArkWeb_ProxyMethod {
                        methodName: LINGXIA_PROXY_GET_PORT.as_ptr() as *const c_char,
                        callback: Some(get_port_callback),
                        userData: std::ptr::null_mut(),
                    },
                    ArkWeb_ProxyMethod {
                        methodName: LINGXIA_PROXY_NATIVE_COMPONENT_UPDATE.as_ptr() as *const c_char,
                        callback: Some(native_component_update_callback),
                        userData: std::ptr::null_mut(),
                    },
                ]),
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

/// Native component props update callback -
/// handles LingXiaProxy.nativeComponentUpdate(...)
/// Accepts both:
/// 1) nativeComponentUpdate(componentId, propsJson)
/// 2) nativeComponentUpdate(propsJsonWithComponentId)
unsafe extern "C" fn native_component_update_callback(
    web_tag: *const std::ffi::c_char,
    bridge_data: *const ArkWeb_JavaScriptBridgeData,
    data_count: usize,
    _user_data: *mut std::ffi::c_void,
) {
    if web_tag.is_null() || data_count < 1 || bridge_data.is_null() {
        log::warn!(
            "native_component_update_callback missing web_tag or args data_count={}",
            data_count
        );
        return;
    }

    unsafe {
        let Ok(webtag_str) = CStr::from_ptr(web_tag).to_str() else {
            log::warn!("native_component_update_callback invalid web_tag");
            return;
        };
        let webtag = WebTag::from(webtag_str);
        let mut component_id = String::new();
        let props_json = if data_count >= 2 {
            let component_data = &*bridge_data.offset(0);
            let props_data = &*bridge_data.offset(1);
            component_id = extract_string_from_bridge_data(component_data)
                .unwrap_or_default()
                .trim()
                .to_string();
            extract_string_from_bridge_data(props_data).unwrap_or_default()
        } else {
            // Single-arg form: payload is expected to be JSON string containing componentId.
            let payload_data = &*bridge_data.offset(0);
            extract_string_from_bridge_data(payload_data).unwrap_or_default()
        };

        if component_id.is_empty() && !props_json.is_empty() {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&props_json) {
                if let Some(id) = json.get("componentId").and_then(|v| v.as_str()) {
                    component_id = id.trim().to_string();
                }
            }
        }

        if component_id.is_empty() || props_json.is_empty() {
            log::warn!(
                "native_component_update_callback empty component_or_props webtag={} component_id_len={} props_len={}",
                webtag.as_str(),
                component_id.len(),
                props_json.len()
            );
            return;
        }

        log::debug!(
            "native_component_update_callback recv webtag={} component_id={} props_len={} data_count={}",
            webtag.as_str(),
            component_id,
            props_json.len(),
            data_count
        );

        if let Err(e) = call_arkts(
            "nativeComponentPropsUpdate",
            &[webtag.as_str(), component_id.as_str(), props_json.as_str()],
        ) {
            log::error!(
                "native_component_update_callback failed for {} {}: {}",
                webtag.as_str(),
                component_id,
                e
            );
        }
    }
}

/// Get port callback - handles LingXiaProxy.getPort(type) calls
unsafe extern "C" fn get_port_callback(
    web_tag: *const std::ffi::c_char,
    bridge_data: *const ArkWeb_JavaScriptBridgeData,
    data_count: usize,
    _user_data: *mut std::ffi::c_void,
) {
    if web_tag.is_null() || data_count < 1 || bridge_data.is_null() {
        log::warn!("get_port_callback missing web_tag or args");
        return;
    }

    unsafe {
        let Ok(webtag_str) = CStr::from_ptr(web_tag).to_str() else {
            log::warn!("get_port_callback invalid web_tag");
            return;
        };
        let webtag = WebTag::from(webtag_str);
        let type_data = &*bridge_data.offset(0);

        if let Some(port_type_str) = extract_string_from_bridge_data(type_data) {
            // Ensure ports exist; create on-demand if onPageBegin hasn't run yet
            match port_type_str.as_str() {
                "ConsolePort" => {
                    let need_setup = find_webview(&webtag)
                        .map(|wv| wv.inner.ports_snapshot().webview_console_port.is_none())
                        .unwrap_or(true);
                    if need_setup
                        && let Err(e) = setup_webmessage_port_for_webtag(
                            &webtag,
                            PortType::Console,
                            on_console_message_received,
                        )
                    {
                        log::error!(
                            "On-demand console port setup failed for {}: {}",
                            webtag.as_str(),
                            e
                        );
                    }
                    if let Err(e) = send_port_to_webview_for_webtag(&webtag, PortType::Console) {
                        log::error!("Failed to send console port: {}", e);
                    }
                }
                "LingXiaPort" => {
                    let need_setup = find_webview(&webtag)
                        .map(|wv| wv.inner.ports_snapshot().webview_native_port.is_none())
                        .unwrap_or(true);

                    if need_setup
                        && let Err(e) = setup_webmessage_port_for_webtag(
                            &webtag,
                            PortType::Message,
                            on_web_message_received,
                        )
                    {
                        log::error!(
                            "On-demand message port setup failed for {}: {}",
                            webtag.as_str(),
                            e
                        );
                    }
                    if let Err(e) = send_port_to_webview_for_webtag(&webtag, PortType::Message) {
                        log::error!("Failed to send message port: {}", e);
                    }
                }
                _ => {
                    log::warn!("Unknown port type: {}", port_type_str);
                }
            }
        } else {
            log::warn!(
                "LingXiaProxy.getPort: failed to parse type arg for webtag={}",
                webtag.as_str()
            );
        }
    }
}

/// Extract string from bridge data
fn extract_string_from_bridge_data(data: &ArkWeb_JavaScriptBridgeData) -> Option<String> {
    unsafe {
        if !data.buffer.is_null() && data.size > 0 {
            let bytes = std::slice::from_raw_parts(data.buffer, data.size);
            let s = std::str::from_utf8(bytes).ok()?;
            let trimmed = s.trim_matches('\0').trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
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
    fn ark_webtag_string(&self) -> String {
        self.ark_webtag
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|p| p.into_inner().to_string())
    }

    fn with_ports<R>(&self, f: impl FnOnce(&mut WebMessagePorts) -> R) -> R {
        match self.ports.lock() {
            Ok(mut ports) => f(&mut ports),
            Err(poisoned) => f(&mut poisoned.into_inner()),
        }
    }

    fn ports_snapshot(&self) -> WebMessagePorts {
        self.with_ports(|ports| *ports)
    }

    /// Create a WebView instance
    pub fn create(
        appid: &str,
        path: &str,
        session_id: Option<u64>,
        effective_options: EffectiveWebViewCreateOptions,
        sender: WebViewCreateSender,
    ) {
        if session_id.is_none() {
            log::warn!(
                "Creating Harmony WebView without session id for {}-{}",
                appid,
                path
            );
        }
        let webtag = WebTag::new(appid, path, session_id);
        let options_token = match encode_options_token(&effective_options) {
            Ok(token) => token,
            Err(e) => {
                sender.fail(WebViewCreateStage::Requested, e);
                return;
            }
        };

        // Create WebView instance, storing the sender
        let webview_inner = WebViewInner {
            webtag: webtag.clone(),
            ark_webtag: Mutex::new(webtag.as_str().to_string()),
            ports: Mutex::new(WebMessagePorts::default()),
            port_ready_signal: (Mutex::new(false), Condvar::new()),
            creation_sender: Mutex::new(Some(sender)),
            proxy_allocs: RefCell::new(Vec::new()),
            callbacks_registered: RefCell::new(false),
            scheme_handlers: RefCell::new(Vec::new()),
        };

        // Create WebView wrapper and register it
        let webview = Arc::new(crate::WebView::new(
            webview_inner,
            effective_options.clone(),
        ));
        register_webview(webview.clone());

        // Call ArkTS to create the WebView controller via TSFN (no callback path).
        // ArkTS will notify native through onWebviewControllerCreated(webtag)
        // once the ArkUI Web component is actually attached (onAppear).
        if let Err(e) = call_arkts(
            "createWebViewController",
            &[webtag.as_str(), &options_token],
        ) {
            log::error!("Failed to call createWebViewController: {}", e);
            if let Some(webview) = find_webview(&webtag)
                && let Ok(mut sender_opt) = webview.inner.creation_sender.lock()
                && let Some(s) = sender_opt.take()
            {
                s.fail(WebViewCreateStage::NativeCreated, e);
            }
            return;
        }

        // Register native ArkWeb scheme handlers driven by registered_schemes
        if let Err(e) = set_webview_scheme_handler(&webtag) {
            log::error!(
                "Failed to set scheme handler for {}: {}",
                webtag.as_str(),
                e
            );
        }
    }

    /// Track scheme handler for cleanup
    pub fn track_scheme_handler(&self, handler: *mut ohos_web_sys::ArkWeb_SchemeHandler) {
        self.scheme_handlers.borrow_mut().push(handler);
    }

    /// Cleanup all tracked proxy allocations (method arrays / callback metadata).
    fn cleanup_proxy_allocs(&self) {
        let proxies = self.proxy_allocs.borrow_mut().drain(..).collect::<Vec<_>>();
        let count = proxies.len();
        for p in proxies {
            unsafe {
                let _ = Box::from_raw(p as *mut ProxyStorage);
            }
        }
        if count > 0 {
            log::info!(
                "Cleaned up {} proxy allocations for {}",
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
        self.set_port_ready(false);
        unsafe {
            // Get port API if available
            if let Ok(port_api) = get_port_api() {
                let mut cleanup_count = 0;
                let ark_webtag = self.ark_webtag_string();
                let webtag_cstr = match cstring_from_str("ark_webtag", &ark_webtag) {
                    Ok(value) => value,
                    Err(e) => {
                        log::error!(
                            "Skip WebMessage port cleanup for {}: {}",
                            self.webtag.as_str(),
                            e
                        );
                        return;
                    }
                };
                self.with_ports(|ports| {
                    // Cleanup native message port
                    if let Some(port) = ports.native_port.take()
                        && let Some(close_fn) = port_api.close
                    {
                        close_fn(port, webtag_cstr.as_ptr());
                        cleanup_count += 1;
                    }

                    // Cleanup webview message port
                    if let Some(port) = ports.webview_native_port.take()
                        && let Some(close_fn) = port_api.close
                    {
                        close_fn(port, webtag_cstr.as_ptr());
                        cleanup_count += 1;
                    }

                    // Cleanup console port
                    if let Some(port) = ports.console_port.take()
                        && let Some(close_fn) = port_api.close
                    {
                        close_fn(port, webtag_cstr.as_ptr());
                        cleanup_count += 1;
                    }

                    // Cleanup webview console port
                    if let Some(port) = ports.webview_console_port.take()
                        && let Some(close_fn) = port_api.close
                    {
                        close_fn(port, webtag_cstr.as_ptr());
                        cleanup_count += 1;
                    }
                });

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
            let ark_webtag = self.ark_webtag_string();
            let webtag_cstr = cstring_from_str("ark_webtag", &ark_webtag)?;
            let controller_api =
                OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_CONTROLLER);
            if controller_api.is_null() {
                return Err(WebViewError::WebView(
                    "Failed to get Controller API".to_string(),
                ));
            }
            let controller = &*(controller_api as *const ArkWeb_ControllerAPI);

            // Use borrow() instead of take() - we need to keep the port reference
            let ports = self.ports_snapshot();
            let (port_opt, message, port_name) = match port_type {
                PortType::Console => (
                    ports.webview_console_port,
                    "LingXia-console-init",
                    "console",
                ),
                PortType::Message => (ports.webview_native_port, "LingXia-port-init", "message"),
            };

            if let Some(webview_port) = port_opt {
                // Prepare stable CStrings for the call duration
                let msg_cstr = cstring_from_str("port_init_message", message)?;
                let target_cstr = cstring_from_str("port_target", "*")?;

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
    fn load_url(&self, url: &str) -> Result<(), WebViewError> {
        let ark_tag = self.ark_webtag_string();
        call_arkts("loadUrl", &[&ark_tag, &url])
    }

    fn load_data(&self, request: LoadDataRequest<'_>) -> Result<(), WebViewError> {
        unsafe {
            let ark_webtag = self.ark_webtag_string();
            let webtag_cstr = cstring_from_str("ark_webtag", &ark_webtag)?;
            let data_cstr = cstring_from_str("load_data.data", request.data)?;
            let base_url_cstr = cstring_from_str("load_data.base_url", request.base_url)?;

            // Use history_url if provided, otherwise use base_url
            let history_url_str = request.history_url.unwrap_or(request.base_url);
            let history_url_cstr = cstring_from_str("load_data.history_url", history_url_str)?;

            // Use the native HarmonyOS OH_NativeArkWeb_LoadData function
            let result = OH_NativeArkWeb_LoadData(
                webtag_cstr.as_ptr(),
                data_cstr.as_ptr(),
                b"text/html\0".as_ptr().cast::<c_char>(), // MIME type: text/html
                b"UTF-8\0".as_ptr().cast::<c_char>(),     // Encoding: UTF-8
                base_url_cstr.as_ptr(),
                history_url_cstr.as_ptr(),
            );

            if result == ArkWeb_ErrorCode_ARKWEB_SUCCESS {
                log::info!(
                    "Successfully loaded data into WebView {} with base URL: {}",
                    self.webtag.as_str(),
                    request.base_url
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

    fn evaluate_javascript(&self, js: &str) -> Result<(), WebViewError> {
        // Evaluate JS via ArkWeb controller API directly.
        unsafe {
            let ark_webtag = self.ark_webtag_string();
            let web_tag_cstr = cstring_from_str("ark_webtag", &ark_webtag)?;

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
            let js_cstr = cstring_from_str("evaluate_javascript.js", js)?;
            let js_object = ArkWeb_JavaScriptObject {
                buffer: js_cstr.as_ptr() as *mut u8,
                size: js.len(),
                callback: None,
                userData: std::ptr::null_mut(),
            };

            if let Some(run_js) = controller.runJavaScript {
                run_js(web_tag_cstr.as_ptr(), &js_object);
                Ok(())
            } else {
                Err(WebViewError::WebView(
                    "runJavaScript function not available".to_string(),
                ))
            }
        }
    }

    fn clear_browsing_data(&self) -> Result<(), WebViewError> {
        let ark_tag = self.ark_webtag_string();
        call_arkts("clearBrowsingData", &[&ark_tag])
    }

    fn set_user_agent(&self, ua: &str) -> Result<(), WebViewError> {
        let ark_tag = self.ark_webtag_string();
        call_arkts("setUserAgent", &[&ark_tag, &ua])
    }

    fn post_message(&self, message: &str) -> Result<(), WebViewError> {
        self.post_message_internal(message)
    }
}

impl WebViewInner {
    /// Set port ready state and notify waiters
    fn set_port_ready(&self, ready: bool) {
        let (lock, cvar) = &self.port_ready_signal;
        let mut is_ready = lock_or_recover(lock, "harmony.port_ready_signal.set");
        *is_ready = ready;
        if ready {
            cvar.notify_all();
        }
    }

    /// Check if port is ready (non-blocking)
    fn is_port_ready(&self) -> bool {
        let (lock, _) = &self.port_ready_signal;
        *lock_or_recover(lock, "harmony.port_ready_signal.get")
    }

    fn refresh_message_port(&self) -> Result<(), WebViewError> {
        self.set_port_ready(false);
        self.cleanup_webmessage_ports();
        setup_webmessage_port_for_webtag(&self.webtag, PortType::Message, on_web_message_received)?;
        self.send_port(PortType::Message)?;
        Ok(())
    }

    /// Wait for message port to become ready (non-busy, uses Condvar)
    fn wait_for_message_port_ready(&self, timeout: Duration) -> bool {
        let (lock, cvar) = &self.port_ready_signal;
        let guard = lock_or_recover(lock, "harmony.port_ready_signal.wait");
        let result = match cvar.wait_timeout_while(guard, timeout, |ready| !*ready) {
            Ok(value) => value,
            Err(poisoned) => {
                log::error!("Condvar wait poisoned at harmony.port_ready_signal.wait, recovering");
                poisoned.into_inner()
            }
        };
        !result.1.timed_out()
    }

    fn post_message_internal(&self, message: &str) -> Result<(), WebViewError> {
        let ark_webtag = self.ark_webtag_string();
        let webtag_cstr = cstring_from_str("ark_webtag", &ark_webtag)?;

        let message_api = get_message_api()
            .map_err(|_| WebViewError::WebView("WebMessage API not available".to_string()))?;
        let port_api = get_port_api()
            .map_err(|_| WebViewError::WebView("WebMessagePort API not available".to_string()))?;

        let post_fn = port_api
            .postMessage
            .ok_or_else(|| WebViewError::WebView("postMessage not available".to_string()))?;
        let create_fn = message_api
            .createWebMessage
            .ok_or_else(|| WebViewError::WebView("createWebMessage not available".to_string()))?;
        let set_data = message_api
            .setData
            .ok_or_else(|| WebViewError::WebView("setData not available".to_string()))?;

        // ArkWeb payload handling differs across devices. Some implementations appear to copy
        // `len` bytes and later treat the buffer as a C string.
        // We include the trailing NUL in `len` to ensure the buffer is safely terminated.
        // The JS side must handle/strip the trailing null if necessary.
        let c_string = CString::new(message).map_err(|_| {
            WebViewError::WebView("Failed to build CString for message".to_string())
        })?;
        let byte_len = c_string.as_bytes_with_nul().len();
        let data_ptr = c_string.as_ptr() as *mut std::ffi::c_void;

        let post_once = |port: *mut ArkWeb_WebMessagePort| -> Result<u32, WebViewError> {
            let web_message = unsafe { create_fn() };
            if web_message.is_null() {
                return Err(WebViewError::WebView(
                    "Failed to create WebMessage".to_string(),
                ));
            }

            if let Some(set_type) = message_api.setType {
                unsafe {
                    set_type(web_message, ArkWeb_WebMessageType_ARKWEB_STRING);
                }
            }

            unsafe {
                set_data(web_message, data_ptr, byte_len);
            }

            let result = unsafe { post_fn(port, webtag_cstr.as_ptr(), web_message) };

            if let Some(destroy_message) = message_api.destroyWebMessage {
                let mut msg_ptr = web_message;
                unsafe {
                    destroy_message(&mut msg_ptr as *mut *mut ArkWeb_WebMessage);
                }
            }

            Ok(result)
        };

        let get_port = || self.ports_snapshot().native_port;

        if get_port().is_none() {
            self.refresh_message_port()?;
        }

        if !self.is_port_ready() {
            let _ = self.send_port(PortType::Message);
            self.wait_for_message_port_ready(Duration::from_millis(200));
        }

        let port = get_port().ok_or_else(|| {
            WebViewError::WebView("native message port not available".to_string())
        })?;
        let result = post_once(port)?;
        if result == 0 {
            return Ok(());
        }

        // Treat any non-zero error as a potentially stale/closed port and recreate the channel.
        log::warn!(
            "postMessage failed for {} (error {}), refreshing WebMessagePort and retrying",
            self.webtag.as_str(),
            result
        );
        self.refresh_message_port()?;
        self.wait_for_message_port_ready(Duration::from_millis(200));

        let port_retry = get_port().ok_or_else(|| {
            WebViewError::WebView("native message port not available".to_string())
        })?;
        let retry_result = post_once(port_retry)?;
        if retry_result == 0 {
            return Ok(());
        }

        Err(WebViewError::WebView(format!(
            "postMessage failed after refresh with error {}",
            retry_result
        )))
    }
}

impl Drop for WebViewInner {
    fn drop(&mut self) {
        // Cleanup all tracked scheme handlers first
        self.cleanup_scheme_handlers();

        // Cleanup WebMessage ports
        self.cleanup_webmessage_ports();

        // Free proxy allocations
        self.cleanup_proxy_allocs();

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

/// Register WebView lifecycle callbacks
fn register_webview_callbacks(webtag: &WebTag) -> Result<(), WebViewError> {
    unsafe {
        let webtag_cstr = cstring_from_str("webtag", webtag.as_str())?;

        // Get the ArkWeb_ComponentAPI using the correct API
        let component_api =
            OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_COMPONENT);
        if component_api.is_null() {
            return Err(WebViewError::WebView(
                "Failed to get ArkWeb_ComponentAPI".to_string(),
            ));
        }

        let api = &*(component_api as *const ArkWeb_ComponentAPI);

        // Register lifecycle callbacks. We use web_tag from callback args and do not rely on user_data.
        if let Some(on_controller_attached) = api.onControllerAttached {
            on_controller_attached(
                webtag_cstr.as_ptr(),
                Some(on_controller_attached_callback),
                std::ptr::null_mut(),
            );
        }

        if let Some(on_page_begin) = api.onPageBegin {
            on_page_begin(
                webtag_cstr.as_ptr(),
                Some(on_page_begin_callback),
                std::ptr::null_mut(),
            );
        }

        if let Some(on_page_end) = api.onPageEnd {
            on_page_end(
                webtag_cstr.as_ptr(),
                Some(on_page_end_callback),
                std::ptr::null_mut(),
            );
        }

        if let Some(on_destroy) = api.onDestroy {
            on_destroy(
                webtag_cstr.as_ptr(),
                Some(on_destroy_callback),
                std::ptr::null_mut(),
            );
        }

        Ok(())
    }
}

// WebView lifecycle callback functions
extern "C" fn on_controller_attached_callback(web_tag: *const c_char, _user_data: *mut c_void) {
    if web_tag.is_null() {
        log::warn!("WebView controller attached callback received null web_tag");
        return;
    }
    if let Ok(webtag_str) = unsafe { CStr::from_ptr(web_tag).to_str() } {
        log::info!("WebView controller attached: {}", webtag_str);
    }
}

extern "C" fn on_page_begin_callback(web_tag: *const c_char, _user_data: *mut c_void) {
    if web_tag.is_null() {
        log::warn!("on_page_begin_callback received null web_tag");
        return;
    }
    if let Ok(webtag_str) = unsafe { CStr::from_ptr(web_tag).to_str() } {
        log::info!("Page begin loading: {}", webtag_str);

        let webtag = WebTag::from(webtag_str);
        if find_webview(&webtag).is_none() {
            log::debug!("Ignoring page begin for stale webview {}", webtag_str);
            return;
        }

        // Only inject console interception script; port setup is deferred to get_port_callback
        if let Err(e) = inject_console_script(&webtag) {
            log::error!("Failed to inject console script for {}: {}", webtag_str, e);
        }

        if let Some(delegate) = find_webview_delegate(&webtag) {
            delegate.on_page_started();
        }
    }
}

extern "C" fn on_page_end_callback(web_tag: *const c_char, _user_data: *mut c_void) {
    if web_tag.is_null() {
        log::warn!("on_page_end_callback received null web_tag");
        return;
    }
    if let Ok(webtag) = unsafe { CStr::from_ptr(web_tag).to_str() } {
        log::info!("Page end loading: {}", webtag);

        let webtag = WebTag::from(webtag);
        if find_webview(&webtag).is_none() {
            log::debug!("Ignoring page end for stale webview {}", webtag);
            return;
        }

        if let Some(delegate) = find_webview_delegate(&webtag) {
            delegate.on_page_finished();
        }
    }
}

extern "C" fn on_destroy_callback(web_tag: *const c_char, _user_data: *mut c_void) {
    if web_tag.is_null() {
        log::warn!("on_destroy_callback received null web_tag");
        return;
    }
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
        // Get APIs
        let controller_api =
            OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_CONTROLLER);
        let port_api =
            OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_WEB_MESSAGE_PORT);

        let controller = &*(controller_api as *const ArkWeb_ControllerAPI);
        let port_api_struct = &*(port_api as *const ArkWeb_WebMessagePortAPI);

        // Use the current Ark tag to avoid stale controller state.
        let webview = find_webview(webtag)
            .ok_or_else(|| WebViewError::WebView("WebView not found".to_string()))?;
        let ark_webtag = webview.inner.ark_webtag_string();
        let webtag_cstr = cstring_from_str("ark_webtag", &ark_webtag)?;

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

        let webview_inner = &webview.inner;
        webview_inner.with_ports(|ports| match port_type {
            PortType::Message => {
                ports.native_port = Some(port1);
                ports.webview_native_port = Some(port2);
            }
            PortType::Console => {
                ports.console_port = Some(port1);
                ports.webview_console_port = Some(port2);
            }
        });

        // Set message event handler
        if let Some(set_handler) = port_api_struct.setMessageEventHandler {
            set_handler(
                port1,
                webtag_cstr.as_ptr(),
                Some(callback_fn),
                std::ptr::null_mut(),
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
            if (window.__LingXiaConsoleInjected) return;
            window.__LingXiaConsoleInjected = true;
            const orig = {
                log: console.log,
                error: console.error,
                warn: console.warn,
                info: console.info
            };
            let port = null;

            function getPort() {
                if (window.LingXiaProxy?.getPort) {
                    const handleInit = (e) => {
                        if (e.data === 'LingXia-console-init') {
                            window.removeEventListener('message', handleInit);
                            port = e.ports[0];
                        }
                    };
                    window.addEventListener('message', handleInit);
                    window.LingXiaProxy.getPort('ConsolePort');
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

    let webview = find_webview(webtag).ok_or_else(|| {
        WebViewError::WebView(format!("WebView not found for webtag: {}", webtag.as_str()))
    })?;

    webview.inner.evaluate_javascript(console_script)
}

/// WebMessage callback
extern "C" fn on_web_message_received(
    web_tag: *const c_char,
    port: *mut ArkWeb_WebMessagePort,
    message: *mut ArkWeb_WebMessage,
    _user_data: *mut c_void,
) {
    if web_tag.is_null() {
        log::error!("on_web_message_received got null web_tag");
        return;
    }
    let Ok(webtag) = (unsafe { CStr::from_ptr(web_tag).to_str() }) else {
        log::error!("Failed to parse web_tag");
        return;
    };

    if message.is_null() {
        log::error!("message is null for {}", webtag);
        return;
    }

    let full_webtag = WebTag::from(webtag);
    let (appid, path) = full_webtag.extract_parts();

    // Keep native_port aligned with the port that delivered the message.
    if !port.is_null()
        && let Some(webview) = find_webview(&full_webtag)
    {
        webview.inner.set_port_ready(true);
        webview.inner.with_ports(|ports| {
            let prev = ports.native_port;
            ports.native_port = Some(port);
            if prev != Some(port) {
                log::debug!(
                    "on_web_message_received: updated native_port for {} (old={:?}, new={:?})",
                    full_webtag.as_str(),
                    prev,
                    port
                );
            }
        });
    }

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
        if let Some(delegate) = find_webview_delegate(&full_webtag) {
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

/// Check navigation policy for a given webtag and URL.
/// Returns `true` to intercept (cancel) the navigation, `false` to allow it.
/// Called from the ArkTS `onLoadIntercept` handler via NAPI.
pub fn check_navigation_policy(webtag_str: &str, url: &str) -> bool {
    let webtag = WebTag::from(webtag_str);
    if let Some(webview) = find_webview(&webtag) {
        return matches!(webview.handle_navigation(url), NavigationPolicy::Cancel);
    }

    false
}

pub fn on_download_start(
    webtag_str: &str,
    url: &str,
    user_agent: &str,
    content_disposition: &str,
    mime_type: &str,
    content_length: i64,
) -> bool {
    let webtag = WebTag::from(webtag_str);
    let Some(webview) = find_webview(&webtag) else {
        return false;
    };

    // Strict/lxapp pages should not trigger in-webview download flows.
    if webview.effective_options().profile != SecurityProfile::BrowserRelaxed {
        return false;
    }

    if !webview.effective_options().has_download_handler {
        return false;
    }

    let request = DownloadRequest {
        url: url.to_string(),
        user_agent: (!user_agent.trim().is_empty()).then(|| user_agent.to_string()),
        content_disposition: (!content_disposition.trim().is_empty())
            .then(|| content_disposition.to_string()),
        mime_type: (!mime_type.trim().is_empty()).then(|| mime_type.to_string()),
        content_length: (content_length >= 0).then_some(content_length as u64),
        suggested_filename: None,
        source_page_url: None,
        cookie: None,
    };
    webview.handle_download(request);
    true
}

pub fn on_file_chooser_requested(
    webtag_str: &str,
    request_id: &str,
    source_url: &str,
    accept_types_json: &str,
    allow_multiple: bool,
    allow_directories: bool,
    capture: bool,
) -> bool {
    let webtag = WebTag::from(webtag_str);
    let Some(webview) = find_webview(&webtag) else {
        return false;
    };

    let accept_types: Vec<String> = serde_json::from_str(accept_types_json).unwrap_or_default();
    let request = FileChooserRequest {
        accept_types,
        allow_multiple,
        allow_directories,
        capture,
        source_page_url: (!source_url.trim().is_empty()).then(|| source_url.to_string()),
    };

    let request_id_owned = request_id.to_string();
    webview.handle_file_chooser(request, move |response| {
        let payload = match response {
            FileChooserResponse::Cancel => "[]".to_string(),
            FileChooserResponse::Files(files) => {
                let selected: Vec<String> = files
                    .into_iter()
                    .filter_map(|file| file.uri.or(file.path))
                    .collect();
                serde_json::to_string(&selected).unwrap_or_else(|_| "[]".to_string())
            }
        };
        let _ = call_arkts(
            "completeWebFileChooserRequest",
            &[&request_id_owned, &payload],
        );
    })
}

fn harmony_load_error_kind(error_code: i32, description: &str) -> LoadErrorKind {
    match error_code {
        -2 => LoadErrorKind::Dns,
        -8 => LoadErrorKind::Timeout,
        -11 | -16 => LoadErrorKind::Security,
        -14 => LoadErrorKind::NotFound,
        -3 | -4 | -5 | -6 | -7 | -9 | -15 => LoadErrorKind::Network,
        -10 | -12 => LoadErrorKind::InvalidUrl,
        _ => {
            let desc = description.trim().to_ascii_lowercase();
            if desc.is_empty() {
                LoadErrorKind::Unknown
            } else if desc.contains("dns")
                || desc.contains("host")
                || desc.contains("name not resolved")
            {
                LoadErrorKind::Dns
            } else if desc.contains("timeout") || desc.contains("timed out") {
                LoadErrorKind::Timeout
            } else if desc.contains("ssl")
                || desc.contains("tls")
                || desc.contains("certificate")
                || desc.contains("secure connection")
            {
                LoadErrorKind::Security
            } else if desc.contains("cancel") || desc.contains("aborted") {
                LoadErrorKind::Cancelled
            } else if desc.contains("bad url")
                || desc.contains("invalid url")
                || desc.contains("malformed")
                || desc.contains("unsupported scheme")
            {
                LoadErrorKind::InvalidUrl
            } else if desc.contains("not found") || desc.contains("no such file") {
                LoadErrorKind::NotFound
            } else if desc.contains("network")
                || desc.contains("offline")
                || desc.contains("internet")
                || desc.contains("connect")
                || desc.contains("connection")
            {
                LoadErrorKind::Network
            } else {
                LoadErrorKind::Unknown
            }
        }
    }
}

pub fn on_load_error(webtag_str: &str, url: &str, error_code: i32, description: &str) {
    let webtag = WebTag::from(webtag_str);
    if let Some(delegate) = find_webview_delegate(&webtag) {
        delegate.on_load_error(&LoadError {
            url: (!url.is_empty()).then(|| url.to_string()),
            kind: harmony_load_error_kind(error_code, description),
            description: description.to_string(),
        });
    }
}

/// Console WebMessage callback
extern "C" fn on_console_message_received(
    web_tag: *const c_char,
    _port: *mut ArkWeb_WebMessagePort,
    message: *mut ArkWeb_WebMessage,
    _user_data: *mut c_void,
) {
    if web_tag.is_null() {
        return;
    }
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

        if let Ok(console_msg) = serde_json::from_str::<serde_json::Value>(msg_str)
            && let (Some(level), Some(console_message)) = (
                console_msg.get("level").and_then(|v| v.as_str()),
                console_msg.get("message").and_then(|v| v.as_str()),
            )
        {
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
            if let Some(delegate) = find_webview_delegate(&full_webtag) {
                delegate.log(log_level, console_message);
            }
        }
    }
}
