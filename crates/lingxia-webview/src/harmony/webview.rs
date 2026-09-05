use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

use crate::events::normalizer::{self, NativeNavigationResult, NativeSignal};
use crate::harmony::schemehandler::set_webview_scheme_handler;
use crate::harmony::tsfn::call_arkts;
use crate::harmony_document::{DocumentCommit, HarmonyDocumentAuthority, PageBegin};
use crate::input_helper::{build_async_eval_body, new_eval_token, parse_wrapped_eval_result};
use crate::traits::{
    DocumentBinding, DocumentGeneration, DocumentOutboundGate, FileChooserRequest,
    FileChooserResponse, LoadError, LoadErrorKind, NavigationPolicy, WebMessageFrame,
    WebMessageSource, WebMessageTransport,
};
use crate::webview::{
    EffectiveWebViewCreateOptions, ProxyActivation, ProxyApplyReport, ProxyConfig, SecurityProfile,
    WebTag, WebViewCreateSender, WebViewCreateStage, find_webview, find_webview_by_native_view_id,
    register_webview,
};
use crate::{
    DownloadRequest, LoadDataRequest, LogLevel, NativeWebViewId, UserAgentOverride, WebView,
    WebViewController, WebViewError, WebViewScriptError,
};
use async_trait::async_trait;
use ohos_web_sys::*;

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_void};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time::timeout;

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
static LINGXIA_PROXY_RESOLVE_EVAL: &[u8] = b"resolveEval\0";

// Keep proxy method array alive for WebView lifetime
#[repr(C)]
struct ProxyStorage {
    method: Box<[ArkWeb_ProxyMethod; 3]>,
    callback_token: u64,
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
type PendingEvalRequests = Arc<Mutex<HashMap<u64, PendingEvalEntry>>>;
type PendingScreenshotRequests = Arc<Mutex<HashMap<u64, PendingScreenshotEntry>>>;

enum PendingEvalResponse {
    Success(String),
    Failure(String),
    Destroyed,
}

struct PendingEvalEntry {
    webtag: String,
    token: String,
    sender: oneshot::Sender<PendingEvalResponse>,
}

enum PendingScreenshotResponse {
    Success(Vec<u8>),
    Failure(String),
    Destroyed,
}

struct PendingScreenshotEntry {
    webtag: String,
    sender: oneshot::Sender<PendingScreenshotResponse>,
}

static PENDING_EVAL_REQUESTS: OnceLock<PendingEvalRequests> = OnceLock::new();
static PENDING_SCREENSHOT_REQUESTS: OnceLock<PendingScreenshotRequests> = OnceLock::new();
static NEXT_EVAL_REQUEST_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_SCREENSHOT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);
const EVAL_TIMEOUT: Duration = Duration::from_secs(10);
const EVAL_PARSE_GUARD_MS: u64 = 1000;
const SCREENSHOT_TIMEOUT: Duration = Duration::from_secs(5);

fn pending_eval_requests() -> &'static PendingEvalRequests {
    PENDING_EVAL_REQUESTS.get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
}

fn pending_screenshot_requests() -> &'static PendingScreenshotRequests {
    PENDING_SCREENSHOT_REQUESTS.get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
}

pub fn complete_pending_screenshot_request(request_id: u64, result: Result<Vec<u8>, String>) {
    if let Ok(mut pending) = pending_screenshot_requests().lock()
        && let Some(entry) = pending.remove(&request_id)
    {
        let message = match result {
            Ok(bytes) => PendingScreenshotResponse::Success(bytes),
            Err(error) => PendingScreenshotResponse::Failure(error),
        };
        let _ = entry.sender.send(message);
    }
}

fn fail_pending_screenshot_requests_for_webtag(webtag: &WebTag) {
    if let Ok(mut pending) = pending_screenshot_requests().lock() {
        let matching = pending
            .iter()
            .filter_map(|(request_id, entry)| {
                (entry.webtag == webtag.as_str()).then_some(*request_id)
            })
            .collect::<Vec<_>>();
        for request_id in matching {
            if let Some(entry) = pending.remove(&request_id) {
                let _ = entry.sender.send(PendingScreenshotResponse::Destroyed);
            }
        }
    }
}

fn complete_pending_eval_request(request_id: u64, token: &str, result: Result<String, String>) {
    if let Ok(mut pending) = pending_eval_requests().lock()
        && pending
            .get(&request_id)
            .is_some_and(|entry| entry.token == token)
        && let Some(entry) = pending.remove(&request_id)
    {
        let message = match result {
            Ok(value) => PendingEvalResponse::Success(value),
            Err(error) => PendingEvalResponse::Failure(error),
        };
        let _ = entry.sender.send(message);
    }
}

