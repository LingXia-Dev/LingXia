use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use tokio::sync::oneshot::Sender;

#[cfg(target_os = "android")]
use crate::android::WebViewInner;

#[cfg(any(target_os = "ios", target_os = "macos"))]
use crate::apple::WebViewInner;

#[cfg(all(target_os = "linux", target_env = "ohos"))]
use crate::harmony::WebViewInner;

use crate::traits::{
    NavigationHandler, NavigationPolicy, NewWindowHandler, NewWindowPolicy, SyncSchemeHandler,
};
use crate::{WebResourceResponse, WebViewController, WebViewDelegate, WebViewError};

/// Security profile for WebView creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SecurityProfile {
    StrictDefault,
    BrowserRelaxed,
}

/// WebView creation options — choose a security profile preset and register scheme handlers.
pub struct WebViewCreateOptions {
    pub(crate) profile: SecurityProfile,
    pub(crate) scheme_handlers: HashMap<String, SyncSchemeHandler>,
    pub(crate) navigation_handler: Option<NavigationHandler>,
    pub(crate) new_window_handler: Option<NewWindowHandler>,
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
            .finish()
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
}

impl Default for WebViewCreateOptions {
    fn default() -> Self {
        Self::strict_default()
    }
}

impl WebViewCreateOptions {
    pub fn strict_default() -> Self {
        Self {
            profile: SecurityProfile::StrictDefault,
            scheme_handlers: HashMap::new(),
            navigation_handler: None,
            new_window_handler: None,
        }
    }

    pub fn browser_relaxed() -> Self {
        Self {
            profile: SecurityProfile::BrowserRelaxed,
            scheme_handlers: HashMap::new(),
            navigation_handler: None,
            new_window_handler: None,
        }
    }

    /// Register a synchronous scheme handler for a custom URL scheme.
    /// The handler receives an `http::Request<Vec<u8>>` and returns
    /// `Some(WebResourceResponse)` to handle it, or `None` to decline.
    pub fn on_scheme<F>(mut self, scheme: &str, handler: F) -> Self
    where
        F: Fn(http::Request<Vec<u8>>) -> Option<WebResourceResponse> + Send + Sync + 'static,
    {
        let normalized = scheme.trim().to_ascii_lowercase();
        if !normalized.is_empty() {
            self.scheme_handlers.insert(normalized, Box::new(handler));
        }
        self
    }

    /// Register a navigation handler that decides whether to allow or cancel navigations.
    /// The handler receives the URL being navigated to and returns a `NavigationPolicy`.
    pub fn on_navigation<F>(mut self, handler: F) -> Self
    where
        F: Fn(&str) -> NavigationPolicy + Send + Sync + 'static,
    {
        self.navigation_handler = Some(Box::new(handler));
        self
    }

    /// Register a new-window handler for `target="_blank"` / `window.open()`.
    /// The handler receives the URL and returns a `NewWindowPolicy`.
    pub fn on_new_window<F>(mut self, handler: F) -> Self
    where
        F: Fn(&str) -> NewWindowPolicy + Send + Sync + 'static,
    {
        self.new_window_handler = Some(Box::new(handler));
        self
    }

    pub(crate) fn normalize(self) -> (EffectiveWebViewCreateOptions, PendingCallbacks) {
        let mut registered_schemes: Vec<String> = self.scheme_handlers.keys().cloned().collect();
        registered_schemes.sort_unstable();
        registered_schemes.dedup();
        let effective = EffectiveWebViewCreateOptions {
            profile: self.profile,
            registered_schemes,
            has_navigation_handler: self.navigation_handler.is_some(),
            has_new_window_handler: self.new_window_handler.is_some(),
        };
        let pending = PendingCallbacks {
            scheme_handlers: self.scheme_handlers,
            navigation_handler: self.navigation_handler,
            new_window_handler: self.new_window_handler,
        };
        (effective, pending)
    }
}

/// Pending callbacks extracted from `WebViewCreateOptions::normalize()`.
/// Stored between `create_webview` (extraction) and `register_webview` (installation).
pub(crate) struct PendingCallbacks {
    pub(crate) scheme_handlers: HashMap<String, SyncSchemeHandler>,
    pub(crate) navigation_handler: Option<NavigationHandler>,
    pub(crate) new_window_handler: Option<NewWindowHandler>,
}

