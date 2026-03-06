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

use crate::{WebViewController, WebViewDelegate, WebViewError};

/// Security profile for WebView creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SecurityProfile {
    StrictDefault,
    BrowserRelaxed,
}

/// WebView creation options — choose a security profile preset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebViewCreateOptions {
    pub(crate) profile: SecurityProfile,
}

/// Effective, normalized options actually applied to a concrete WebView instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct EffectiveWebViewCreateOptions {
    pub(crate) profile: SecurityProfile,
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
        }
    }

    pub fn browser_relaxed() -> Self {
        Self {
            profile: SecurityProfile::BrowserRelaxed,
        }
    }

    pub(crate) fn normalize(&self) -> EffectiveWebViewCreateOptions {
        EffectiveWebViewCreateOptions {
            profile: self.profile,
        }
    }
}

/// WebView type that includes inner implementation and delegate
pub struct WebView {
    pub(crate) inner: WebViewInner,
    effective_options: EffectiveWebViewCreateOptions,
    // Hold a strong reference to the delegate; PageInner::drop removes it to break cycles
    delegate: RwLock<Option<Arc<dyn WebViewDelegate>>>,
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

/// Create a WebView instance asynchronously with strict-default options.
pub fn create_webview(webtag: &WebTag, sender: Sender<Result<Arc<WebView>, WebViewError>>) {
    create_webview_with_options(webtag, WebViewCreateOptions::strict_default(), sender);
}

/// Create a WebView instance asynchronously with explicit options.
pub fn create_webview_with_options(
    webtag: &WebTag,
    options: WebViewCreateOptions,
    sender: Sender<Result<Arc<WebView>, WebViewError>>,
) {
    let (appid, path) = webtag.extract_parts();
    let effective_options = options.normalize();

    log::info!(
        "Creating WebView for key={} profile={:?}",
        webtag.key(),
        effective_options.profile
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
    if let Some(instances) = WEBVIEW_INSTANCES.get()
        && let Ok(mut webviews) = instances.lock()
    {
        let webtag = webview.webtag();
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
}
