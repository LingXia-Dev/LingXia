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
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::mpsc::{SyncSender, sync_channel};
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
    AsyncSchemeHandler, ClickOptions, DownloadHandler, DownloadRequest, FileChooserRequest,
    FileChooserResponse, FillOptions, NavigationHandler, NavigationPolicy, NewWindowHandler,
    NewWindowPolicy, PressOptions, SchemeOutcome, ScrollOptions, TypeOptions,
    WebViewInputController,
};
use crate::{
    LoadDataRequest, WebResourceResponse, WebViewController, WebViewCookie,
    WebViewCookieSetRequest, WebViewDelegate, WebViewError, WebViewInputError, WebViewScriptError,
};
use async_trait::async_trait;

const APPLE_INTERNAL_SCHEME: &str = "lx-apple";

#[cfg(not(any(
    target_os = "android",
    target_os = "ios",
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", target_env = "ohos")
)))]
fn unsupported_webview_error(action: &str) -> WebViewError {
    WebViewError::WebView(format!("{action} is not supported on this platform"))
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

    fn set_user_agent(&self, _ua: &str) -> Result<(), WebViewError> {
        Err(unsupported_webview_error("set_user_agent"))
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

pub(crate) type FileChooserFuture =
    Pin<Box<dyn Future<Output = FileChooserResponse> + Send + 'static>>;
pub(crate) type FileChooserHandler =
    Box<dyn Fn(FileChooserRequest) -> FileChooserFuture + Send + Sync>;

/// Internal WebView creation options.
pub(crate) struct WebViewCreateOptions {
    pub(crate) profile: SecurityProfile,
    pub(crate) scheme_handlers: HashMap<String, AsyncSchemeHandler>,
    pub(crate) navigation_handler: Option<NavigationHandler>,
    pub(crate) new_window_handler: Option<NewWindowHandler>,
    pub(crate) download_handler: Option<DownloadHandler>,
    pub(crate) file_chooser_handler: Option<FileChooserHandler>,
    pub(crate) delegate: Option<Arc<dyn WebViewDelegate>>,
}

impl std::fmt::Debug for WebViewCreateOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebViewCreateOptions")
            .field("profile", &self.profile)
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
            scheme_handlers: HashMap::new(),
            navigation_handler: None,
            new_window_handler: None,
            download_handler: None,
            file_chooser_handler: None,
            delegate: None,
        }
    }

    fn browser() -> Self {
        Self {
            profile: SecurityProfile::BrowserRelaxed,
            scheme_handlers: HashMap::new(),
            navigation_handler: None,
            new_window_handler: None,
            download_handler: None,
            file_chooser_handler: None,
            delegate: None,
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
    fn on_scheme<F, Fut>(mut self, scheme: &str, handler: F) -> Self
    where
        F: Fn(http::Request<Vec<u8>>) -> Fut + Send + Sync + 'static,
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

    /// Register a navigation handler that decides whether to allow or cancel navigations.
    /// The handler receives the URL being navigated to and returns a `NavigationPolicy`.
    fn on_navigation<F>(mut self, handler: F) -> Self
    where
        F: Fn(&str) -> NavigationPolicy + Send + Sync + 'static,
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
            registered_schemes,
            has_navigation_handler: self.navigation_handler.is_some(),
            has_new_window_handler: self.new_window_handler.is_some(),
            has_download_handler: self.download_handler.is_some(),
            has_file_chooser_handler: self.file_chooser_handler.is_some(),
            has_delegate: self.delegate.is_some(),
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

    pub fn on_scheme<F, Fut>(mut self, scheme: &str, handler: F) -> Self
    where
        F: Fn(http::Request<Vec<u8>>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = SchemeOutcome> + Send + 'static,
    {
        self.options = self.options.on_scheme(scheme, handler);
        self
    }

    pub fn on_navigation<F>(mut self, handler: F) -> Self
    where
        F: Fn(&str) -> NavigationPolicy + Send + Sync + 'static,
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

    pub fn on_navigation<F>(mut self, handler: F) -> Self
    where
        F: Fn(&str) -> NavigationPolicy + Send + Sync + 'static,
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
    effective_options: EffectiveWebViewCreateOptions,
    // Hold a strong reference to the delegate; runtime destroy clears it to break cycles.
    delegate: RwLock<Option<Arc<dyn WebViewDelegate>>>,
    // Closure-based scheme handlers registered via builders.
    scheme_handlers: RwLock<HashMap<String, AsyncSchemeHandler>>,
    navigation_handler: RwLock<Option<NavigationHandler>>,
    new_window_handler: RwLock<Option<NewWindowHandler>>,
    download_handler: RwLock<Option<DownloadHandler>>,
    file_chooser_handler: RwLock<Option<FileChooserHandler>>,
}

impl WebView {
    pub(crate) fn new(
        inner: WebViewInner,
        effective_options: EffectiveWebViewCreateOptions,
    ) -> Self {
        Self {
            inner,
            effective_options,
            delegate: RwLock::new(None),
            scheme_handlers: RwLock::new(HashMap::new()),
            navigation_handler: RwLock::new(None),
            new_window_handler: RwLock::new(None),
            download_handler: RwLock::new(None),
            file_chooser_handler: RwLock::new(None),
        }
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
    pub(crate) fn handle_scheme_request(
        &self,
        scheme: &str,
        request: http::Request<Vec<u8>>,
    ) -> Option<WebResourceResponse> {
        #[cfg(any(target_os = "ios", target_os = "macos"))]
        if let Some(response) = self.inner.handle_internal_bridge_request(&request) {
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
    pub fn handle_navigation(&self, url: &str) -> NavigationPolicy {
        if let Ok(guard) = self.navigation_handler.read()
            && let Some(handler) = guard.as_ref()
        {
            return handler(url);
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
    pub fn handle_new_window(&self, url: &str) -> NewWindowPolicy {
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
    #[cfg(any(target_os = "ios", all(target_os = "linux", target_env = "ohos")))]
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
              try {{ el.dispatchEvent(new MouseEvent('mousedown', opts)); }} catch(_e) {{}} \
              try {{ el.dispatchEvent(new MouseEvent('mouseup', opts)); }} catch(_e) {{}} \
              try {{ el.dispatchEvent(new MouseEvent('click', opts)); }} catch(_e) {{}} \
              return {{ ok:true, count:els.length }}; \
            }})({selector_json}, {idx})"
        );
        let result = self
            .inner
            .eval_js(&script)
            .await
            .map_err(WebViewInputError::Script)?;
        if result.get("ok").and_then(|v| v.as_bool()) == Some(true) {
            Ok(())
        } else {
            let err_msg = result
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("click failed")
                .to_string();
            if result.get("interactable").and_then(|v| v.as_bool()) == Some(false) {
                Err(WebViewInputError::ElementNotInteractable(err_msg))
            } else {
                Err(WebViewInputError::ElementNotFound(err_msg))
            }
        }
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

    fn set_user_agent(&self, ua: &str) -> Result<(), WebViewError> {
        self.inner.set_user_agent(ua)
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
}

#[async_trait]
impl WebViewInputController for WebView {
    async fn click(
        &self,
        _selector: &str,
        _options: ClickOptions,
    ) -> Result<(), WebViewInputError> {
        #[cfg(all(feature = "webview-input", target_os = "macos"))]
        {
            return self.inner.click_inner(_selector, _options).await;
        }
        #[cfg(target_os = "android")]
        {
            return self.inner.click_inner(_selector, _options).await;
        }
        #[cfg(target_os = "ios")]
        {
            return self.click_via_js(_selector, _options.index).await;
        }
        #[cfg(all(target_os = "linux", target_env = "ohos"))]
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
            return self.inner.type_text_inner(_selector, _text, _options).await;
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
            let _ = _options;
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
        #[allow(unreachable_code)]
        Err(WebViewInputError::Unsupported(
            "input control is not implemented for this platform",
        ))
    }

    async fn press(&self, _key: &str, _options: PressOptions) -> Result<(), WebViewInputError> {
        #[cfg(all(feature = "webview-input", target_os = "macos"))]
        {
            return self.inner.press_inner(_key, _options).await;
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
        #[cfg(all(feature = "webview-input", target_os = "macos"))]
        {
            return self.inner.scroll_inner(_dx, _dy, _options).await;
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
            return self.inner.scroll_to_inner(_selector, _options).await;
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
    signals: Arc<WebViewSessionSignals>,
}

impl WebViewCreateSender {
    fn new(signals: Arc<WebViewSessionSignals>) -> Self {
        Self { signals }
    }

    pub(crate) fn succeed(self, webview: Arc<WebView>) {
        self.signals
            .publish_result(Ok(webview), WebViewCreateStage::Requested);
    }

    pub(crate) fn fail(self, stage: WebViewCreateStage, error: WebViewError) {
        self.signals.publish_result(Err(error), stage);
    }
}

/// Global WebView instances storage
static WEBVIEW_INSTANCES: OnceLock<WebViewInstancesMap> = OnceLock::new();

/// Pending callbacks: keyed by webtag string -> callbacks struct.
/// Stored here between builder-based session creation and `register_webview`.
static PENDING_CALLBACKS: OnceLock<Mutex<HashMap<String, PendingCallbacks>>> = OnceLock::new();
static WEBVIEW_SESSIONS: OnceLock<Mutex<HashMap<String, Arc<WebViewSessionSignals>>>> =
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
        any(
            not(target_os = "windows"),
            all(target_os = "windows", not(feature = "windows-host"))
        ),
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
        "Creating WebView for key={} profile={:?} schemes={:?}",
        webtag.key(),
        effective_options.profile,
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
            map.insert(webtag.key().to_string(), pending_callbacks);
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
    let signals = WebViewSessionSignals::new();
    let session = signals.subscribe(webtag.clone());
    let sender = WebViewCreateSender::new(signals.clone());
    replace_session_signals(&webtag, signals);
    request_create_webview(&webtag, sender, options);
    session
}

pub(crate) fn register_webview(webview: Arc<WebView>) {
    let webtag = webview.webtag();

    // Install any pending callbacks
    if let Some(pending) = PENDING_CALLBACKS.get()
        && let Ok(mut map) = pending.lock()
        && let Some(callbacks) = map.remove(webtag.key())
    {
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

#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", target_env = "ohos")
))]
pub(crate) fn find_webview_delegate(webtag: &WebTag) -> Option<Arc<dyn WebViewDelegate>> {
    find_webview(webtag).and_then(|webview| webview.get_delegate())
}

/// Destroy a WebView instance by WebTag and remove it from global storage
pub(crate) fn destroy_webview(webtag: &WebTag) {
    let removed = if let Some(instances) = WEBVIEW_INSTANCES.get()
        && let Ok(mut webviews) = instances.lock()
    {
        webviews.remove(webtag.key())
    } else {
        None
    };
    if let Some(webview) = removed {
        webview.remove_delegate();
    }
    clear_pending_callbacks(webtag);
    if let Some(signals) = remove_session_signals(webtag) {
        signals.publish_destroyed();
    }
}