fn fail_pending_eval_requests_for_webtag(webtag: &WebTag) {
    if let Ok(mut pending) = pending_eval_requests().lock() {
        let matching = pending
            .iter()
            .filter_map(|(request_id, entry)| {
                (entry.webtag == webtag.as_str()).then_some(*request_id)
            })
            .collect::<Vec<_>>();
        for request_id in matching {
            if let Some(entry) = pending.remove(&request_id) {
                let _ = entry.sender.send(PendingEvalResponse::Destroyed);
            }
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct WebMessagePorts {
    document_generation: Option<DocumentGeneration>,
    native_port: Option<*mut ArkWeb_WebMessagePort>,
    native_message_callback_token: Option<u64>,
    console_port: Option<*mut ArkWeb_WebMessagePort>,
    console_message_callback_token: Option<u64>,
    webview_native_port: Option<*mut ArkWeb_WebMessagePort>,
    webview_console_port: Option<*mut ArkWeb_WebMessagePort>,
}

// ArkWeb retains `user_data` beyond the Rust call that registers a callback.
// Use a non-dereferenced numeric token rather than a heap pointer: a late
// callback can then only look up a removed binding and is never an UAF.
static NEXT_MESSAGE_CALLBACK_TOKEN: AtomicU64 = AtomicU64::new(1);
#[derive(Clone, Copy)]
struct MessageCallbackBinding {
    native_view_id: NativeWebViewId,
    document_generation: DocumentGeneration,
    port_type: PortType,
}

static MESSAGE_CALLBACK_BINDINGS: OnceLock<Mutex<HashMap<u64, MessageCallbackBinding>>> =
    OnceLock::new();
static NEXT_LIFECYCLE_CALLBACK_TOKEN: AtomicU64 = AtomicU64::new(1);
static LIFECYCLE_CALLBACK_BINDINGS: OnceLock<Mutex<HashMap<u64, NativeWebViewId>>> =
    OnceLock::new();
static NEXT_PROXY_CALLBACK_TOKEN: AtomicU64 = AtomicU64::new(1);
static PROXY_CALLBACK_BINDINGS: OnceLock<Mutex<HashMap<u64, NativeWebViewId>>> = OnceLock::new();

fn message_callback_bindings() -> &'static Mutex<HashMap<u64, MessageCallbackBinding>> {
    MESSAGE_CALLBACK_BINDINGS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn bind_message_callback(
    native_view_id: NativeWebViewId,
    document_generation: DocumentGeneration,
    port_type: PortType,
) -> u64 {
    // Never wrap: a late ArkWeb callback must not acquire a token later
    // reused for another concrete native WebView.
    let token = NEXT_MESSAGE_CALLBACK_TOKEN
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .expect("Harmony WebMessage callback token space exhausted");
    lock_or_recover(
        message_callback_bindings(),
        "harmony.message_callback_bindings.insert",
    )
    .insert(
        token,
        MessageCallbackBinding {
            native_view_id,
            document_generation,
            port_type,
        },
    );
    token
}

fn unbind_message_callback(token: u64) {
    if token != 0 {
        lock_or_recover(
            message_callback_bindings(),
            "harmony.message_callback_bindings.remove",
        )
        .remove(&token);
    }
}

fn binding_for_message_callback(user_data: *mut c_void) -> Option<MessageCallbackBinding> {
    let token = user_data as usize as u64;
    (token != 0).then(|| {
        lock_or_recover(
            message_callback_bindings(),
            "harmony.message_callback_bindings.get",
        )
        .get(&token)
        .copied()
    })?
}

fn lifecycle_callback_bindings() -> &'static Mutex<HashMap<u64, NativeWebViewId>> {
    LIFECYCLE_CALLBACK_BINDINGS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn bind_lifecycle_callback(native_view_id: NativeWebViewId) -> u64 {
    let token = NEXT_LIFECYCLE_CALLBACK_TOKEN
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .expect("Harmony lifecycle callback token space exhausted");
    lock_or_recover(
        lifecycle_callback_bindings(),
        "harmony.lifecycle_callback_bindings.insert",
    )
    .insert(token, native_view_id);
    token
}

fn unbind_lifecycle_callback(token: u64) {
    if token != 0 {
        lock_or_recover(
            lifecycle_callback_bindings(),
            "harmony.lifecycle_callback_bindings.remove",
        )
        .remove(&token);
    }
}

fn native_view_id_for_lifecycle_callback(user_data: *mut c_void) -> Option<NativeWebViewId> {
    let token = user_data as usize as u64;
    (token != 0).then(|| {
        lock_or_recover(
            lifecycle_callback_bindings(),
            "harmony.lifecycle_callback_bindings.get",
        )
        .get(&token)
        .copied()
    })?
}

fn proxy_callback_bindings() -> &'static Mutex<HashMap<u64, NativeWebViewId>> {
    PROXY_CALLBACK_BINDINGS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn bind_proxy_callback(native_view_id: NativeWebViewId) -> u64 {
    let token = NEXT_PROXY_CALLBACK_TOKEN
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .expect("Harmony proxy callback token space exhausted");
    lock_or_recover(
        proxy_callback_bindings(),
        "harmony.proxy_callback_bindings.insert",
    )
    .insert(token, native_view_id);
    token
}

fn unbind_proxy_callback(token: u64) {
    if token != 0 {
        lock_or_recover(
            proxy_callback_bindings(),
            "harmony.proxy_callback_bindings.remove",
        )
        .remove(&token);
    }
}

fn native_view_id_for_proxy_callback(user_data: *mut c_void) -> Option<NativeWebViewId> {
    let token = user_data as usize as u64;
    (token != 0).then(|| {
        lock_or_recover(
            proxy_callback_bindings(),
            "harmony.proxy_callback_bindings.get",
        )
        .get(&token)
        .copied()
    })?
}

pub struct WebViewInner {
    pub(crate) webtag: WebTag,
    /// Monotonic native-view callback token, unrelated to `DocumentGeneration`.
    /// Create/destroy travel to ArkTS on separate
    /// ThreadSafe channels with no cross-channel ordering, and webtags repeat
    /// across generations — ArkTS uses this to drop a stale destroy instead
    /// of tearing down the successor registered under the same tag.
    native_generation: String,
    native_view_id: NativeWebViewId,
    /// ArkWeb-facing tag for controller operations (may include `#session` suffix).
    ark_webtag: Mutex<String>,
    ports: Mutex<WebMessagePorts>,
    pending_port_requests: Mutex<(bool, bool)>,
    document_authority: HarmonyDocumentAuthority,
    /// Condition variable for message port readiness (avoids busy-wait)
    port_ready_signal: (Mutex<bool>, Condvar),
    creation_sender: Mutex<Option<WebViewCreationSender>>,
    // Keep proxy allocations alive for lifetime
    proxy_allocs: RefCell<Vec<*mut c_void>>,
    // Whether lifecycle callbacks have been registered with ArkWeb
    callbacks_registered: RefCell<bool>,
    lifecycle_callback_token: RefCell<Option<u64>>,
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

type WebMessageCallback =
    extern "C" fn(*const c_char, *mut ArkWeb_WebMessagePort, *mut ArkWeb_WebMessage, *mut c_void);

impl std::fmt::Display for PortType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PortType::Console => write!(f, "ConsolePort"),
            PortType::Message => write!(f, "MessagePort"),
        }
    }
}

pub fn webview_controller_created(
    webtag_str: &str,
    native_view_token: &str,
) -> Result<(), WebViewError> {
    let webtag = WebTag::from(webtag_str);
    let webview = webview_for_native_generation(&webtag, native_view_token).ok_or_else(|| {
        WebViewError::WebView(format!("WebView token is stale or missing: {webtag_str}"))
    })?;

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
        if let Err(e) = register_webview_callbacks(&webview) {
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
    if let Err(e) = register_proxy_for_webtag(&webtag, webview.native_view_id()) {
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
pub fn webview_controller_destroyed(webtag_str: &str, native_view_token: &str) {
    let webtag = WebTag::from(webtag_str);
    if let Some(webview) = webview_for_native_generation(&webtag, native_view_token) {
        let _ = on_render_exited(webtag_str, native_view_token);
        fail_pending_eval_requests_for_webtag(&webtag);
        fail_pending_screenshot_requests_for_webtag(&webtag);
        // Allow callbacks to be re-registered if a new controller is later created
        *webview.inner.callbacks_registered.borrow_mut() = false;
        webview.inner.cleanup_lifecycle_callbacks();

        // Idempotent cleanup of native resources tied to the old controller.
        webview.inner.cleanup_webmessage_ports();
        webview.inner.cleanup_proxy_allocs();
        webview.inner.cleanup_scheme_handlers();
    }
}

/// Register LingXiaProxy for a specific webtag
fn register_proxy_for_webtag(
    webtag: &WebTag,
    native_view_id: NativeWebViewId,
) -> Result<(), WebViewError> {
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
        let webview = find_webview_by_native_view_id(webtag, native_view_id).ok_or_else(|| {
            WebViewError::WebView(format!("WebView not found for webtag: {}", webtag.as_str()))
        })?;

        if let Some(register_proxy) = controller.registerJavaScriptProxy {
            // If storage already exists, reuse it to rebind into current page JS world
            if let Some(p) = webview.inner.proxy_allocs.borrow().first().copied() {
                let storage = p as *mut ProxyStorage;
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

            // First-time allocation path
            let callback_token = bind_proxy_callback(webview.native_view_id());
            let storage = Box::new(ProxyStorage {
                method: Box::new([
                    ArkWeb_ProxyMethod {
                        methodName: LINGXIA_PROXY_GET_PORT.as_ptr() as *const c_char,
                        callback: Some(get_port_callback),
                        userData: callback_token as usize as *mut c_void,
                    },
                    ArkWeb_ProxyMethod {
                        methodName: LINGXIA_PROXY_NATIVE_COMPONENT_UPDATE.as_ptr() as *const c_char,
                        callback: Some(native_component_update_callback),
                        userData: callback_token as usize as *mut c_void,
                    },
                    ArkWeb_ProxyMethod {
                        methodName: LINGXIA_PROXY_RESOLVE_EVAL.as_ptr() as *const c_char,
                        callback: Some(resolve_eval_callback),
                        userData: callback_token as usize as *mut c_void,
                    },
                ]),
                callback_token,
            });
            let storage = Box::into_raw(storage);

            let proxy_object = ArkWeb_ProxyObject {
                objName: LINGXIA_PROXY_NAME.as_ptr() as *const c_char,
                methodList: (*storage).method.as_ptr(),
                size: (*storage).method.len(),
            };
            register_proxy(webtag_cstr.as_ptr(), &proxy_object);

            // Keep allocations alive for WebView lifetime
            webview
                .inner
                .proxy_allocs
                .borrow_mut()
                .push(storage as *mut c_void);
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
    user_data: *mut std::ffi::c_void,
) {
    let Some(native_view_id) = native_view_id_for_proxy_callback(user_data) else {
        log::debug!("Dropping Harmony component callback without a live native-view binding");
        return;
    };
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
        if find_webview_by_native_view_id(&webtag, native_view_id).is_none() {
            log::debug!(
                "Dropping stale Harmony component callback for {}",
                webtag.as_str()
            );
            return;
        }
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

/// LingXiaProxy.resolveEval(requestIdStr, token, envelopeJson) — the Rust-issued
/// `eval_js` wrapper script awaits the user expression, builds an
/// {ok,value|error} envelope, then calls this proxy method with the requestId
/// and token it was given. We forward the envelope into the pending-eval
/// registry so the awaiting `eval_js` future resolves.
unsafe extern "C" fn resolve_eval_callback(
    web_tag: *const std::ffi::c_char,
    bridge_data: *const ArkWeb_JavaScriptBridgeData,
    data_count: usize,
    user_data: *mut std::ffi::c_void,
) {
    let Some(native_view_id) = native_view_id_for_proxy_callback(user_data) else {
        log::debug!("Dropping Harmony eval callback without a live native-view binding");
        return;
    };
    if web_tag.is_null() {
        return;
    }
    if bridge_data.is_null() || data_count < 3 {
        log::warn!(
            "resolve_eval_callback missing args data_count={}",
            data_count
        );
        return;
    }
    unsafe {
        let Ok(webtag_str) = CStr::from_ptr(web_tag).to_str() else {
            return;
        };
        if find_webview_by_native_view_id(&WebTag::from(webtag_str), native_view_id).is_none() {
            log::debug!("Dropping stale Harmony eval callback for {}", webtag_str);
            return;
        }
        let request_id_data = &*bridge_data.offset(0);
        let token_data = &*bridge_data.offset(1);
        let envelope_data = &*bridge_data.offset(2);
        let Some(request_id_str) = extract_string_from_bridge_data(request_id_data) else {
            log::warn!("resolve_eval_callback missing requestId");
            return;
        };
        let Ok(request_id) = request_id_str.parse::<u64>() else {
            log::warn!(
                "resolve_eval_callback invalid requestId: {}",
                request_id_str
            );
            return;
        };
        let Some(token) = extract_string_from_bridge_data(token_data) else {
            log::warn!("resolve_eval_callback missing token");
            return;
        };
        let envelope = extract_string_from_bridge_data(envelope_data).unwrap_or_default();
        complete_pending_eval_request(request_id, &token, Ok(envelope));
    }
}

/// Get port callback - handles LingXiaProxy.getPort(type) calls
unsafe extern "C" fn get_port_callback(
    web_tag: *const std::ffi::c_char,
    bridge_data: *const ArkWeb_JavaScriptBridgeData,
    data_count: usize,
    user_data: *mut std::ffi::c_void,
) {
    let Some(native_view_id) = native_view_id_for_proxy_callback(user_data) else {
        log::debug!("Dropping Harmony port callback without a live native-view binding");
        return;
    };
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
        let Some(webview) = find_webview_by_native_view_id(&webtag, native_view_id) else {
            log::debug!(
                "Dropping stale Harmony port callback for {}",
                webtag.as_str()
            );
            return;
        };
        let type_data = &*bridge_data.offset(0);

        if let Some(port_type_str) = extract_string_from_bridge_data(type_data) {
            // Ensure ports exist; create on-demand if onPageBegin hasn't run yet
            // A view asks for its port exactly when it is ready to receive
            // one. A pair pushed earlier may have been transferred into a
            // document that was not listening yet (a port transfers once), so
            // an explicit request always mints a fresh pair — reusing the old
            // one hands the view a dead channel and the bridge never
            // handshakes (blank-but-rendered re-entered pages on Harmony).
            let (port_type, callback) = match port_type_str.as_str() {
                "ConsolePort" => (
                    PortType::Console,
                    on_console_message_received as WebMessageCallback,
                ),
                "LingXiaPort" => (
                    PortType::Message,
                    on_web_message_received as WebMessageCallback,
                ),
                _ => {
                    log::warn!("Unknown port type: {}", port_type_str);
                    return;
                }
            };
            if port_type == PortType::Console
                && crate::webview::platform_console_delivery(
                    webview.effective_options().profile,
                    crate::webview::PlatformConsoleBackend::Harmony,
                ) != crate::webview::PlatformConsoleDelivery::DirectDelegate
            {
                return;
            }
            let DocumentBinding::Bound(document_generation) = webview.current_document_binding()
            else {
                webview.inner.defer_port_request(port_type);
                log::debug!(
                    "Deferring Harmony {} until a document commits for {}",
                    port_type,
                    webtag.as_str()
                );
                return;
            };
            if port_type == PortType::Message {
                webview.inner.set_port_ready(false);
            }
            if let Err(error) =
                setup_and_send_port_for_document(&webview, document_generation, port_type, callback)
            {
                log::error!(
                    "On-demand {} setup failed for {}: {}",
                    port_type,
                    webtag.as_str(),
                    error
                );
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

fn setup_and_send_port_for_document(
    webview: &WebView,
    document_generation: DocumentGeneration,
    port_type: PortType,
    callback: WebMessageCallback,
) -> Result<(), WebViewError> {
    let mut result = Err(WebViewError::WebView(
        "Harmony document changed before port setup".to_string(),
    ));
    let mut setup_if_harmony_document_current = || {
        result = setup_webmessage_port_for_webtag(
            &webview.inner.webtag,
            webview.native_view_id(),
            document_generation,
            port_type,
            callback,
        )
        .and_then(|()| webview.inner.send_port_unchecked(port_type));
    };
    let mut setup = || {
        webview
            .inner
            .document_authority
            .with_current_generation(document_generation, &mut setup_if_harmony_document_current);
    };
    normalizer::with_current_document_binding(
        webview.native_view_id(),
        document_generation,
        &mut setup,
    );
    result
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

        static NATIVE_GENERATION: std::sync::atomic::AtomicU64 =
            std::sync::atomic::AtomicU64::new(0);
        let native_generation = NATIVE_GENERATION
            .fetch_update(
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
                |current| current.checked_add(1),
            )
            .expect("Harmony native generation space exhausted")
            .checked_add(1)
            .expect("Harmony native generation space exhausted")
            .to_string();

        // Reserve the non-reusable native identity before the sender moves
        // into the platform inner state and before any callback is installed.
        let native_view_id = sender.native_view_id();

        // Create WebView instance, storing the sender
        let webview_inner = WebViewInner {
            webtag: webtag.clone(),
            native_generation: native_generation.clone(),
            native_view_id,
            ark_webtag: Mutex::new(webtag.as_str().to_string()),
            ports: Mutex::new(WebMessagePorts::default()),
            pending_port_requests: Mutex::new((false, false)),
            document_authority: HarmonyDocumentAuthority::default(),
            port_ready_signal: (Mutex::new(false), Condvar::new()),
            creation_sender: Mutex::new(Some(sender)),
            proxy_allocs: RefCell::new(Vec::new()),
            callbacks_registered: RefCell::new(false),
            lifecycle_callback_token: RefCell::new(None),
            scheme_handlers: RefCell::new(Vec::new()),
        };

        // Create WebView wrapper and register it
        let webview = Arc::new(crate::WebView::new(
            webview_inner,
            effective_options.clone(),
            native_view_id,
        ));
        register_webview(webview.clone());

        // Call ArkTS to create the WebView controller via TSFN (no callback path).
        // ArkTS will notify native through onWebviewControllerCreated(webtag)
        // once the ArkUI Web component is actually attached (onAppear).
        log::info!(
            "Requesting WebView controller {} (generation {})",
            webtag.as_str(),
            native_generation
        );
        if let Err(e) = call_arkts(
            "createWebViewController",
            &[webtag.as_str(), &options_token, &native_generation],
        ) {
            log::error!("Failed to call createWebViewController: {}", e);
            if let Some(webview) = find_webview_by_native_view_id(&webtag, native_view_id)
                && let Ok(mut sender_opt) = webview.inner.creation_sender.lock()
                && let Some(s) = sender_opt.take()
            {
                s.fail(WebViewCreateStage::NativeCreated, e);
            }
            return;
        }

        // Register native ArkWeb scheme handlers driven by registered_schemes
        if let Err(e) = set_webview_scheme_handler(&webtag, native_view_id) {
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
                let storage = Box::from_raw(p as *mut ProxyStorage);
                unbind_proxy_callback(storage.callback_token);
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
        *lock_or_recover(
            &self.pending_port_requests,
            "harmony.pending_port_requests.cleanup",
        ) = (false, false);
        self.set_port_ready(false);
        self.with_ports(|ports| {
            ports.document_generation = None;
            if let Some(token) = ports.native_message_callback_token.take() {
                unbind_message_callback(token);
            }
            if let Some(token) = ports.console_message_callback_token.take() {
                unbind_message_callback(token);
            }
        });
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

    fn cleanup_lifecycle_callbacks(&self) {
        if let Some(token) = self.lifecycle_callback_token.borrow_mut().take() {
            unbind_lifecycle_callback(token);
        }
    }

    fn send_port_unchecked(&self, port_type: PortType) -> Result<(), WebViewError> {
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

#[async_trait]
impl WebViewController for WebViewInner {
    fn load_url(&self, url: &str) -> Result<(), WebViewError> {
        record_map(&RECENT_NAV_URLS, &self.webtag, url);
        let ark_tag = self.ark_webtag_string();
        call_arkts("loadUrl", &[&ark_tag, &url])
    }

    fn load_data(&self, request: LoadDataRequest<'_>) -> Result<(), WebViewError> {
        let history_url = request.history_url.unwrap_or(request.base_url);
        self.load_data_with_history_url(request, history_url)
    }

    fn exec_js(&self, js: &str) -> Result<(), WebViewError> {
        self.dispatch_javascript_without_result(js)
    }

    async fn eval_js(&self, js: &str) -> Result<serde_json::Value, WebViewScriptError> {
        // CSP-safe + await-aware: same pattern as Android — wrap the user
        // expression in an async IIFE that builds a `{ok, value|error}`
        // envelope and routes it back via `LingXiaProxy.resolveEval(reqId, …)`.
        // ArkWeb's `runJavaScript` returns synchronously without awaiting
        // Promises (it would just JSON-stringify the Promise object), so the
        // bridge round-trip is what gives us native `await` semantics. No
        // `(0,eval)` involved → page CSPs without `'unsafe-eval'` are fine.
        let request_id = NEXT_EVAL_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
        let token = new_eval_token(request_id);
        let (tx, rx) = oneshot::channel();

        pending_eval_requests()
            .lock()
            .map_err(|_| {
                WebViewScriptError::Platform("Harmony pending eval_js map poisoned".to_string())
            })?
            .insert(
                request_id,
                PendingEvalEntry {
                    webtag: self.webtag.to_string(),
                    token: token.clone(),
                    sender: tx,
                },
            );

        let request_id_json = serde_json::to_string(&request_id.to_string()).map_err(|err| {
            WebViewScriptError::Platform(format!("Failed to encode eval request id: {err}"))
        })?;
        let token_json = serde_json::to_string(&token).map_err(|err| {
            WebViewScriptError::Platform(format!("Failed to encode eval token: {err}"))
        })?;
        let resolve_expr =
            format!("LingXiaProxy.resolveEval({request_id_json}, {token_json}, __lxR)");
        let body = build_async_eval_body(js, Some(&resolve_expr));
        let parse_guard_script = format!(
            "(function(){{ \
               const id={request_id_json}; const token={token_json}; \
               const timers=window.__LingXiaEvalParseTimers||(window.__LingXiaEvalParseTimers=Object.create(null)); \
               if (timers[id]) clearTimeout(timers[id]); \
               timers[id]=setTimeout(function(){{ \
                 try {{ LingXiaProxy.resolveEval(id, token, JSON.stringify({{ok:false, error:'JavaScript evaluation failed to start; source may contain a syntax error'}})); }} catch(_){{}} \
               }}, {EVAL_PARSE_GUARD_MS}); \
             }})()"
        );
        let clear_parse_guard = format!(
            "try {{ \
               const __lxTimers=window.__LingXiaEvalParseTimers; \
               if (__lxTimers) {{ clearTimeout(__lxTimers[{request_id_json}]); delete __lxTimers[{request_id_json}]; }} \
             }} catch(_){{}}"
        );
        let script = format!(
            "(async () => {{ {clear_parse_guard} {body} }})().catch(e => {{ \
               try {{ LingXiaProxy.resolveEval({request_id_json}, {token_json}, JSON.stringify({{ok:false, error: String(e)}})); }} catch(_){{}} \
             }})"
        );

        if let Err(err) = self.dispatch_javascript_without_result(&parse_guard_script) {
            if let Ok(mut pending) = pending_eval_requests().lock() {
                pending.remove(&request_id);
            }
            return Err(WebViewScriptError::Platform(format!(
                "Harmony parse guard dispatch failed: {err:?}"
            )));
        }

        if let Err(err) = self.dispatch_javascript_without_result(&script) {
            if let Ok(mut pending) = pending_eval_requests().lock() {
                pending.remove(&request_id);
            }
            return Err(WebViewScriptError::Platform(format!(
                "Harmony runJavaScript dispatch failed: {err:?}"
            )));
        }

        match timeout(EVAL_TIMEOUT, rx).await {
            Ok(Ok(PendingEvalResponse::Success(envelope))) => parse_wrapped_eval_result(&envelope),
            Ok(Ok(PendingEvalResponse::Failure(err))) => Err(WebViewScriptError::Platform(err)),
            Ok(Ok(PendingEvalResponse::Destroyed)) => Err(WebViewScriptError::Destroyed),
            Ok(Err(_)) => Err(WebViewScriptError::Destroyed),
            Err(_) => {
                if let Ok(mut pending) = pending_eval_requests().lock() {
                    pending.remove(&request_id);
                }
                Err(WebViewScriptError::Timeout)
            }
        }
    }

    fn clear_browsing_data(&self) -> Result<(), WebViewError> {
        let ark_tag = self.ark_webtag_string();
        call_arkts("clearBrowsingData", &[&ark_tag])
    }

    fn set_user_agent_override(&self, user_agent: UserAgentOverride) -> Result<(), WebViewError> {
        let ark_tag = self.ark_webtag_string();
        let (mode, user_agent) = match user_agent {
            UserAgentOverride::Default => ("default", String::new()),
            UserAgentOverride::Custom(user_agent) => ("custom", user_agent),
        };
        call_arkts("setUserAgentOverride", &[&ark_tag, mode, &user_agent])
    }

    fn post_message(&self, message: &str) -> Result<(), WebViewError> {
        let DocumentBinding::Bound(generation) =
            normalizer::current_document_binding(self.native_view_id)
        else {
            return Err(WebViewError::WebView(
                "Harmony message post requires a committed document".to_string(),
            ));
        };
        let mut result = Err(WebViewError::WebView(
            "Harmony document changed before message delivery".to_string(),
        ));
        let mut post_if_harmony_document_current = || {
            result = self.post_message_internal(generation, message);
        };
        let mut post = || {
            self.document_authority
                .with_current_generation(generation, &mut post_if_harmony_document_current);
        };
        normalizer::with_current_document_binding(self.native_view_id, generation, &mut post);
        result
    }

    fn post_message_to_document(
        &self,
        expected_generation: DocumentGeneration,
        gate: Arc<dyn DocumentOutboundGate>,
        message: &str,
    ) -> Result<(), WebViewError> {
        let mut result = Err(WebViewError::WebView(
            "Harmony document session is no longer active".to_string(),
        ));
        let mut post_if_harmony_document_current = || {
            let mut post = || {
                result = self.post_message_internal(expected_generation, message);
            };
            gate.with_active(&mut post);
        };
        let mut post_if_session_active = || {
            self.document_authority.with_current_generation(
                expected_generation,
                &mut post_if_harmony_document_current,
            );
        };
        normalizer::with_current_document_binding(
            self.native_view_id,
            expected_generation,
            &mut post_if_session_active,
        );
        result
    }

    async fn take_screenshot(&self) -> Result<Vec<u8>, WebViewError> {
        // A freshly built document races its first paint: ArkWeb rejects
        // snapshots until the Web component is associated and painted. Both
        // states are transient, so retry instead of failing the caller.
        const RETRY_DELAY: Duration = Duration::from_millis(150);
        const ATTEMPTS: usize = 16;
        let mut last_err = WebViewError::WebView("screenshot never attempted".to_string());
        for _ in 0..ATTEMPTS {
            match self.take_screenshot_once().await {
                Ok(bytes) => return Ok(bytes),
                Err(WebViewError::WebView(msg))
                    if msg.contains("not painted")
                        || msg.contains("associated with a Web component")
                        || msg.contains("returned no PixelMap") =>
                {
                    last_err = WebViewError::WebView(msg);
                    tokio::time::sleep(RETRY_DELAY).await;
                }
                Err(err) => return Err(err),
            }
        }
        Err(last_err)
    }
}

impl WebViewInner {
    fn load_data_with_history_url(
        &self,
        request: LoadDataRequest<'_>,
        history_url: &str,
    ) -> Result<(), WebViewError> {
        unsafe {
            let ark_webtag = self.ark_webtag_string();
            let webtag_cstr = cstring_from_str("ark_webtag", &ark_webtag)?;
            let data_cstr = cstring_from_str("load_data.data", request.data)?;
            let base_url_cstr = cstring_from_str("load_data.base_url", request.base_url)?;
            let history_url_cstr = cstring_from_str("load_data.history_url", history_url)?;
            let result = OH_NativeArkWeb_LoadData(
                webtag_cstr.as_ptr(),
                data_cstr.as_ptr(),
                b"text/html\0".as_ptr().cast::<c_char>(),
                b"UTF-8\0".as_ptr().cast::<c_char>(),
                base_url_cstr.as_ptr(),
                history_url_cstr.as_ptr(),
            );
            if result == ArkWeb_ErrorCode_ARKWEB_SUCCESS {
                Ok(())
            } else {
                Err(WebViewError::WebView(format!(
                    "Failed to load data into WebView: error code {:?}",
                    result
                )))
            }
        }
    }

    pub(crate) fn load_trusted_data(
        &self,
        intent: crate::TrustedLoadIntent,
        request: LoadDataRequest<'_>,
    ) -> Result<(), WebViewError> {
        let public_url = request.history_url.unwrap_or(request.base_url);
        let armed = self
            .document_authority
            .arm(intent, public_url, &self.native_generation);
        log::debug!(
            "Armed Harmony trusted document load for {} key={}",
            self.webtag,
            armed.key
        );
        if let Some(replaced) = armed.replaced {
            normalizer::revoke_trusted_load(&self.webtag, self.native_view_id, replaced);
        }
        if let Err(error) = self.load_data_with_history_url(request, &armed.platform_url) {
            self.document_authority.invalidate();
            normalizer::revoke_trusted_load(&self.webtag, self.native_view_id, intent);
            return Err(error);
        }
        Ok(())
    }

    async fn take_screenshot_once(&self) -> Result<Vec<u8>, WebViewError> {
        let request_id = NEXT_SCREENSHOT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();

        pending_screenshot_requests()
            .lock()
            .map_err(|_| {
                WebViewError::WebView("Harmony pending screenshot map poisoned".to_string())
            })?
            .insert(
                request_id,
                PendingScreenshotEntry {
                    webtag: self.webtag.to_string(),
                    sender: tx,
                },
            );

        let ark_tag = self.ark_webtag_string();
        let request_id_str = request_id.to_string();
        if let Err(err) = call_arkts("captureScreenshot", &[&ark_tag, &request_id_str]) {
            if let Ok(mut pending) = pending_screenshot_requests().lock() {
                pending.remove(&request_id);
            }
            return Err(err);
        }

        match timeout(SCREENSHOT_TIMEOUT, rx).await {
            Ok(Ok(PendingScreenshotResponse::Success(bytes))) => Ok(bytes),
            Ok(Ok(PendingScreenshotResponse::Failure(err))) => Err(WebViewError::WebView(err)),
            Ok(Ok(PendingScreenshotResponse::Destroyed)) => Err(WebViewError::WebView(
                "WebView was destroyed before screenshot completed".to_string(),
            )),
            Ok(Err(_)) => Err(WebViewError::WebView(
                "screenshot request was canceled".to_string(),
            )),
            Err(_) => {
                if let Ok(mut pending) = pending_screenshot_requests().lock() {
                    pending.remove(&request_id);
                }
                Err(WebViewError::WebView("screenshot timed out".to_string()))
            }
        }
    }
}

impl WebViewInner {
    fn defer_port_request(&self, port_type: PortType) {
        let mut pending = lock_or_recover(
            &self.pending_port_requests,
            "harmony.pending_port_requests.defer",
        );
        match port_type {
            PortType::Console => pending.0 = true,
            PortType::Message => pending.1 = true,
        }
    }

    fn fulfill_pending_port_requests(&self, generation: DocumentGeneration) {
        let pending = {
            let mut pending = lock_or_recover(
                &self.pending_port_requests,
                "harmony.pending_port_requests.fulfill",
            );
            std::mem::take(&mut *pending)
        };
        for (requested, port_type, callback) in [
            (
                pending.0,
                PortType::Console,
                on_console_message_received as WebMessageCallback,
            ),
            (
                pending.1,
                PortType::Message,
                on_web_message_received as WebMessageCallback,
            ),
        ] {
            if !requested {
                continue;
            }
            if let Err(error) = setup_webmessage_port_for_webtag(
                &self.webtag,
                self.native_view_id,
                generation,
                port_type,
                callback,
            )
            .and_then(|()| self.send_port_unchecked(port_type))
            {
                log::error!(
                    "Failed to fulfill deferred {} for {}: {}",
                    port_type,
                    self.webtag,
                    error
                );
            }
        }
    }

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

    fn refresh_message_port(
        &self,
        document_generation: DocumentGeneration,
    ) -> Result<(), WebViewError> {
        self.set_port_ready(false);
        self.cleanup_webmessage_ports();
        setup_webmessage_port_for_webtag(
            &self.webtag,
            self.native_view_id,
            document_generation,
            PortType::Message,
            on_web_message_received,
        )?;
        self.send_port_unchecked(PortType::Message)?;
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

    fn post_message_internal(
        &self,
        document_generation: DocumentGeneration,
        message: &str,
    ) -> Result<(), WebViewError> {
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

        let get_port = || {
            let ports = self.ports_snapshot();
            (ports.document_generation == Some(document_generation))
                .then_some(ports.native_port)
                .flatten()
        };

        if get_port().is_none() {
            self.refresh_message_port(document_generation)?;
        }

        if !self.is_port_ready() {
            let _ = self.send_port_unchecked(PortType::Message);
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
        self.refresh_message_port(document_generation)?;
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

    fn dispatch_javascript_without_result(&self, js: &str) -> Result<(), WebViewError> {
        unsafe {
            let ark_webtag = self.ark_webtag_string();
            let web_tag_cstr = cstring_from_str("ark_webtag", &ark_webtag)?;
            let controller_api =
                OH_ArkWeb_GetNativeAPI(ArkWeb_NativeAPIVariantKind_ARKWEB_NATIVE_CONTROLLER);
            if controller_api.is_null() {
                return Err(WebViewError::WebView(
                    "Failed to get Controller API".to_string(),
                ));
            }
            let controller = &*(controller_api as *const ArkWeb_ControllerAPI);
            let js_cstr = cstring_from_str("evaluate_javascript.js", js)?;
            let js_object = ArkWeb_JavaScriptObject {
                buffer: js_cstr.as_ptr() as *const u8,
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
}

impl Drop for WebViewInner {
    fn drop(&mut self) {
        fail_pending_eval_requests_for_webtag(&self.webtag);
        fail_pending_screenshot_requests_for_webtag(&self.webtag);
        // Cleanup all tracked scheme handlers first
        self.cleanup_scheme_handlers();

        // Cleanup WebMessage ports
        self.cleanup_webmessage_ports();

        self.cleanup_lifecycle_callbacks();

        // Free proxy allocations
        self.cleanup_proxy_allocs();

        // Ask ArkTS to destroy the controller; ArkTS will notify native via
        // onWebviewControllerDestroyed. The generation lets ArkTS drop this
        // if a newer WebView already re-claimed the same tag.
        log::info!(
            "Releasing WebView controller {} (generation {})",
            self.webtag.as_str(),
            self.native_generation
        );
        if let Err(e) = call_arkts(
            "destroyWebViewController",
            &[self.webtag.as_str(), &self.native_generation],
        ) {
            log::error!("Failed to destroy WebView controller: {:?}", e);
        }
        log::info!(
            "[WebViewInner] Harmony WebViewInner dropped and destroyed ({})",
            self.webtag.as_str()
        );
    }
}

/// Register WebView lifecycle callbacks
fn register_webview_callbacks(webview: &Arc<crate::WebView>) -> Result<(), WebViewError> {
    unsafe {
        let webtag = webview.webtag();
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

        let lifecycle_token = (api.onControllerAttached.is_some() || api.onDestroy.is_some())
            .then(|| bind_lifecycle_callback(webview.native_view_id()));
        if let Some(token) = lifecycle_token {
            if let Some(previous) = webview
                .inner
                .lifecycle_callback_token
                .borrow_mut()
                .replace(token)
            {
                unbind_lifecycle_callback(previous);
            }
        }
        let lifecycle_user_data = lifecycle_token
            .map(|token| token as usize as *mut c_void)
            .unwrap_or(std::ptr::null_mut());

        // Page lifecycle callbacks must carry their concrete native-view ID:
        // the logical tag can be reused by a replacement WebView.
        if let Some(on_controller_attached) = api.onControllerAttached {
            on_controller_attached(
                webtag_cstr.as_ptr(),
                Some(on_controller_attached_callback),
                lifecycle_user_data,
            );
        }

        if let Some(on_destroy) = api.onDestroy {
            on_destroy(
                webtag_cstr.as_ptr(),
                Some(on_destroy_callback),
                lifecycle_user_data,
            );
        }

        Ok(())
    }
}

// WebView lifecycle callback functions
/// ArkWeb's C callbacks carry no URLs, so the adapter captures them where
/// they are visible: API loads, navigation-policy checks, and the ets state
/// samples. `Started`/`Succeeded` fall back to these captures.
static RECENT_NAV_URLS: OnceLock<Mutex<std::collections::HashMap<String, String>>> =
    OnceLock::new();
static LAST_LOCATIONS: OnceLock<Mutex<std::collections::HashMap<String, String>>> = OnceLock::new();

fn record_map(
    map: &'static OnceLock<Mutex<std::collections::HashMap<String, String>>>,
    webtag: &WebTag,
    url: &str,
) {
    if url.is_empty() {
        return;
    }
    map.get_or_init(|| Mutex::new(Default::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(webtag.key().to_string(), url.to_string());
}

extern "C" fn on_controller_attached_callback(web_tag: *const c_char, user_data: *mut c_void) {
    let Some(native_view_id) = native_view_id_for_lifecycle_callback(user_data) else {
        log::debug!("Dropping Harmony controller-attached callback without a live binding");
        return;
    };
    if web_tag.is_null() {
        log::warn!("WebView controller attached callback received null web_tag");
        return;
    }
    if let Ok(webtag_str) = unsafe { CStr::from_ptr(web_tag).to_str() } {
        let webtag = WebTag::from(webtag_str);
        if find_webview_by_native_view_id(&webtag, native_view_id).is_some() {
            log::info!("WebView controller attached: {}", webtag_str);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_callback_token_cannot_bind_a_replacement_webview() {
        let token = bind_lifecycle_callback(NativeWebViewId::new(41));
        assert_eq!(
            native_view_id_for_lifecycle_callback(token as usize as *mut c_void),
            Some(NativeWebViewId::new(41))
        );
        unbind_lifecycle_callback(token);
        assert_eq!(
            native_view_id_for_lifecycle_callback(token as usize as *mut c_void),
            None
        );
    }

    #[test]
    fn message_callback_token_cannot_cross_document_generations() {
        let native_view = NativeWebViewId::new(42);
        let first =
            bind_message_callback(native_view, DocumentGeneration::new(1), PortType::Message);
        let old_user_data = first as usize as *mut c_void;
        assert!(matches!(
            binding_for_message_callback(old_user_data),
            Some(binding)
                if binding.native_view_id == native_view
                    && binding.document_generation == DocumentGeneration::new(1)
        ));
        unbind_message_callback(first);
        let replacement =
            bind_message_callback(native_view, DocumentGeneration::new(2), PortType::Message);
        assert_ne!(first, replacement);
        assert!(binding_for_message_callback(old_user_data).is_none());
        unbind_message_callback(replacement);
    }

    #[test]
    fn proxy_callback_token_cannot_bind_a_replacement_webview() {
        let token = bind_proxy_callback(NativeWebViewId::new(73));
        let user_data = token as usize as *mut c_void;
        assert_eq!(
            native_view_id_for_proxy_callback(user_data),
            Some(NativeWebViewId::new(73))
        );
        unbind_proxy_callback(token);
        assert_eq!(native_view_id_for_proxy_callback(user_data), None);
    }

    #[test]
    fn native_generation_rejects_missing_or_stale_callback_tokens() {
        assert!(native_generation_matches("42", "42"));
        assert!(!native_generation_matches("42", ""));
        assert!(!native_generation_matches("42", "41"));
    }
}

fn webview_for_native_generation(webtag: &WebTag, native_generation: &str) -> Option<Arc<WebView>> {
    let webview = find_webview(webtag)?;
    native_generation_matches(&webview.inner.native_generation, native_generation)
        .then_some(webview)
}

fn native_generation_matches(current: &str, supplied: &str) -> bool {
    !supplied.is_empty() && current == supplied
}

pub fn on_page_begin(
    webtag_str: &str,
    native_generation: &str,
    page_epoch: u64,
    url: &str,
) -> bool {
    let webtag = WebTag::from(webtag_str);
    let Some(webview) = webview_for_native_generation(&webtag, native_generation) else {
        return false;
    };
    let decision = webview.inner.document_authority.page_begin(url, page_epoch);
    let (key, public_url) = match decision {
        PageBegin::Attest {
            intent,
            key,
            public_url,
        } => {
            normalizer::submit(
                &webtag,
                webview.native_view_id(),
                NativeSignal::NavigationStarted {
                    key: Some(key),
                    url: public_url.clone(),
                },
            );
            if !normalizer::attest_trusted_load(&webtag, webview.native_view_id(), intent, key) {
                webview.inner.document_authority.invalidate();
                return false;
            }
            (key, public_url)
        }
        PageBegin::Untrusted {
            key,
            public_url,
            revoked,
        } => {
            if let Some(intent) = revoked {
                normalizer::revoke_trusted_load(&webtag, webview.native_view_id(), intent);
            }
            normalizer::submit(
                &webtag,
                webview.native_view_id(),
                NativeSignal::NavigationStarted {
                    key: Some(key),
                    url: public_url.clone(),
                },
            );
            (key, public_url)
        }
        PageBegin::Invalid => return false,
    };
    // Revoking the previous document in the normalizer is the linearization
    // point for outbound delivery. Only then may its native ports be closed.
    webview.inner.cleanup_webmessage_ports();
    record_map(&RECENT_NAV_URLS, &webtag, &public_url);
    log::debug!("Harmony top-level page begin {} key={key}", webtag);
    true
}

pub fn on_document_commit(
    webtag_str: &str,
    native_generation: &str,
    page_epoch: u64,
    url: &str,
) -> bool {
    let webtag = WebTag::from(webtag_str);
    let Some(webview) = webview_for_native_generation(&webtag, native_generation) else {
        return false;
    };
    let committed = match webview
        .inner
        .document_authority
        .document_commit(url, page_epoch)
    {
        DocumentCommit::Committed(committed) => committed,
        DocumentCommit::Restored { public_url } => {
            normalizer::submit(
                &webtag,
                webview.native_view_id(),
                NativeSignal::DocumentInvalidated,
            );
            webview.inner.cleanup_webmessage_ports();
            if let Some(delegate) = webview.get_delegate() {
                delegate.on_document_restored(webview.native_view_id(), &public_url);
            }
            return true;
        }
        DocumentCommit::Invalid => return false,
    };
    normalizer::submit(
        &webtag,
        webview.native_view_id(),
        NativeSignal::DocumentCommitted {
            key: Some(committed.key),
        },
    );
    let DocumentBinding::Bound(generation) = webview.current_document_binding() else {
        return false;
    };
    if !webview
        .inner
        .document_authority
        .bind_generation(committed.key, generation)
    {
        return false;
    }
    let mut fulfilled = false;
    let mut fulfill = || {
        webview.inner.fulfill_pending_port_requests(generation);
        fulfilled = true;
    };
    let mut fulfill_if_harmony_document_current = || {
        webview
            .inner
            .document_authority
            .with_current_generation(generation, &mut fulfill);
    };
    normalizer::with_current_document_binding(
        webview.native_view_id(),
        generation,
        &mut fulfill_if_harmony_document_current,
    );
    if !fulfilled {
        return false;
    }
    if crate::webview::platform_console_delivery(
        webview.effective_options().profile,
        crate::webview::PlatformConsoleBackend::Harmony,
    ) == crate::webview::PlatformConsoleDelivery::DirectDelegate
    {
        if let Err(error) = inject_console_script(&webtag, webview.native_view_id()) {
            log::debug!("Harmony console injection at commit failed for {webtag}: {error}");
        }
    }
    true
}

pub fn on_page_end(webtag_str: &str, native_generation: &str, page_epoch: u64, url: &str) -> bool {
    let webtag = WebTag::from(webtag_str);
    let Some(webview) = webview_for_native_generation(&webtag, native_generation) else {
        return false;
    };
    let Some(terminal) = webview.inner.document_authority.page_end(url, page_epoch) else {
        return false;
    };
    normalizer::submit(
        &webtag,
        webview.native_view_id(),
        NativeSignal::NavigationFinished {
            key: Some(terminal.key),
            result: NativeNavigationResult::Succeeded {
                final_url: terminal.public_url,
            },
        },
    );
    true
}

pub fn on_render_exited(webtag_str: &str, native_generation: &str) -> bool {
    let webtag = WebTag::from(webtag_str);
    let Some(webview) = webview_for_native_generation(&webtag, native_generation) else {
        return false;
    };
    if let Some(intent) = webview.inner.document_authority.invalidate() {
        normalizer::revoke_trusted_load(&webtag, webview.native_view_id(), intent);
    }
    normalizer::submit(
        &webtag,
        webview.native_view_id(),
        NativeSignal::DocumentInvalidated,
    );
    webview.inner.cleanup_webmessage_ports();
    if let Some(delegate) = webview.get_delegate() {
        delegate.on_web_content_process_terminated(webview.native_view_id());
    }
    true
}

/// ArkWeb exposes URL, title, and back/forward availability only through the
/// ArkTS `WebviewController`, so the ets layer samples them on page and
/// progress callbacks and pushes them here — completing the Harmony
/// adapter's observable-state reporting on the shared delegate contract.
/// Empty strings mean "no sample" and are skipped.
pub fn notify_webview_state(
    web_tag: &str,
    native_generation: &str,
    url: &str,
    title: &str,
    can_go_back: bool,
    can_go_forward: bool,
) {
    let webtag = WebTag::from(web_tag);
    let Some(webview) = webview_for_native_generation(&webtag, native_generation) else {
        log::debug!("Ignoring webview state for stale native generation {web_tag}");
        return;
    };
    if !url.is_empty() {
        let public_url = webview.inner.document_authority.public_url(url);
        record_map(&LAST_LOCATIONS, &webtag, &public_url);
        normalizer::submit(
            &webtag,
            webview.native_view_id(),
            NativeSignal::LocationChanged { url: public_url },
        );
    }
    if !title.is_empty() {
        normalizer::submit(
            &webtag,
            webview.native_view_id(),
            NativeSignal::TitleChanged {
                title: Some(title.to_string()),
            },
        );
    }
    normalizer::submit(
        &webtag,
        webview.native_view_id(),
        NativeSignal::BackForwardChanged {
            can_go_back,
            can_go_forward,
        },
    );
}

extern "C" fn on_destroy_callback(web_tag: *const c_char, user_data: *mut c_void) {
    let Some(native_view_id) = native_view_id_for_lifecycle_callback(user_data) else {
        log::debug!("Dropping Harmony destroy callback without a live native-view binding");
        return;
    };
    if web_tag.is_null() {
        log::warn!("on_destroy_callback received null web_tag");
        return;
    }
    if let Ok(webtag_str) = unsafe { CStr::from_ptr(web_tag).to_str() } {
        let webtag = WebTag::from(webtag_str);
        if find_webview_by_native_view_id(&webtag, native_view_id).is_none() {
            return;
        }
        // ArkWeb component level reports WebView is destroyed; only log here.
        // Resource cleanup is unified through ArkTS -> onWebviewControllerDestroyed(NAPI) -> webview_controller_destroyed,
        // to avoid double-free caused by duplicate calls.
        log::info!("WebView destroyed (ArkWeb onDestroy): {}", webtag_str);
    }
}

/// Generic WebMessage port setup function for webtag
fn setup_webmessage_port_for_webtag(
    webtag: &WebTag,
    native_view_id: NativeWebViewId,
    document_generation: DocumentGeneration,
    port_type: PortType,
    callback_fn: WebMessageCallback,
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
        let webview = find_webview_by_native_view_id(webtag, native_view_id)
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
        let callback_token =
            bind_message_callback(webview.native_view_id(), document_generation, port_type);

        let webview_inner = &webview.inner;
        webview_inner.with_ports(|ports| {
            // Replace, and close, any previous pair of this type: a pair can
            // only be transferred to a document once, so a re-setup must never
            // leave a stale native end behind.
            let close = port_api_struct.close;
            match port_type {
                PortType::Message => {
                    if let (Some(old), Some(close_fn)) = (ports.native_port.take(), close) {
                        close_fn(old, webtag_cstr.as_ptr());
                    }
                    if let Some(token) = ports.native_message_callback_token.take() {
                        unbind_message_callback(token);
                    }
                    ports.webview_native_port.take();
                    ports.native_port = Some(port1);
                    ports.webview_native_port = Some(port2);
                    ports.native_message_callback_token = Some(callback_token);
                }
                PortType::Console => {
                    if let (Some(old), Some(close_fn)) = (ports.console_port.take(), close) {
                        close_fn(old, webtag_cstr.as_ptr());
                    }
                    if let Some(token) = ports.console_message_callback_token.take() {
                        unbind_message_callback(token);
                    }
                    ports.webview_console_port.take();
                    ports.console_port = Some(port1);
                    ports.webview_console_port = Some(port2);
                    ports.console_message_callback_token = Some(callback_token);
                }
            }
            ports.document_generation = Some(document_generation);
        });

        // Set message event handler
        if let Some(set_handler) = port_api_struct.setMessageEventHandler {
            set_handler(
                port1,
                webtag_cstr.as_ptr(),
                Some(callback_fn),
                callback_token as usize as *mut c_void,
            );
        } else {
            unbind_message_callback(callback_token);
            return Err(WebViewError::WebView(format!(
                "setMessageEventHandler not available for {:?}",
                port_type
            )));
        }

        log::info!("Setup {} port for {}", port_type, webtag);
        Ok(())
    }
}

/// Inject console interception script
fn inject_console_script(
    webtag: &WebTag,
    native_view_id: NativeWebViewId,
) -> Result<(), WebViewError> {
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

    let webview = find_webview_by_native_view_id(webtag, native_view_id).ok_or_else(|| {
        WebViewError::WebView(format!("WebView not found for webtag: {}", webtag.as_str()))
    })?;

    webview
        .inner
        .dispatch_javascript_without_result(console_script)
}

/// WebMessage callback
extern "C" fn on_web_message_received(
    web_tag: *const c_char,
    port: *mut ArkWeb_WebMessagePort,
    message: *mut ArkWeb_WebMessage,
    user_data: *mut c_void,
) {
    let Some(binding) = binding_for_message_callback(user_data) else {
        log::warn!("Dropping Harmony WebMessage callback without a live native-view binding");
        return;
    };
    if binding.port_type != PortType::Message {
        log::warn!("Dropping Harmony WebMessage callback with the wrong port binding");
        return;
    }
    let native_view_id = binding.native_view_id;
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
    let Some(webview) = find_webview_by_native_view_id(&full_webtag, native_view_id) else {
        log::debug!(
            "Dropping stale Harmony WebMessage callback for {} (native view {:?})",
            full_webtag.as_str(),
            native_view_id
        );
        return;
    };

    let callback_is_current = webview.inner.with_ports(|ports| {
        ports.document_generation == Some(binding.document_generation)
            && ports.native_port == Some(port)
            && ports.native_message_callback_token == Some(user_data as usize as u64)
    });
    if !callback_is_current {
        log::debug!("Dropping stale Harmony document-port callback for {full_webtag}");
        return;
    }

    // Keep readiness aligned only with the exact bound document port.
    if !port.is_null() {
        webview.inner.set_port_ready(true);
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
        if !crate::webview::web_message_bytes_within_limit(data_length) {
            webview.reject_oversized_web_message();
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

        let mut enqueue = || {
            webview.enqueue_web_message(
                msg_str.to_string(),
                WebMessageFrame::TopLevel,
                WebMessageTransport::HarmonyMessagePort,
                WebMessageSource::unavailable(),
            );
        };
        let mut enqueue_if_harmony_document_current = || {
            webview
                .inner
                .document_authority
                .with_current_generation(binding.document_generation, &mut enqueue);
        };
        if !normalizer::with_current_document_binding(
            native_view_id,
            binding.document_generation,
            &mut enqueue_if_harmony_document_current,
        ) {
            log::debug!("Dropping Harmony message after document revocation for {full_webtag}");
        }
    }
}

/// Check navigation policy for a given webtag and URL.
/// Returns `true` to intercept (cancel) the navigation, `false` to allow it.
/// Called from the ArkTS `onLoadIntercept` handler via NAPI.
pub fn check_navigation_policy(
    webtag_str: &str,
    native_view_token: &str,
    url: &str,
    has_user_gesture: bool,
    is_main_frame: bool,
) -> bool {
    let webtag = WebTag::from(webtag_str);
    if let Some(webview) = webview_for_native_generation(&webtag, native_view_token) {
        record_map(&RECENT_NAV_URLS, &webtag, url);
        let request = crate::NavigationRequest::new(url, has_user_gesture, is_main_frame);
        return matches!(
            webview.handle_navigation(&request),
            NavigationPolicy::Cancel
        );
    }

    false
}

pub fn on_download_start(
    webtag_str: &str,
    native_view_token: &str,
    url: &str,
    user_agent: &str,
    content_disposition: &str,
    mime_type: &str,
    content_length: i64,
) -> bool {
    let webtag = WebTag::from(webtag_str);
    let Some(webview) = webview_for_native_generation(&webtag, native_view_token) else {
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
    native_view_token: &str,
    request_id: &str,
    source_url: &str,
    accept_types_json: &str,
    allow_multiple: bool,
    allow_directories: bool,
    capture: bool,
) -> bool {
    let webtag = WebTag::from(webtag_str);
    let Some(webview) = webview_for_native_generation(&webtag, native_view_token) else {
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
            FileChooserResponse::Error(message) => {
                log::warn!("Harmony file chooser failed: {}", message);
                "[]".to_string()
            }
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

pub fn on_load_error(
    webtag_str: &str,
    native_generation: &str,
    page_epoch: u64,
    url: &str,
    error_code: i32,
    description: &str,
) {
    let webtag = WebTag::from(webtag_str);
    let Some(webview) = webview_for_native_generation(&webtag, native_generation) else {
        log::debug!("Ignoring load error for stale native generation {webtag_str}");
        return;
    };
    let Some(terminal) = webview
        .inner
        .document_authority
        .page_failed(url, page_epoch)
    else {
        log::debug!("Ignoring stale Harmony load error for {webtag_str}");
        return;
    };
    webview.inner.cleanup_webmessage_ports();
    // Cancellation is control flow, never an application-visible load error.
    let desc = description.trim().to_ascii_lowercase();
    let result = if desc.contains("cancel") || desc.contains("aborted") {
        log::debug!(
            "Cancelled navigation webtag={} error={}",
            webtag,
            description
        );
        NativeNavigationResult::Cancelled(None)
    } else {
        NativeNavigationResult::Failed(LoadError {
            failing_url: Some(terminal.public_url),
            kind: harmony_load_error_kind(error_code, description),
            description: description.to_string(),
        })
    };
    normalizer::submit(
        &webtag,
        webview.native_view_id(),
        NativeSignal::NavigationFinished {
            key: Some(terminal.key),
            result,
        },
    );
}

/// Console WebMessage callback
extern "C" fn on_console_message_received(
    web_tag: *const c_char,
    _port: *mut ArkWeb_WebMessagePort,
    message: *mut ArkWeb_WebMessage,
    user_data: *mut c_void,
) {
    let Some(binding) = binding_for_message_callback(user_data) else {
        log::debug!("Dropping Harmony console callback without a live native-view binding");
        return;
    };
    if binding.port_type != PortType::Console {
        return;
    }
    let native_view_id = binding.native_view_id;
    if web_tag.is_null() {
        return;
    }
    let Ok(webtag) = (unsafe { CStr::from_ptr(web_tag).to_str() }) else {
        return;
    };
    let full_webtag = WebTag::from(webtag);
    let Some(webview) = find_webview_by_native_view_id(&full_webtag, native_view_id) else {
        return;
    };
    if crate::webview::platform_console_delivery(
        webview.effective_options().profile,
        crate::webview::PlatformConsoleBackend::Harmony,
    ) != crate::webview::PlatformConsoleDelivery::DirectDelegate
    {
        return;
    }
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
        if !crate::webview::web_message_bytes_within_limit(data_length) {
            let full_webtag = WebTag::from(webtag);
            if let Some(webview) = find_webview_by_native_view_id(&full_webtag, native_view_id) {
                webview.reject_oversized_web_message();
            }
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
            // Convert log level for lxapp crate
            let log_level = match level {
                "error" => LogLevel::Error,
                "warn" => LogLevel::Warn,
                "info" => LogLevel::Info,
                "debug" => LogLevel::Debug,
                _ => LogLevel::Info,
            };

            // Forward to delegate for logging
            let mut deliver = || {
                if let Some(delegate) = find_webview_by_native_view_id(&full_webtag, native_view_id)
                    .and_then(|webview| webview.get_delegate())
                {
                    delegate.log(log_level, console_message);
                }
            };
            let mut deliver_if_harmony_document_current = || {
                if let Some(webview) = find_webview_by_native_view_id(&full_webtag, native_view_id)
                {
                    webview
                        .inner
                        .document_authority
                        .with_current_generation(binding.document_generation, &mut deliver);
                }
            };
            normalizer::with_current_document_binding(
                native_view_id,
                binding.document_generation,
                &mut deliver_if_harmony_document_current,
            );
        }
    }
}
