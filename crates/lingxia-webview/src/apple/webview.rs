use crate::traits::{
    FileChooserRequest, FileChooserResponse, LoadError, LoadErrorKind, NavigationPolicy,
    NewWindowPolicy,
};
use crate::webview::{find_webview, find_webview_delegate};
use crate::{
    DownloadRequest, LoadDataRequest, LogLevel, SystemPipeReader, WebResourceResponse,
    WebViewController, WebViewError,
};
use block2::{Block, RcBlock, StackBlock};
use dispatch2::DispatchQueue;
use http::{Method, Response, StatusCode};
use objc2::runtime::{AnyObject, NSObject, ProtocolObject};
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, class, define_class, msg_send,
    rc::Retained,
};
use objc2_foundation::{
    NSArray, NSDate, NSError, NSJSONSerialization, NSJSONWritingOptions, NSObjectProtocol, NSPoint,
    NSRect, NSSize, NSString, NSURL, NSURLRequest,
};
use objc2_web_kit::{
    WKAudiovisualMediaTypes, WKNavigation, WKNavigationDelegate, WKUIDelegate, WKURLSchemeHandler,
    WKWebViewConfiguration, WKWebsiteDataStore,
};
#[cfg(target_os = "ios")]
use objc2_web_kit::{WKContentRuleList, WKContentRuleListStore, WKUserContentController};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::Write;
use std::os::fd::IntoRawFd;
use std::os::unix::net::UnixStream;
#[cfg(target_os = "ios")]
use std::ptr::NonNull;
use std::sync::mpsc::sync_channel;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::{ffi::CString, ffi::c_char, ffi::c_void};

use crate::webview::{
    EffectiveWebViewCreateOptions, ProxyActivation, ProxyApplyReport, ProxyConfig, SecurityProfile,
    WebTag, WebViewCreateSender, WebViewCreateStage, configured_proxy_for_new_webviews,
};

#[cfg(target_os = "ios")]
const HTTPS_BLOCK_RULE_IDENTIFIER: &str = "LingXiaHTTPSBlocker";
const INTERNAL_BRIDGE_DOWNSTREAM_PATH: &str = "/__lingxia/bridge/downstream";
const APPLE_BRIDGE_QUEUE_LIMIT: usize = 1024;
#[cfg(target_os = "ios")]
const HTTPS_BLOCK_RULE_JSON: &str = r#"
[
    {
        "trigger": {
            "url-filter": "^https:.*"
        },
        "action": {
            "type": "block"
        }
    }
]
"#;

#[link(name = "Network", kind = "framework")]
unsafe extern "C" {
    fn nw_endpoint_create_host(hostname: *const c_char, port: *const c_char) -> *mut AnyObject;
    fn nw_proxy_config_create_http_connect(
        proxy_endpoint: *mut AnyObject,
        proxy_tls_options: *mut c_void,
    ) -> *mut AnyObject;
    fn nw_proxy_config_add_excluded_domain(config: *mut AnyObject, domain: *const c_char);
    fn nw_release(object: *mut AnyObject);
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

type OpenPanelStringArrayCopyFn = unsafe extern "C" fn(*mut AnyObject) -> *mut AnyObject;

static COPY_ALLOWED_MIME_TYPES: OnceLock<Option<OpenPanelStringArrayCopyFn>> = OnceLock::new();
static COPY_ACCEPTED_FILE_EXTENSIONS: OnceLock<Option<OpenPanelStringArrayCopyFn>> =
    OnceLock::new();

fn resolve_open_panel_copy_fn(
    cache: &OnceLock<Option<OpenPanelStringArrayCopyFn>>,
    symbol: &str,
) -> Option<OpenPanelStringArrayCopyFn> {
    *cache.get_or_init(|| {
        let symbol = CString::new(symbol).ok()?;
        let handle = (-2isize) as *mut c_void; // RTLD_DEFAULT on Darwin
        let raw = unsafe { dlsym(handle, symbol.as_ptr()) };
        if raw.is_null() {
            None
        } else {
            Some(unsafe { std::mem::transmute::<*mut c_void, OpenPanelStringArrayCopyFn>(raw) })
        }
    })
}

unsafe fn nsarray_string_values(array_ptr: *mut AnyObject) -> Vec<String> {
    let Some(array) = (unsafe { Retained::<NSArray<NSString>>::from_raw(array_ptr.cast()) }) else {
        return Vec::new();
    };
    array
        .to_vec()
        .into_iter()
        .map(|value| value.to_string())
        .collect()
}

fn open_panel_accept_types(parameters: *mut AnyObject) -> Vec<String> {
    let mut accept_types = Vec::new();

    if let Some(copy_allowed_mime_types) = resolve_open_panel_copy_fn(
        &COPY_ALLOWED_MIME_TYPES,
        "_WKOpenPanelParametersCopyAllowedMIMETypes",
    ) {
        accept_types.extend(unsafe { nsarray_string_values(copy_allowed_mime_types(parameters)) });
    }

    if let Some(copy_accepted_file_extensions) = resolve_open_panel_copy_fn(
        &COPY_ACCEPTED_FILE_EXTENSIONS,
        "_WKOpenPanelParametersCopyAcceptedFileExtensions",
    ) {
        accept_types
            .extend(unsafe { nsarray_string_values(copy_accepted_file_extensions(parameters)) });
    }

    accept_types
}

fn source_page_url_from_webview(webview: *mut AnyObject) -> Option<String> {
    unsafe {
        if webview.is_null() {
            return None;
        }
        let current_url: *mut AnyObject = msg_send![webview, URL];
        if current_url.is_null() {
            return None;
        }
        let absolute: *mut AnyObject = msg_send![current_url, absoluteString];
        if absolute.is_null() {
            return None;
        }
        let url_cstring: *const std::ffi::c_char = msg_send![absolute, UTF8String];
        if url_cstring.is_null() {
            return None;
        }
        Some(
            std::ffi::CStr::from_ptr(url_cstring)
                .to_string_lossy()
                .to_string(),
        )
    }
}

fn complete_open_panel_request(completion_ptr: usize, value: *mut AnyObject) {
    let completion_ptr = completion_ptr as *mut Block<dyn Fn(*mut AnyObject)>;
    let Some(handler) = (unsafe { RcBlock::from_raw(completion_ptr) }) else {
        return;
    };
    handler.call((value,));
}

struct NwOwned(*mut AnyObject);

impl NwOwned {
    fn new(ptr: *mut AnyObject, what: &str) -> Result<Self, WebViewError> {
        if ptr.is_null() {
            Err(WebViewError::WebView(format!(
                "Failed to create Apple {}",
                what
            )))
        } else {
            Ok(Self(ptr))
        }
    }

    fn as_mut_ptr(&self) -> *mut AnyObject {
        self.0
    }
}

impl Drop for NwOwned {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                nw_release(self.0);
            }
        }
    }
}

struct AppleBridgeConnection {
    id: u64,
    writer: UnixStream,
}

struct AppleBridgeTransportState {
    queue: VecDeque<Vec<u8>>,
    connection: Option<AppleBridgeConnection>,
    next_connection_id: u64,
    shutdown: bool,
}

struct AppleBridgeTransport {
    webtag: WebTag,
    state: Mutex<AppleBridgeTransportState>,
    signal: Condvar,
}

impl AppleBridgeTransport {
    fn new(webtag: WebTag) -> Arc<Self> {
        let transport = Arc::new(Self {
            webtag,
            state: Mutex::new(AppleBridgeTransportState {
                queue: VecDeque::new(),
                connection: None,
                next_connection_id: 0,
                shutdown: false,
            }),
            signal: Condvar::new(),
        });
        let worker = Arc::clone(&transport);
        std::thread::spawn(move || worker.run_writer_loop());
        transport
    }

    fn connect_downstream(&self) -> Result<SystemPipeReader, WebViewError> {
        let (read_end, write_end) = UnixStream::pair().map_err(|e| {
            WebViewError::WebView(format!(
                "Failed to create Apple bridge downstream pipe: {e}"
            ))
        })?;
        let read_fd = read_end.into_raw_fd();
        let reader = unsafe { SystemPipeReader::from_raw_fd(read_fd) };

        let (replaced_existing, dropped_queued_frames) = {
            let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
            guard.next_connection_id += 1;
            let replaced = guard.connection.is_some();
            let dropped = guard.queue.len();
            guard.queue.clear();
            guard.connection = Some(AppleBridgeConnection {
                id: guard.next_connection_id,
                writer: write_end,
            });
            (replaced, dropped)
        };
        self.signal.notify_all();
        let dropped_suffix = if dropped_queued_frames > 0 {
            format!(" (dropped {} stale queued frame(s))", dropped_queued_frames)
        } else {
            String::new()
        };
        log::info!(
            "Apple bridge downstream connected webtag={}{}{}",
            self.webtag,
            if replaced_existing {
                " (replaced existing stream)"
            } else {
                ""
            },
            dropped_suffix
        );
        Ok(reader)
    }

    fn enqueue_message(&self, message: &str) -> Result<(), WebViewError> {
        let mut frame = Vec::with_capacity(message.len() + 1);
        frame.extend_from_slice(message.as_bytes());
        frame.push(b'\n');

        let queued_len = {
            let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if guard.shutdown {
                return Err(WebViewError::WebView(format!(
                    "Apple bridge downstream is closed for {}",
                    self.webtag
                )));
            }
            if guard.queue.len() >= APPLE_BRIDGE_QUEUE_LIMIT {
                return Err(WebViewError::WebView(format!(
                    "Apple bridge downstream queue overflow for {} (limit={})",
                    self.webtag, APPLE_BRIDGE_QUEUE_LIMIT
                )));
            }
            guard.queue.push_back(frame);
            guard.queue.len()
        };
        if queued_len > (APPLE_BRIDGE_QUEUE_LIMIT / 2) {
            log::warn!(
                "Apple bridge downstream backlog webtag={} queued={}",
                self.webtag,
                queued_len
            );
        }
        self.signal.notify_one();
        Ok(())
    }

    fn shutdown(&self) {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        guard.shutdown = true;
        guard.connection = None;
        self.signal.notify_all();
    }

