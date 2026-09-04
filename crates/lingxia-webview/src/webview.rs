#![cfg_attr(
    not(any(
        target_os = "android",
        target_os = "ios",
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", target_env = "ohos")
    )),
    allow(dead_code)
)]

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{Sender, SyncSender, channel, sync_channel};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use tokio::sync::watch;

#[cfg(target_os = "android")]
use crate::android::WebViewInner;

#[cfg(any(target_os = "ios", target_os = "macos"))]
use crate::apple::WebViewInner;

#[cfg(all(target_os = "linux", target_env = "ohos"))]
use crate::harmony::WebViewInner;

#[cfg(target_os = "windows")]
use crate::windows::WebViewInner;

#[cfg(not(any(
    target_os = "android",
    target_os = "ios",
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", target_env = "ohos")
)))]
pub(crate) struct WebViewInner {
    webtag: WebTag,
}

use crate::traits::{
    AsyncSchemeHandler, ClickOptions, ContextualSchemeRequest, DownloadHandler, DownloadRequest,
    FileChooserRequest, FileChooserResponse, FillOptions, NativeWebViewId, NavigationHandler,
    NavigationPolicy, NavigationRequest, NewWindowHandler, NewWindowPolicy, PressOptions,
    SchemeOutcome, SchemeRequestFrame, ScrollOptions, TypeOptions, UserAgentOverride,
    WebMessageContext, WebMessageFrame, WebMessageSource, WebMessageTransport,
    WebViewInputController,
};
use crate::{
    ClearSiteDataOptions, ClearSiteDataResult, IncomingWebMessage, LoadDataRequest,
    NetworkCaptureSnapshot, TrustedLoadIntent, WebResourceResponse, WebViewController,
    WebViewCookie, WebViewCookieSetRequest, WebViewDelegate, WebViewError, WebViewInputError,
    WebViewScriptError,
};
use async_trait::async_trait;

const APPLE_INTERNAL_SCHEME: &str = "lx-apple";
const MAX_PENDING_WEB_MESSAGES: usize = 1024;
const WEB_MESSAGE_WORKER_COUNT: usize = 4;

static NEXT_NATIVE_WEBVIEW_ID: AtomicU64 = AtomicU64::new(1);

fn next_native_webview_id() -> NativeWebViewId {
    // Zero is deliberately never allocated, so a platform's default integer
    // cannot accidentally match a real native instance. Exhaustion is safer
    // than wrapping and allowing a retired native callback to match again.
    let raw = NEXT_NATIVE_WEBVIEW_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .expect("native WebView identity space exhausted");
    NativeWebViewId::new(raw)
}

#[derive(Default)]
struct WebMessageIngress {
    state: Mutex<WebMessageIngressState>,
}

#[derive(Default)]
struct WebMessageIngressState {
    queue: VecDeque<IncomingWebMessage>,
    scheduled: bool,
    closed: bool,
    in_flight: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WebMessageEnqueue {
    Queued,
    Schedule,
    Full,
    Closed,
}

impl WebMessageIngress {
    /// Enqueue one message and report whether this instance must be scheduled.
    /// A bounded queue prevents an untrusted page from growing native memory
    /// without limit; accepted messages remain FIFO.
    fn enqueue(&self, message: IncomingWebMessage) -> WebMessageEnqueue {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.closed {
            return WebMessageEnqueue::Closed;
        }
        if state.queue.len() >= MAX_PENDING_WEB_MESSAGES {
            return WebMessageEnqueue::Full;
        }
        state.queue.push_back(message);
        if state.scheduled {
            WebMessageEnqueue::Queued
        } else {
            state.scheduled = true;
            WebMessageEnqueue::Schedule
        }
    }

    /// Stop accepting messages and discard all queued work.
    ///
    /// A message becomes in-flight while holding this mutex, immediately
    /// before its delegate lookup. Closing does not interrupt that already
    /// admitted delivery, but prevents every queued and future message from
    /// reaching a delegate.
    fn close(&self) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.closed = true;
        state.queue.clear();
        if !state.in_flight {
            state.scheduled = false;
        }
    }

    /// Pop the next admitted message for this instance's sole scheduled job.
    /// Delegate code never runs while this lock is held, so re-entrant ingress
    /// appends after the current message.
    fn begin_delivery(&self) -> Option<IncomingWebMessage> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.closed {
            state.scheduled = false;
            return None;
        }
        match state.queue.pop_front() {
            Some(message) => {
                state.in_flight = true;
                Some(message)
            }
            None => {
                state.scheduled = false;
                None
            }
        }
    }

    fn finish_delivery(&self) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        debug_assert!(state.in_flight);
        state.in_flight = false;
    }

    fn drain<F>(&self, mut deliver: F)
    where
        F: FnMut(IncomingWebMessage),
    {
        while let Some(message) = self.begin_delivery() {
            let result = catch_unwind(AssertUnwindSafe(|| deliver(message)));
            self.finish_delivery();
            if result.is_err() {
                log::error!("WebView message delegate panicked; continuing ingress drain");
            }
        }
    }
}

struct WebMessageJob {
    ingress: Arc<WebMessageIngress>,
    webview: std::sync::Weak<WebView>,
}

/// Process-lifetime, fixed-size executor for WebView message ingress.
///
/// Each ingress schedules no more than one job, so an instance stays serial;
/// different instances are distributed across workers and can make progress in
/// parallel without creating a thread per callback or idle burst.
struct WebMessageExecutor {
    senders: Vec<Sender<WebMessageJob>>,
    next_worker: AtomicUsize,
}

impl WebMessageExecutor {
    fn global() -> &'static Self {
        static EXECUTOR: OnceLock<WebMessageExecutor> = OnceLock::new();
        EXECUTOR.get_or_init(Self::new)
    }

    fn new() -> Self {
        let mut senders = Vec::with_capacity(WEB_MESSAGE_WORKER_COUNT);
        for worker_index in 0..WEB_MESSAGE_WORKER_COUNT {
            let (sender, receiver) = channel::<WebMessageJob>();
            std::thread::Builder::new()
                .name(format!("lingxia-web-message-worker-{worker_index}"))
                .spawn(move || {
                    while let Ok(job) = receiver.recv() {
                        let ingress = Arc::clone(&job.ingress);
                        job.ingress.drain(|message| {
                            let Some(webview) = job.webview.upgrade() else {
                                ingress.close();
                                return;
                            };
                            if let Some(delegate) = webview.get_delegate() {
                                delegate.handle_post_message(message);
                            } else {
                                log::debug!(
                                    "Dropping WebView message before delegate installation ({})",
                                    webview.webtag()
                                );
                            }
                        });
                    }
                })
                .expect("failed to start fixed WebView message worker");
            senders.push(sender);
        }
        Self {
            senders,
            next_worker: AtomicUsize::new(0),
        }
    }

    fn schedule(&self, job: WebMessageJob) -> Result<(), WebMessageJob> {
        let worker = self.next_worker.fetch_add(1, Ordering::Relaxed) % self.senders.len();
        self.senders[worker].send(job).map_err(|error| error.0)
    }
}

#[cfg(not(any(
    target_os = "android",
    target_os = "ios",
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", target_env = "ohos")
)))]
fn unsupported_webview_error(action: &str) -> WebViewError {
    WebViewError::Unsupported(action.to_string())
}

#[cfg(not(any(
    target_os = "android",
    target_os = "ios",
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", target_env = "ohos")
)))]
impl WebViewInner {
    pub(crate) fn create(
        appid: &str,
        path: &str,
        session_id: Option<u64>,
        _effective_options: EffectiveWebViewCreateOptions,
        sender: WebViewCreateSender,
    ) {
        let _webtag = WebTag::new(appid, path, session_id);
        sender.fail(
            WebViewCreateStage::Requested,
            unsupported_webview_error("webview creation"),
        );
    }
}

#[cfg(not(any(
    target_os = "android",
    target_os = "ios",
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", target_env = "ohos")
)))]
#[async_trait]
impl WebViewController for WebViewInner {
    fn load_url(&self, _url: &str) -> Result<(), WebViewError> {
        Err(unsupported_webview_error("load_url"))
    }

    fn load_data(&self, _request: LoadDataRequest<'_>) -> Result<(), WebViewError> {
        Err(unsupported_webview_error("load_data"))
    }

    fn exec_js(&self, _js: &str) -> Result<(), WebViewError> {
        Err(unsupported_webview_error("exec_js"))
    }

    async fn eval_js(&self, _js: &str) -> Result<serde_json::Value, WebViewScriptError> {
        Err(WebViewScriptError::Unsupported(
            "JavaScript evaluation is not supported on this platform",
        ))
    }

    fn post_message(&self, _message: &str) -> Result<(), WebViewError> {
        Err(unsupported_webview_error("post_message"))
    }

    fn clear_browsing_data(&self) -> Result<(), WebViewError> {
        Err(unsupported_webview_error("clear_browsing_data"))
    }

    fn set_user_agent_override(&self, _user_agent: UserAgentOverride) -> Result<(), WebViewError> {
        Err(unsupported_webview_error("set_user_agent_override"))
    }
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

fn scheme_waker_from_sender(sender: SyncSender<()>) -> Waker {
    // SAFETY: RawWaker functions maintain Arc refcounts correctly.
    unsafe { Waker::from_raw(scheme_raw_waker(Arc::new(sender))) }
}

fn scheme_raw_waker(sender: Arc<SyncSender<()>>) -> RawWaker {
    RawWaker::new(Arc::into_raw(sender) as *const (), &SCHEME_WAKER_VTABLE)
}

unsafe fn scheme_waker_clone(data: *const ()) -> RawWaker {
    // SAFETY: data is created from Arc<SyncSender<()>> in scheme_raw_waker.
    let arc = unsafe { Arc::<SyncSender<()>>::from_raw(data as *const SyncSender<()>) };
    let cloned = Arc::clone(&arc);
    let _ = Arc::into_raw(arc);
    scheme_raw_waker(cloned)
}

unsafe fn scheme_waker_wake(data: *const ()) {
    // SAFETY: data is created from Arc<SyncSender<()>> in scheme_raw_waker.
    let arc = unsafe { Arc::<SyncSender<()>>::from_raw(data as *const SyncSender<()>) };
    let _ = arc.try_send(());
}

unsafe fn scheme_waker_wake_by_ref(data: *const ()) {
    // SAFETY: data is created from Arc<SyncSender<()>> in scheme_raw_waker.
    let arc = unsafe { Arc::<SyncSender<()>>::from_raw(data as *const SyncSender<()>) };
    let _ = arc.try_send(());
    let _ = Arc::into_raw(arc);
}

unsafe fn scheme_waker_drop(data: *const ()) {
    // SAFETY: data is created from Arc<SyncSender<()>> in scheme_raw_waker.
    let _ = unsafe { Arc::<SyncSender<()>>::from_raw(data as *const SyncSender<()>) };
}

static SCHEME_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    scheme_waker_clone,
    scheme_waker_wake,
    scheme_waker_wake_by_ref,
    scheme_waker_drop,
);

fn block_on_scheme_future<F>(future: F) -> F::Output
where
    F: Future,
{
    let (tx, rx) = sync_channel::<()>(1);
    let waker = scheme_waker_from_sender(tx);
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);

    loop {
        match Pin::as_mut(&mut future).poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => {
                if rx.recv().is_err() {
                    std::thread::yield_now();
                }
            }
        }
    }
}

/// Security profile for WebView creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SecurityProfile {
    StrictDefault,
    BrowserRelaxed,
}

/// Website-data lifetime for a WebView.
///
/// This is independent of the security profile: a browser-profile WebView can
/// use an ephemeral data store without giving up browser navigation features.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebViewDataMode {
    /// Keep the platform behavior associated with the selected security
    /// profile. Browser-profile WebViews use the shared persistent store.
    #[default]
    ProfileDefault,
    /// Isolate cookies and site storage from persistent/shared browser data
    /// and discard them when the WebView is destroyed.
    Ephemeral,
}

pub(crate) type FileChooserFuture =
    Pin<Box<dyn Future<Output = FileChooserResponse> + Send + 'static>>;
pub(crate) type FileChooserHandler =
    Box<dyn Fn(FileChooserRequest) -> FileChooserFuture + Send + Sync>;

/// Internal WebView creation options.
pub(crate) struct WebViewCreateOptions {
    pub(crate) profile: SecurityProfile,
    pub(crate) data_mode: WebViewDataMode,
    pub(crate) scheme_handlers: HashMap<String, AsyncSchemeHandler>,
    pub(crate) navigation_handler: Option<NavigationHandler>,
    pub(crate) new_window_handler: Option<NewWindowHandler>,
    pub(crate) download_handler: Option<DownloadHandler>,
    pub(crate) file_chooser_handler: Option<FileChooserHandler>,
    pub(crate) delegate: Option<Arc<dyn WebViewDelegate>>,
    /// The webview belongs to a surface, not the app's page container; the
    /// platform shell must not adopt it into stack-page presentation.
    pub(crate) surface_owned: bool,
}

impl std::fmt::Debug for WebViewCreateOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebViewCreateOptions")
            .field("profile", &self.profile)
            .field("data_mode", &self.data_mode)
            .field(
                "scheme_handlers",
                &self.scheme_handlers.keys().collect::<Vec<_>>(),
            )
            .field("has_navigation_handler", &self.navigation_handler.is_some())
            .field("has_new_window_handler", &self.new_window_handler.is_some())
            .field("has_download_handler", &self.download_handler.is_some())
            .field(
                "has_file_chooser_handler",
                &self.file_chooser_handler.is_some(),
            )
            .field("has_delegate", &self.delegate.is_some())
            .finish()
    }
}

/// Global HTTP proxy configuration shared by all WebViews in the process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub bypass: Vec<String>,
}

impl ProxyConfig {
    pub fn new(host: impl Into<String>, port: u16) -> Result<Self, WebViewError> {
        let cfg = Self {
            host: host.into(),
            port,
            bypass: Vec::new(),
        };
        cfg.validate()
    }