/// WebView type that includes inner implementation and delegate
pub struct WebView {
    pub(crate) inner: WebViewInner,
    effective_options: EffectiveWebViewCreateOptions,
    // Hold a strong reference to the delegate; PageInner::drop removes it to break cycles
    delegate: RwLock<Option<Arc<dyn WebViewDelegate>>>,
    // Closure-based scheme handlers registered via WebViewCreateOptions
    scheme_handlers: RwLock<HashMap<String, SyncSchemeHandler>>,
    navigation_handler: RwLock<Option<NavigationHandler>>,
    new_window_handler: RwLock<Option<NewWindowHandler>>,
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

    /// Set delegate for this WebView
    pub fn set_delegate(&self, delegate: Arc<dyn WebViewDelegate>) {
        if let Ok(mut guard) = self.delegate.write() {
            *guard = Some(delegate);
        }
    }

    /// Get delegate for this WebView
    pub fn get_delegate(&self) -> Option<Arc<dyn WebViewDelegate>> {
        self.delegate.read().ok().and_then(|guard| guard.clone())
    }

    /// Remove delegate for this WebView
    pub fn remove_delegate(&self) {
        if let Ok(mut guard) = self.delegate.write() {
            *guard = None;
        }
    }

    /// Install all pending callbacks into this WebView (called once during creation).
    pub(crate) fn install_callbacks(&self, callbacks: PendingCallbacks) {
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
    pub fn handle_scheme_request(
        &self,
        scheme: &str,
        request: http::Request<Vec<u8>>,
    ) -> Option<WebResourceResponse> {
        let guard = self.scheme_handlers.read().ok()?;
        let handler = guard.get(scheme)?;
        handler(request)
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

    /// Toggle docked DevTools (macOS only, uses private _inspector API)
    #[cfg(target_os = "macos")]
    pub fn toggle_devtools(&self) {
        self.inner.toggle_devtools();
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
}

impl WebViewController for WebView {
    fn load_url(&self, url: String) -> Result<(), WebViewError> {
        self.inner.load_url(url)
    }

    fn load_data(
        &self,
        data: String,
        base_url: String,
        history_url: Option<String>,
    ) -> Result<(), WebViewError> {
        self.inner.load_data(data, base_url, history_url)
    }

    fn evaluate_javascript(&self, js: String) -> Result<(), WebViewError> {
        self.inner.evaluate_javascript(js)
    }

    fn post_message(&self, message: String) -> Result<(), WebViewError> {
        self.inner.post_message(message)
    }

    fn clear_browsing_data(&self) -> Result<(), WebViewError> {
        self.inner.clear_browsing_data()
    }

    fn set_user_agent(&self, ua: String) -> Result<(), WebViewError> {
        self.inner.set_user_agent(ua)
    }
}

/// Type alias for WebView instances storage to reduce complexity
type WebViewInstancesMap = Arc<Mutex<HashMap<String, Arc<WebView>>>>;

/// Global WebView instances storage
static WEBVIEW_INSTANCES: OnceLock<WebViewInstancesMap> = OnceLock::new();

/// Pending callbacks: keyed by webtag string → callbacks struct.
/// Stored here between `create_webview` (which extracts them from options)
/// and `register_webview` (which installs them on the WebView).
static PENDING_CALLBACKS: OnceLock<Mutex<HashMap<String, PendingCallbacks>>> = OnceLock::new();

fn clear_pending_callbacks(webtag: &WebTag) {
    if let Some(pending) = PENDING_CALLBACKS.get()
        && let Ok(mut map) = pending.lock()
    {
        map.remove(webtag.key());
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
        self.0
            .split_once(':')
            .map(|(appid, path_with_session)| {
                let path = path_with_session
                    .split('#')
                    .next()
                    .unwrap_or(path_with_session);
                (appid.to_string(), path.to_string())
            })
            .unwrap()
    }

    /// Extract session id (if present) from the webtag
    pub fn session_id(&self) -> Option<u64> {
        self.0
            .split('#')
            .nth(1)
            .and_then(|raw| raw.parse::<u64>().ok())
    }
}

impl From<&str> for WebTag {
    fn from(webtag_str: &str) -> Self {
        Self(webtag_str.to_string())
    }
}

/// Initialize WebView manager
pub fn init_webview_manager() {
    let _ = WEBVIEW_INSTANCES.set(Arc::new(Mutex::new(HashMap::new())));
}

/// Create a WebView instance asynchronously.
///
/// When `options` is `None`, strict-default options are used.
pub fn create_webview(
    webtag: &WebTag,
    sender: Sender<Result<Arc<WebView>, WebViewError>>,
    options: Option<WebViewCreateOptions>,
) {
    let (appid, path) = webtag.extract_parts();
    let (effective_options, pending_callbacks) = options
        .unwrap_or_else(WebViewCreateOptions::strict_default)
        .normalize();

    log::info!(
        "Creating WebView for key={} profile={:?} schemes={:?}",
        webtag.key(),
        effective_options.profile,
        effective_options.registered_schemes,
    );

    // Get or initialize the global instances map
    let instances = WEBVIEW_INSTANCES.get_or_init(|| Arc::new(Mutex::new(HashMap::new())));

    // Check if WebView already exists (first-create-wins by full webtag key)
    if let Ok(webviews) = instances.lock()
        && let Some(existing_webview) = webviews.get(webtag.key())
    {
        if existing_webview.effective_options() != &effective_options {
            log::warn!(
                "WebView already exists with different options, reusing first-created instance: key={} existing={:?} requested={:?}",
                webtag.key(),
                existing_webview.effective_options(),
                effective_options
            );
        } else {
            log::info!("WebView already exists, reusing: {}", webtag.key());
        }
        let _ = sender.send(Ok(existing_webview.clone()));
        return;
    }

    // Drop stale pending callbacks from previously failed create attempts.
    clear_pending_callbacks(webtag);

    // Stash pending callbacks for install during register_webview()
    let has_callbacks = !pending_callbacks.scheme_handlers.is_empty()
        || pending_callbacks.navigation_handler.is_some()
        || pending_callbacks.new_window_handler.is_some();
    if has_callbacks {
        let pending = PENDING_CALLBACKS.get_or_init(|| Mutex::new(HashMap::new()));
        if let Ok(mut map) = pending.lock() {
            map.insert(webtag.key().to_string(), pending_callbacks);
        }
    }

    // Delegate WebView creation to the platform-specific implementation
    WebViewInner::create(
        &appid,
        &path,
        webtag.session_id(),
        effective_options,
        sender,
    );
}

pub(crate) fn register_webview(webview: Arc<WebView>) {
    let webtag = webview.webtag();

    // Install any pending callbacks
    if let Some(pending) = PENDING_CALLBACKS.get()
        && let Ok(mut map) = pending.lock()
        && let Some(callbacks) = map.remove(webtag.key())
    {
        log::info!(
            "Installing callbacks for {} (schemes={}, nav={}, new_window={})",
            webtag.key(),
            callbacks.scheme_handlers.len(),
            callbacks.navigation_handler.is_some(),
            callbacks.new_window_handler.is_some(),
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

/// Find WebView by WebTag
pub fn find_webview(webtag: &WebTag) -> Option<Arc<WebView>> {
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

/// Set delegate for a WebView by WebTag
pub fn set_webview_delegate(webtag: &WebTag, delegate: Arc<dyn WebViewDelegate>) -> bool {
    if let Some(webview) = find_webview(webtag) {
        webview.set_delegate(delegate);
        true
    } else {
        false
    }
}

/// Get delegate from WebView by webtag (for internal use by platform implementations)
pub fn get_webview_delegate(webtag: &WebTag) -> Option<Arc<dyn WebViewDelegate>> {
    if let Some(webview) = find_webview(webtag) {
        webview.get_delegate()
    } else {
        None
    }
}

/// Destroy a WebView instance by WebTag and remove it from global storage
pub fn destroy_webview(webtag: &WebTag) {
    if let Some(instances) = WEBVIEW_INSTANCES.get()
        && let Ok(mut webviews) = instances.lock()
    {
        webviews.remove(webtag.key());
    }
    clear_pending_callbacks(webtag);
}