    fn run_writer_loop(self: Arc<Self>) {
        loop {
            let (connection_id, mut writer, frame) = {
                let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
                while !guard.shutdown && (guard.connection.is_none() || guard.queue.is_empty()) {
                    guard = self.signal.wait(guard).unwrap_or_else(|e| e.into_inner());
                }
                if guard.shutdown {
                    return;
                }
                let Some(frame) = guard.queue.pop_front() else {
                    continue;
                };
                let Some(connection) = guard.connection.as_ref() else {
                    guard.queue.push_front(frame);
                    continue;
                };
                let writer = match connection.writer.try_clone() {
                    Ok(writer) => writer,
                    Err(e) => {
                        log::warn!(
                            "Apple bridge downstream clone failed webtag={}: {}",
                            self.webtag,
                            e
                        );
                        guard.queue.push_front(frame);
                        guard.connection = None;
                        continue;
                    }
                };
                (connection.id, writer, frame)
            };

            if let Err(e) = writer.write_all(&frame) {
                log::debug!(
                    "Apple bridge downstream write failed webtag={}: {}",
                    self.webtag,
                    e
                );
                let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
                guard.queue.push_front(frame);
                if guard
                    .connection
                    .as_ref()
                    .is_some_and(|connection| connection.id == connection_id)
                {
                    guard.connection = None;
                }
            }
        }
    }
}

fn supports_proxy_configurations(store: &WKWebsiteDataStore) -> bool {
    unsafe {
        let supported: objc2::runtime::Bool =
            msg_send![store, respondsToSelector: objc2::sel!(setProxyConfigurations:)];
        supported.as_bool()
    }
}

fn apply_proxy_to_data_store(
    store: &WKWebsiteDataStore,
    config: Option<&ProxyConfig>,
) -> Result<(), WebViewError> {
    if !supports_proxy_configurations(store) {
        return Err(WebViewError::WebView(
            "proxyConfigurations API is unavailable on this Apple runtime".to_string(),
        ));
    }

    unsafe {
        match config {
            Some(proxy) => {
                let host = CString::new(proxy.host.as_str()).map_err(|_| {
                    WebViewError::WebView("Apple proxy host contains interior NUL byte".to_string())
                })?;
                let port = CString::new(proxy.port.to_string()).map_err(|_| {
                    WebViewError::WebView("Apple proxy port contains interior NUL byte".to_string())
                })?;
                // `nw_*_create*` returns +1 objects; release explicitly via Drop guard.
                let endpoint = NwOwned::new(
                    nw_endpoint_create_host(host.as_ptr(), port.as_ptr()),
                    "proxy endpoint",
                )?;

                // HTTP CONNECT proxies relay TCP streams (HTTP and HTTPS over TCP).
                let proxy_config = NwOwned::new(
                    nw_proxy_config_create_http_connect(
                        endpoint.as_mut_ptr(),
                        std::ptr::null_mut(),
                    ),
                    "HTTP CONNECT proxy configuration",
                )?;

                for rule in &proxy.bypass {
                    let trimmed = rule.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let domain = CString::new(trimmed).map_err(|_| {
                        WebViewError::WebView(
                            "Apple proxy bypass rule contains interior NUL byte".to_string(),
                        )
                    })?;
                    nw_proxy_config_add_excluded_domain(proxy_config.as_mut_ptr(), domain.as_ptr());
                }

                let configs: *mut NSArray<AnyObject> =
                    msg_send![class!(NSArray), arrayWithObject: proxy_config.as_mut_ptr()];
                let _: () = msg_send![store, setProxyConfigurations: configs];
            }
            None => {
                let empty_configs: *mut NSArray<AnyObject> = msg_send![class!(NSArray), array];
                let _: () = msg_send![store, setProxyConfigurations: empty_configs];
            }
        }
    }

    Ok(())
}

fn apply_http_proxy_on_main(
    config: Option<&ProxyConfig>,
    mtm: MainThreadMarker,
) -> Result<ProxyApplyReport, WebViewError> {
    let store = unsafe { WKWebsiteDataStore::defaultDataStore(mtm) };
    if !supports_proxy_configurations(&store) {
        return Ok(ProxyApplyReport::unsupported(
            "Apple proxy requires iOS 17+ / macOS 14+",
        ));
    }

    apply_proxy_to_data_store(&store, config)?;
    let report = if config.is_some() {
        ProxyApplyReport::applied(ProxyActivation::NewWebViewsOnly)
    } else {
        ProxyApplyReport::cleared(ProxyActivation::NewWebViewsOnly)
    };
    Ok(report)
}

pub(crate) fn apply_http_proxy(
    config: Option<&ProxyConfig>,
) -> Result<ProxyApplyReport, WebViewError> {
    if let Some(mtm) = MainThreadMarker::new() {
        return apply_http_proxy_on_main(config, mtm);
    }

    let config_owned = config.cloned();
    let (tx, rx) = sync_channel(1);
    DispatchQueue::main().exec_async(move || {
        let result = match MainThreadMarker::new() {
            Some(mtm) => apply_http_proxy_on_main(config_owned.as_ref(), mtm),
            None => Err(WebViewError::WebView(
                "No MainThreadMarker available on main thread".to_string(),
            )),
        };
        let _ = tx.send(result);
    });

    rx.recv().map_err(|_| {
        WebViewError::WebView("Failed to receive Apple proxy apply result".to_string())
    })?
}

/// Extract a `LoadError` from a raw `*mut NSError`.
///
/// # Safety
/// `error` must be either null or a valid `NSError` pointer.
unsafe fn ns_error_to_load_error(error: *mut NSError) -> LoadError {
    if error.is_null() {
        return LoadError {
            url: None,
            kind: LoadErrorKind::Unknown,
            description: "unknown error".to_string(),
        };
    }
    let description = {
        let s: *mut NSString = unsafe { msg_send![error, localizedDescription] };
        unsafe { s.as_ref() }
            .map(|v| v.to_string())
            .unwrap_or_else(|| "unknown error".to_string())
    };
    let error_code: i64 = unsafe { msg_send![error, code] };
    let url = {
        let user_info: *mut AnyObject = unsafe { msg_send![error, userInfo] };
        if user_info.is_null() {
            None
        } else {
            let key = NSString::from_str("NSErrorFailingURLStringKey");
            let val: *mut NSString = unsafe { msg_send![user_info, objectForKey: &*key] };
            unsafe { val.as_ref() }.map(|v| v.to_string())
        }
    };
    let kind = match error_code {
        -999 => LoadErrorKind::Cancelled,
        -1001 => LoadErrorKind::Timeout,
        -1002 => LoadErrorKind::InvalidUrl,
        -1003 | -1006 => LoadErrorKind::Dns,
        -1004 | -1005 | -1009 => LoadErrorKind::Network,
        -1022 | -1206..=-1200 => LoadErrorKind::Security,
        -1100 => LoadErrorKind::NotFound,
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
    };
    LoadError {
        url,
        kind,
        description,
    }
}