    pub fn with_bypass<I, S>(mut self, bypass: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.bypass = bypass.into_iter().map(Into::into).collect();
        self
    }

    fn validate(self) -> Result<Self, WebViewError> {
        let host = self.host.trim().to_string();
        if host.is_empty() {
            return Err(WebViewError::InvalidCreateOptions(
                "proxy host cannot be empty".to_string(),
            ));
        }
        if host.contains(char::is_whitespace) {
            return Err(WebViewError::InvalidCreateOptions(
                "proxy host cannot contain whitespace".to_string(),
            ));
        }
        if self.port == 0 {
            return Err(WebViewError::InvalidCreateOptions(
                "proxy port must be greater than 0".to_string(),
            ));
        }

        let mut seen = HashSet::new();
        let mut bypass = Vec::new();
        for raw in self.bypass {
            let rule = raw.trim();
            if rule.is_empty() {
                continue;
            }
            let key = rule.to_ascii_lowercase();
            if seen.insert(key) {
                bypass.push(rule.to_string());
            }
        }

        Ok(Self {
            host,
            bypass,
            ..self
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyApplyStatus {
    Applied,
    Cleared,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyActivation {
    EffectiveNow,
    NewWebViewsOnly,
    EngineRecreateRequired,
    NotApplied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyApplyReport {
    pub status: ProxyApplyStatus,
    pub activation: ProxyActivation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ProxyApplyReport {
    pub fn applied(activation: ProxyActivation) -> Self {
        Self {
            status: ProxyApplyStatus::Applied,
            activation,
            detail: None,
        }
    }

    pub fn cleared(activation: ProxyActivation) -> Self {
        Self {
            status: ProxyApplyStatus::Cleared,
            activation,
            detail: None,
        }
    }

    pub fn unsupported(detail: impl Into<String>) -> Self {
        Self {
            status: ProxyApplyStatus::Unsupported,
            activation: ProxyActivation::NotApplied,
            detail: Some(detail.into()),
        }
    }
}

/// Effective, normalized options actually applied to a concrete WebView instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct EffectiveWebViewCreateOptions {
    pub(crate) profile: SecurityProfile,
    /// Website-data lifetime, independent of the security profile.
    #[serde(default)]
    pub(crate) data_mode: WebViewDataMode,
    /// Scheme names registered via `on_scheme` (serializable).
    #[serde(default)]
    pub(crate) registered_schemes: Vec<String>,
    #[serde(default)]
    pub(crate) has_navigation_handler: bool,
    #[serde(default)]
    pub(crate) has_new_window_handler: bool,
    #[serde(default)]
    pub(crate) has_download_handler: bool,
    #[serde(default)]
    pub(crate) has_file_chooser_handler: bool,
    #[serde(default)]
    pub(crate) has_delegate: bool,
    #[serde(default)]
    pub(crate) surface_owned: bool,
}

impl Default for WebViewCreateOptions {
    fn default() -> Self {
        Self::strict()
    }
}

impl WebViewCreateOptions {
    fn strict() -> Self {
        Self {
            profile: SecurityProfile::StrictDefault,
            data_mode: WebViewDataMode::ProfileDefault,
            scheme_handlers: HashMap::new(),
            navigation_handler: None,
            new_window_handler: None,
            download_handler: None,
            file_chooser_handler: None,
            delegate: None,
            surface_owned: false,
        }
    }

    fn browser() -> Self {
        Self {
            profile: SecurityProfile::BrowserRelaxed,
            data_mode: WebViewDataMode::ProfileDefault,
            scheme_handlers: HashMap::new(),
            navigation_handler: None,
            new_window_handler: None,
            download_handler: None,
            file_chooser_handler: None,
            delegate: None,
            surface_owned: false,
        }
    }

    /// Register a scheme handler for a custom URL scheme.
    ///
    /// The handler is async by design.
    ///
    /// Usage:
    /// - Async workload:
    ///   `options.on_scheme("lx", |req| async move { ... })`
    /// - Immediate response:
    ///   `options.on_scheme("lx", |req| async move { immediate(req).into() })`
    fn on_contextual_scheme<F, Fut>(mut self, scheme: &str, handler: F) -> Self
    where
        F: Fn(ContextualSchemeRequest) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = SchemeOutcome> + Send + 'static,
    {
        let normalized = scheme.trim().to_ascii_lowercase();
        if !normalized.is_empty() {
            self.scheme_handlers.insert(
                normalized,
                Arc::new(move |req| {
                    let fut = handler(req);
                    Box::pin(fut)
                }),
            );
        }
        self
    }

    fn on_scheme<F, Fut>(self, scheme: &str, handler: F) -> Self
    where
        F: Fn(http::Request<Vec<u8>>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = SchemeOutcome> + Send + 'static,
    {
        self.on_contextual_scheme(scheme, move |request| handler(request.into_request()))
    }

    /// Register a navigation handler that decides whether to allow or cancel navigations.
    /// The handler receives the URL and available platform navigation metadata.
    fn on_navigation<F>(mut self, handler: F) -> Self
    where
        F: Fn(&NavigationRequest) -> NavigationPolicy + Send + Sync + 'static,
    {
        self.navigation_handler = Some(Box::new(handler));
        self
    }

    /// Register a new-window handler for `target="_blank"` / `window.open()`.
    /// The handler receives the URL and returns a `NewWindowPolicy`.
    fn on_new_window<F>(mut self, handler: F) -> Self
    where
        F: Fn(&str) -> NewWindowPolicy + Send + Sync + 'static,
    {
        self.new_window_handler = Some(Box::new(handler));
        self
    }

    /// Register a download handler for browser-mode downloads.
    ///
    /// The handler runs synchronously on the platform callback thread. Keep it fast and
    /// spawn background work onto your runtime inside the closure.
    ///
    /// This callback is only valid for browser profile.
    /// Public API: `WebViewBuilder::browser(webtag).on_download(...).create()`.
    /// In this mode, download requests are routed to the callback path instead of in-WebView
    /// download UI.
    fn on_download<F>(mut self, handler: F) -> Self
    where
        F: Fn(DownloadRequest) + Send + Sync + 'static,
    {
        self.download_handler = Some(Box::new(handler));
        self
    }

    fn on_file_chooser<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(FileChooserRequest) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = FileChooserResponse> + Send + 'static,
    {
        self.file_chooser_handler = Some(Box::new(move |request| Box::pin(handler(request))));
        self
    }

    fn delegate(mut self, delegate: Arc<dyn WebViewDelegate>) -> Self {
        self.delegate = Some(delegate);
        self
    }

    fn data_mode(mut self, data_mode: WebViewDataMode) -> Self {
        self.data_mode = data_mode;
        self
    }

    fn surface_owned(mut self, surface_owned: bool) -> Self {
        self.surface_owned = surface_owned;
        self
    }

    pub(crate) fn normalize(
        self,
    ) -> Result<(EffectiveWebViewCreateOptions, PendingCallbacks), WebViewError> {
        if self.profile != SecurityProfile::BrowserRelaxed && self.download_handler.is_some() {
            return Err(WebViewError::InvalidCreateOptions(
                "download callback is only supported in browser profile; use WebViewBuilder::browser(webtag).on_download(...).create()".to_string(),
            ));
        }
        if self.scheme_handlers.contains_key(APPLE_INTERNAL_SCHEME) {
            return Err(WebViewError::InvalidCreateOptions(format!(
                "scheme '{APPLE_INTERNAL_SCHEME}' is reserved for LingXia Apple bridge transport"
            )));
        }
        let mut registered_schemes: Vec<String> = self.scheme_handlers.keys().cloned().collect();
        registered_schemes.sort_unstable();
        registered_schemes.dedup();
        let effective = EffectiveWebViewCreateOptions {
            profile: self.profile,
            data_mode: self.data_mode,
            registered_schemes,
            has_navigation_handler: self.navigation_handler.is_some(),
            has_new_window_handler: self.new_window_handler.is_some(),
            has_download_handler: self.download_handler.is_some(),
            has_file_chooser_handler: self.file_chooser_handler.is_some(),
            has_delegate: self.delegate.is_some(),
            surface_owned: self.surface_owned,
        };
        let pending = PendingCallbacks {
            scheme_handlers: self.scheme_handlers,
            navigation_handler: self.navigation_handler,
            new_window_handler: self.new_window_handler,
            download_handler: self.download_handler,
            file_chooser_handler: self.file_chooser_handler,
            delegate: self.delegate,
        };
        Ok((effective, pending))
    }
}

/// Entry point for mode-specific WebView creation.
///
/// Typical usage:
/// - Strict lxapp page:
///   `WebViewBuilder::strict(tag).on_scheme(...).on_navigation(...).create()`
/// - Browser page:
///   `WebViewBuilder::browser(tag).on_new_window(...).on_download(...).create()`
pub struct WebViewBuilder;

#[must_use = "call .create() to start WebView creation"]
pub struct StrictWebViewBuilder {
    webtag: WebTag,
    options: WebViewCreateOptions,
}

#[must_use = "call .create() to start WebView creation"]
pub struct BrowserWebViewBuilder {
    webtag: WebTag,
    options: WebViewCreateOptions,
}

impl WebViewBuilder {
    /// Start a strict-profile WebView builder.
    #[must_use = "call .create() to start WebView creation"]
    pub fn strict(webtag: WebTag) -> StrictWebViewBuilder {
        StrictWebViewBuilder {
            webtag,
            options: WebViewCreateOptions::strict(),
        }
    }

    /// Start a browser-profile WebView builder.
    #[must_use = "call .create() to start WebView creation"]
    pub fn browser(webtag: WebTag) -> BrowserWebViewBuilder {
        BrowserWebViewBuilder {
            webtag,
            options: WebViewCreateOptions::browser(),
        }
    }
}

impl StrictWebViewBuilder {
    /// Bind a `WebViewDelegate` during creation.
    ///
    /// This is the only supported way to configure delegate callbacks.
    pub fn delegate(mut self, delegate: Arc<dyn WebViewDelegate>) -> Self {
        self.options = self.options.delegate(delegate);
        self
    }

    /// Select the website-data lifetime independently of the security profile.
    pub fn data_mode(mut self, data_mode: WebViewDataMode) -> Self {
        self.options = self.options.data_mode(data_mode);
        self
    }

    /// Mark the webview as surface-owned so platform shells leave its
    /// presentation to the surface instead of the page container.
    pub fn surface_owned(mut self, surface_owned: bool) -> Self {
        self.options = self.options.surface_owned(surface_owned);
        self
    }

    pub fn on_scheme<F, Fut>(mut self, scheme: &str, handler: F) -> Self
    where
        F: Fn(http::Request<Vec<u8>>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = SchemeOutcome> + Send + 'static,
    {
        self.options = self.options.on_scheme(scheme, handler);
        self
    }

    /// Register a scheme handler which receives native-instance and frame
    /// context alongside the HTTP request.
    pub fn on_contextual_scheme<F, Fut>(mut self, scheme: &str, handler: F) -> Self
    where
        F: Fn(ContextualSchemeRequest) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = SchemeOutcome> + Send + 'static,
    {
        self.options = self.options.on_contextual_scheme(scheme, handler);
        self
    }

    pub fn on_navigation<F>(mut self, handler: F) -> Self
    where
        F: Fn(&NavigationRequest) -> NavigationPolicy + Send + Sync + 'static,
    {
        self.options = self.options.on_navigation(handler);
        self
    }

    pub fn on_new_window<F>(mut self, handler: F) -> Self
    where
        F: Fn(&str) -> NewWindowPolicy + Send + Sync + 'static,
    {
        self.options = self.options.on_new_window(handler);
        self
    }

    pub fn on_file_chooser<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(FileChooserRequest) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = FileChooserResponse> + Send + 'static,
    {
        self.options = self.options.on_file_chooser(handler);
        self
    }

    /// Create a strict-profile WebView session.
    ///
    /// Re-creating with the same `webtag` follows strict rules:
    /// - Different options => creation fails.
    /// - Same options but new callback registrations => creation fails.
    /// - Same options and no callbacks => existing instance is reused.
    pub fn create(self) -> WebViewSession {
        create_webview_session(self.webtag, self.options)
    }
}

impl BrowserWebViewBuilder {
    /// Bind a `WebViewDelegate` during creation.
    ///
    /// This is the only supported way to configure delegate callbacks.
    pub fn delegate(mut self, delegate: Arc<dyn WebViewDelegate>) -> Self {
        self.options = self.options.delegate(delegate);
        self
    }

    pub fn on_scheme<F, Fut>(mut self, scheme: &str, handler: F) -> Self
    where
        F: Fn(http::Request<Vec<u8>>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = SchemeOutcome> + Send + 'static,
    {
        self.options = self.options.on_scheme(scheme, handler);
        self
    }

    /// Register a scheme handler which receives native-instance and frame
    /// context alongside the HTTP request.
    pub fn on_contextual_scheme<F, Fut>(mut self, scheme: &str, handler: F) -> Self
    where
        F: Fn(ContextualSchemeRequest) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = SchemeOutcome> + Send + 'static,
    {
        self.options = self.options.on_contextual_scheme(scheme, handler);
        self
    }

    pub fn on_navigation<F>(mut self, handler: F) -> Self
    where
        F: Fn(&NavigationRequest) -> NavigationPolicy + Send + Sync + 'static,
    {
        self.options = self.options.on_navigation(handler);
        self
    }

    pub fn on_new_window<F>(mut self, handler: F) -> Self
    where
        F: Fn(&str) -> NewWindowPolicy + Send + Sync + 'static,
    {
        self.options = self.options.on_new_window(handler);
        self
    }

    /// Register a download callback (browser profile only).
    ///
    /// The callback runs on the platform callback thread; keep it fast and offload
    /// expensive work to your app runtime.
    pub fn on_download<F>(mut self, handler: F) -> Self
    where
        F: Fn(DownloadRequest) + Send + Sync + 'static,
    {
        self.options = self.options.on_download(handler);
        self
    }

    /// Select the website-data lifetime independently of the security profile.
    pub fn data_mode(mut self, data_mode: WebViewDataMode) -> Self {
        self.options = self.options.data_mode(data_mode);
        self
    }

    pub fn on_file_chooser<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(FileChooserRequest) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = FileChooserResponse> + Send + 'static,
    {
        self.options = self.options.on_file_chooser(handler);
        self
    }

    /// Create a browser-profile WebView session.
    ///
    /// Re-creating with the same `webtag` follows strict rules:
    /// - Different options => creation fails.
    /// - Same options but new callback registrations => creation fails.
    /// - Same options and no callbacks => existing instance is reused.
    pub fn create(self) -> WebViewSession {
        create_webview_session(self.webtag, self.options)
    }
}

/// Pending callbacks extracted from internal option normalization.
/// Stored between session creation and `register_webview` installation.
pub(crate) struct PendingCallbacks {
    pub(crate) scheme_handlers: HashMap<String, AsyncSchemeHandler>,
    pub(crate) navigation_handler: Option<NavigationHandler>,
    pub(crate) new_window_handler: Option<NewWindowHandler>,
    pub(crate) download_handler: Option<DownloadHandler>,
    pub(crate) file_chooser_handler: Option<FileChooserHandler>,
    pub(crate) delegate: Option<Arc<dyn WebViewDelegate>>,
}

impl PendingCallbacks {
    fn has_any(&self) -> bool {
        !self.scheme_handlers.is_empty()
            || self.navigation_handler.is_some()
            || self.new_window_handler.is_some()
            || self.download_handler.is_some()
            || self.file_chooser_handler.is_some()
            || self.delegate.is_some()
    }
}

/// WebView type that includes inner implementation and delegate
pub struct WebView {
    pub(crate) inner: WebViewInner,
    native_view_id: NativeWebViewId,
    effective_options: EffectiveWebViewCreateOptions,
    // Hold a strong reference to the delegate; runtime destroy clears it to break cycles.
    delegate: RwLock<Option<Arc<dyn WebViewDelegate>>>,
    // Closure-based scheme handlers registered via builders.
    scheme_handlers: RwLock<HashMap<String, AsyncSchemeHandler>>,
    navigation_handler: RwLock<Option<NavigationHandler>>,
    new_window_handler: RwLock<Option<NewWindowHandler>>,
    download_handler: RwLock<Option<DownloadHandler>>,
    file_chooser_handler: RwLock<Option<FileChooserHandler>>,
    message_ingress: Arc<WebMessageIngress>,
}

/// A one-shot reservation for a trusted native HTML load.
///
/// It is issued for one exact [`WebView`] and can neither be cloned nor moved
/// to another view. Consumers register [`Self::intent`] with their own
/// document state before calling [`Self::load`]. Dropping it, or a failed
/// load, revokes the intent before it can become an admission.
pub struct TrustedDataLoadReservation<'a> {
    webview: &'a WebView,
    webtag: WebTag,
    native_view_id: NativeWebViewId,
    intent: Option<TrustedLoadIntent>,
}

impl TrustedDataLoadReservation<'_> {
    /// The opaque token to register before initiating the native load.
    pub fn intent(&self) -> TrustedLoadIntent {
        self.intent
            .expect("trusted data load reservation must retain its intent until consumed")
    }

    /// Consume this reservation and start its one native HTML load.
    pub fn load(mut self, request: LoadDataRequest<'_>) -> Result<TrustedLoadIntent, WebViewError> {
        let intent = self
            .intent
            .take()
            .expect("trusted data load reservation must be consumed once");
        let key = match self.webview.load_trusted_data_on_platform(request) {
            Ok(key) => key,
            Err(error) => {
                crate::events::normalizer::revoke_trusted_load(
                    &self.webtag,
                    self.native_view_id,
                    intent,
                );
                return Err(error);
            }
        };

        if crate::events::normalizer::attest_trusted_load(
            &self.webtag,
            self.native_view_id,
            intent,
            key,
        ) {
            Ok(intent)
        } else {
            // A destroy/recreate or concurrent later direct load won between
            // issuing and binding. The native load may still render, but it is
            // intentionally ineligible for trusted admission.
            crate::events::normalizer::revoke_trusted_load(
                &self.webtag,
                self.native_view_id,
                intent,
            );
            Err(WebViewError::WebView(
                "trusted data load lost its WebView lifecycle binding".to_string(),
            ))
        }
    }
}

impl Drop for TrustedDataLoadReservation<'_> {
    fn drop(&mut self) {
        if let Some(intent) = self.intent.take() {
            crate::events::normalizer::revoke_trusted_load(
                &self.webtag,
                self.native_view_id,
                intent,
            );
        }
    }
}

fn snapshot_web_message_context(
    native_view_id: NativeWebViewId,
    frame: WebMessageFrame,
    transport: WebMessageTransport,
    source: WebMessageSource,
) -> WebMessageContext {
    // This load is the message callback's document-binding linearization
    // point. A navigation start which wins it produces Unbound; a commit which
    // wins it produces that generation. The queued message retains this value.
    WebMessageContext::new(
        native_view_id,
        crate::events::normalizer::current_document_binding(native_view_id),
        frame,
        transport,
        source,
    )
}

impl WebView {
    pub(crate) fn new(
        inner: WebViewInner,
        effective_options: EffectiveWebViewCreateOptions,
        native_view_id: NativeWebViewId,
    ) -> Self {
        Self {
            inner,
            native_view_id,
            effective_options,
            delegate: RwLock::new(None),
            scheme_handlers: RwLock::new(HashMap::new()),
            navigation_handler: RwLock::new(None),
            new_window_handler: RwLock::new(None),
            download_handler: RwLock::new(None),
            file_chooser_handler: RwLock::new(None),
            message_ingress: Arc::new(WebMessageIngress::default()),
        }
    }

    /// Opaque identity for this concrete native WebView instance.
    ///
    /// It is readable for identity comparison across crates, but cannot be
    /// constructed, serialized, or converted to a raw platform handle by
    /// consumers.
    pub const fn native_view_id(&self) -> NativeWebViewId {
        self.native_view_id
    }

    /// Reserve a trusted native HTML load before starting the native operation.
    ///
    /// Register [`TrustedDataLoadReservation::intent`] with document state
    /// before calling `load`; a native callback is permitted to arrive before
    /// `load` returns. The reservation's Drop implementation revokes an
    /// unused token. HTML and base URLs are resource-location inputs, never
    /// authority.
    pub fn prepare_trusted_data_load(
        &self,
    ) -> Result<TrustedDataLoadReservation<'_>, WebViewError> {
        let webtag = self.webtag();
        let native_view_id = self.native_view_id;
        let intent = crate::events::normalizer::issue_trusted_load(&webtag, native_view_id)
            .ok_or_else(|| {
                WebViewError::WebView(
                    "trusted data load requires a live matching WebView lifecycle".to_string(),
                )
            })?;
        Ok(TrustedDataLoadReservation {
            webview: self,
            webtag,
            native_view_id,
            intent: Some(intent),
        })
    }

    #[cfg(any(target_os = "ios", target_os = "macos"))]
    fn load_trusted_data_on_platform(
        &self,
        request: LoadDataRequest<'_>,
    ) -> Result<crate::events::normalizer::NativeKey, WebViewError> {
        self.inner.load_trusted_data(request)
    }

    #[cfg(not(any(target_os = "ios", target_os = "macos")))]
    fn load_trusted_data_on_platform(
        &self,
        _request: LoadDataRequest<'_>,
    ) -> Result<crate::events::normalizer::NativeKey, WebViewError> {
        Err(WebViewError::Unsupported(
            "trusted direct HTML loads require a platform navigation key".to_string(),
        ))
    }

    /// Enqueue a platform message whose frame proof is known by the adapter.
    ///
    /// The document binding is snapshotted from the normalizer while this
    /// concrete native WebView is current; adapters cannot claim a generation
    /// through this entry point.
    pub(crate) fn enqueue_web_message(
        self: &Arc<Self>,
        body: String,
        frame: WebMessageFrame,
        transport: WebMessageTransport,
        source: WebMessageSource,
    ) {
        let context = snapshot_web_message_context(self.native_view_id, frame, transport, source);
        match self
            .message_ingress
            .enqueue(IncomingWebMessage::new(body, context))
        {
            WebMessageEnqueue::Queued => return,
            WebMessageEnqueue::Closed => {
                log::debug!(
                    "Dropping WebView message after ingress closure ({})",
                    self.webtag()
                );
                return;
            }
            WebMessageEnqueue::Full => {
                log::warn!(
                    "Dropping WebView message because its ingress queue is full ({})",
                    self.webtag()
                );
                return;
            }
            WebMessageEnqueue::Schedule => {}
        }

        if WebMessageExecutor::global()
            .schedule(WebMessageJob {
                ingress: Arc::clone(&self.message_ingress),
                webview: Arc::downgrade(self),
            })
            .is_err()
        {
            // A fixed worker unexpectedly exiting must not leave this ingress
            // scheduled forever, nor let later callbacks restart its delivery.
            self.message_ingress.close();
            log::error!(
                "Dropping WebView message queue because the fixed executor stopped ({})",
                self.webtag()
            );
        }
    }

    fn close_message_ingress(&self) {
        self.message_ingress.close();
    }

    /// Read the document binding currently owned by this native WebView.
    ///
    /// This is intentionally read-only. Only accepted top-level navigation
    /// starts and reliable commits in the event normalizer can change it.
    pub fn current_document_binding(&self) -> crate::DocumentBinding {
        crate::events::normalizer::current_document_binding(self.native_view_id)
    }

    /// Get the appid
    pub fn appid(&self) -> String {
        self.inner.webtag.extract_appid()
    }

    /// Get the path
    pub fn path(&self) -> String {
        self.inner.webtag.extract_parts().1
    }

    /// Get the webtag (computed from appid and path)
    pub fn webtag(&self) -> WebTag {
        self.inner.webtag.clone()
    }

    pub(crate) fn effective_options(&self) -> &EffectiveWebViewCreateOptions {
        &self.effective_options
    }

    /// Get delegate for this WebView
    pub(crate) fn get_delegate(&self) -> Option<Arc<dyn WebViewDelegate>> {
        self.delegate.read().ok().and_then(|guard| guard.clone())
    }

    /// Remove delegate for this WebView
    pub(crate) fn remove_delegate(&self) {
        if let Ok(mut guard) = self.delegate.write() {
            *guard = None;
        }
    }

    /// Install all pending callbacks into this WebView (called once during creation).
    pub(crate) fn install_callbacks(&self, callbacks: PendingCallbacks) {
        if let Some(delegate) = callbacks.delegate
            && let Ok(mut guard) = self.delegate.write()
        {
            *guard = Some(delegate);
        }
        if let Ok(mut guard) = self.scheme_handlers.write() {
            *guard = callbacks.scheme_handlers;
        }
        if let Some(handler) = callbacks.navigation_handler
            && let Ok(mut guard) = self.navigation_handler.write()
        {
            *guard = Some(handler);
        }
        if let Some(handler) = callbacks.new_window_handler
            && let Ok(mut guard) = self.new_window_handler.write()
        {
            *guard = Some(handler);
        }
        if let Some(handler) = callbacks.download_handler
            && let Ok(mut guard) = self.download_handler.write()
        {
            *guard = Some(handler);
        }
        if let Some(handler) = callbacks.file_chooser_handler
            && let Ok(mut guard) = self.file_chooser_handler.write()
        {
            *guard = Some(handler);
        }
    }

    /// Check if a scheme handler is registered for the given scheme.
    pub fn has_scheme_handler(&self, scheme: &str) -> bool {
        self.scheme_handlers
            .read()
            .ok()
            .is_some_and(|guard| guard.contains_key(scheme))
    }

    /// Synchronously invoke the registered scheme handler for `scheme`.
    /// Returns `None` if no handler is registered or the handler declines.
    ///
    /// Compatibility ingress for out-of-tree / older adapters. It preserves
    /// the exact owning native instance but has no frame proof, so it always
    /// invokes contextual handlers with [`SchemeRequestFrame::Unproven`].
    #[allow(dead_code)]
    pub(crate) fn handle_scheme_request(
        &self,
        scheme: &str,
        request: http::Request<Vec<u8>>,
    ) -> Option<WebResourceResponse> {
        self.handle_contextual_scheme_request(
            scheme,
            ContextualSchemeRequest::new(
                request,
                self.native_view_id(),
                SchemeRequestFrame::Unproven,
            ),
        )
    }

    /// Invoke a registered scheme handler with adapter-attested callback
    /// context. Platform code must validate its callback identity before
    /// constructing the request.
    pub(crate) fn handle_contextual_scheme_request(
        &self,
        scheme: &str,
        request: ContextualSchemeRequest,
    ) -> Option<WebResourceResponse> {
        #[cfg(any(target_os = "ios", target_os = "macos"))]
        if let Some(response) = self.inner.handle_internal_bridge_request(request.request()) {
            return Some(response);
        }

        let guard = self.scheme_handlers.read().ok()?;
        let handler = guard.get(scheme)?;
        let outcome = block_on_scheme_future(handler(request));
        match outcome {
            SchemeOutcome::Handled(response) => Some(response),
            SchemeOutcome::PassThrough => None,
        }
    }

    /// Call the navigation handler. Returns `Allow` if no handler is registered.
    ///
    /// A URL matching an open [`crate::url_callback`] channel is delivered to
    /// that channel and cancelled before any per-webview handler runs.
    pub fn handle_navigation(&self, request: &NavigationRequest) -> NavigationPolicy {
        if crate::url_callback::dispatch(&request.url) {
            return NavigationPolicy::Cancel;
        }
        if let Ok(guard) = self.navigation_handler.read()
            && let Some(handler) = guard.as_ref()
        {
            return handler(request);
        }
        NavigationPolicy::Allow
    }

    /// Check if a new-window handler is registered.
    pub fn has_new_window_handler(&self) -> bool {
        self.new_window_handler
            .read()
            .ok()
            .is_some_and(|guard| guard.is_some())
    }

    /// Call the new-window handler. Returns `Cancel` if no handler is registered.
    ///
    /// A URL matching an open [`crate::url_callback`] channel is delivered to
    /// that channel and cancelled before any per-webview handler runs.
    pub fn handle_new_window(&self, url: &str) -> NewWindowPolicy {
        if crate::url_callback::dispatch(url) {
            return NewWindowPolicy::Cancel;
        }
        if let Ok(guard) = self.new_window_handler.read()
            && let Some(handler) = guard.as_ref()
        {
            return handler(url);
        }
        NewWindowPolicy::Cancel
    }

    /// Dispatch a download request to the registered handler.
    pub(crate) fn handle_download(&self, request: DownloadRequest) {
        if let Ok(guard) = self.download_handler.read()
            && let Some(handler) = guard.as_ref()
        {
            handler(request);
        }
    }

    // Consulted only by the Windows download-event path.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub(crate) fn has_download_handler(&self) -> bool {
        self.download_handler
            .read()
            .ok()
            .is_some_and(|guard| guard.is_some())
    }

    #[cfg_attr(target_os = "windows", allow(dead_code))]
    pub(crate) fn handle_file_chooser<C>(&self, request: FileChooserRequest, completion: C) -> bool
    where
        C: FnOnce(FileChooserResponse) + Send + 'static,
    {
        let Some(future) = self.make_file_chooser_future(request) else {
            return false;
        };
        std::thread::spawn(move || {
            completion(block_on_scheme_future(future));
        });
        true
    }

    #[cfg_attr(target_os = "windows", allow(dead_code))]
    fn make_file_chooser_future(&self, request: FileChooserRequest) -> Option<FileChooserFuture> {
        let Ok(guard) = self.file_chooser_handler.read() else {
            return None;
        };
        let handler = guard.as_ref()?;
        Some(handler(request))
    }

    /// Toggle docked DevTools (macOS only, uses private _inspector API)
    #[cfg(target_os = "macos")]
    pub fn toggle_devtools(&self) {
        self.inner.toggle_devtools();
    }

    /// Toggle detached DevTools (macOS only, uses private _inspector API)
    #[cfg(target_os = "macos")]
    pub fn toggle_devtools_detached(&self) {
        self.inner.toggle_devtools_detached();
    }

    /// Get platform-specific pointer for interop (Apple platforms only)
    #[cfg(any(target_os = "ios", target_os = "macos"))]
    pub fn get_swift_webview_ptr(&self) -> usize {
        self.inner.get_swift_webview_ptr()
    }

    /// Get Java WebView reference (Android only)
    #[cfg(target_os = "android")]
    pub fn get_java_webview(&self) -> &jni::objects::Global<jni::objects::JObject<'static>> {
        self.inner.get_java_webview()
    }

    pub async fn evaluate_javascript(
        &self,
        js: &str,
    ) -> Result<serde_json::Value, crate::WebViewScriptError> {
        self.inner.eval_js(js).await
    }

    /// Synthetic-event click for platforms that don't expose a native touch
    /// injection API (iOS WKWebView, ArkWeb on Harmony). Looks up the
    /// selector, scrolls it into view, and dispatches a synthetic
    /// `MouseEvent` (or sets `focus="true"` for `<lx-*>` custom elements
    /// that proxy focus to a native overlay).
    /// Run a page-input action script and decode its `{ok, error, interactable}`
    /// result.
    #[cfg(any(
        target_os = "ios",
        target_os = "android",
        all(feature = "webview-input", target_os = "macos"),
        all(target_os = "linux", target_env = "ohos")
    ))]
    async fn run_js_action(&self, script: &str) -> Result<(), WebViewInputError> {
        let result = self
            .inner
            .eval_js(script)
            .await
            .map_err(WebViewInputError::Script)?;
        if result.get("ok").and_then(|v| v.as_bool()) == Some(true) {
            return Ok(());
        }
        let err_msg = result
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("input action failed")
            .to_string();
        if result.get("interactable").and_then(|v| v.as_bool()) == Some(false) {
            Err(WebViewInputError::ElementNotInteractable(err_msg))
        } else {
            Err(WebViewInputError::ElementNotFound(err_msg))
        }
    }

    /// Click an element by synthesizing DOM events. The shared input mechanism
    /// for platforms/hosts where native event dispatch cannot reach the page:
    /// iOS (no `UITouch` synthesis), OpenHarmony, and macOS when the WebView is
    /// detached (AppUI renders pages off-surface). `lx-` custom elements proxy
    /// focus to their native overlay instead of receiving mouse events.
    #[cfg(any(
        target_os = "ios",
        all(feature = "webview-input", target_os = "macos"),
        all(target_os = "linux", target_env = "ohos")
    ))]
    pub(crate) async fn click_via_js(
        &self,
        selector: &str,
        index: Option<usize>,
    ) -> Result<(), WebViewInputError> {
        let selector_json = serde_json::to_string(selector)
            .map_err(|err| WebViewInputError::Platform(format!("Invalid selector: {err}")))?;
        let idx = index.unwrap_or(0);
        let script = format!(
            "((sel, i) => {{ \
              const els = document.querySelectorAll(sel); \
              if (!els.length || i < 0 || i >= els.length) return {{ ok:false, error:'no match', count:els.length }}; \
              const el = els[i]; \
              try {{ el.scrollIntoView({{block:'center', inline:'center'}}); }} catch(_e) {{}} \
              const rect = el.getBoundingClientRect(); \
              const style = window.getComputedStyle(el); \
              const disabled = !!el.disabled || el.getAttribute('aria-disabled') === 'true'; \
              const visible = rect.width > 0 && rect.height > 0 && rect.bottom > 0 && rect.right > 0 && \
                rect.top < window.innerHeight && rect.left < window.innerWidth && \
                style.visibility !== 'hidden' && style.display !== 'none' && Number(style.opacity || '1') !== 0; \
              if (!visible) return {{ ok:false, error:'not visible', interactable:false, count:els.length }}; \
              if (disabled) return {{ ok:false, error:'not enabled', interactable:false, count:els.length }}; \
              const tag = (el.tagName || '').toLowerCase(); \
              if (tag.indexOf('lx-') === 0) {{ \
                el.setAttribute('focus', 'true'); \
                if (typeof el.syncNativeProps === 'function') {{ try {{ el.syncNativeProps(); }} catch(_e) {{}} }} \
                return {{ ok:true, count:els.length, native:true }}; \
              }} \
              if (typeof el.focus === 'function') {{ try {{ el.focus({{preventScroll:true}}); }} catch(_e) {{ try {{ el.focus(); }} catch(__){{}} }} }} \
              const opts = {{ bubbles:true, cancelable:true, view:window, clientX: rect.left + rect.width/2, clientY: rect.top + rect.height/2 }}; \
              try {{ if (window.PointerEvent) el.dispatchEvent(new PointerEvent('pointerdown', Object.assign({{pointerId:1, isPrimary:true, pointerType:'mouse'}}, opts))); }} catch(_e) {{}} \
              try {{ el.dispatchEvent(new MouseEvent('mousedown', opts)); }} catch(_e) {{}} \
              try {{ if (window.PointerEvent) el.dispatchEvent(new PointerEvent('pointerup', Object.assign({{pointerId:1, isPrimary:true, pointerType:'mouse'}}, opts))); }} catch(_e) {{}} \
              try {{ el.dispatchEvent(new MouseEvent('mouseup', opts)); }} catch(_e) {{}} \
              try {{ el.dispatchEvent(new MouseEvent('click', opts)); }} catch(_e) {{}} \
              return {{ ok:true, count:els.length }}; \
            }})({selector_json}, {idx})"
        );
        self.run_js_action(&script).await
    }

    /// Type text into an editable element by synthesizing DOM events. Goes
    /// through the native value setter so framework-tracked inputs (React) fire
    /// their `onChange`. `lx-` custom elements set their value + sync native.
    #[cfg(any(
        target_os = "ios",
        target_os = "android",
        all(feature = "webview-input", target_os = "macos"),
        all(target_os = "linux", target_env = "ohos")
    ))]
    pub(crate) async fn type_via_js(
        &self,
        selector: &str,
        index: Option<usize>,
        text: &str,
        replace: bool,
    ) -> Result<(), WebViewInputError> {
        let selector_json = serde_json::to_string(selector)
            .map_err(|err| WebViewInputError::Platform(format!("Invalid selector: {err}")))?;
        let text_json = serde_json::to_string(text)
            .map_err(|err| WebViewInputError::Platform(format!("Invalid text: {err}")))?;
        let idx = index.unwrap_or(0);
        let script = format!(
            "((sel, i, text, replace) => {{ \
              const els = document.querySelectorAll(sel); \
              if (!els.length || i < 0 || i >= els.length) return {{ ok:false, error:'no match', count:els.length }}; \
              const el = els[i]; \
              try {{ el.scrollIntoView({{block:'center', inline:'center'}}); }} catch(_e) {{}} \
              if (typeof el.focus === 'function') {{ try {{ el.focus({{preventScroll:true}}); }} catch(_e) {{ try {{ el.focus(); }} catch(__){{}} }} }} \
              const tag = (el.tagName || '').toLowerCase(); \
              if (tag === 'input' || tag === 'textarea') {{ \
                const proto = tag === 'textarea' ? window.HTMLTextAreaElement.prototype : window.HTMLInputElement.prototype; \
                const desc = Object.getOwnPropertyDescriptor(proto, 'value'); \
                const next = (replace ? '' : (el.value || '')) + text; \
                if (desc && desc.set) {{ desc.set.call(el, next); }} else {{ el.value = next; }} \
                el.dispatchEvent(new InputEvent('input', {{ bubbles:true, cancelable:true, data:text, inputType:'insertText' }})); \
                el.dispatchEvent(new Event('change', {{ bubbles:true }})); \
                return {{ ok:true, count:els.length }}; \
              }} \
              if (el.isContentEditable) {{ \
                el.textContent = (replace ? '' : (el.textContent || '')) + text; \
                el.dispatchEvent(new InputEvent('input', {{ bubbles:true, data:text, inputType:'insertText' }})); \
                return {{ ok:true, count:els.length }}; \
              }} \
              if (tag.indexOf('lx-') === 0) {{ \
                try {{ el.value = (replace ? '' : (el.value || '')) + text; }} catch(_e) {{}} \
                if (typeof el.syncNativeProps === 'function') {{ try {{ el.syncNativeProps(); }} catch(_e) {{}} }} \
                el.dispatchEvent(new Event('input', {{ bubbles:true }})); \
                return {{ ok:true, count:els.length, native:true }}; \
              }} \
              return {{ ok:false, error:'not editable', interactable:false, count:els.length }}; \
            }})({selector_json}, {idx}, {text_json}, {replace})"
        );
        self.run_js_action(&script).await
    }

    /// Press a key by synthesizing keydown/keyup on the selected or focused element.
    #[cfg(any(
        target_os = "ios",
        target_os = "android",
        all(feature = "webview-input", target_os = "macos"),
        all(target_os = "linux", target_env = "ohos")
    ))]
    pub(crate) async fn press_via_js(
        &self,
        key: &str,
        selector: Option<&str>,
        index: Option<usize>,
    ) -> Result<(), WebViewInputError> {
        let key_json = serde_json::to_string(key)
            .map_err(|err| WebViewInputError::Platform(format!("Invalid key: {err}")))?;
        let selector_json = serde_json::to_string(&selector)
            .map_err(|err| WebViewInputError::Platform(format!("Invalid selector: {err}")))?;
        let idx = index.unwrap_or(0);
        let script = format!(
            "((key, sel, i) => {{ \
              const els = sel === null ? null : document.querySelectorAll(sel); \
              if (els && (!els.length || i < 0 || i >= els.length)) return {{ ok:false, error:'no match', count:els.length }}; \
              const el = els ? els[i] : (document.activeElement || document.body); \
              if (els) {{ \
                try {{ el.scrollIntoView({{block:'center', inline:'center'}}); }} catch(_e) {{}} \
                if (typeof el.focus === 'function') {{ try {{ el.focus({{preventScroll:true}}); }} catch(_e) {{ try {{ el.focus(); }} catch(__){{}} }} }} \
              }} \
              const map = {{ enter:'Enter', 'return':'Enter', tab:'Tab', esc:'Escape', escape:'Escape', backspace:'Backspace', 'delete':'Delete', forwarddelete:'Delete', space:' ', up:'ArrowUp', down:'ArrowDown', left:'ArrowLeft', right:'ArrowRight', arrowup:'ArrowUp', arrowdown:'ArrowDown', arrowleft:'ArrowLeft', arrowright:'ArrowRight', home:'Home', end:'End', pageup:'PageUp', pagedown:'PageDown' }}; \
              const norm = String(key).toLowerCase(); \
              const k = map[norm] || key; \
              const opts = {{ bubbles:true, cancelable:true, composed:true, key:k, view:window }}; \
              el.dispatchEvent(new KeyboardEvent('keydown', opts)); \
              el.dispatchEvent(new KeyboardEvent('keyup', opts)); \
              return {{ ok:true }}; \
            }})({key_json}, {selector_json}, {idx})"
        );
        self.run_js_action(&script).await
    }

    /// Scroll by `(dx, dy)` in the DOM. Walks up from the element at the given
    /// viewport point (default: center) to the nearest scrollable ancestor, so
    /// it scrolls internal scroll containers, not just the document. When a
    /// webview reports `innerWidth/Height` as 0, the center point is unusable,
    /// so it falls back to the largest scrollable element, then the document
    /// scroller. Uses direct `scrollTop`/`scrollLeft` assignment, not
    /// `scrollBy`: on iOS WKWebView `scrollBy` animates sub-scrollers and
    /// overshoots to 2x the delta. NB: the built script must contain no `//`
    /// line comments — the `\`-continued format string collapses to one line.
    #[cfg(any(
        target_os = "ios",
        all(feature = "webview-input", target_os = "macos"),
        all(target_os = "linux", target_env = "ohos")
    ))]
    pub(crate) async fn scroll_via_js(
        &self,
        at: Option<(f64, f64)>,
        dx: f64,
        dy: f64,
    ) -> Result<(), WebViewInputError> {
        let (px, py) = at.unwrap_or((-1.0, -1.0));
        let script = format!(
            "((px, py, dx, dy) => {{ \
              const overflows = (v) => (/(auto|scroll|overlay)/).test(v); \
              const ancestor = (node) => {{ \
                while (node && node !== document.body && node !== document.documentElement) {{ \
                  const s = window.getComputedStyle(node); \
                  if ((overflows(s.overflowY) && node.scrollHeight > node.clientHeight) || \
                      (overflows(s.overflowX) && node.scrollWidth > node.clientWidth)) return node; \
                  node = node.parentElement; \
                }} \
                return null; \
              }}; \
              const largest = () => {{ \
                let best = null, range = 0; \
                const all = document.querySelectorAll('*'); \
                for (let k = 0; k < all.length; k++) {{ \
                  const n = all[k], s = window.getComputedStyle(n); \
                  const ry = overflows(s.overflowY) ? (n.scrollHeight - n.clientHeight) : 0; \
                  const rx = overflows(s.overflowX) ? (n.scrollWidth - n.clientWidth) : 0; \
                  const r = ry > rx ? ry : rx; \
                  if (r > range) {{ range = r; best = n; }} \
                }} \
                return best; \
              }}; \
              const vw = window.innerWidth || document.documentElement.clientWidth || 0; \
              const vh = window.innerHeight || document.documentElement.clientHeight || 0; \
              let target = null; \
              if (px >= 0 && py >= 0) target = ancestor(document.elementFromPoint(px, py) || document.body); \
              else if (vw > 0 && vh > 0) target = ancestor(document.elementFromPoint(vw >> 1, vh >> 1) || document.body); \
              if (!target) {{ \
                const se = document.scrollingElement || document.documentElement; \
                target = (se && se.scrollHeight > se.clientHeight) ? se : (largest() || se); \
              }} \
              target.scrollLeft += dx; target.scrollTop += dy; \
              return {{ ok:true }}; \
            }})({px}, {py}, {dx}, {dy})"
        );
        self.run_js_action(&script).await
    }

    /// Scroll an element into view (`scrollIntoView`).
    #[cfg(any(
        target_os = "ios",
        all(feature = "webview-input", target_os = "macos"),
        all(target_os = "linux", target_env = "ohos")
    ))]
    pub(crate) async fn scroll_to_via_js(
        &self,
        selector: &str,
        index: Option<usize>,
    ) -> Result<(), WebViewInputError> {
        let selector_json = serde_json::to_string(selector)
            .map_err(|err| WebViewInputError::Platform(format!("Invalid selector: {err}")))?;
        let idx = index.unwrap_or(0);
        let script = format!(
            "((sel, i) => {{ \
              const els = document.querySelectorAll(sel); \
              if (!els.length || i < 0 || i >= els.length) return {{ ok:false, error:'no match', count:els.length }}; \
              try {{ els[i].scrollIntoView({{ block:'center', inline:'center' }}); }} catch(_e) {{ els[i].scrollIntoView(); }} \
              return {{ ok:true, count:els.length }}; \
            }})({selector_json}, {idx})"
        );
        self.run_js_action(&script).await
    }

    pub async fn current_url(&self) -> Result<Option<String>, WebViewError> {
        self.inner.current_url().await
    }

    pub fn reload(&self) -> Result<(), WebViewError> {
        self.inner.reload()
    }

    pub fn go_back(&self) -> Result<(), WebViewError> {
        self.inner.go_back()
    }

    pub fn go_forward(&self) -> Result<(), WebViewError> {
        self.inner.go_forward()
    }

    pub async fn list_cookies(&self) -> Result<Vec<WebViewCookie>, WebViewError> {
        self.inner.list_cookies().await
    }

    pub async fn set_cookie(&self, request: WebViewCookieSetRequest) -> Result<(), WebViewError> {
        self.inner.set_cookie(request).await
    }

    pub async fn delete_cookie(
        &self,
        name: &str,
        domain: &str,
        path: &str,
    ) -> Result<(), WebViewError> {
        self.inner.delete_cookie(name, domain, path).await
    }

    pub async fn clear_cookies(&self) -> Result<(), WebViewError> {
        self.inner.clear_cookies().await
    }

    pub async fn start_network_capture(&self) -> Result<(), WebViewError> {
        self.inner.start_network_capture().await
    }

    pub async fn stop_network_capture(&self) -> Result<(), WebViewError> {
        self.inner.stop_network_capture().await
    }

    pub async fn network_entries(&self) -> Result<NetworkCaptureSnapshot, WebViewError> {
        self.inner.network_entries().await
    }

    pub async fn clear_network_capture(&self) -> Result<(), WebViewError> {
        self.inner.clear_network_capture().await
    }

    pub async fn take_screenshot(&self) -> Result<Vec<u8>, WebViewError> {
        self.inner.take_screenshot().await
    }

    pub async fn click(
        &self,
        selector: &str,
        options: ClickOptions,
    ) -> Result<(), WebViewInputError> {
        <Self as WebViewInputController>::click(self, selector, options).await
    }

    pub async fn type_text(
        &self,
        selector: &str,
        text: &str,
        options: TypeOptions,
    ) -> Result<(), WebViewInputError> {
        <Self as WebViewInputController>::type_text(self, selector, text, options).await
    }

    pub async fn fill(
        &self,
        selector: &str,
        text: &str,
        options: FillOptions,
    ) -> Result<(), WebViewInputError> {
        <Self as WebViewInputController>::fill(self, selector, text, options).await
    }

    pub async fn press(&self, key: &str, options: PressOptions) -> Result<(), WebViewInputError> {
        <Self as WebViewInputController>::press(self, key, options).await
    }

    pub async fn scroll(
        &self,
        dx: f64,
        dy: f64,
        options: ScrollOptions,
    ) -> Result<(), WebViewInputError> {
        <Self as WebViewInputController>::scroll(self, dx, dy, options).await
    }

    pub async fn scroll_to(
        &self,
        selector: &str,
        options: ScrollOptions,
    ) -> Result<(), WebViewInputError> {
        <Self as WebViewInputController>::scroll_to(self, selector, options).await
    }
}