// Custom Navigation Delegate for handling page lifecycle events
pub struct LingXiaNavigationDelegateIvars {
    webtag: WebTag,
    intercept_https_navigation: bool,
    pending_browser_download_url: RefCell<Option<String>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "LingXiaNavigationDelegate"]
    #[thread_kind = MainThreadOnly]
    #[ivars = LingXiaNavigationDelegateIvars]
    pub struct LingXiaNavigationDelegate;

    unsafe impl NSObjectProtocol for LingXiaNavigationDelegate {}

    unsafe impl WKNavigationDelegate for LingXiaNavigationDelegate {
        #[unsafe(method(webView:didStartProvisionalNavigation:))]
        fn did_start_provisional_navigation(
            &self,
            _webview: *mut AnyObject,
            _navigation: &WKNavigation,
        ) {
            let webtag = &self.ivars().webtag;
            let (appid, path) = webtag.extract_parts();

            // Call delegate's on_page_started
            if let Some(delegate) = find_webview_delegate(webtag) {
                delegate.on_page_started();
            }
            log::info!("WebView page started: {} at {}", appid, path);
        }

        #[unsafe(method(webView:didFinishNavigation:))]
        fn did_finish_navigation(&self, _webview: *mut AnyObject, _navigation: &WKNavigation) {
            let webtag = &self.ivars().webtag;
            let (appid, path) = webtag.extract_parts();

            // Call delegate's on_page_finished
            if let Some(delegate) = find_webview_delegate(webtag) {
                delegate.on_page_finished();
            }
            log::info!("WebView page finished: {} at {}", appid, path);
        }

        #[unsafe(method(webView:didFailProvisionalNavigation:withError:))]
        fn did_fail_provisional_navigation(
            &self,
            _webview: *mut AnyObject,
            _navigation: *mut AnyObject,
            error: *mut NSError,
        ) {
            let webtag = &self.ivars().webtag;
            let load_error = unsafe { ns_error_to_load_error(error) };
            if load_error.kind == LoadErrorKind::Cancelled {
                log::debug!(
                    "Ignoring cancelled provisional navigation webtag={} error={}",
                    webtag,
                    load_error.description
                );
                return;
            }
            log::warn!(
                "WebView provisional navigation failed webtag={} error={}",
                webtag,
                load_error.description
            );
            if let Some(delegate) = find_webview_delegate(webtag) {
                delegate.on_load_error(&load_error);
            }
        }

        #[unsafe(method(webView:didFailNavigation:withError:))]
        fn did_fail_navigation(
            &self,
            _webview: *mut AnyObject,
            _navigation: *mut AnyObject,
            error: *mut NSError,
        ) {
            let webtag = &self.ivars().webtag;
            let load_error = unsafe { ns_error_to_load_error(error) };
            if load_error.kind == LoadErrorKind::Cancelled {
                log::debug!(
                    "Ignoring cancelled navigation webtag={} error={}",
                    webtag,
                    load_error.description
                );
                return;
            }
            log::warn!(
                "WebView navigation failed webtag={} error={}",
                webtag,
                load_error.description
            );
            if let Some(delegate) = find_webview_delegate(webtag) {
                delegate.on_load_error(&load_error);
            }
        }

        #[unsafe(method(webView:decidePolicyForNavigationAction:decisionHandler:))]
        fn decide_policy_for_navigation_action(
            &self,
            webview: *mut AnyObject,
            navigation_action: *mut AnyObject,
            decision_handler: *mut AnyObject,
        ) {
            // Helper function to call decision handler with policy
            let call_decision_handler = |policy: i32| {
                let handler: &Block<dyn Fn(i32)> =
                    unsafe { &*(decision_handler as *const Block<dyn Fn(i32)>) };
                handler.call((policy,));
            };

            // Helper function to allow navigation
            let allow_navigation = || call_decision_handler(1); // WKNavigationActionPolicyAllow = 1
            let cancel_navigation = || call_decision_handler(0); // WKNavigationActionPolicyCancel = 0
            // Extract URL from navigation action
            let url = match self.extract_url_from_navigation_action(navigation_action) {
                Some(url) => url,
                None => {
                    allow_navigation();
                    return;
                }
            };

            let webtag = &self.ivars().webtag;
            let target_frame: *mut AnyObject = unsafe { msg_send![navigation_action, targetFrame] };
            log::info!(
                "Apple decidePolicy webtag={} url={} target_frame_null={} intercept_https={}",
                webtag,
                url,
                target_frame.is_null(),
                self.ivars().intercept_https_navigation
            );

            // Always allow internal inspector/devtools navigations on Apple.
            // These URLs are not page content navigations and must bypass strict handlers.
            let lower_url = url.to_ascii_lowercase();
            let is_internal_inspector_nav = lower_url.starts_with("about:blank")
                || lower_url.starts_with("about:srcdoc")
                || lower_url.starts_with("webkit:")
                || lower_url.starts_with("x-webkit-")
                || lower_url.starts_with("inspector:")
                || lower_url.starts_with("devtools:");
            if is_internal_inspector_nav {
                log::info!(
                    "Apple decidePolicy allow internal-inspector webtag={} url={}",
                    webtag,
                    url
                );
                allow_navigation();
                return;
            }

            let should_perform_download = unsafe {
                let value: objc2::runtime::Bool =
                    msg_send![navigation_action, shouldPerformDownload];
                value.as_bool()
            };
            if should_perform_download {
                if let Some(managed_webview) = find_webview(webtag)
                    && managed_webview.effective_options().profile
                        == SecurityProfile::BrowserRelaxed
                {
                    self.ivars()
                        .pending_browser_download_url
                        .replace(Some(url.clone()));
                    log::info!(
                        "Apple decidePolicy mark browser download webtag={} url={}",
                        webtag,
                        url
                    );
                    allow_navigation();
                    return;
                }
            }

            // Check closure-based navigation handler first
            if let Some(webview) = find_webview(webtag) {
                match webview.handle_navigation(&url) {
                    NavigationPolicy::Cancel => {
                        log::info!(
                            "Apple decidePolicy canceled by navigation handler webtag={} url={}",
                            webtag,
                            url
                        );
                        cancel_navigation();
                        return;
                    }
                    NavigationPolicy::Allow => {} // fall through to existing logic
                }
            }

            if !self.ivars().intercept_https_navigation {
                allow_navigation();
                return;
            }

            // Only intercept HTTPS navigation requests
            if !url.starts_with("https://") {
                allow_navigation();
                return;
            }

            // Build HTTP request for lxapp to check
            let http_request = match http::Request::builder()
                .method("GET")
                .uri(&url)
                .body(Vec::new())
            {
                Ok(req) => req,
                Err(_) => {
                    log::warn!("Failed to build HTTP request for URL: {}", url);
                    allow_navigation();
                    return;
                }
            };

            // Dispatch to closure-based scheme handler
            let response = if let Some(webview) = find_webview(webtag) {
                webview.handle_scheme_request("https", http_request)
            } else {
                None
            };
            match response {
                Some(_) => {
                    // log::info!("Miniapp handling HTTPS navigation, canceling: {}", url);
                    cancel_navigation();
                }
                None => {
                    // log::info!("Miniapp allows HTTPS navigation: {}", url);
                    allow_navigation();
                }
            }
        }

        #[unsafe(method(webView:decidePolicyForNavigationResponse:decisionHandler:))]
        fn decide_policy_for_navigation_response(
            &self,
            webview: *mut AnyObject,
            navigation_response: *mut AnyObject,
            decision_handler: *mut AnyObject,
        ) {
            let call_decision_handler = |policy: i32| {
                let handler: &Block<dyn Fn(i32)> =
                    unsafe { &*(decision_handler as *const Block<dyn Fn(i32)>) };
                handler.call((policy,));
            };
            let allow_navigation = || call_decision_handler(1); // WKNavigationResponsePolicyAllow
            let cancel_navigation = || call_decision_handler(0); // WKNavigationResponsePolicyCancel

            let webtag = &self.ivars().webtag;
            let Some(managed_webview) = find_webview(webtag) else {
                allow_navigation();
                return;
            };

            // Strict/lxapp pages never trigger in-webview download handling.
            if managed_webview.effective_options().profile != SecurityProfile::BrowserRelaxed {
                allow_navigation();
                return;
            }

            let can_show_mime: objc2::runtime::Bool =
                unsafe { msg_send![navigation_response, canShowMIMEType] };
            let maybe_req = self
                .extract_download_request_from_navigation_response(webview, navigation_response);
            let Some(request) = maybe_req else {
                allow_navigation();
                return;
            };

            let disposition_is_attachment = request
                .content_disposition
                .as_ref()
                .map(|value| value.to_ascii_lowercase().contains("attachment"))
                .unwrap_or(false);
            let forced_download = self
                .ivars()
                .pending_browser_download_url
                .borrow_mut()
                .take()
                .as_ref()
                .is_some_and(|pending_url| pending_url == &request.url);
            let should_download =
                forced_download || disposition_is_attachment || !can_show_mime.as_bool();
            if !should_download {
                allow_navigation();
                return;
            }

            if managed_webview.effective_options().has_download_handler {
                managed_webview.handle_download(request.clone());
                log::info!(
                    "Apple download request dispatched webtag={} url={}",
                    webtag,
                    request.url
                );
            } else {
                log::info!(
                    "Apple browser download suppressed without handler webtag={} url={}",
                    webtag,
                    request.url
                );
            }

            // Browser default policy: always cancel in-webview navigation for downloads.
            cancel_navigation();
        }
    }
);

impl LingXiaNavigationDelegate {
    fn nsstring_ptr_to_string(ns: *mut AnyObject) -> Option<String> {
        unsafe {
            if ns.is_null() {
                return None;
            }
            let cstr: *const std::ffi::c_char = msg_send![ns, UTF8String];
            if cstr.is_null() {
                None
            } else {
                Some(std::ffi::CStr::from_ptr(cstr).to_string_lossy().to_string())
            }
        }
    }

    /// Extract URL string from navigation action, returns None if extraction fails
    fn extract_url_from_navigation_action(
        &self,
        navigation_action: *mut AnyObject,
    ) -> Option<String> {
        unsafe {
            let request: *mut AnyObject = msg_send![navigation_action, request];
            if request.is_null() {
                return None;
            }

            let url_obj: *mut AnyObject = msg_send![request, URL];
            if url_obj.is_null() {
                return None;
            }

            let url_string: *mut AnyObject = msg_send![url_obj, absoluteString];
            if url_string.is_null() {
                return None;
            }

            let url_cstring: *const std::ffi::c_char = msg_send![url_string, UTF8String];
            if url_cstring.is_null() {
                return None;
            }

            Some(
                std::ffi::CStr::from_ptr(url_cstring)
                    .to_string_lossy()
                    .to_string(),
            )
        }
    }

    fn extract_source_page_url(&self, webview: *mut AnyObject) -> Option<String> {
        unsafe {
            if webview.is_null() {
                return None;
            }
            let current_url: *mut AnyObject = msg_send![webview, URL];
            if current_url.is_null() {
                return None;
            }
            let absolute: *mut AnyObject = msg_send![current_url, absoluteString];
            Self::nsstring_ptr_to_string(absolute)
        }
    }

    fn extract_download_request_from_navigation_response(
        &self,
        webview: *mut AnyObject,
        navigation_response: *mut AnyObject,
    ) -> Option<DownloadRequest> {
        unsafe {
            if navigation_response.is_null() {
                return None;
            }
            let response: *mut AnyObject = msg_send![navigation_response, response];
            if response.is_null() {
                return None;
            }

            let url_obj: *mut AnyObject = msg_send![response, URL];
            if url_obj.is_null() {
                return None;
            }
            let absolute: *mut AnyObject = msg_send![url_obj, absoluteString];
            let url = Self::nsstring_ptr_to_string(absolute)?;

            let mime_type = {
                let mime_obj: *mut AnyObject = msg_send![response, MIMEType];
                Self::nsstring_ptr_to_string(mime_obj)
            };

            let suggested_filename = {
                let name_obj: *mut AnyObject = msg_send![response, suggestedFilename];
                Self::nsstring_ptr_to_string(name_obj)
            };

            let expected_len: i64 = msg_send![response, expectedContentLength];
            let content_length = if expected_len >= 0 {
                Some(expected_len as u64)
            } else {
                None
            };

            let content_disposition = {
                let headers: *mut AnyObject = msg_send![response, allHeaderFields];
                if headers.is_null() {
                    None
                } else {
                    let key = NSString::from_str("Content-Disposition");
                    let value: *mut AnyObject = msg_send![headers, objectForKey: &*key];
                    Self::nsstring_ptr_to_string(value)
                }
            };

            let user_agent = {
                let ua_obj: *mut AnyObject = msg_send![webview, customUserAgent];
                Self::nsstring_ptr_to_string(ua_obj)
            };

            Some(DownloadRequest {
                url,
                user_agent,
                content_disposition,
                mime_type,
                content_length,
                suggested_filename,
                source_page_url: self.extract_source_page_url(webview),
                cookie: None,
            })
        }
    }
    pub fn new(
        appid: String,
        path: String,
        session_id: Option<u64>,
        intercept_https_navigation: bool,
        mtm: MainThreadMarker,
    ) -> Retained<Self> {
        let webtag = WebTag::new(&appid, &path, session_id);
        let delegate =
            mtm.alloc::<LingXiaNavigationDelegate>()
                .set_ivars(LingXiaNavigationDelegateIvars {
                    webtag,
                    intercept_https_navigation,
                    pending_browser_download_url: RefCell::new(None),
                });

        unsafe { msg_send![super(delegate), init] }
    }
}