#[async_trait]
impl WebViewController for WebView {
    fn load_url(&self, url: &str) -> Result<(), WebViewError> {
        self.inner.load_url(url)
    }

    fn load_data(&self, request: LoadDataRequest<'_>) -> Result<(), WebViewError> {
        self.inner.load_data(request)
    }

    fn exec_js(&self, js: &str) -> Result<(), WebViewError> {
        self.inner.exec_js(js)
    }

    async fn eval_js(&self, js: &str) -> Result<serde_json::Value, WebViewScriptError> {
        self.inner.eval_js(js).await
    }

    async fn current_url(&self) -> Result<Option<String>, WebViewError> {
        self.inner.current_url().await
    }

    fn post_message(&self, message: &str) -> Result<(), WebViewError> {
        self.inner.post_message(message)
    }

    fn clear_browsing_data(&self) -> Result<(), WebViewError> {
        self.inner.clear_browsing_data()
    }

    fn set_user_agent_override(&self, user_agent: UserAgentOverride) -> Result<(), WebViewError> {
        user_agent.validate()?;
        self.inner.set_user_agent_override(user_agent)
    }

    fn reload(&self) -> Result<(), WebViewError> {
        self.inner.reload()
    }

    fn go_back(&self) -> Result<(), WebViewError> {
        self.inner.go_back()
    }

    fn go_forward(&self) -> Result<(), WebViewError> {
        self.inner.go_forward()
    }

    async fn list_cookies(&self) -> Result<Vec<WebViewCookie>, WebViewError> {
        self.inner.list_cookies().await
    }

    async fn set_cookie(&self, request: WebViewCookieSetRequest) -> Result<(), WebViewError> {
        self.inner.set_cookie(request).await
    }

    async fn delete_cookie(
        &self,
        name: &str,
        domain: &str,
        path: &str,
    ) -> Result<(), WebViewError> {
        self.inner.delete_cookie(name, domain, path).await
    }

    async fn clear_cookies(&self) -> Result<(), WebViewError> {
        self.inner.clear_cookies().await
    }

    async fn clear_site_data(
        &self,
        url: &str,
        options: ClearSiteDataOptions,
    ) -> Result<ClearSiteDataResult, WebViewError> {
        self.inner.clear_site_data(url, options).await
    }

    // Callers reach this through the inherent method today, but the trait
    // impl must stay exhaustive: a missed forward silently resolves to the
    // trait's Err default for dyn/generic dispatch (how clear_site_data
    // shipped broken).
    async fn take_screenshot(&self) -> Result<Vec<u8>, WebViewError> {
        self.inner.take_screenshot().await
    }

    async fn start_network_capture(&self) -> Result<(), WebViewError> {
        self.inner.start_network_capture().await
    }

    async fn stop_network_capture(&self) -> Result<(), WebViewError> {
        self.inner.stop_network_capture().await
    }

    async fn network_entries(&self) -> Result<NetworkCaptureSnapshot, WebViewError> {
        self.inner.network_entries().await
    }

    async fn clear_network_capture(&self) -> Result<(), WebViewError> {
        self.inner.clear_network_capture().await
    }
}

#[async_trait]
impl WebViewInputController for WebView {
    async fn click(
        &self,
        _selector: &str,
        _options: ClickOptions,
    ) -> Result<(), WebViewInputError> {
        // macOS uses DOM synthesis for selector clicks: AppKit does not expose
        // a reliable permission-free way to update WKWebView hit testing from
        // an in-process NSEvent. Text and key input still use native WebKit
        // editing paths below. iOS/OpenHarmony likewise have no native touch
        // synthesis.
        #[cfg(all(feature = "webview-input", target_os = "macos"))]
        {
            return self.click_via_js(_selector, _options.index).await;
        }
        #[cfg(all(feature = "webview-input", target_os = "windows"))]
        {
            return self.inner.click_inner(_selector, _options).await;
        }
        #[cfg(target_os = "android")]
        {
            return self.inner.click_inner(_selector, _options).await;
        }
        #[cfg(any(target_os = "ios", all(target_os = "linux", target_env = "ohos")))]
        {
            return self.click_via_js(_selector, _options.index).await;
        }
        #[allow(unreachable_code)]
        Err(WebViewInputError::Unsupported(
            "input control is not implemented for this platform",
        ))
    }

    async fn type_text(
        &self,
        _selector: &str,
        _text: &str,
        _options: TypeOptions,
    ) -> Result<(), WebViewInputError> {
        #[cfg(all(feature = "webview-input", target_os = "macos"))]
        {
            if self.inner.is_window_attached().await {
                return self.inner.type_text_inner(_selector, _text, _options).await;
            }
            return self
                .type_via_js(_selector, _options.index, _text, _options.replace)
                .await;
        }
        #[cfg(all(feature = "webview-input", target_os = "windows"))]
        {
            return self.inner.type_text_inner(_selector, _text, _options).await;
        }
        #[cfg(any(
            target_os = "ios",
            target_os = "android",
            all(target_os = "linux", target_env = "ohos")
        ))]
        {
            return self
                .type_via_js(_selector, _options.index, _text, _options.replace)
                .await;
        }
        #[allow(unreachable_code)]
        Err(WebViewInputError::Unsupported(
            "input control is not implemented for this platform",
        ))
    }

    async fn fill(
        &self,
        _selector: &str,
        _text: &str,
        _options: FillOptions,
    ) -> Result<(), WebViewInputError> {
        #[cfg(all(feature = "webview-input", target_os = "macos"))]
        {
            // `fill` is a framework-aware replacement operation. WebKit's
            // native InsertText command can report success before a controlled
            // React/Vue input observes the edit, leaving dependent controls in
            // their old state. `type` retains the native keyboard path.
            return self
                .type_via_js(_selector, _options.index, _text, true)
                .await;
        }
        #[cfg(all(feature = "webview-input", target_os = "windows"))]
        {
            return self
                .inner
                .type_text_inner(
                    _selector,
                    _text,
                    TypeOptions {
                        index: _options.index,
                        replace: true,
                    },
                )
                .await;
        }
        #[cfg(any(
            target_os = "ios",
            target_os = "android",
            all(target_os = "linux", target_env = "ohos")
        ))]
        {
            return self
                .type_via_js(_selector, _options.index, _text, true)
                .await;
        }
        #[allow(unreachable_code)]
        Err(WebViewInputError::Unsupported(
            "input control is not implemented for this platform",
        ))
    }

    async fn press(&self, _key: &str, _options: PressOptions) -> Result<(), WebViewInputError> {
        if _options.index.is_some() && _options.selector.is_none() {
            return Err(WebViewInputError::Platform(
                "press index requires a selector".to_string(),
            ));
        }
        #[cfg(all(feature = "webview-input", target_os = "macos"))]
        {
            if self.inner.is_window_attached().await {
                return self.inner.press_inner(_key, _options).await;
            }
            return self
                .press_via_js(_key, _options.selector.as_deref(), _options.index)
                .await;
        }
        #[cfg(all(feature = "webview-input", target_os = "windows"))]
        {
            return self.inner.press_inner(_key, _options).await;
        }
        #[cfg(any(
            target_os = "ios",
            target_os = "android",
            all(target_os = "linux", target_env = "ohos")
        ))]
        {
            return self
                .press_via_js(_key, _options.selector.as_deref(), _options.index)
                .await;
        }
        #[allow(unreachable_code)]
        Err(WebViewInputError::Unsupported(
            "input control is not implemented for this platform",
        ))
    }

    async fn scroll(
        &self,
        _dx: f64,
        _dy: f64,
        _options: ScrollOptions,
    ) -> Result<(), WebViewInputError> {
        // AppUI renders lxapp pages as native surfaces with the WKWebView
        // detached, so native scroll wheel events can't reach the DOM — use JS.
        #[cfg(all(feature = "webview-input", target_os = "macos"))]
        {
            if self.inner.is_window_attached().await {
                return self.inner.scroll_inner(_dx, _dy, _options).await;
            }
            return self.scroll_via_js(None, _dx, _dy).await;
        }
        #[cfg(all(feature = "webview-input", target_os = "windows"))]
        {
            return self.inner.scroll_inner(_dx, _dy, _options).await;
        }
        // Android scrolls page content in the native View layer (the DOM
        // document has no scroll extent), so drive WebView.scrollBy natively.
        #[cfg(target_os = "android")]
        {
            return self.inner.scroll_inner(_dx, _dy, _options).await;
        }
        // iOS has no native scroll synthesis; Harmony webview is always detached.
        #[cfg(any(target_os = "ios", all(target_os = "linux", target_env = "ohos")))]
        {
            return self.scroll_via_js(None, _dx, _dy).await;
        }
        #[allow(unreachable_code)]
        Err(WebViewInputError::Unsupported(
            "input control is not implemented for this platform",
        ))
    }

    async fn scroll_to(
        &self,
        _selector: &str,
        _options: ScrollOptions,
    ) -> Result<(), WebViewInputError> {
        #[cfg(all(feature = "webview-input", target_os = "macos"))]
        {
            if self.inner.is_window_attached().await {
                return self.inner.scroll_to_inner(_selector, _options).await;
            }
            return self.scroll_to_via_js(_selector, None).await;
        }
        #[cfg(all(feature = "webview-input", target_os = "windows"))]
        {
            return self.inner.scroll_to_inner(_selector, _options).await;
        }
        #[cfg(target_os = "android")]
        {
            return self.inner.scroll_to_inner(_selector, _options).await;
        }
        #[cfg(any(target_os = "ios", all(target_os = "linux", target_env = "ohos")))]
        {
            return self.scroll_to_via_js(_selector, None).await;
        }
        #[allow(unreachable_code)]
        Err(WebViewInputError::Unsupported(
            "input control is not implemented for this platform",
        ))
    }
}