// UI Delegate for browser mode and new-window handling:
// - Handles target="_blank" / window.open() via closure handler
// - Shows native NSAlert for JavaScript alert/confirm/prompt dialogs
pub struct LingXiaUIDelegateIvars {
    webtag: WebTag,
    allow_js_dialogs: bool,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "LingXiaUIDelegate"]
    #[thread_kind = MainThreadOnly]
    #[ivars = LingXiaUIDelegateIvars]
    pub struct LingXiaUIDelegate;

    unsafe impl NSObjectProtocol for LingXiaUIDelegate {}

    unsafe impl WKUIDelegate for LingXiaUIDelegate {
        #[unsafe(method(webView:createWebViewWithConfiguration:forNavigationAction:windowFeatures:))]
        fn create_web_view(
            &self,
            webview: *mut AnyObject,
            _configuration: &WKWebViewConfiguration,
            navigation_action: *mut AnyObject,
            _window_features: *mut AnyObject,
        ) -> *mut AnyObject {
            unsafe {
                let target_frame: *mut AnyObject = msg_send![navigation_action, targetFrame];
                if target_frame.is_null() {
                    let request: *mut AnyObject = msg_send![navigation_action, request];
                    if !request.is_null() {
                        let url_obj: *mut AnyObject = msg_send![request, URL];
                        if !url_obj.is_null() {
                            let abs: *mut AnyObject = msg_send![url_obj, absoluteString];
                            if !abs.is_null() {
                                let cstr: *const std::ffi::c_char = msg_send![abs, UTF8String];
                                if !cstr.is_null() {
                                    let url = std::ffi::CStr::from_ptr(cstr)
                                        .to_string_lossy()
                                        .to_string();
                                    let webtag = &self.ivars().webtag;
                                    log::info!(
                                        "Apple createWebView(new-window) webtag={} url={}",
                                        webtag,
                                        url
                                    );
                                    if let Some(wv) = find_webview(webtag)
                                        && wv.has_new_window_handler()
                                    {
                                        let policy = wv.handle_new_window(&url);
                                        log::info!(
                                            "Apple new-window policy webtag={} url={} policy={:?}",
                                            webtag,
                                            url,
                                            policy
                                        );
                                        match policy {
                                            NewWindowPolicy::LoadInSelf => {
                                                if url.len() >= "http://".len()
                                                    && url[..7].eq_ignore_ascii_case("http://")
                                                {
                                                    let upgraded = format!("https://{}", &url[7..]);
                                                    let ns_upgraded = NSString::from_str(&upgraded);
                                                    let upgraded_url_obj: *mut AnyObject = msg_send![
                                                        class!(NSURL),
                                                        URLWithString: &*ns_upgraded
                                                    ];
                                                    if !upgraded_url_obj.is_null() {
                                                        let upgraded_request: *mut AnyObject = msg_send![
                                                            class!(NSURLRequest),
                                                            requestWithURL: upgraded_url_obj
                                                        ];
                                                        if !upgraded_request.is_null() {
                                                            let _: () = msg_send![webview, loadRequest: upgraded_request];
                                                            log::info!(
                                                                "Apple new-window upgraded http->https webtag={} from={} to={}",
                                                                webtag,
                                                                url,
                                                                upgraded
                                                            );
                                                        } else {
                                                            let _: () = msg_send![webview, loadRequest: request];
                                                            log::warn!(
                                                                "Apple new-window failed to build upgraded request, fallback original webtag={} url={}",
                                                                webtag,
                                                                url
                                                            );
                                                        }
                                                    } else {
                                                        let _: () = msg_send![webview, loadRequest: request];
                                                        log::warn!(
                                                            "Apple new-window failed to build upgraded URL, fallback original webtag={} url={}",
                                                            webtag,
                                                            url
                                                        );
                                                    }
                                                } else {
                                                    let _: () =
                                                        msg_send![webview, loadRequest: request];
                                                    log::info!(
                                                        "Apple new-window loaded in self webtag={} url={}",
                                                        webtag,
                                                        url
                                                    );
                                                }
                                            }
                                            NewWindowPolicy::Cancel => {}
                                        }
                                    } else {
                                        log::warn!(
                                            "Apple new-window handler missing webtag={} url={}",
                                            webtag,
                                            url
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // Return nil so WebKit does not create a separate window.
            std::ptr::null_mut()
        }

        // JavaScript alert()
        #[unsafe(method(webView:runJavaScriptAlertPanelWithMessage:initiatedByFrame:completionHandler:))]
        fn run_javascript_alert(
            &self,
            _webview: *mut AnyObject,
            message: &NSString,
            _frame: *mut AnyObject,
            completion_handler: *mut AnyObject,
        ) {
            if !self.ivars().allow_js_dialogs {
                log::info!(
                    "Apple suppressed JavaScript alert in strict profile webtag={}",
                    self.ivars().webtag
                );
                let handler: &Block<dyn Fn()> =
                    unsafe { &*(completion_handler as *const Block<dyn Fn()>) };
                handler.call(());
                return;
            }
            #[cfg(target_os = "macos")]
            unsafe {
                let alert: *mut AnyObject = msg_send![class!(NSAlert), new];
                let _: () = msg_send![alert, setMessageText: message];
                let ok_title = NSString::from_str("OK");
                let _: () = msg_send![alert, addButtonWithTitle: &*ok_title];
                let _: i64 = msg_send![alert, runModal];
                let _: () = msg_send![alert, release];
            }
            let handler: &Block<dyn Fn()> =
                unsafe { &*(completion_handler as *const Block<dyn Fn()>) };
            handler.call(());
        }

        // JavaScript confirm()
        #[unsafe(method(webView:runJavaScriptConfirmPanelWithMessage:initiatedByFrame:completionHandler:))]
        fn run_javascript_confirm(
            &self,
            _webview: *mut AnyObject,
            message: &NSString,
            _frame: *mut AnyObject,
            completion_handler: *mut AnyObject,
        ) {
            if !self.ivars().allow_js_dialogs {
                log::info!(
                    "Apple suppressed JavaScript confirm in strict profile webtag={}",
                    self.ivars().webtag
                );
                let handler: &Block<dyn Fn(objc2::runtime::Bool)> =
                    unsafe { &*(completion_handler as *const Block<dyn Fn(objc2::runtime::Bool)>) };
                handler.call((objc2::runtime::Bool::new(false),));
                return;
            }
            let result;
            #[cfg(target_os = "macos")]
            unsafe {
                let alert: *mut AnyObject = msg_send![class!(NSAlert), new];
                let _: () = msg_send![alert, setMessageText: message];
                let ok_title = NSString::from_str("OK");
                let cancel_title = NSString::from_str("Cancel");
                let _: () = msg_send![alert, addButtonWithTitle: &*ok_title];
                let _: () = msg_send![alert, addButtonWithTitle: &*cancel_title];
                let response: i64 = msg_send![alert, runModal];
                let _: () = msg_send![alert, release];
                // NSAlertFirstButtonReturn = 1000
                result = response == 1000;
            }
            #[cfg(not(target_os = "macos"))]
            {
                // Browser mode is desktop-only in phase-1; fail closed on non-macOS.
                result = false;
            }
            let handler: &Block<dyn Fn(objc2::runtime::Bool)> =
                unsafe { &*(completion_handler as *const Block<dyn Fn(objc2::runtime::Bool)>) };
            handler.call((objc2::runtime::Bool::new(result),));
        }

        // JavaScript prompt()
        #[unsafe(method(webView:runJavaScriptTextInputPanelWithPrompt:defaultText:initiatedByFrame:completionHandler:))]
        fn run_javascript_prompt(
            &self,
            _webview: *mut AnyObject,
            prompt: &NSString,
            default_text: *mut AnyObject,
            _frame: *mut AnyObject,
            completion_handler: *mut AnyObject,
        ) {
            if !self.ivars().allow_js_dialogs {
                log::info!(
                    "Apple suppressed JavaScript prompt in strict profile webtag={}",
                    self.ivars().webtag
                );
                let handler: &Block<dyn Fn(*mut AnyObject)> =
                    unsafe { &*(completion_handler as *const Block<dyn Fn(*mut AnyObject)>) };
                handler.call((std::ptr::null_mut(),));
                return;
            }
            let input_value: *mut AnyObject;
            #[cfg(target_os = "macos")]
            unsafe {
                let alert: *mut AnyObject = msg_send![class!(NSAlert), new];
                let _: () = msg_send![alert, setMessageText: prompt];
                let ok_title = NSString::from_str("OK");
                let cancel_title = NSString::from_str("Cancel");
                let _: () = msg_send![alert, addButtonWithTitle: &*ok_title];
                let _: () = msg_send![alert, addButtonWithTitle: &*cancel_title];

                // Add text input field
                let input: *mut AnyObject = msg_send![class!(NSTextField), alloc];
                let frame = NSRect {
                    origin: NSPoint { x: 0.0, y: 0.0 },
                    size: NSSize {
                        width: 200.0,
                        height: 24.0,
                    },
                };
                let input: *mut AnyObject = msg_send![input, initWithFrame: frame];
                if !default_text.is_null() {
                    let _: () = msg_send![input, setStringValue: default_text];
                }
                let _: () = msg_send![alert, setAccessoryView: input];
                // `input` is +1 from alloc/init; alert retains accessory view.
                let _: () = msg_send![input, release];

                let response: i64 = msg_send![alert, runModal];
                // Read stringValue BEFORE releasing alert, because alert owns
                // the accessory view (input) and release may deallocate it.
                if response == 1000 {
                    input_value = msg_send![input, stringValue];
                } else {
                    input_value = std::ptr::null_mut();
                }
                let _: () = msg_send![alert, release];
            }
            #[cfg(not(target_os = "macos"))]
            {
                // Browser mode is desktop-only in phase-1; fail closed on non-macOS.
                let _ = default_text;
                input_value = std::ptr::null_mut();
            }
            let handler: &Block<dyn Fn(*mut AnyObject)> =
                unsafe { &*(completion_handler as *const Block<dyn Fn(*mut AnyObject)>) };
            handler.call((input_value,));
        }

        #[unsafe(method(webView:runOpenPanelWithParameters:initiatedByFrame:completionHandler:))]
        fn run_open_panel(
            &self,
            _webview: *mut AnyObject,
            parameters: *mut AnyObject,
            _frame: *mut AnyObject,
            completion_handler: *mut AnyObject,
        ) {
            let handler: &Block<dyn Fn(*mut AnyObject)> =
                unsafe { &*(completion_handler as *const Block<dyn Fn(*mut AnyObject)>) };

            let request = unsafe {
                let allows_multiple: objc2::runtime::Bool =
                    msg_send![parameters, allowsMultipleSelection];
                #[cfg(target_os = "macos")]
                let allows_directories: objc2::runtime::Bool =
                    msg_send![parameters, allowsDirectories];
                #[cfg(not(target_os = "macos"))]
                let allows_directories = objc2::runtime::Bool::new(false);
                FileChooserRequest {
                    // Resolve these helpers at runtime so the static library
                    // does not hard-link private WebKit SPI symbols.
                    accept_types: open_panel_accept_types(parameters),
                    allow_multiple: allows_multiple.as_bool(),
                    allow_directories: allows_directories.as_bool(),
                    capture: false,
                    source_page_url: source_page_url_from_webview(_webview),
                }
            };

            let Some(webview) = find_webview(&self.ivars().webtag) else {
                handler.call((std::ptr::null_mut(),));
                return;
            };

            // Copy the completion block before returning from the WebKit callback;
            // otherwise an async chooser response could invoke a dead stack block.
            let completion_ptr = RcBlock::into_raw(handler.copy()) as usize;
            if !webview.handle_file_chooser(request, move |response| {
                let exec = move || match response {
                    FileChooserResponse::Cancel => {
                        complete_open_panel_request(completion_ptr, std::ptr::null_mut());
                    }
                    FileChooserResponse::Error(message) => {
                        log::warn!("Apple file chooser failed: {}", message);
                        complete_open_panel_request(completion_ptr, std::ptr::null_mut());
                    }
                    FileChooserResponse::Files(files) => {
                        let urls: Vec<Retained<NSURL>> = files
                            .into_iter()
                            .filter_map(|file| {
                                if let Some(uri) = file
                                    .uri
                                    .as_ref()
                                    .map(|value| value.trim())
                                    .filter(|value| !value.is_empty())
                                {
                                    let ns = NSString::from_str(uri);
                                    return NSURL::URLWithString(&ns);
                                }
                                let value =
                                    file.path.as_ref().map(|value| value.trim()).unwrap_or("");
                                if value.is_empty() {
                                    return None;
                                }
                                let ns = NSString::from_str(value);
                                Some(NSURL::fileURLWithPath(&ns))
                            })
                            .collect();
                        let array = NSArray::from_retained_slice(&urls);
                        let ptr = (&*array) as *const NSArray<NSURL> as *mut AnyObject;
                        complete_open_panel_request(completion_ptr, ptr);
                    }
                };

                if MainThreadMarker::new().is_some() {
                    exec();
                } else {
                    DispatchQueue::main().exec_async(exec);
                }
            }) {
                log::warn!(
                    "Apple file chooser requested without handler webtag={}",
                    self.ivars().webtag
                );
                complete_open_panel_request(completion_ptr, std::ptr::null_mut());
                return;
            }
        }
    }
);

impl LingXiaUIDelegate {
    pub fn new(webtag: WebTag, allow_js_dialogs: bool, mtm: MainThreadMarker) -> Retained<Self> {
        let delegate = mtm
            .alloc::<LingXiaUIDelegate>()
            .set_ivars(LingXiaUIDelegateIvars {
                webtag,
                allow_js_dialogs,
            });
        unsafe { msg_send![super(delegate), init] }
    }
}

pub struct WebViewInner {
    webview: *mut AnyObject,
    _navigation_delegate: Retained<LingXiaNavigationDelegate>,
    _ui_delegate: Option<Retained<LingXiaUIDelegate>>,
    _message_handler: Retained<LingXiaMessageHandler>,
    pub(crate) webtag: WebTag,
    apple_bridge_transport: Arc<AppleBridgeTransport>,
}

#[cfg(target_os = "macos")]
pub(crate) fn toggle_devtools_by_swift_ptr(swift_ptr: usize, detached: bool) -> bool {
    if swift_ptr == 0 {
        return false;
    }
    let exec = move || unsafe {
        let webview = swift_ptr as *mut AnyObject;
        let can_set_attachment: objc2::runtime::Bool =
            msg_send![webview, respondsToSelector: objc2::sel!(_setInspectorAttachmentView:)];
        if detached && can_set_attachment.as_bool() {
            let nil_view: *mut AnyObject = std::ptr::null_mut();
            let _: () = msg_send![webview, _setInspectorAttachmentView: nil_view];
        }
        let inspector: *mut AnyObject = msg_send![webview, _inspector];
        if inspector.is_null() {
            return;
        }
        let visible: objc2::runtime::Bool = msg_send![inspector, isVisible];
        if visible.as_bool() {
            let _: () = msg_send![inspector, close];
        } else {
            let _: () = msg_send![inspector, show];
        }
    };
    if MainThreadMarker::new().is_some() {
        exec();
    } else {
        DispatchQueue::main().exec_async(exec);
    }
    true
}

impl std::fmt::Debug for WebViewInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebViewInner")
            .field("webview", &self.webview)
            .field("webtag", &self.webtag)
            .field("navigation_delegate", &"<LingXiaNavigationDelegate>")
            .field("message_handler", &"<LingXiaMessageHandler>")
            .finish()
    }
}

unsafe impl Send for WebViewInner {}
unsafe impl Sync for WebViewInner {}

impl WebViewInner {
    pub(crate) fn handle_internal_bridge_request(
        &self,
        request: &http::Request<Vec<u8>>,
    ) -> Option<WebResourceResponse> {
        if request.uri().path() != INTERNAL_BRIDGE_DOWNSTREAM_PATH {
            return None;
        }

        if request.method() != Method::GET {
            let response = Response::builder()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .header("Content-Type", "text/plain; charset=utf-8")
                .body(())
                .unwrap_or_else(|_| {
                    Response::builder()
                        .status(StatusCode::METHOD_NOT_ALLOWED)
                        .body(())
                        .expect("Failed to build method not allowed response")
                });
            let (parts, _) = response.into_parts();
            return Some((parts, b"Bridge downstream only accepts GET.".to_vec()).into());
        }

        let reader = match self.apple_bridge_transport.connect_downstream() {
            Ok(reader) => reader,
            Err(err) => {
                let response = Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header("Content-Type", "text/plain; charset=utf-8")
                    .body(())
                    .unwrap_or_else(|_| {
                        Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .body(())
                            .expect("Failed to build bridge error response")
                    });
                let (parts, _) = response.into_parts();
                return Some((parts, err.to_string().into_bytes()).into());
            }
        };

        let response = Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/x-ndjson; charset=utf-8")
            .header("Cache-Control", "no-store")
            .body(())
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(())
                    .expect("Failed to build bridge downstream response")
            });
        let (parts, _) = response.into_parts();
        Some((parts, reader).into())
    }

    /// Create a new WebView asynchronously with channel sender
    pub(crate) fn create(
        appid: &str,
        path: &str,
        session_id: Option<u64>,
        effective_options: EffectiveWebViewCreateOptions,
        sender: WebViewCreateSender,
    ) {
        let appid_owned = appid.to_string();
        let path_owned = path.to_string();

        // Check if we're already on the main thread
        if let Some(mtm) = MainThreadMarker::new() {
            // Already on main thread, create directly
            Self::create_and_register_sync(
                &appid_owned,
                &path_owned,
                session_id,
                effective_options,
                mtm,
                sender,
            );
        } else {
            // Not on main thread, dispatch to main thread
            let session_id_copy = session_id;
            let options_copy = effective_options.clone();
            DispatchQueue::main().exec_async(move || match MainThreadMarker::new() {
                Some(mtm) => {
                    Self::create_and_register_sync(
                        &appid_owned,
                        &path_owned,
                        session_id_copy,
                        options_copy,
                        mtm,
                        sender,
                    );
                }
                None => {
                    let error = WebViewError::WebView(
                        "No MainThreadMarker available on main thread".to_string(),
                    );
                    log::error!(
                        "Failed to get MainThreadMarker for {}-{}: {:?}",
                        appid_owned,
                        path_owned,
                        error
                    );
                    sender.fail(WebViewCreateStage::Requested, error);
                }
            });
        }
    }

    /// Helper method to create and register WebView synchronously on main thread
    fn create_and_register_sync(
        appid: &str,
        path: &str,
        session_id: Option<u64>,
        effective_options: EffectiveWebViewCreateOptions,
        mtm: MainThreadMarker,
        sender: WebViewCreateSender,
    ) {
        let result = Self::create_with_marker(appid, path, session_id, &effective_options, mtm);

        match result {
            Ok(webview_inner) => {
                // Wrap WebViewInner in WebView
                let webview = Arc::new(crate::WebView::new(webview_inner, effective_options));

                // Register the WebView instance for future lookups
                crate::webview::register_webview(webview.clone());

                sender.succeed(webview);
                log::info!("WebView created successfully for {}-{}", appid, path);
            }
            Err(e) => {
                log::error!("Failed to create WebView for {}-{}: {:?}", appid, path, e);
                sender.fail(WebViewCreateStage::Requested, e);
            }
        }
    }

    /// Create WebView with MainThreadMarker (must be called on main thread)
    fn create_with_marker(
        appid: &str,
        path: &str,
        session_id: Option<u64>,
        effective_options: &EffectiveWebViewCreateOptions,
        mtm: MainThreadMarker,
    ) -> Result<Self, WebViewError> {
        unsafe {
            // Create WKWebViewConfiguration
            let config = WKWebViewConfiguration::new(mtm);

            // Access preferences to set security settings
            let prefs = config.preferences();
            let allow_new_windows = effective_options.profile == SecurityProfile::BrowserRelaxed;
            let is_strict = effective_options.profile == SecurityProfile::StrictDefault;
            let allow_js_windows = allow_new_windows || effective_options.has_new_window_handler;
            prefs.setJavaScriptCanOpenWindowsAutomatically(allow_js_windows);

            // Enable Web Inspector support for all webviews (lxapp pages + browser tabs).
            {
                let developer_extras_key = NSString::from_str("developerExtrasEnabled");
                let ns_true: *mut AnyObject =
                    msg_send![class!(NSNumber), numberWithBool: objc2::runtime::Bool::YES];
                let prefs_obj: *mut AnyObject = Retained::as_ptr(&prefs).cast_mut().cast();
                let _: () = msg_send![prefs_obj, setValue:ns_true, forKey:&*developer_extras_key];
            }

            // Disable local file access for security
            let allow_file_access_key = NSString::from_str("allowFileAccessFromFileURLs");
            let ns_false: *mut AnyObject =
                msg_send![class!(NSNumber), numberWithBool: objc2::runtime::Bool::NO];
            let prefs_obj: *mut AnyObject = Retained::as_ptr(&prefs).cast_mut().cast();
            let _: () = msg_send![prefs_obj, setValue:ns_false, forKey:&*allow_file_access_key];

            // Configure media playback
            config.setMediaTypesRequiringUserActionForPlayback(WKAudiovisualMediaTypes::None);

            // Disable navigation restrictions for better compatibility
            config.setLimitsNavigationsToAppBoundDomains(false);

            // Disable HTTPS upgrade for local development
            config.setUpgradeKnownHostsToHTTPS(false);

            // Profile policy on Apple data store:
            // - StrictDefault: non-persistent (ephemeral DOM storage, cleared on destroy)
            // - BrowserRelaxed: default (persistent localStorage / database)
            if is_strict {
                let non_persistent_store = WKWebsiteDataStore::nonPersistentDataStore(mtm);
                config.setWebsiteDataStore(&non_persistent_store);
            } else if let Some(proxy) = configured_proxy_for_new_webviews() {
                let default_store = WKWebsiteDataStore::defaultDataStore(mtm);
                if let Err(e) = apply_proxy_to_data_store(&default_store, Some(&proxy)) {
                    log::warn!(
                        "Failed to apply configured proxy to default data store webtag={}: {}",
                        WebTag::new(appid, path, session_id),
                        e
                    );
                }
            }

            // Register custom scheme handlers for this WebView.
            // Use registered_schemes from options; fall back to "lx" for backward compatibility.
            let webtag = WebTag::new(appid, path, session_id);
            let schemes_to_register: Vec<String> =
                if effective_options.registered_schemes.is_empty() {
                    vec!["lx".to_string()]
                } else {
                    // Only register non-standard schemes with WKURLSchemeHandler
                    // (http/https are handled via navigation delegate, not scheme handler)
                    effective_options
                        .registered_schemes
                        .iter()
                        .filter(|s| *s != "http" && *s != "https")
                        .cloned()
                        .collect()
                };

            for scheme_name in &schemes_to_register {
                if let Some(scheme_handler) =
                    super::schemehandler::LingXiaSchemeHandler::new(webtag.clone())
                {
                    let ns_scheme = NSString::from_str(scheme_name);
                    let proto_scheme_handler: &ProtocolObject<dyn WKURLSchemeHandler> =
                        ProtocolObject::from_ref(&*scheme_handler);
                    config.setURLSchemeHandler_forURLScheme(Some(proto_scheme_handler), &ns_scheme);

                    log::info!(
                        "Successfully registered scheme `{}` handler for webtag={}",
                        scheme_name,
                        webtag.as_str()
                    );
                } else {
                    log::error!(
                        "CRITICAL: Failed to create scheme `{}` handler!",
                        scheme_name
                    );
                    return Err(WebViewError::WebView(format!(
                        "Failed to create scheme `{}` handler",
                        scheme_name
                    )));
                }
            }

            // Profile policy on Apple:
            // - StrictDefault: block HTTPS subresources
            // - BrowserRelaxed: allow HTTPS subresources (no blocker on fresh config)
            #[cfg(target_os = "ios")]
            if is_strict {
                Self::enable_https_resource_blocker(&config);
            }

            // Create frame with zero size and hide the webview initially to prevent flicker due to size change on macOS.
            // It will be resized and unhidden by the Swift layout code.
            let frame = NSRect {
                origin: NSPoint { x: 0.0, y: 0.0 },
                size: NSSize {
                    width: 0.0,
                    height: 0.0,
                },
            };

            // Get WKWebView class
            #[cfg(target_os = "macos")]
            let webview_class = if effective_options.profile == SecurityProfile::BrowserRelaxed {
                let class_name = "LingXiaBrowserContextMenuWebView";
                match CString::new(class_name)
                    .ok()
                    .and_then(|name| objc2::runtime::AnyClass::get(name.as_c_str()))
                {
                    Some(class) => {
                        log::info!(
                            "Using browser context menu webview subclass webtag={} class={}",
                            webtag.as_str(),
                            class_name
                        );
                        class
                    }
                    None => {
                        log::warn!(
                            "Browser context menu webview subclass unavailable; falling back to WKWebView webtag={} class={}",
                            webtag.as_str(),
                            class_name
                        );
                        objc2::class!(WKWebView)
                    }
                }
            } else {
                objc2::class!(WKWebView)
            };
            #[cfg(not(target_os = "macos"))]
            let webview_class = objc2::class!(WKWebView);

            // Allocate WebView
            let webview: *mut AnyObject = msg_send![webview_class, alloc];
            if webview.is_null() {
                log::error!("Failed to allocate WKWebView");
                return Err(WebViewError::WebView(
                    "Failed to allocate WKWebView".to_string(),
                ));
            }

            // Initialize WebView
            let webview: *mut AnyObject =
                msg_send![webview, initWithFrame: frame, configuration: &*config];
            if webview.is_null() {
                log::error!("Failed to initialize WKWebView");
                return Err(WebViewError::WebView(
                    "Failed to initialize WKWebView".to_string(),
                ));
            }

            // Make all webviews inspectable (lxapp pages + browser tabs).
            {
                let can_set_remote_inspection: objc2::runtime::Bool = msg_send![webview, respondsToSelector: objc2::sel!(_setAllowsRemoteInspection:)];
                if can_set_remote_inspection.as_bool() {
                    let _: () = msg_send![
                        webview,
                        _setAllowsRemoteInspection: objc2::runtime::Bool::YES
                    ];
                }

                let can_set_inspectable: objc2::runtime::Bool =
                    msg_send![webview, respondsToSelector: objc2::sel!(setInspectable:)];
                if can_set_inspectable.as_bool() {
                    let _: () = msg_send![webview, setInspectable: objc2::runtime::Bool::YES];
                }
            }

            // Immediately hide the webview. It will be made visible by Swift once it's sized and positioned.
            let _: () = msg_send![webview, setHidden: true];

            // Create navigation delegate
            let navigation_delegate = LingXiaNavigationDelegate::new(
                appid.to_string(),
                path.to_string(),
                session_id,
                // Profile policy on Apple:
                // - StrictDefault: intercept https:// top-level navigation
                // - BrowserRelaxed: do not intercept
                is_strict,
                mtm,
            );

            // Set the navigation delegate on the WebView
            let proto_navigation_delegate: &ProtocolObject<dyn WKNavigationDelegate> =
                ProtocolObject::from_ref(&*navigation_delegate);
            let _: () = msg_send![webview, setNavigationDelegate: Some(proto_navigation_delegate)];

            // Set UI delegate in strict mode to suppress JS dialogs, and in browser/new-window
            // mode to handle target="_blank", window.open(), and JS dialogs.
            let needs_ui_delegate =
                is_strict || allow_new_windows || effective_options.has_new_window_handler;
            let ui_delegate = if needs_ui_delegate {
                let delegate = LingXiaUIDelegate::new(webtag.clone(), !is_strict, mtm);
                let proto_ui_delegate: &ProtocolObject<dyn WKUIDelegate> =
                    ProtocolObject::from_ref(&*delegate);
                let _: () = msg_send![webview, setUIDelegate: Some(proto_ui_delegate)];
                Some(delegate)
            } else {
                None
            };

            // Set up message handler for bridge communication
            let message_handler = Self::setup_message_handler(webview, appid, path, session_id)?;

            // Create WebViewInner instance with navigation delegate and message handler
            let webview_inner = WebViewInner {
                webview,
                _navigation_delegate: navigation_delegate,
                _ui_delegate: ui_delegate,
                _message_handler: message_handler,
                apple_bridge_transport: AppleBridgeTransport::new(webtag.clone()),
                webtag,
            };

            Ok(webview_inner)
        }
    }

    /// Install a content blocking rule list that blocks all HTTPS subresource loads.
    #[cfg(target_os = "ios")]
    fn enable_https_resource_blocker(config: &WKWebViewConfiguration) {
        let Some(mtm) = MainThreadMarker::new() else {
            log::warn!("HTTPS resource blocker requires MainThreadMarker");
            return;
        };

        let user_content_controller: *mut WKUserContentController =
            unsafe { msg_send![config, userContentController] };
        let Some(controller_ptr) = NonNull::new(user_content_controller) else {
            log::warn!("Failed to obtain userContentController for HTTPS blocker");
            return;
        };

        let Some(store) = (unsafe { WKContentRuleListStore::defaultStore(mtm) }) else {
            log::warn!("Failed to access WKContentRuleListStore for HTTPS blocker");
            return;
        };

        let controller_ref = unsafe { controller_ptr.as_ref() };
        unsafe {
            controller_ref.removeAllContentRuleLists();
        }

        let identifier = NSString::from_str(HTTPS_BLOCK_RULE_IDENTIFIER);
        let rule_json = NSString::from_str(HTTPS_BLOCK_RULE_JSON);

        let controller_raw = controller_ptr.as_ptr();
        let completion = StackBlock::new(
            move |rule_list_ptr: *mut WKContentRuleList, error_ptr: *mut NSError| {
                if let Some(rule_list) = NonNull::new(rule_list_ptr) {
                    unsafe {
                        let controller = &*controller_raw;
                        controller.addContentRuleList(rule_list.as_ref());
                    }
                    log::info!("HTTPS subresource blocking is ON for WebView");
                } else if let Some(error) = NonNull::new(error_ptr) {
                    let description = unsafe {
                        let description_ptr: *mut NSString =
                            msg_send![error.as_ptr(), localizedDescription];
                        description_ptr
                            .as_ref()
                            .map(|ns_string| ns_string.to_string())
                    };

                    if let Some(description) = description {
                        log::error!("Failed to compile HTTPS block rule: {}", description);
                    } else {
                        log::error!("Failed to compile HTTPS block rule with unknown error");
                    }
                } else {
                    log::error!("Failed to compile HTTPS block rule with no error provided");
                }
            },
        )
        .copy();

        unsafe {
            store.compileContentRuleListForIdentifier_encodedContentRuleList_completionHandler(
                Some(&identifier),
                Some(&rule_json),
                Some(&completion),
            );
        }
    }

    /// Set up message handlers for bridge communication (like Swift version)
    fn setup_message_handler(
        webview: *mut AnyObject,
        appid: &str,
        path: &str,
        session_id: Option<u64>,
    ) -> Result<Retained<LingXiaMessageHandler>, WebViewError> {
        unsafe {
            // Get the configuration from the WebView
            let config: *mut AnyObject = msg_send![webview, configuration];
            let user_content_controller: *mut AnyObject = msg_send![config, userContentController];

            let console_script = r#"
                (function() {
                    const originalLog = console.log;
                    const originalError = console.error;
                    const originalWarn = console.warn;
                    const originalInfo = console.info;

                    function sendConsoleMessage(level, args) {
                        const message = args.map(arg =>
                            typeof arg === 'object' ? JSON.stringify(arg) : String(arg)
                        ).join(' ');

                        try {
                            // Use dedicated console handler (like Swift version)
                            window.webkit.messageHandlers.LingXiaConsole.postMessage({
                                level: level,
                                message: message
                            });
                        } catch (e) {
                            // Fallback if message handler not ready
                        }
                    }

                    console.log = function(...args) {
                        sendConsoleMessage('log', args);
                        originalLog.apply(console, args);
                    };

                    console.error = function(...args) {
                        sendConsoleMessage('error', args);
                        originalError.apply(console, args);
                    };

                    console.warn = function(...args) {
                        sendConsoleMessage('warn', args);
                        originalWarn.apply(console, args);
                    };

                    console.info = function(...args) {
                        sendConsoleMessage('info', args);
                        originalInfo.apply(console, args);
                    };
                })();
            "#;

            // Inject console interceptor script
            let console_js_string = NSString::from_str(console_script);
            let user_script_class = objc2::class!(WKUserScript);
            let console_user_script: *mut AnyObject = msg_send![user_script_class, alloc];
            let injection_time: objc2::ffi::NSInteger = 0; // WKUserScriptInjectionTimeAtDocumentStart
            let console_user_script: *mut AnyObject = msg_send![console_user_script,
                initWithSource: &*console_js_string,
                injectionTime: injection_time,
                forMainFrameOnly: false];

            let _: () = msg_send![user_content_controller, addUserScript: console_user_script];

            let message_handler = LingXiaMessageHandler::new(
                appid.to_string(),
                path.to_string(),
                session_id,
                MainThreadMarker::new().unwrap(),
            )
            .ok_or_else(|| WebViewError::WebView("Failed to create message handler".to_string()))?;

            // Register message handlers with userContentController (like Swift version)
            let lingxia_name = NSString::from_str("LingXia");
            let console_name = NSString::from_str("LingXiaConsole");
            let _: () = msg_send![user_content_controller, addScriptMessageHandler: &*message_handler, name: &*lingxia_name];
            let _: () = msg_send![user_content_controller, addScriptMessageHandler: &*message_handler, name: &*console_name];
            Ok(message_handler)
        }
    }

    /// Get the raw pointer to the WebView for Swift interop
    pub fn get_swift_webview_ptr(&self) -> usize {
        self.webview as usize
    }

    #[cfg(target_os = "macos")]
    fn toggle_devtools_impl(&self, detached: bool) {
        let _ = toggle_devtools_by_swift_ptr(self.webview as usize, detached);
    }

    /// Toggle docked DevTools using WKWebView's private `_inspector` API.
    /// `show` opens the inspector respecting `_setInspectorAttachmentView:` (docked).
    /// `isInspectable` and `developerExtrasEnabled` are set at WebView creation time.
    #[cfg(target_os = "macos")]
    pub fn toggle_devtools(&self) {
        self.toggle_devtools_impl(false);
    }

    /// Toggle detached DevTools (forces `_setInspectorAttachmentView:nil` before toggle).
    #[cfg(target_os = "macos")]
    pub fn toggle_devtools_detached(&self) {
        self.toggle_devtools_impl(true);
    }

    /// Helper method to load URL on main thread
    fn load_url_on_main_thread(&self, url: &str) -> Result<(), WebViewError> {
        unsafe {
            let ns_url_string = NSString::from_str(url);
            if let Some(ns_url) = NSURL::URLWithString(&ns_url_string) {
                let request = NSURLRequest::requestWithURL(&ns_url);
                let _: *mut AnyObject = msg_send![self.webview, loadRequest: &*request];
                Ok(())
            } else {
                Err(WebViewError::WebView(format!("Invalid URL: {}", url)))
            }
        }
    }

    /// Helper method to load HTML data on main thread
    fn load_data_on_main_thread(&self, request: LoadDataRequest<'_>) -> Result<(), WebViewError> {
        unsafe {
            let data_nsstring = NSString::from_str(request.data);
            let base_url_nsstring = NSString::from_str(request.base_url);
            if let Some(base_url_obj) = NSURL::URLWithString(&base_url_nsstring) {
                let _: *mut AnyObject = msg_send![self.webview, loadHTMLString: &*data_nsstring, baseURL: &*base_url_obj];
                log::info!(
                    "Loaded HTML data into WebView with base URL: {}",
                    request.base_url
                );
                Ok(())
            } else {
                Err(WebViewError::WebView(format!(
                    "Invalid base URL: {}",
                    request.base_url
                )))
            }
        }
    }

    /// Helper method to evaluate JavaScript on main thread
    fn evaluate_javascript_on_main_thread(&self, js: &str) -> Result<(), WebViewError> {
        unsafe {
            let js_string = NSString::from_str(js);
            let completion =
                StackBlock::new(|_result: *mut AnyObject, _error: *mut NSError| {}).copy();
            // Note: evaluateJavaScript is async, but we're treating it as fire-and-forget
            // In a more complete implementation, we might want to handle the completion
            let _: () = msg_send![
                self.webview,
                evaluateJavaScript: &*js_string,
                completionHandler: Some(&*completion)
            ];
            Ok(())
        }
    }

    /// Helper method to clear browsing data on main thread
    fn clear_browsing_data_on_main_thread(&self) -> Result<(), WebViewError> {
        unsafe {
            let Some(mtm) = MainThreadMarker::new() else {
                return Err(WebViewError::WebView("Not on main thread".to_string()));
            };

            // Get the website data store from the configuration
            let config: *mut AnyObject = msg_send![self.webview, configuration];
            let config_ref = &*(config as *const WKWebViewConfiguration);
            let data_store = config_ref.websiteDataStore();

            let all_types = WKWebsiteDataStore::allWebsiteDataTypes(mtm);
            let distant_past = NSDate::distantPast();
            let completion = StackBlock::new(|| {}).copy();
            data_store.removeDataOfTypes_modifiedSince_completionHandler(
                &all_types,
                &distant_past,
                &completion,
            );

            log::info!("Clearing browsing data");
            Ok(())
        }
    }

    /// Helper method to set user agent on main thread
    fn set_user_agent_on_main_thread(&self, ua: &str) -> Result<(), WebViewError> {
        unsafe {
            let ua_string = NSString::from_str(ua);
            let _: () = msg_send![self.webview, setCustomUserAgent: if ua.is_empty() { std::ptr::null::<NSString>() } else { &*ua_string }];
            Ok(())
        }
    }
}

impl WebViewController for WebViewInner {
    fn load_url(&self, url: &str) -> Result<(), WebViewError> {
        if MainThreadMarker::new().is_some() {
            // Already on main thread, execute directly
            self.load_url_on_main_thread(url)
        } else {
            // Not on main thread, dispatch to main thread using GCD
            let webview_ptr_addr = self.webview as usize;
            let url_clone = url.to_string();

            DispatchQueue::main().exec_async(move || unsafe {
                let webview_ptr = webview_ptr_addr as *mut AnyObject;
                let url_nsstring = NSString::from_str(&url_clone);
                let url = NSURL::URLWithString(&url_nsstring);
                if let Some(url) = url {
                    let request = NSURLRequest::requestWithURL(&url);
                    let _: *mut AnyObject = msg_send![webview_ptr, loadRequest: &*request];
                }
            });

            Ok(())
        }
    }

    fn load_data(&self, request: LoadDataRequest<'_>) -> Result<(), WebViewError> {
        if MainThreadMarker::new().is_some() {
            // Already on main thread, execute directly
            self.load_data_on_main_thread(request)
        } else {
            // Not on main thread, dispatch to main thread using GCD
            let webview_ptr_addr = self.webview as usize;
            let data_clone = request.data.to_string();
            let base_url_clone = request.base_url.to_string();

            DispatchQueue::main().exec_async(move || unsafe {
                let webview_ptr = webview_ptr_addr as *mut AnyObject;
                let data_nsstring = NSString::from_str(&data_clone);
                let base_url_nsstring = NSString::from_str(&base_url_clone);
                let base_url = NSURL::URLWithString(&base_url_nsstring);

                if let Some(base_url) = base_url {
                    let _: *mut AnyObject =
                        msg_send![webview_ptr, loadHTMLString: &*data_nsstring, baseURL: &*base_url];
                }
            });

            Ok(())
        }
    }

    fn evaluate_javascript(&self, js: &str) -> Result<(), WebViewError> {
        if MainThreadMarker::new().is_some() {
            // Already on main thread, execute directly
            self.evaluate_javascript_on_main_thread(js)
        } else {
            // Not on main thread, dispatch to main thread using GCD
            let webview_ptr_addr = self.webview as usize;
            let js_clone = js.to_string();

            DispatchQueue::main().exec_async(move || unsafe {
                let webview_ptr = webview_ptr_addr as *mut AnyObject;
                let js_nsstring = NSString::from_str(&js_clone);
                let completion =
                    StackBlock::new(|_result: *mut AnyObject, _error: *mut NSError| {}).copy();
                let _: () = msg_send![
                    webview_ptr,
                    evaluateJavaScript: &*js_nsstring,
                    completionHandler: Some(&*completion)
                ];
            });

            Ok(())
        }
    }

    fn post_message(&self, message: &str) -> Result<(), WebViewError> {
        self.apple_bridge_transport.enqueue_message(message)
    }

    fn clear_browsing_data(&self) -> Result<(), WebViewError> {
        if MainThreadMarker::new().is_some() {
            // Already on main thread, execute directly
            self.clear_browsing_data_on_main_thread()
        } else {
            // Not on main thread, dispatch to main thread using GCD
            let webview_ptr_addr = self.webview as usize;

            DispatchQueue::main().exec_async(move || unsafe {
                let webview_ptr = webview_ptr_addr as *mut AnyObject;
                let Some(mtm) = MainThreadMarker::new() else {
                    return;
                };

                let configuration: *mut AnyObject = msg_send![webview_ptr, configuration];
                let config_ref = &*(configuration as *const WKWebViewConfiguration);
                let data_store = config_ref.websiteDataStore();

                let data_types = WKWebsiteDataStore::allWebsiteDataTypes(mtm);
                let distant_past = NSDate::distantPast();
                let completion = StackBlock::new(|| {}).copy();
                data_store.removeDataOfTypes_modifiedSince_completionHandler(
                    &data_types,
                    &distant_past,
                    &completion,
                );
            });

            Ok(())
        }
    }

    fn set_user_agent(&self, ua: &str) -> Result<(), WebViewError> {
        if MainThreadMarker::new().is_some() {
            // Already on main thread, execute directly
            self.set_user_agent_on_main_thread(ua)
        } else {
            // Not on main thread, dispatch to main thread using GCD
            let webview_ptr_addr = self.webview as usize;
            let ua_clone = ua.to_string();

            DispatchQueue::main().exec_async(move || unsafe {
                let webview_ptr = webview_ptr_addr as *mut AnyObject;
                let ua_nsstring = NSString::from_str(&ua_clone);
                let _: () = msg_send![webview_ptr, setCustomUserAgent: &*ua_nsstring];
            });

            Ok(())
        }
    }
}

impl Drop for WebViewInner {
    fn drop(&mut self) {
        self.apple_bridge_transport.shutdown();
        // WebView cleanup - only cleanup the actual WebView resources
        // Runtime storage is managed separately
        if MainThreadMarker::new().is_some() {
            // Wrap in ObjC exception handler — WKWebView dealloc can throw
            // ObjC exceptions that Rust's catch_unwind cannot handle.
            let result =
                objc2::exception::catch(std::panic::AssertUnwindSafe(|| self.cleanup_webview()));
            if let Err(exception) = result {
                log::error!(
                    "[WebViewInner] ObjC exception during WebView cleanup ({}): {:?}",
                    self.webtag.as_str(),
                    exception
                );
            }
        } else {
            let webview_ptr_addr = self.webview as usize;
            DispatchQueue::main().exec_async(move || {
                unsafe {
                    let webview_ptr = webview_ptr_addr as *mut AnyObject;
                    // Direct cleanup without creating temporary instance
                    let _: () = msg_send![webview_ptr, removeFromSuperview];
                    let _: () = msg_send![webview_ptr, stopLoading];
                }
            });
            log::info!(
                "[WebViewInner] Apple WebViewInner dropped and cleanup requested ({})",
                self.webtag.as_str()
            );
        }
    }
}

impl WebViewInner {
    /// Cleanup WebView resources on main thread and properly release the WebView
    fn cleanup_webview(&self) {
        unsafe {
            // Remove from superview if attached to prevent memory leaks
            let _: () = msg_send![self.webview, removeFromSuperview];

            // Stop loading any ongoing requests
            let _: () = msg_send![self.webview, stopLoading];

            // Clear navigation delegate to prevent callbacks after deallocation
            let _: () = msg_send![self.webview, setNavigationDelegate: std::ptr::null::<*const AnyObject>()];

            // Clear UI delegate to prevent callbacks after deallocation
            let _: () =
                msg_send![self.webview, setUIDelegate: std::ptr::null::<*const AnyObject>()];

            // Clear scroll view delegate if any (iOS only — macOS WKWebView has no scrollView property)
            #[cfg(target_os = "ios")]
            {
                let scroll_view: *mut AnyObject = msg_send![self.webview, scrollView];
                if !scroll_view.is_null() {
                    let _: () =
                        msg_send![scroll_view, setDelegate: std::ptr::null::<*const AnyObject>()];
                }
            }

            // Release the WebView object
            // This is critical for proper memory management
            let _: () = msg_send![self.webview, release];

            log::info!(
                "[WebViewInner] Apple WebView instance completely released: removed from superview, cleared all delegates, and released object ({})",
                self.webtag.as_str()
            );
        }
    }
}

// Message Handler Implementation (like Swift WebViewMessageHandler)
use objc2_web_kit::WKScriptMessageHandler;

#[derive(Debug)]
pub struct LingXiaMessageHandlerIvars {
    appid: String,
    path: String,
    session_id: Option<u64>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "LingXiaMessageHandler"]
    #[thread_kind = MainThreadOnly]
    #[ivars = LingXiaMessageHandlerIvars]
    pub struct LingXiaMessageHandler;

    unsafe impl NSObjectProtocol for LingXiaMessageHandler {}

    unsafe impl WKScriptMessageHandler for LingXiaMessageHandler {
        #[unsafe(method(userContentController:didReceiveScriptMessage:))]
        fn user_content_controller_did_receive_script_message(
            &self,
            _controller: *mut AnyObject,
            message: *mut AnyObject,
        ) {
            unsafe {
                // Get message body
                let body: *mut AnyObject = msg_send![message, body];

                let message_string = if !body.is_null() {
                    // Try to get as string first
                    let is_string: objc2::runtime::Bool =
                        msg_send![body, isKindOfClass: objc2::class!(NSString)];
                    if is_string.as_bool() {
                        let ns_string = &*(body as *const NSString);
                        ns_string.to_string()
                    } else {
                        // Try to convert to JSON if it's a dictionary
                        let is_dict: objc2::runtime::Bool =
                            msg_send![body, isKindOfClass: objc2::class!(NSDictionary)];
                        if is_dict.as_bool() {
                            // Convert NSDictionary to JSON string (like Swift version)
                            let body_obj: &AnyObject = &*(body as *const AnyObject);
                            let json_data =
                                match NSJSONSerialization::dataWithJSONObject_options_error(
                                    body_obj,
                                    NSJSONWritingOptions(0),
                                ) {
                                    Ok(data) => data,
                                    Err(err) => {
                                        log::error!(
                                            "Failed to serialize dictionary to JSON: {}",
                                            err.localizedDescription().to_string()
                                        );
                                        return;
                                    }
                                };

                            let json_string = match NSString::initWithData_encoding(
                                NSString::alloc(),
                                &json_data,
                                4, // NSUTF8StringEncoding
                            ) {
                                Some(s) => s,
                                None => {
                                    log::error!("Failed to convert JSON data to string");
                                    return;
                                }
                            };
                            json_string.to_string()
                        } else {
                            log::error!("Unsupported message body type");
                            return;
                        }
                    }
                } else {
                    log::error!("Message body is null");
                    return;
                };

                // Get message name to determine handler type
                let name: *mut AnyObject = msg_send![message, name];
                let handler_name = if !name.is_null() {
                    let ns_string = &*(name as *const NSString);
                    ns_string.to_string()
                } else {
                    "unknown".to_string()
                };

                // Route to appropriate handler based on name (like Swift version)
                match handler_name.as_str() {
                    "LingXia" => {
                        self.handle_bridge_message(message_string);
                    }
                    "LingXiaConsole" => {
                        self.handle_console_message(message_string);
                    }
                    _ => {
                        log::warn!("Unknown message handler: {}", handler_name);
                    }
                }
            }
        }
    }
);

impl LingXiaMessageHandler {
    /// Create a new message handler
    pub fn new(
        appid: String,
        path: String,
        session_id: Option<u64>,
        mtm: MainThreadMarker,
    ) -> Option<Retained<Self>> {
        unsafe {
            let instance = Self::alloc(mtm);
            let instance = instance.set_ivars(LingXiaMessageHandlerIvars {
                appid,
                path,
                session_id,
            });
            let instance: Retained<Self> = msg_send![super(instance), init];
            Some(instance)
        }
    }

    /// Handle bridge messages
    fn handle_bridge_message(&self, message: String) {
        let ivars = self.ivars();

        let webtag = WebTag::new(&ivars.appid, &ivars.path, ivars.session_id);
        if let Some(delegate) = find_webview_delegate(&webtag) {
            delegate.handle_post_message(message);
        }
    }

    /// Handle console messages
    fn handle_console_message(&self, message: String) {
        let ivars = self.ivars();

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&message) {
            if let (Some(level), Some(console_message)) = (
                json.get("level").and_then(|v| v.as_str()),
                json.get("message").and_then(|v| v.as_str()),
            ) {
                let log_level = match level {
                    "error" => LogLevel::Error,
                    "warn" => LogLevel::Warn,
                    "info" => LogLevel::Info,
                    "debug" => LogLevel::Debug,
                    _ => LogLevel::Info,
                };

                let webtag = WebTag::new(&ivars.appid, &ivars.path, ivars.session_id);
                if let Some(delegate) = find_webview_delegate(&webtag) {
                    delegate.log(log_level, console_message);
                }
            } else {
                log::error!("Failed to parse console message fields: {}", message);
            }
        } else {
            log::error!("Failed to parse console message JSON: {}", message);
        }
    }
}