/// Type alias for WebView instances storage to reduce complexity
type WebViewInstancesMap = Arc<Mutex<HashMap<String, Arc<WebView>>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebViewCreateStage {
    Requested,
    NativeCreated,
    ControllerAttached,
    Ready,
    Destroyed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebViewEvent {
    Stage(WebViewCreateStage),
    Failed {
        stage: WebViewCreateStage,
        error: WebViewError,
    },
}

type WebViewReadyState = Option<Result<Arc<WebView>, WebViewError>>;

#[derive(Clone)]
pub struct WebViewEventSubscription {
    rx: watch::Receiver<WebViewEvent>,
}

impl WebViewEventSubscription {
    pub fn current(&self) -> WebViewEvent {
        self.rx.borrow().clone()
    }

    pub async fn changed(&mut self) -> Result<WebViewEvent, WebViewError> {
        self.rx.changed().await.map_err(|_| {
            WebViewError::WebView("webview event channel unexpectedly closed".to_string())
        })?;
        Ok(self.current())
    }
}

#[derive(Clone)]
pub struct WebViewSession {
    webtag: WebTag,
    event_rx: watch::Receiver<WebViewEvent>,
    ready_rx: watch::Receiver<WebViewReadyState>,
    signals: Arc<WebViewSessionSignals>,
}

impl WebViewSession {
    pub fn webtag(&self) -> &WebTag {
        &self.webtag
    }

    pub fn subscribe_events(&self) -> WebViewEventSubscription {
        WebViewEventSubscription {
            rx: self.event_rx.clone(),
        }
    }

    pub fn current_event(&self) -> WebViewEvent {
        self.event_rx.borrow().clone()
    }

    pub async fn wait_ready(&self) -> Result<Arc<WebView>, WebViewError> {
        let mut rx = self.ready_rx.clone();
        loop {
            if let Some(result) = self.signals.terminal_result() {
                return result;
            }
            if let Some(result) = rx.borrow().clone() {
                return result;
            }
            if rx.changed().await.is_err() {
                if let Some(result) = self.signals.terminal_result() {
                    return result;
                }
                return Err(WebViewError::WebView(
                    "webview ready channel unexpectedly closed".to_string(),
                ));
            }
        }
    }
}

struct WebViewSessionSignals {
    event_tx: watch::Sender<WebViewEvent>,
    ready_tx: watch::Sender<WebViewReadyState>,
    state: Mutex<WebViewSessionState>,
}

#[derive(Default)]
struct WebViewSessionState {
    terminal_result: Option<Result<Arc<WebView>, WebViewError>>,
    destroyed: bool,
}

impl WebViewSessionSignals {
    fn new() -> Arc<Self> {
        let (event_tx, _event_rx) =
            watch::channel(WebViewEvent::Stage(WebViewCreateStage::Requested));
        let (ready_tx, _ready_rx) = watch::channel(None);
        Arc::new(Self {
            event_tx,
            ready_tx,
            state: Mutex::new(WebViewSessionState::default()),
        })
    }

    fn subscribe(self: &Arc<Self>, webtag: WebTag) -> WebViewSession {
        WebViewSession {
            webtag,
            event_rx: self.event_tx.subscribe(),
            ready_rx: self.ready_tx.subscribe(),
            signals: Arc::clone(self),
        }
    }

    fn terminal_result(&self) -> Option<Result<Arc<WebView>, WebViewError>> {
        let state = lock_or_recover(&self.state, "webview_session_state.terminal_result");
        state.terminal_result.clone()
    }

    // Only consulted by the Apple create path's registry-race guard.
    #[cfg_attr(not(any(target_os = "macos", target_os = "ios")), allow(dead_code))]
    fn is_destroyed(&self) -> bool {
        let state = lock_or_recover(&self.state, "webview_session_state.is_destroyed");
        state.destroyed
    }

    fn publish_result(
        &self,
        result: Result<Arc<WebView>, WebViewError>,
        stage_on_error: WebViewCreateStage,
    ) {
        let mut state = lock_or_recover(&self.state, "webview_session_state.publish_result");
        if state.destroyed || state.terminal_result.is_some() {
            return;
        }
        state.terminal_result = Some(result.clone());
        drop(state);

        match result {
            Ok(webview) => {
                self.event_tx
                    .send_replace(WebViewEvent::Stage(WebViewCreateStage::NativeCreated));
                self.event_tx
                    .send_replace(WebViewEvent::Stage(WebViewCreateStage::ControllerAttached));
                self.ready_tx.send_replace(Some(Ok(webview)));
                self.event_tx
                    .send_replace(WebViewEvent::Stage(WebViewCreateStage::Ready));
            }
            Err(error) => {
                self.ready_tx.send_replace(Some(Err(error.clone())));
                self.event_tx.send_replace(WebViewEvent::Failed {
                    stage: stage_on_error,
                    error,
                });
            }
        }
    }

    fn publish_destroyed(&self) {
        let mut state = lock_or_recover(&self.state, "webview_session_state.publish_destroyed");
        if state.destroyed {
            return;
        }
        state.destroyed = true;
        if state.terminal_result.is_none() {
            state.terminal_result = Some(Err(WebViewError::WebView(
                "webview destroyed before ready".to_string(),
            )));
        }
        let terminal_result = state.terminal_result.clone();
        drop(state);

        self.event_tx
            .send_replace(WebViewEvent::Stage(WebViewCreateStage::Destroyed));
        if let Some(result) = terminal_result {
            self.ready_tx.send_replace(Some(result));
        }
    }
}

pub(crate) struct WebViewCreateSender {
    webtag: WebTag,
    signals: Arc<WebViewSessionSignals>,
    native_view_id: NativeWebViewId,
}

impl WebViewCreateSender {
    fn new(webtag: WebTag, signals: Arc<WebViewSessionSignals>) -> Self {
        Self {
            webtag,
            signals,
            native_view_id: next_native_webview_id(),
        }
    }

    /// The concrete native-instance identity reserved before platform callback
    /// registration. Platform closures must capture this value and validate it
    /// against a lookup before delivering a message for a reusable WebTag.
    pub(crate) const fn native_view_id(&self) -> NativeWebViewId {
        self.native_view_id
    }

    pub(crate) fn succeed(self, webview: Arc<WebView>) {
        self.signals
            .publish_result(Ok(webview), WebViewCreateStage::Requested);
    }

    pub(crate) fn fail(self, stage: WebViewCreateStage, error: WebViewError) {
        if remove_session_signals_if_matches(&self.webtag, &self.signals) {
            crate::events::normalizer::destroy(&self.webtag);
        }
        self.signals.publish_result(Err(error), stage);
    }

    /// Complete only this create generation after a newer same-tag session
    /// replaced it. The current generation's registry and callbacks belong to
    /// a different signals identity and must remain untouched.
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub(crate) fn cancel_superseded(self) {
        if remove_session_signals_if_matches(&self.webtag, &self.signals) {
            crate::events::normalizer::destroy(&self.webtag);
        }
        self.signals.publish_destroyed();
    }

    /// True if the session was destroyed (e.g. the tab was closed/discarded)
    /// while the native WebView was still being built. The platform create
    /// path checks this before registering, to avoid leaving a zombie in the
    /// global registry. Apple and Windows consult it around native registration.
    #[cfg_attr(
        not(any(target_os = "macos", target_os = "ios", target_os = "windows")),
        allow(dead_code)
    )]
    pub(crate) fn is_destroyed(&self) -> bool {
        self.signals.is_destroyed()
    }
}

/// Global WebView instances storage
static WEBVIEW_INSTANCES: OnceLock<WebViewInstancesMap> = OnceLock::new();

/// Pending callbacks: keyed by webtag string -> callbacks struct.
/// Stored here between builder-based session creation and `register_webview`.
struct PendingCallbacksEntry {
    #[cfg(target_os = "android")]
    signals: Arc<WebViewSessionSignals>,
    callbacks: PendingCallbacks,
}

static PENDING_CALLBACKS: OnceLock<Mutex<HashMap<String, PendingCallbacksEntry>>> = OnceLock::new();
static WEBVIEW_SESSIONS: OnceLock<Mutex<HashMap<String, Arc<WebViewSessionSignals>>>> =
    OnceLock::new();
#[cfg(target_os = "windows")]
static WEBVIEW_CREATE_LOCKS: OnceLock<Mutex<HashMap<String, std::sync::Weak<Mutex<()>>>>> =
    OnceLock::new();
static DESIRED_PROXY_FOR_NEW_WEBVIEWS: OnceLock<RwLock<Option<ProxyConfig>>> = OnceLock::new();
static PROXY_APPLY_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn apply_http_proxy_platform(
    config: Option<&ProxyConfig>,
) -> Result<ProxyApplyReport, WebViewError> {
    #[cfg(target_os = "android")]
    {
        crate::android::apply_http_proxy(config)
    }

    #[cfg(any(target_os = "ios", target_os = "macos"))]
    {
        crate::apple::apply_http_proxy(config)
    }

    #[cfg(all(target_os = "linux", target_env = "ohos"))]
    {
        crate::harmony::apply_http_proxy(config)
    }

    #[cfg(not(any(
        target_os = "android",
        target_os = "ios",
        target_os = "macos",
        all(target_os = "linux", target_env = "ohos")
    )))]
    {
        let _ = config;
        Ok(ProxyApplyReport::unsupported(
            "proxy is not supported on this platform",
        ))
    }
}

/// Configure the proxy that should be used for newly created WebViews in this process.
///
/// This only updates the desired configuration kept in process memory. It does
/// not live-apply the proxy to currently active WebViews.
pub fn configure_proxy_for_new_webviews(config: Option<ProxyConfig>) -> Result<(), WebViewError> {
    let apply_lock = PROXY_APPLY_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock_or_recover(apply_lock, "webview_proxy_apply_lock");

    let normalized_config = match config {
        Some(cfg) => Some(cfg.validate()?),
        None => None,
    };

    let state = DESIRED_PROXY_FOR_NEW_WEBVIEWS.get_or_init(|| RwLock::new(None));
    match state.write() {
        Ok(mut guard) => {
            *guard = normalized_config;
        }
        Err(poisoned) => {
            log::error!("RwLock poisoned at webview_desired_proxy.write, recovering");
            *poisoned.into_inner() = normalized_config;
        }
    }
    Ok(())
}

/// Apply or clear process-level HTTP proxy for the current platform runtime now.
///
/// - `Some(config)`: set proxy
/// - `None`: clear proxy
pub fn apply_proxy_to_current_runtime(
    config: Option<ProxyConfig>,
) -> Result<ProxyApplyReport, WebViewError> {
    let apply_lock = PROXY_APPLY_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock_or_recover(apply_lock, "webview_proxy_apply_lock");

    let normalized_config = match config {
        Some(cfg) => Some(cfg.validate()?),
        None => None,
    };

    let report = apply_http_proxy_platform(normalized_config.as_ref())?;

    if matches!(
        report.status,
        ProxyApplyStatus::Applied | ProxyApplyStatus::Cleared
    ) {
        let state = DESIRED_PROXY_FOR_NEW_WEBVIEWS.get_or_init(|| RwLock::new(None));
        match state.write() {
            Ok(mut guard) => {
                *guard = normalized_config;
            }
            Err(poisoned) => {
                log::error!("RwLock poisoned at webview_desired_proxy.write, recovering");
                *poisoned.into_inner() = normalized_config;
            }
        }
    }

    Ok(report)
}

/// Get the configured proxy that will be used for newly created WebViews.
pub fn configured_proxy_for_new_webviews() -> Option<ProxyConfig> {
    let state = DESIRED_PROXY_FOR_NEW_WEBVIEWS.get()?;
    match state.read() {
        Ok(guard) => guard.clone(),
        Err(poisoned) => {
            log::error!("RwLock poisoned at webview_desired_proxy.read, recovering");
            poisoned.into_inner().clone()
        }
    }
}

fn clear_pending_callbacks(webtag: &WebTag) {
    if let Some(pending) = PENDING_CALLBACKS.get()
        && let Ok(mut map) = pending.lock()
    {
        map.remove(webtag.key());
    }
}

fn replace_session_signals(webtag: &WebTag, signals: Arc<WebViewSessionSignals>) {
    let sessions = WEBVIEW_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = lock_or_recover(sessions, "webview_sessions.replace");
    guard.insert(webtag.key().to_string(), signals);
}

fn remove_session_signals(webtag: &WebTag) -> Option<Arc<WebViewSessionSignals>> {
    let sessions = WEBVIEW_SESSIONS.get()?;
    let mut guard = lock_or_recover(sessions, "webview_sessions.remove");
    guard.remove(webtag.key())
}

fn remove_session_signals_if_matches(
    webtag: &WebTag,
    expected: &Arc<WebViewSessionSignals>,
) -> bool {
    let Some(sessions) = WEBVIEW_SESSIONS.get() else {
        return false;
    };
    let mut guard = lock_or_recover(sessions, "webview_sessions.remove_if_matches");
    if guard
        .get(webtag.key())
        .is_some_and(|current| Arc::ptr_eq(current, expected))
    {
        guard.remove(webtag.key());
        true
    } else {
        false
    }
}

/// WebView identifier combining appid, path, and optional session id.
/// Example: `appid:path#123`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WebTag(String);

impl std::fmt::Display for WebTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl WebTag {
    pub fn new(appid: &str, path: &str, session_id: Option<u64>) -> Self {
        let mut tag = format!("{}:{}", appid, path);
        if let Some(session) = session_id {
            tag.push('#');
            tag.push_str(&session.to_string());
        }
        Self(tag)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Storage key for this tag.
    /// This preserves the optional `#session` suffix so instances are isolated
    /// per runtime session.
    pub fn key(&self) -> &str {
        &self.0
    }

    /// Extract appid from the webtag
    pub fn extract_appid(&self) -> String {
        self.0.split(':').next().unwrap_or("").to_string()
    }

    /// Extract appid and path from WebTag
    /// This will always succeed since WebTag is constructed with a valid format
    pub fn extract_parts(&self) -> (String, String) {
        if let Some((appid, path_with_session)) = self.0.split_once(':') {
            let path = path_with_session
                .split('#')
                .next()
                .unwrap_or(path_with_session);
            (appid.to_string(), path.to_string())
        } else {
            log::error!("Invalid webtag format: {}", self.0);
            ("".to_string(), self.0.clone())
        }
    }

    /// Extract session id (if present) from the webtag
    pub fn session_id(&self) -> Option<u64> {
        self.0
            .split('#')
            .next_back()
            .and_then(|raw| raw.parse::<u64>().ok())
    }

    /// Grouping key combining appid and session id (`appid#session`), with the
    /// session defaulting to `0` when the tag carries no `#session` suffix.
    /// Tags without an `appid:` prefix are returned unchanged.
    #[cfg_attr(
        any(not(target_os = "windows"), target_os = "windows"),
        allow(dead_code)
    )]
    pub(crate) fn group_key(&self) -> String {
        let Some((appid, path_with_session)) = self.0.split_once(':') else {
            return self.0.clone();
        };
        let session = path_with_session
            .rsplit_once('#')
            .and_then(|(_, suffix)| suffix.parse::<u64>().ok())
            .map(|session| session.to_string())
            .unwrap_or_else(|| "0".to_string());
        format!("{appid}#{session}")
    }

    fn key_path(&self) -> String {
        let Some((_, path_with_suffix)) = self.0.split_once(':') else {
            return self.0.clone();
        };
        if self.session_id().is_some()
            && let Some((path, _)) = path_with_suffix.rsplit_once('#')
        {
            return path.to_string();
        }
        path_with_suffix.to_string()
    }
}

impl From<&str> for WebTag {
    fn from(webtag_str: &str) -> Self {
        Self(webtag_str.to_string())
    }
}

fn request_create_webview(
    webtag: &WebTag,
    sender: WebViewCreateSender,
    options: WebViewCreateOptions,
) {
    let (appid, _) = webtag.extract_parts();
    let (effective_options, pending_callbacks) = match options.normalize() {
        Ok(value) => value,
        Err(error) => {
            sender.fail(WebViewCreateStage::Requested, error);
            return;
        }
    };

    log::info!(
        "Creating WebView for key={} profile={:?} data_mode={:?} schemes={:?}",
        webtag.key(),
        effective_options.profile,
        effective_options.data_mode,
        effective_options.registered_schemes,
    );

    // Get or initialize the global instances map
    let instances = WEBVIEW_INSTANCES.get_or_init(|| Arc::new(Mutex::new(HashMap::new())));

    // Existing instance policy:
    // - Different options: fail fast (do not silently reuse incompatible instance).
    // - Same options + callback registrations: fail fast because callbacks are immutable after first create.
    // - Same options + no callbacks: return existing instance.
    if let Ok(webviews) = instances.lock()
        && let Some(existing_webview) = webviews.get(webtag.key())
    {
        if existing_webview.effective_options() != &effective_options {
            sender.fail(
                WebViewCreateStage::Requested,
                WebViewError::InvalidCreateOptions(format!(
                    "webview already exists with different options: key={} existing={:?} requested={:?}",
                    webtag.key(),
                    existing_webview.effective_options(),
                    effective_options
                )),
            );
            return;
        }

        if pending_callbacks.has_any() {
            sender.fail(
                WebViewCreateStage::Requested,
                WebViewError::InvalidCreateOptions(format!(
                    "webview already exists and callback registrations are immutable: key={} options={:?}",
                    webtag.key(),
                    existing_webview.effective_options()
                )),
            );
            log::warn!(
                "Rejected recreate with callbacks for existing webview key={} options={:?}",
                webtag.key(),
                existing_webview.effective_options()
            );
            return;
        }

        log::info!("WebView already exists, reusing: {}", webtag.key());
        sender.succeed(existing_webview.clone());
        return;
    }

    // Drop stale pending callbacks from previously failed create attempts.
    clear_pending_callbacks(webtag);

    // Stash pending callbacks for install during register_webview()
    if pending_callbacks.has_any() {
        let pending = PENDING_CALLBACKS.get_or_init(|| Mutex::new(HashMap::new()));
        if let Ok(mut map) = pending.lock() {
            map.insert(
                webtag.key().to_string(),
                PendingCallbacksEntry {
                    #[cfg(target_os = "android")]
                    signals: Arc::clone(&sender.signals),
                    callbacks: pending_callbacks,
                },
            );
        }
    }

    // Delegate WebView creation to the platform-specific implementation
    WebViewInner::create(
        &appid,
        &webtag.key_path(),
        webtag.session_id(),
        effective_options,
        sender,
    );
}

fn create_webview_session(webtag: WebTag, options: WebViewCreateOptions) -> WebViewSession {
    // Windows creation blocks until its WebView2 UI thread registers the
    // native instance. Serialize the whole same-tag transaction, including
    // session replacement and pending callbacks, so a discard/reactivate race
    // cannot cross-wire two generations of callbacks.
    #[cfg(target_os = "windows")]
    let create_lock = windows_webview_create_lock(webtag.key());
    #[cfg(target_os = "windows")]
    let _create_guard = lock_or_recover(&create_lock, "windows_webview_create_lock");

    let signals = WebViewSessionSignals::new();
    let session = signals.subscribe(webtag.clone());
    let sender = WebViewCreateSender::new(webtag.clone(), signals.clone());
    replace_session_signals(&webtag, signals);
    crate::events::normalizer::begin(&webtag, sender.native_view_id());
    request_create_webview(&webtag, sender, options);
    session
}

#[cfg(target_os = "windows")]
fn windows_webview_create_lock(webtag_key: &str) -> Arc<Mutex<()>> {
    let locks = WEBVIEW_CREATE_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut locks = lock_or_recover(locks, "windows_webview_create_locks");
    locks.retain(|_, lock| lock.strong_count() > 0);
    if let Some(lock) = locks.get(webtag_key).and_then(std::sync::Weak::upgrade) {
        return lock;
    }
    let lock = Arc::new(Mutex::new(()));
    locks.insert(webtag_key.to_string(), Arc::downgrade(&lock));
    lock
}

#[cfg_attr(target_os = "android", allow(dead_code))]
pub(crate) fn register_webview(webview: Arc<WebView>) {
    let webtag = webview.webtag();
    crate::events::normalizer::bind_native_view(&webtag, webview.native_view_id());

    // Install any pending callbacks
    if let Some(pending) = PENDING_CALLBACKS.get()
        && let Ok(mut map) = pending.lock()
        && let Some(entry) = map.remove(webtag.key())
    {
        let callbacks = entry.callbacks;
        log::info!(
            "Installing callbacks for {} (schemes={}, nav={}, new_window={}, download={}, file_chooser={}, delegate={})",
            webtag.key(),
            callbacks.scheme_handlers.len(),
            callbacks.navigation_handler.is_some(),
            callbacks.new_window_handler.is_some(),
            callbacks.download_handler.is_some(),
            callbacks.file_chooser_handler.is_some(),
            callbacks.delegate.is_some()
        );
        webview.install_callbacks(callbacks);
    }

    if let Some(instances) = WEBVIEW_INSTANCES.get()
        && let Ok(mut webviews) = instances.lock()
    {
        webviews.insert(webtag.key().to_string(), webview.clone());
        log::info!("WebView created and stored: {}", webtag.key());
    }
}

#[cfg(target_os = "android")]
pub(crate) fn register_android_webview_if_current(
    webview: Arc<WebView>,
    sender: &WebViewCreateSender,
) -> bool {
    let webtag = webview.webtag();
    let sessions = WEBVIEW_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
    let session_guard = lock_or_recover(sessions, "webview_sessions.register_android");
    if !session_guard
        .get(webtag.key())
        .is_some_and(|current| Arc::ptr_eq(current, &sender.signals))
    {
        return false;
    }

    if let Some(pending) = PENDING_CALLBACKS.get()
        && let Ok(mut map) = pending.lock()
        && map
            .get(webtag.key())
            .is_some_and(|entry| Arc::ptr_eq(&entry.signals, &sender.signals))
        && let Some(entry) = map.remove(webtag.key())
    {
        webview.install_callbacks(entry.callbacks);
    }

    let instances = WEBVIEW_INSTANCES.get_or_init(|| Arc::new(Mutex::new(HashMap::new())));
    let mut webviews = lock_or_recover(instances, "webview_instances.register_android");
    crate::events::normalizer::bind_native_view(&webtag, webview.native_view_id());
    webviews.insert(webtag.key().to_string(), webview);
    true
}

/// Find WebView by WebTag.
pub(crate) fn find_webview(webtag: &WebTag) -> Option<Arc<WebView>> {
    if let Some(instances) = WEBVIEW_INSTANCES.get() {
        if let Ok(webviews) = instances.lock() {
            webviews.get(webtag.key()).cloned()
        } else {
            None
        }
    } else {
        None
    }
}

/// Resolve a logical WebTag only while it still names the native instance
/// which registered the callback. This prevents a late callback from a
/// destroyed WebView from being delivered to a replacement that reused its
/// tag.
// This is consumed by conditionally compiled platform callback adapters.
#[allow(dead_code)]
pub(crate) fn find_webview_by_native_view_id(
    webtag: &WebTag,
    native_view_id: NativeWebViewId,
) -> Option<Arc<WebView>> {
    find_webview(webtag).filter(|webview| webview.native_view_id() == native_view_id)
}

#[cfg(target_os = "windows")]
pub(crate) fn first_browser_webview() -> Option<Arc<WebView>> {
    WEBVIEW_INSTANCES
        .get()
        .and_then(|instances| instances.lock().ok())
        .and_then(|webviews| {
            webviews
                .values()
                .find(|webview| {
                    webview.effective_options.profile == SecurityProfile::BrowserRelaxed
                })
                .cloned()
        })
}

pub(crate) fn list_webviews() -> Vec<WebTag> {
    if let Some(instances) = WEBVIEW_INSTANCES.get()
        && let Ok(webviews) = instances.lock()
    {
        let mut tags: Vec<WebTag> = webviews.values().map(|webview| webview.webtag()).collect();
        tags.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        return tags;
    }
    Vec::new()
}

/// Resolve a delegate only while a callback's concrete native WebView still
/// owns the logical tag. A retired normalizer must never deliver its queued
/// output to a replacement delegate.
pub(crate) fn find_webview_delegate_by_native_view_id(
    webtag: &WebTag,
    native_view_id: NativeWebViewId,
) -> Option<Arc<dyn WebViewDelegate>> {
    find_webview_by_native_view_id(webtag, native_view_id)
        .and_then(|webview| webview.get_delegate())
}

fn remove_arc_if_matches<T>(
    entries: &mut HashMap<String, Arc<T>>,
    key: &str,
    expected: &Arc<T>,
) -> Option<Arc<T>> {
    entries
        .get(key)
        .is_some_and(|current| Arc::ptr_eq(current, expected))
        .then(|| entries.remove(key))
        .flatten()
}

/// Remove one ready WebView only while it is still the instance registered for
/// its tag. Tag-scoped session, callback, and navigation state may already
/// belong to a newer create cycle and is deliberately left untouched.
pub(crate) fn destroy_webview_if_matches(webtag: &WebTag, expected: &Arc<WebView>) -> bool {
    // Close before touching the registry: a queued message must not cross the
    // remove-to-close window and become in-flight. Closing an already detached
    // expected instance is harmless and still rejects its late native callbacks.
    expected.close_message_ingress();
    let removed = if let Some(instances) = WEBVIEW_INSTANCES.get()
        && let Ok(mut webviews) = instances.lock()
    {
        remove_arc_if_matches(&mut webviews, webtag.key(), expected)
    } else {
        None
    };
    if let Some(webview) = removed {
        #[cfg(target_os = "windows")]
        {
            let _ = webview.inner.set_content_visible(false);
            webview.inner.request_shutdown();
        }
        webview.remove_delegate();
        true
    } else {
        false
    }
}

/// Destroy whichever WebView is currently registered for `webtag`.
///
/// This is intentionally tag-scoped host lifecycle behavior. Callers that
/// retain a concrete instance must use [`destroy_webview_if_matches`] instead.
pub(crate) fn destroy_current_webview(webtag: &WebTag) {
    // Close ingress before lifecycle notifications and native teardown. A
    // delivery already admitted under its own mutex may finish; queued and
    // future callbacks are rejected immediately.
    if let Some(webview) = find_webview(webtag) {
        webview.close_message_ingress();
    }
    // Drain active navigations as Cancelled(WebViewDestroyed) while the
    // delegate can still observe them, then drop the normalizer.
    crate::events::normalizer::destroy(webtag);
    // Mark the session destroyed FIRST. If a native create is still in flight
    // (built on the main thread but not yet registered), it observes this via
    // `WebViewCreateSender::is_destroyed()` after registering and tears the
    // instance back down — so a destroy that races ahead of registration can't
    // leave a zombie in the global registry.
    if let Some(signals) = remove_session_signals(webtag) {
        signals.publish_destroyed();
    }
    let removed = if let Some(instances) = WEBVIEW_INSTANCES.get()
        && let Ok(mut webviews) = instances.lock()
    {
        webviews.remove(webtag.key())
    } else {
        None
    };
    if let Some(webview) = removed {
        webview.close_message_ingress();
        // Windows composition teardown is asynchronous. Hide the controller
        // synchronously while it is still callable so a closed browser tab or
        // surface cannot leave its last composed frame over the replacement.
        #[cfg(target_os = "windows")]
        {
            let _ = webview.inner.set_content_visible(false);
            // Other owners can keep the retired Arc alive after it leaves the
            // registry. Stop its native thread now so a same-tag replacement
            // can safely begin instead of waiting for the final Arc to drop.
            webview.inner.request_shutdown();
        }
        webview.remove_delegate();
    }
    clear_pending_callbacks(webtag);
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_PENDING_WEB_MESSAGES, WEBVIEW_SESSIONS, WebMessageEnqueue, WebMessageIngress, WebTag,
        WebViewCreateOptions, WebViewCreateSender, WebViewSessionSignals, block_on_scheme_future,
        next_native_webview_id, remove_arc_if_matches, remove_session_signals_if_matches,
        replace_session_signals, snapshot_web_message_context,
    };
    use crate::{
        ContextualSchemeRequest, DocumentBinding, IncomingWebMessage, NativeWebViewId,
        SchemeOutcome, SchemeRequestFrame, WebMessageContext, WebMessageFrame, WebMessageSource,
        WebMessageTransport,
    };
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, mpsc};
    use std::thread;

    fn message(body: &str, native_view: crate::NativeWebViewId) -> IncomingWebMessage {
        IncomingWebMessage::new(
            body.to_string(),
            WebMessageContext::new(
                native_view,
                DocumentBinding::Unbound,
                WebMessageFrame::Unproven,
                WebMessageTransport::Other,
                WebMessageSource::unavailable(),
            ),
        )
    }

    #[test]
    fn native_webview_ids_are_not_reused_across_logical_tag_reuse() {
        let retired = next_native_webview_id();
        let replacement = next_native_webview_id();
        assert_ne!(retired, replacement);

        let late_message = message("late", retired);
        assert_ne!(late_message.context().native_view(), replacement);
        assert_eq!(late_message.context().document(), DocumentBinding::Unbound);
    }

    #[test]
    fn legacy_scheme_handler_runs_from_contextual_ingress() {
        let options = WebViewCreateOptions::strict().on_scheme("lx", |request| async move {
            assert_eq!(request.uri(), "lx://app/index.html");
            SchemeOutcome::PassThrough
        });
        let (_, callbacks) = options.normalize().unwrap();
        let handler = callbacks.scheme_handlers.get("lx").unwrap();
        let outcome = block_on_scheme_future(handler(ContextualSchemeRequest::new(
            http::Request::builder()
                .uri("lx://app/index.html")
                .body(Vec::new())
                .unwrap(),
            NativeWebViewId::new(101),
            SchemeRequestFrame::TopLevelDocument,
        )));
        assert!(matches!(outcome, SchemeOutcome::PassThrough));
    }

    #[test]
    fn ingress_snapshots_document_binding_at_enqueue_time() {
        let webtag = WebTag::new("test-app", "binding-snapshot", Some(1));
        let native_view = next_native_webview_id();
        crate::events::normalizer::begin(&webtag, native_view);
        crate::events::normalizer::submit(
            &webtag,
            native_view,
            crate::events::normalizer::NativeSignal::NavigationStarted {
                key: Some(71),
                url: "https://first/".into(),
            },
        );
        crate::events::normalizer::submit(
            &webtag,
            native_view,
            crate::events::normalizer::NativeSignal::DocumentCommitted { key: Some(71) },
        );

        let ingress = WebMessageIngress::default();
        let bound = IncomingWebMessage::new(
            "bound".to_string(),
            snapshot_web_message_context(
                native_view,
                WebMessageFrame::Unproven,
                WebMessageTransport::Other,
                WebMessageSource::unavailable(),
            ),
        );
        assert_eq!(
            bound.context().document(),
            DocumentBinding::Bound(crate::DocumentGeneration::new(1))
        );
        assert_eq!(ingress.enqueue(bound), WebMessageEnqueue::Schedule);

        crate::events::normalizer::submit(
            &webtag,
            native_view,
            crate::events::normalizer::NativeSignal::NavigationStarted {
                key: Some(72),
                url: "https://second/".into(),
            },
        );
        let revoked = IncomingWebMessage::new(
            "revoked".to_string(),
            snapshot_web_message_context(
                native_view,
                WebMessageFrame::Unproven,
                WebMessageTransport::Other,
                WebMessageSource::unavailable(),
            ),
        );
        assert_eq!(revoked.context().document(), DocumentBinding::Unbound);
        assert_eq!(ingress.enqueue(revoked), WebMessageEnqueue::Queued);

        let mut bindings = Vec::new();
        while let Some(message) = ingress.begin_delivery() {
            bindings.push(message.context().document());
            ingress.finish_delivery();
        }
        assert_eq!(
            bindings,
            vec![
                DocumentBinding::Bound(crate::DocumentGeneration::new(1)),
                DocumentBinding::Unbound,
            ]
        );
        crate::events::normalizer::destroy(&webtag);
    }

    #[test]
    fn message_ingress_preserves_fifo_across_reentrant_enqueue() {
        let ingress = WebMessageIngress::default();
        let native_view = next_native_webview_id();
        let delivered = Mutex::new(Vec::new());

        assert_eq!(
            ingress.enqueue(message("first", native_view)),
            WebMessageEnqueue::Schedule
        );
        while let Some(incoming) = ingress.begin_delivery() {
            let body = incoming.body().to_string();
            delivered.lock().unwrap().push(body.clone());
            if body == "first" {
                assert_eq!(
                    ingress.enqueue(message("second", native_view)),
                    WebMessageEnqueue::Queued
                );
                assert_eq!(
                    ingress.enqueue(message("third", native_view)),
                    WebMessageEnqueue::Queued
                );
            }
            ingress.finish_delivery();
        }

        assert_eq!(*delivered.lock().unwrap(), vec!["first", "second", "third"]);
    }

    #[test]
    fn message_ingress_bounds_untrusted_backlog_without_reordering_accepted_messages() {
        let ingress = WebMessageIngress::default();
        let native_view = next_native_webview_id();
        assert_eq!(
            ingress.enqueue(message("0", native_view)),
            WebMessageEnqueue::Schedule
        );
        for index in 1..MAX_PENDING_WEB_MESSAGES {
            assert_eq!(
                ingress.enqueue(message(&index.to_string(), native_view)),
                WebMessageEnqueue::Queued
            );
        }
        assert_eq!(
            ingress.enqueue(message("overflow", native_view)),
            WebMessageEnqueue::Full
        );

        let mut accepted = Vec::new();
        while let Some(incoming) = ingress.begin_delivery() {
            accepted.push(incoming.body().to_owned());
            ingress.finish_delivery();
        }
        assert_eq!(accepted.len(), MAX_PENDING_WEB_MESSAGES);
        assert_eq!(accepted.first().map(String::as_str), Some("0"));
        let last = (MAX_PENDING_WEB_MESSAGES - 1).to_string();
        assert_eq!(accepted.last().map(String::as_str), Some(last.as_str()));
    }

    #[test]
    fn message_ingress_preserves_fifo_across_producer_threads() {
        let ingress = Arc::new(WebMessageIngress::default());
        let native_view = next_native_webview_id();
        let (a_to_b_tx, a_to_b_rx) = mpsc::channel();
        let (b_to_a_tx, b_to_a_rx) = mpsc::channel();

        let a_ingress = Arc::clone(&ingress);
        let producer_a = thread::spawn(move || {
            assert_eq!(
                a_ingress.enqueue(message("0", native_view)),
                WebMessageEnqueue::Schedule
            );
            a_to_b_tx.send(()).unwrap();
            for value in (2..64).step_by(2) {
                b_to_a_rx.recv().unwrap();
                assert_eq!(
                    a_ingress.enqueue(message(&value.to_string(), native_view)),
                    WebMessageEnqueue::Queued
                );
                a_to_b_tx.send(()).unwrap();
            }
        });

        let b_ingress = Arc::clone(&ingress);
        let producer_b = thread::spawn(move || {
            for value in (1..64).step_by(2) {
                a_to_b_rx.recv().unwrap();
                assert_eq!(
                    b_ingress.enqueue(message(&value.to_string(), native_view)),
                    WebMessageEnqueue::Queued
                );
                if value != 63 {
                    b_to_a_tx.send(()).unwrap();
                }
            }
        });

        producer_a.join().unwrap();
        producer_b.join().unwrap();

        let mut accepted = Vec::new();
        while let Some(incoming) = ingress.begin_delivery() {
            accepted.push(incoming.body().to_owned());
            ingress.finish_delivery();
        }
        assert_eq!(
            accepted,
            (0..64).map(|value| value.to_string()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn destroying_ingress_discards_queued_messages_but_allows_admitted_delivery() {
        let ingress = Arc::new(WebMessageIngress::default());
        let native_view = next_native_webview_id();
        assert_eq!(
            ingress.enqueue(message("in-flight", native_view)),
            WebMessageEnqueue::Schedule
        );
        assert_eq!(
            ingress.enqueue(message("queued", native_view)),
            WebMessageEnqueue::Queued
        );

        let delivered = Mutex::new(Vec::new());
        let ingress_for_delivery = Arc::clone(&ingress);
        ingress.drain(|incoming| {
            delivered.lock().unwrap().push(incoming.body().to_owned());
            ingress_for_delivery.close();
            assert_eq!(
                ingress_for_delivery.enqueue(message("after-close", native_view)),
                WebMessageEnqueue::Closed
            );
        });

        assert_eq!(*delivered.lock().unwrap(), vec!["in-flight"]);
        assert!(ingress.begin_delivery().is_none());
    }

    #[test]
    fn closing_before_registry_removal_prevents_queued_message_admission() {
        let ingress = WebMessageIngress::default();
        let native_view = next_native_webview_id();
        assert_eq!(
            ingress.enqueue(message("queued", native_view)),
            WebMessageEnqueue::Schedule
        );

        // This mirrors `destroy_webview_if_matches`: close is the destroy
        // linearization point and happens before the registry mutation.
        ingress.close();

        assert!(ingress.begin_delivery().is_none());
        assert_eq!(
            ingress.enqueue(message("late", native_view)),
            WebMessageEnqueue::Closed
        );
    }

    #[test]
    fn delegate_panic_does_not_stall_following_messages() {
        let ingress = WebMessageIngress::default();
        let native_view = next_native_webview_id();
        assert_eq!(
            ingress.enqueue(message("panic", native_view)),
            WebMessageEnqueue::Schedule
        );
        assert_eq!(
            ingress.enqueue(message("after", native_view)),
            WebMessageEnqueue::Queued
        );

        let delivered = Mutex::new(Vec::new());
        ingress.drain(|incoming| {
            if incoming.body() == "panic" {
                panic!("test delegate panic");
            }
            delivered.lock().unwrap().push(incoming.body().to_owned());
        });

        assert_eq!(*delivered.lock().unwrap(), vec!["after"]);
        assert_eq!(
            ingress.enqueue(message("recovered", native_view)),
            WebMessageEnqueue::Schedule
        );
    }

    #[test]
    fn conditional_instance_removal_uses_arc_identity() {
        let current = Arc::new(7_u8);
        let same_value_different_instance = Arc::new(7_u8);
        let mut entries = HashMap::from([("tab".to_string(), current.clone())]);

        assert!(
            remove_arc_if_matches(&mut entries, "tab", &same_value_different_instance).is_none()
        );
        assert!(Arc::ptr_eq(entries.get("tab").unwrap(), &current));

        let removed = remove_arc_if_matches(&mut entries, "tab", &current).unwrap();
        assert!(Arc::ptr_eq(&removed, &current));
        assert!(!entries.contains_key("tab"));
    }

    #[test]
    fn superseded_sender_completes_without_removing_current_generation() {
        let webtag = WebTag::from("test:pages/superseded#9173");
        let superseded = WebViewSessionSignals::new();
        let current = WebViewSessionSignals::new();
        replace_session_signals(&webtag, current.clone());

        WebViewCreateSender::new(webtag.clone(), superseded.clone()).cancel_superseded();

        assert!(
            superseded
                .terminal_result()
                .is_some_and(|result| result.is_err())
        );
        assert!(current.terminal_result().is_none());
        let sessions = WEBVIEW_SESSIONS.get().unwrap().lock().unwrap();
        assert!(
            sessions
                .get(webtag.key())
                .is_some_and(|signals| Arc::ptr_eq(signals, &current))
        );
        drop(sessions);
        assert!(remove_session_signals_if_matches(&webtag, &current));
    }
}
