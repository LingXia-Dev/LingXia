//! Third-party documents a page shows inside itself.
//!
//! The page instance owns each one under a child tag and takes it down with
//! itself. The document is not trusted: it gets the embedded WebView profile
//! (no bridge), and every top-level load is judged here before it happens.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use http::{Response, StatusCode};
use lingxia_webview::runtime::destroy_webview_if_matches;
use lingxia_webview::{
    IncomingWebMessage, LogLevel, NativeWebViewId, NavigationEvent, NavigationPolicy,
    NavigationProgress, NewWindowPolicy, SchemeOutcome, WebResourceResponse, WebTag, WebView,
    WebViewBuilder, WebViewController, WebViewDelegate, WebViewStateChange, canonical_origin,
};

use super::PageInstance;
use crate::error::LxAppError;
use crate::lxapp::LxApp;

const MAX_SOURCE_BYTES: usize = 8192;
const ANDROID_LOCAL_DOMAIN_SUFFIX: &str = ".lx-local.invalid";

static NEXT_EMBED_FILE_HOST: AtomicU64 = AtomicU64::new(1);

fn next_embed_file_host() -> String {
    let id = NEXT_EMBED_FILE_HOST
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
        .expect("embedded file host space exhausted");
    format!("embed-{id}")
}

fn is_reserved_local_host(host: &str) -> bool {
    let lower = host.to_ascii_lowercase();
    lower == "lx-local.invalid" || lower.ends_with(ANDROID_LOCAL_DOMAIN_SUFFIX)
}

fn is_internal_local_url(raw_url: &str) -> bool {
    if raw_url
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("lx://"))
    {
        return true;
    }
    let without_fragment = raw_url.split('#').next().unwrap_or(raw_url);
    let Ok(uri) = without_fragment.parse::<http::Uri>() else {
        return false;
    };
    uri.host().is_some_and(is_reserved_local_host)
}

/// Android `WebViewCompat` (`addDocumentStartJavaScript` / `addWebMessageListener`)
/// requires an HTTP/HTTPS standard scheme; custom schemes like `lx://` get an opaque
/// `"null"` origin in Chromium and never receive the mailbox. Serving packaged files
/// on a per-load `https://embed-<id>.lx-local.invalid/doc` origin intercepted locally
/// gives Android a real isolated standard origin while Apple and HarmonyOS use `lx://`.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalOriginScheme {
    Lx,
    HttpsInvalid,
}

impl LocalOriginScheme {
    const CURRENT: Self = if cfg!(target_os = "android") {
        Self::HttpsInvalid
    } else {
        Self::Lx
    };
}

/// Why a load did not happen in an embedded document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockedReason {
    /// A top-level load of a URL the page may not show.
    Navigation,
    /// A response that would have downloaded.
    Download,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct BlockedLoad {
    pub url: String,
    pub reason: BlockedReason,
}

/// Typed `<webview>` error category matching `WebViewErrorCode` in View contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddedErrorCode {
    Source,
    NotGranted,
    Network,
    Crashed,
}

impl EmbeddedErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Source => "WEBVIEW_SOURCE",
            Self::NotGranted => "WEBVIEW_NOT_GRANTED",
            Self::Network => "WEBVIEW_NETWORK",
            Self::Crashed => "WEBVIEW_CRASHED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedSourceError {
    pub code: EmbeddedErrorCode,
    pub message: String,
}

/// Message delivered from `window.lingxia.postMessage` after host origin verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbedDocumentMessage {
    /// Validated UTF-8 JSON (`<= 65536` bytes).
    pub json: String,
    /// `true` when `{ queue: true }` was requested.
    pub queued: bool,
    /// `"lxapp"` for packaged/`lx://` files, otherwise canonical `scheme://host[:port]`.
    pub origin: String,
}

/// Runs on a platform callback thread; keep it fast.
pub type BlockedLoadHandler = Arc<dyn Fn(BlockedLoad) + Send + Sync>;
pub type DocumentLoadHandler = Arc<dyn Fn(String, String) + Send + Sync>;
pub type DocumentMessageHandler = Arc<dyn Fn(EmbedDocumentMessage) + Send + Sync>;
pub type DocumentErrorHandler = Arc<dyn Fn(EmbeddedErrorCode, String) + Send + Sync>;
pub type DocumentFullscreenHandler = Arc<dyn Fn(bool) + Send + Sync>;
/// The loaded document's title changed; not a load.
pub type DocumentTitleHandler = Arc<dyn Fn(String) + Send + Sync>;

#[derive(Default)]
pub struct EmbeddedDocumentOptions {
    /// Narrows the lxapp's trusted domains to these hosts. It never widens
    /// them: a host must pass both.
    pub hosts: Option<Vec<String>>,
    pub on_blocked: Option<BlockedLoadHandler>,
    pub on_load: Option<DocumentLoadHandler>,
    pub on_title: Option<DocumentTitleHandler>,
    pub on_message: Option<DocumentMessageHandler>,
    pub on_error: Option<DocumentErrorHandler>,
    pub on_fullscreen: Option<DocumentFullscreenHandler>,
}

#[derive(Debug, Clone)]
struct PinnedLocalFile {
    scheme: LocalOriginScheme,
    embed_host: String,
    file_path: PathBuf,
    mime_type: String,
    author_base: String,
}

impl PinnedLocalFile {
    fn origin(&self) -> String {
        match self.scheme {
            LocalOriginScheme::Lx => format!("lx://{}", self.embed_host),
            LocalOriginScheme::HttpsInvalid => {
                format!("https://{}{ANDROID_LOCAL_DOMAIN_SUFFIX}", self.embed_host)
            }
        }
    }

    fn internal_url(&self, query: Option<&str>, fragment: Option<&str>) -> String {
        let mut out = format!("{}/doc", self.origin());
        if let Some(q) = query {
            out.push('?');
            out.push_str(q);
        }
        if let Some(f) = fragment {
            out.push('#');
            out.push_str(f);
        }
        out
    }

    fn matches_uri(&self, uri: &http::Uri) -> bool {
        if uri.path() != "/doc" {
            return false;
        }
        match self.scheme {
            LocalOriginScheme::Lx => {
                uri.scheme_str()
                    .is_some_and(|s| s.eq_ignore_ascii_case("lx"))
                    && uri
                        .host()
                        .is_some_and(|h| h.eq_ignore_ascii_case(&self.embed_host))
            }
            LocalOriginScheme::HttpsInvalid => {
                let expected_host = format!("{}{ANDROID_LOCAL_DOMAIN_SUFFIX}", self.embed_host);
                uri.scheme_str()
                    .is_some_and(|s| s.eq_ignore_ascii_case("https"))
                    && uri.port_u16().is_none_or(|p| p == 443)
                    && uri
                        .host()
                        .is_some_and(|h| h.eq_ignore_ascii_case(&expected_host))
            }
        }
    }

    fn matches_url(&self, raw_url: &str) -> bool {
        let without_fragment = raw_url.split('#').next().unwrap_or(raw_url);
        let Ok(uri) = without_fragment.parse::<http::Uri>() else {
            return false;
        };
        self.matches_uri(&uri)
    }

    fn public_url(&self, loaded_url: &str) -> String {
        let fragment = loaded_url.split_once('#').map(|(_, f)| f);
        match fragment {
            Some(f) => format!("{}#{f}", self.author_base),
            None => self.author_base.clone(),
        }
    }
}

#[derive(Default)]
struct DocumentState {
    author_src: String,
    author_base: String,
    pinned_local: Option<PinnedLocalFile>,
    current_top_origin: Option<String>,
    current_url: String,
    current_title: String,
    progress: NavigationProgress,
    loaded: bool,
    failed: bool,
    crashed: bool,
}

impl DocumentState {
    fn public_url(&self, raw_url: &str) -> String {
        if let Some(local) = &self.pinned_local
            && local.matches_url(raw_url)
        {
            return local.public_url(raw_url);
        }
        raw_url.to_string()
    }
}

/// A live embedded document of one page.
#[derive(Clone)]
pub struct EmbeddedDocument {
    webtag: WebTag,
    webview: Arc<WebView>,
    policy: LoadPolicy,
    state: Arc<Mutex<DocumentState>>,
}

impl EmbeddedDocument {
    pub fn webtag(&self) -> &WebTag {
        &self.webtag
    }

    pub fn webview(&self) -> &Arc<WebView> {
        &self.webview
    }

    pub fn exit_fullscreen(&self) -> Result<(), LxAppError> {
        self.webview.exit_fullscreen().map_err(Into::into)
    }

    /// Navigate this document to a new `src`, performing an in-place
    /// `hashchange` without reloading when only `#fragment` changed.
    pub fn set_source(&self, src: &str) -> Result<(), EmbeddedSourceError> {
        let Some(lxapp) = self.policy.lxapp.upgrade() else {
            return Err(EmbeddedSourceError {
                code: EmbeddedErrorCode::Source,
                message: "owning lxapp is closed".to_string(),
            });
        };
        let resolved = resolve_embedded_source(
            src,
            self.policy.hosts.as_deref(),
            crate::is_dev_session(),
            |host| lxapp.is_domain_allowed(host),
            |path| lxapp.resolve_accessible_path(path),
        )?;

        let fragment_only = {
            let mut guard = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if guard.loaded
                && !guard.failed
                && !guard.crashed
                && guard.author_base == resolved.author_base()
            {
                if guard.author_src == src {
                    return Ok(());
                }
                guard.author_src = src.to_string();
                Some(resolved.fragment().map(str::to_string))
            } else {
                None
            }
        };

        if let Some(fragment) = fragment_only {
            let hash_value = match fragment {
                Some(frag) => format!("#{frag}"),
                None => String::new(),
            };
            let encoded =
                serde_json::to_string(&hash_value).expect("serializing Rust string cannot fail");
            let script = format!("try {{ location.hash = {encoded}; }} catch (_) {{}}");
            return self
                .webview
                .exec_js(&script)
                .map_err(|err| EmbeddedSourceError {
                    code: EmbeddedErrorCode::Network,
                    message: err.to_string(),
                });
        }

        let target_url = self.apply_resolved_target(&lxapp, src, resolved);
        self.webview
            .load_url(&target_url)
            .map_err(|err| EmbeddedSourceError {
                code: EmbeddedErrorCode::Network,
                message: err.to_string(),
            })
    }

    fn apply_resolved_target(
        &self,
        _lxapp: &LxApp,
        raw_src: &str,
        resolved: ResolvedEmbeddedSource,
    ) -> String {
        let mut guard = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.author_src = raw_src.to_string();
        guard.author_base = resolved.author_base().to_string();
        guard.loaded = false;
        guard.failed = false;
        guard.crashed = false;
        match resolved {
            ResolvedEmbeddedSource::Network { url, .. } => {
                guard.pinned_local = None;
                guard.current_top_origin = canonical_origin(&url);
                url
            }
            ResolvedEmbeddedSource::LocalFile {
                author_base,
                file_path,
                query,
                fragment,
            } => {
                let reuse_host = guard
                    .pinned_local
                    .as_ref()
                    .filter(|prev| prev.file_path == file_path)
                    .map(|prev| prev.embed_host.clone());
                let embed_host = reuse_host.unwrap_or_else(next_embed_file_host);
                let mime_type = LxApp::infer_mime_type(&file_path.to_string_lossy()).to_string();
                let pinned = PinnedLocalFile {
                    scheme: LocalOriginScheme::CURRENT,
                    embed_host,
                    file_path,
                    mime_type,
                    author_base,
                };
                let url = pinned.internal_url(query.as_deref(), fragment.as_deref());
                guard.current_top_origin = Some(pinned.origin());
                guard.pinned_local = Some(pinned);
                url
            }
        }
    }

    fn destroy(&self) {
        destroy_webview_if_matches(&self.webtag, &self.webview);
    }
}

/// What the top level of an embedded document may load.
#[derive(Clone)]
struct LoadPolicy {
    hosts: Option<Vec<String>>,
    lxapp: std::sync::Weak<crate::LxApp>,
}

impl LoadPolicy {
    fn allows_network(&self, url: &str) -> bool {
        let Some(lxapp) = self.lxapp.upgrade() else {
            return false;
        };
        top_level_load_allowed(
            url,
            self.hosts.as_deref(),
            crate::is_dev_session(),
            |host| lxapp.is_domain_allowed(host),
        )
    }

    fn allows_top_level(&self, url: &str, state: &Mutex<DocumentState>) -> bool {
        if is_internal_local_url(url) {
            return state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .pinned_local
                .as_ref()
                .is_some_and(|pinned| pinned.matches_url(url));
        }
        self.allows_network(url)
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ResolvedEmbeddedSource {
    Network {
        url: String,
        author_base: String,
        fragment: Option<String>,
    },
    LocalFile {
        author_base: String,
        file_path: PathBuf,
        query: Option<String>,
        fragment: Option<String>,
    },
}

impl ResolvedEmbeddedSource {
    fn author_base(&self) -> &str {
        match self {
            Self::Network { author_base, .. } | Self::LocalFile { author_base, .. } => author_base,
        }
    }

    fn fragment(&self) -> Option<&str> {
        match self {
            Self::Network { fragment, .. } | Self::LocalFile { fragment, .. } => {
                fragment.as_deref()
            }
        }
    }
}

/// Resolve an author `<webview src>` into either an allowed network URL or an
/// accessible packaged / `lx://` file, distinguishing `WEBVIEW_SOURCE` from
/// `WEBVIEW_NOT_GRANTED`.
fn resolve_embedded_source(
    raw_src: &str,
    hosts: Option<&[String]>,
    dev_session: bool,
    domain_allowed: impl Fn(&str) -> bool,
    resolve_file: impl Fn(&str) -> Result<PathBuf, LxAppError>,
) -> Result<ResolvedEmbeddedSource, EmbeddedSourceError> {
    let src = raw_src.trim();
    if src.is_empty() || src.len() > MAX_SOURCE_BYTES || src.bytes().any(|b| b < 0x20 || b == 0x7f)
    {
        return Err(EmbeddedSourceError {
            code: EmbeddedErrorCode::Source,
            message: "invalid <webview src>".to_string(),
        });
    }

    let (without_fragment, fragment) = match src.split_once('#') {
        Some((base, frag)) => (base, Some(frag.to_string())),
        None => (src, None),
    };

    let lower = without_fragment.to_ascii_lowercase();
    if lower.starts_with("https://") || lower.starts_with("http://") {
        let secure = lower.starts_with("https://");
        let uri: http::Uri = without_fragment.parse().map_err(|_| EmbeddedSourceError {
            code: EmbeddedErrorCode::Source,
            message: format!("invalid URL in <webview src>: {raw_src}"),
        })?;
        let Some(host) = uri.host().filter(|host| !host.is_empty()) else {
            return Err(EmbeddedSourceError {
                code: EmbeddedErrorCode::Source,
                message: format!("missing host in <webview src>: {raw_src}"),
            });
        };
        if !secure && !dev_session {
            return Err(EmbeddedSourceError {
                code: EmbeddedErrorCode::Source,
                message: "<webview src> requires https, a packaged file, or an lx:// file"
                    .to_string(),
            });
        }
        if is_reserved_local_host(host) {
            return Err(EmbeddedSourceError {
                code: EmbeddedErrorCode::Source,
                message: format!("reserved internal host in <webview src>: {host}"),
            });
        }
        if let Some(hosts) = hosts
            && !hosts
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(host))
        {
            return Err(EmbeddedSourceError {
                code: EmbeddedErrorCode::NotGranted,
                message: format!("{host} is not in trustedDomains"),
            });
        }
        if !domain_allowed(host) {
            return Err(EmbeddedSourceError {
                code: EmbeddedErrorCode::NotGranted,
                message: format!("{host} is not in trustedDomains"),
            });
        }
        return Ok(ResolvedEmbeddedSource::Network {
            url: src.to_string(),
            author_base: without_fragment.to_string(),
            fragment,
        });
    }

    let (bare_path, query) = match without_fragment.split_once('?') {
        Some((path, q)) => (path, Some(q.to_string())),
        None => (without_fragment, None),
    };
    if !bare_path.starts_with("lx://") && (bare_path.starts_with('/') || bare_path.contains(':')) {
        return Err(EmbeddedSourceError {
            code: EmbeddedErrorCode::Source,
            message: format!(
                "<webview src> must be an https URL, a packaged path, or an lx:// file: {raw_src}"
            ),
        });
    }
    let file_path = resolve_file(bare_path).map_err(|err| EmbeddedSourceError {
        code: EmbeddedErrorCode::Source,
        message: err.to_string(),
    })?;
    if !file_path.is_file() {
        return Err(EmbeddedSourceError {
            code: EmbeddedErrorCode::Source,
            message: format!("packaged or lx:// file not found: {bare_path}"),
        });
    }
    Ok(ResolvedEmbeddedSource::LocalFile {
        author_base: without_fragment.to_string(),
        file_path,
        query,
        fragment,
    })
}

/// `https` to a host the lxapp trusts. A dev session also admits `http`, so
/// a suite can use a fixture server; which hosts it reaches is still the
/// domain check's decision. Everything else (`tel:`, `about:`, custom
/// schemes, a URL with no host) is refused.
fn top_level_load_allowed(
    url: &str,
    hosts: Option<&[String]>,
    dev_session: bool,
    domain_allowed: impl Fn(&str) -> bool,
) -> bool {
    let without_fragment = url.split('#').next().unwrap_or(url);
    let Ok(uri) = without_fragment.parse::<http::Uri>() else {
        return false;
    };
    let secure = match uri.scheme_str() {
        Some(scheme) if scheme.eq_ignore_ascii_case("https") => true,
        Some(scheme) if scheme.eq_ignore_ascii_case("http") => false,
        _ => return false,
    };
    let Some(host) = uri.host().filter(|host| !host.is_empty()) else {
        return false;
    };
    if !secure && !dev_session {
        return false;
    }
    if is_reserved_local_host(host) {
        return false;
    }
    if let Some(hosts) = hosts
        && !hosts
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(host))
    {
        return false;
    }
    domain_allowed(host)
}

/// Verify whether a web permission request (`origin`, `is_main_frame`) belongs
/// to the currently committed, authorized top-level document.
fn permission_origin_allowed(
    raw_origin: &str,
    is_main_frame: Option<bool>,
    current_top_origin: Option<&str>,
    policy_allows_network: impl Fn(&str) -> bool,
) -> bool {
    if is_main_frame == Some(false) {
        return false;
    }
    let Some(origin) = canonical_origin(raw_origin) else {
        return false;
    };
    let Some(top_origin) = current_top_origin else {
        return false;
    };
    if !origin.eq_ignore_ascii_case(top_origin) {
        return false;
    }
    if is_internal_local_url(&origin) {
        return true;
    }
    policy_allows_network(&origin)
}

struct EmbeddedWebViewDelegate {
    state: Arc<Mutex<DocumentState>>,
    on_load: Option<DocumentLoadHandler>,
    on_title: Option<DocumentTitleHandler>,
    on_error: Option<DocumentErrorHandler>,
}

impl WebViewDelegate for EmbeddedWebViewDelegate {
    fn on_navigation_event(&self, event: NavigationEvent) {
        enum Action {
            None,
            Loaded { url: String, title: String },
            Failed { message: String },
        }

        let action = {
            let mut guard = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            guard.progress.apply(&event);
            match event {
                NavigationEvent::Started { .. } => {
                    guard.loaded = false;
                    guard.failed = false;
                    guard.current_title.clear();
                    Action::None
                }
                NavigationEvent::Succeeded { final_url, .. } => {
                    if final_url.eq_ignore_ascii_case("about:blank") {
                        Action::None
                    } else {
                        guard.loaded = true;
                        guard.failed = false;
                        guard.crashed = false;
                        guard.current_top_origin = canonical_origin(&final_url);
                        let public_url = guard.public_url(&final_url);
                        guard.current_url = public_url.clone();
                        Action::Loaded {
                            url: public_url,
                            title: guard.current_title.clone(),
                        }
                    }
                }
                NavigationEvent::Failed { error, .. } => {
                    guard.loaded = false;
                    guard.failed = true;
                    Action::Failed {
                        message: if error.description.trim().is_empty() {
                            "failed to load web page".to_string()
                        } else {
                            error.description
                        },
                    }
                }
                NavigationEvent::Cancelled { .. } => Action::None,
            }
        };

        match action {
            Action::None => {}
            Action::Loaded { url, title } => {
                if let Some(on_load) = &self.on_load {
                    on_load(url, title);
                }
            }
            Action::Failed { message } => {
                if let Some(on_error) = &self.on_error {
                    on_error(EmbeddedErrorCode::Network, message);
                }
            }
        }
    }

    fn on_webview_state_change(&self, change: WebViewStateChange) {
        // WebKit reports a page's title after it finished loading: a title is not a second load.
        let mut retitled = None;
        let in_page_load = {
            let mut guard = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            match change {
                WebViewStateChange::Title { title } => {
                    let next_title = title.unwrap_or_default();
                    if next_title != guard.current_title {
                        guard.current_title = next_title;
                        if guard.loaded
                            && !guard.failed
                            && !guard.crashed
                            && !guard.current_url.is_empty()
                        {
                            retitled = Some(guard.current_title.clone());
                        }
                    }
                    None
                }
                WebViewStateChange::Location { url } => {
                    if url.is_empty() || url.eq_ignore_ascii_case("about:blank") {
                        None
                    } else {
                        let public_url = guard.public_url(&url);
                        if guard.loaded
                            && !guard.failed
                            && !guard.crashed
                            && public_url != guard.current_url
                        {
                            guard.current_url = public_url.clone();
                            Some((public_url, guard.current_title.clone()))
                        } else {
                            None
                        }
                    }
                }
                _ => None,
            }
        };
        if let (Some((url, title)), Some(on_load)) = (in_page_load, &self.on_load) {
            on_load(url, title);
        }
        if let (Some(title), Some(on_title)) = (retitled, &self.on_title) {
            on_title(title);
        }
    }

    fn on_web_content_process_terminated(&self, _native_view: NativeWebViewId) {
        {
            let mut guard = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            guard.loaded = false;
            guard.crashed = true;
        }
        if let Some(on_error) = &self.on_error {
            on_error(
                EmbeddedErrorCode::Crashed,
                "WebView content process terminated".to_string(),
            );
        }
    }

    fn handle_post_message(&self, _message: IncomingWebMessage) {
        // EmbeddedDocument installs no lxapp bridge transport.
    }

    fn log(&self, _level: LogLevel, _message: &str) {
        // Third-party document console never enters lxapp logs.
    }
}

fn serve_pinned_local_file(
    req: http::Request<Vec<u8>>,
    state: &Mutex<DocumentState>,
) -> SchemeOutcome {
    let pinned = state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .pinned_local
        .clone();
    let Some(pinned) = pinned else {
        return not_found_scheme_response();
    };
    if !pinned.matches_uri(req.uri()) {
        return not_found_scheme_response();
    }
    let Ok(meta) = std::fs::metadata(&pinned.file_path) else {
        return not_found_scheme_response();
    };
    let response = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", pinned.mime_type.as_str())
        .header("Content-Length", meta.len().to_string())
        .header("Cache-Control", "no-store")
        .header("Access-Control-Allow-Origin", "null")
        .body(())
        .expect("static response headers are valid");
    let (parts, _) = response.into_parts();
    let res: WebResourceResponse = (parts, pinned.file_path).into();
    SchemeOutcome::Handled(res)
}

fn not_found_scheme_response() -> SchemeOutcome {
    let response = Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("Content-Type", "text/plain; charset=utf-8")
        .body(())
        .expect("static 404 headers are valid");
    let (parts, _) = response.into_parts();
    let res: WebResourceResponse = (parts, b"Not Found".to_vec()).into();
    SchemeOutcome::Handled(res)
}

impl PageInstance {
    /// Validate an author `<webview src>` synchronously without creating a WebView.
    pub fn check_embedded_source(
        &self,
        src: &str,
        hosts: Option<&[String]>,
    ) -> Result<(), EmbeddedSourceError> {
        let lxapp = self.owning_lxapp();
        resolve_embedded_source(
            src,
            hosts,
            crate::is_dev_session(),
            |host| lxapp.is_domain_allowed(host),
            |path| lxapp.resolve_accessible_path(path),
        )
        .map(|_| ())
    }

    /// Create the embedded document `node_id` of this page and load `url`.
    ///
    /// Fails when `url` is not one the page may show, when the node already
    /// has a document, or when the WebView cannot be created.
    pub async fn open_embedded_document(
        &self,
        node_id: &str,
        url: &str,
        options: EmbeddedDocumentOptions,
    ) -> Result<EmbeddedDocument, LxAppError> {
        self.open_embedded_document_typed(node_id, url, options)
            .await
            .map_err(|err| match err {
                OpenEmbeddedError::Source(source) => LxAppError::InvalidParameter(format!(
                    "{}: {}",
                    source.code.as_str(),
                    source.message
                )),
                OpenEmbeddedError::Runtime(error) => error,
            })
    }

    /// Typed variant distinguishing `WEBVIEW_SOURCE` / `WEBVIEW_NOT_GRANTED`
    /// from WebView creation / network errors.
    pub async fn open_embedded_document_typed(
        &self,
        node_id: &str,
        url: &str,
        options: EmbeddedDocumentOptions,
    ) -> Result<EmbeddedDocument, OpenEmbeddedError> {
        let webtag = self.webtag().child(node_id).ok_or_else(|| {
            OpenEmbeddedError::Runtime(LxAppError::InvalidParameter(format!(
                "invalid embedded document id: {node_id:?}"
            )))
        })?;
        let lxapp = self.owning_lxapp();
        let resolved = resolve_embedded_source(
            url,
            options.hosts.as_deref(),
            crate::is_dev_session(),
            |host| lxapp.is_domain_allowed(host),
            |path| lxapp.resolve_accessible_path(path),
        )
        .map_err(OpenEmbeddedError::Source)?;

        if self.embedded_document(node_id).is_some() {
            return Err(OpenEmbeddedError::Runtime(LxAppError::InvalidParameter(
                format!("embedded document {node_id:?} is already open"),
            )));
        }

        let policy = LoadPolicy {
            hosts: options.hosts,
            lxapp: Arc::downgrade(&lxapp),
        };
        let state = Arc::new(Mutex::new(DocumentState::default()));
        let blocked: BlockedLoadHandler = options.on_blocked.unwrap_or_else(|| Arc::new(|_| {}));
        let report = |reason: BlockedReason| {
            let blocked = blocked.clone();
            move |url: &str| {
                blocked(BlockedLoad {
                    url: url.to_string(),
                    reason,
                })
            }
        };

        let delegate = Arc::new(EmbeddedWebViewDelegate {
            state: Arc::clone(&state),
            on_load: options.on_load,
            on_title: options.on_title,
            on_error: options.on_error,
        });

        let navigation_policy = policy.clone();
        let navigation_state = Arc::clone(&state);
        let navigation_blocked = report(BlockedReason::Navigation);

        let window_policy = policy.clone();
        let window_state = Arc::clone(&state);
        let window_blocked = report(BlockedReason::Navigation);

        let scheme_state = Arc::clone(&state);
        let message_policy = policy.clone();
        let message_state = Arc::clone(&state);
        let on_message = options.on_message;

        let perm_policy = policy.clone();
        let perm_state = Arc::clone(&state);

        let mut builder = WebViewBuilder::embedded(webtag.clone())
            .delegate(delegate)
            .on_scheme("lx", move |req| {
                let state = Arc::clone(&scheme_state);
                async move { serve_pinned_local_file(req, &state) }
            })
            // Subframes and subresources are the document's own business; the
            // page only decides which document it shows.
            .on_navigation(move |request| {
                if !request.is_main_frame {
                    return NavigationPolicy::Allow;
                }
                if navigation_policy.allows_top_level(&request.url, &navigation_state) {
                    if !is_internal_local_url(&request.url) {
                        let mut guard = navigation_state
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        // Navigating from a packaged file to an allowed https URL
                        // drops the pinned local file.
                        guard.pinned_local = None;
                        guard.current_top_origin = canonical_origin(&request.url);
                    }
                    NavigationPolicy::Allow
                } else {
                    navigation_blocked(&request.url);
                    NavigationPolicy::Cancel
                }
            })
            // Judged here, not left to the navigation handler: a load the
            // host starts itself does not reach that handler on Android.
            .on_new_window(move |target_url| {
                if window_policy.allows_top_level(target_url, &window_state) {
                    if !is_internal_local_url(target_url) {
                        let mut guard = window_state
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        guard.pinned_local = None;
                        guard.current_top_origin = canonical_origin(target_url);
                    }
                    NewWindowPolicy::LoadInSelf
                } else {
                    window_blocked(target_url);
                    NewWindowPolicy::Cancel
                }
            })
            .on_download_blocked(report(BlockedReason::Download))
            .on_permission_origin(move |origin, is_main_frame| {
                let top_origin = perm_state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .current_top_origin
                    .clone();
                permission_origin_allowed(
                    origin,
                    is_main_frame,
                    top_origin.as_deref(),
                    |net_origin| perm_policy.allows_network(net_origin),
                )
            });

        #[cfg(target_os = "android")]
        {
            let https_scheme_state = Arc::clone(&state);
            builder = builder.on_scheme("https", move |req| {
                let state = Arc::clone(&https_scheme_state);
                async move {
                    if req.uri().host().is_some_and(is_reserved_local_host) {
                        serve_pinned_local_file(req, &state)
                    } else {
                        SchemeOutcome::PassThrough
                    }
                }
            });
        }

        if let Some(on_message) = on_message {
            builder = builder.on_embed_message(move |msg| {
                let is_local = is_internal_local_url(&msg.origin);
                let allowed = if is_local {
                    message_state
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .pinned_local
                        .as_ref()
                        .is_some_and(|pinned| msg.origin.eq_ignore_ascii_case(&pinned.origin()))
                } else {
                    message_policy.allows_network(&msg.origin)
                };
                if !allowed {
                    return;
                }
                let origin = if is_local {
                    "lxapp".to_string()
                } else {
                    msg.origin
                };
                on_message(EmbedDocumentMessage {
                    json: msg.json,
                    queued: msg.queued,
                    origin,
                });
            });
        }

        if let Some(on_fullscreen) = options.on_fullscreen {
            builder = builder.on_fullscreen(move |fullscreen| on_fullscreen(fullscreen));
        }

        let session = builder.create();
        let webview = session
            .wait_ready()
            .await
            .map_err(|e| OpenEmbeddedError::Runtime(e.into()))?;
        let document = EmbeddedDocument {
            webtag,
            webview,
            policy,
            state,
        };

        let raced = {
            let mut documents = self.embedded_documents();
            if documents.contains_key(node_id) || self.is_discarded() {
                true
            } else {
                documents.insert(node_id.to_string(), document.clone());
                false
            }
        };
        if raced {
            document.destroy();
            return Err(OpenEmbeddedError::Runtime(LxAppError::InvalidParameter(
                format!("embedded document {node_id:?} could not be attached to its page"),
            )));
        }

        let initial_url = document.apply_resolved_target(&lxapp, url, resolved);
        if let Err(error) = document.webview.load_url(&initial_url) {
            self.close_embedded_document(node_id);
            return Err(OpenEmbeddedError::Runtime(error.into()));
        }
        Ok(document)
    }

    /// The live embedded document `node_id` of this page.
    pub fn embedded_document(&self, node_id: &str) -> Option<EmbeddedDocument> {
        self.embedded_documents().get(node_id).cloned()
    }

    /// Destroy the embedded document `node_id`. `false` when there is none.
    pub fn close_embedded_document(&self, node_id: &str) -> bool {
        let document = self.embedded_documents().remove(node_id);
        document.inspect(EmbeddedDocument::destroy).is_some()
    }

    /// Destroy every embedded document belonging to this page.
    pub fn close_all_embedded_documents(&self) {
        destroy_all(&self.inner.embedded_documents);
    }

    fn embedded_documents(
        &self,
    ) -> std::sync::MutexGuard<'_, std::collections::HashMap<String, EmbeddedDocument>> {
        self.inner
            .embedded_documents
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[derive(Debug)]
pub enum OpenEmbeddedError {
    Source(EmbeddedSourceError),
    Runtime(LxAppError),
}

/// Take down every embedded document of a page that is going away.
pub(super) fn destroy_all(
    documents: &std::sync::Mutex<std::collections::HashMap<String, EmbeddedDocument>>,
) {
    let documents: Vec<EmbeddedDocument> = documents
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .drain()
        .map(|(_, document)| document)
        .collect();
    for document in documents {
        document.destroy();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EmbeddedErrorCode, LocalOriginScheme, PinnedLocalFile, ResolvedEmbeddedSource,
        permission_origin_allowed, resolve_embedded_source, top_level_load_allowed,
    };
    use crate::error::LxAppError;
    use std::path::PathBuf;

    fn allowed(url: &str, hosts: Option<&[&str]>, dev_session: bool) -> bool {
        let hosts: Option<Vec<String>> =
            hosts.map(|hosts| hosts.iter().map(|host| host.to_string()).collect());
        top_level_load_allowed(url, hosts.as_deref(), dev_session, |host| {
            host != "untrusted.example"
        })
    }

    #[test]
    fn only_https_to_a_trusted_host_loads() {
        assert!(allowed("https://maps.example/a?b=1#c", None, false));
        assert!(allowed("HTTPS://Maps.Example/", None, false));
        assert!(!allowed("https://untrusted.example/", None, false));
        assert!(!allowed("http://maps.example/", None, false));
        for url in [
            "tel:10086",
            "mailto:a@maps.example",
            "about:blank",
            "intent://maps.example/#Intent;end",
            "lx://app/index.html",
            "javascript:alert(1)",
            "data:text/html,hi",
            "https:///no-host",
            "",
        ] {
            assert!(!allowed(url, None, true), "{url}");
        }
    }

    #[test]
    fn a_dev_session_admits_http_and_nothing_else() {
        assert!(allowed("http://127.0.0.1:18473/page", None, true));
        assert!(!allowed("http://untrusted.example/", None, true));
        assert!(!allowed("ftp://maps.example/", None, true));
    }

    #[test]
    fn the_host_list_narrows_and_never_widens() {
        let hosts: &[&str] = &["maps.example", "untrusted.example"];
        assert!(allowed("https://MAPS.example/", Some(hosts), false));
        assert!(!allowed("https://other.example/", Some(hosts), false));
        // Listed, but the lxapp does not trust it.
        assert!(!allowed("https://untrusted.example/", Some(hosts), false));
        assert!(!allowed("https://maps.example/", Some(&[]), false));
    }

    #[test]
    fn source_resolver_distinguishes_source_error_from_not_granted() {
        let tmp = std::env::temp_dir().join("lx_embed_doc_test.html");
        std::fs::write(&tmp, b"<!doctype html><title>ok</title>").unwrap();

        let resolve = |src: &str, dev: bool| {
            let tmp = tmp.clone();
            resolve_embedded_source(
                src,
                None,
                dev,
                |host| host == "maps.example",
                move |path| {
                    if path == "pages/pick/map.html" || path == "lx://userdata/map.html" {
                        Ok(tmp.clone())
                    } else {
                        Err(LxAppError::ResourceNotFound(path.to_string()))
                    }
                },
            )
        };

        assert_eq!(
            resolve("https://maps.example/pick?z=12#31,121", false).unwrap(),
            ResolvedEmbeddedSource::Network {
                url: "https://maps.example/pick?z=12#31,121".to_string(),
                author_base: "https://maps.example/pick?z=12".to_string(),
                fragment: Some("31,121".to_string()),
            }
        );
        assert_eq!(
            resolve("https://evil.example/pick", false)
                .unwrap_err()
                .code,
            EmbeddedErrorCode::NotGranted
        );
        assert_eq!(
            resolve("http://maps.example/pick", false).unwrap_err().code,
            EmbeddedErrorCode::Source
        );
        assert_eq!(
            resolve("tel:10086", true).unwrap_err().code,
            EmbeddedErrorCode::Source
        );
        assert_eq!(
            resolve("pages/pick/missing.html", false).unwrap_err().code,
            EmbeddedErrorCode::Source
        );
        assert_eq!(
            resolve("pages/pick/map.html?mode=dark#1,2", false).unwrap(),
            ResolvedEmbeddedSource::LocalFile {
                author_base: "pages/pick/map.html?mode=dark".to_string(),
                file_path: tmp.clone(),
                query: Some("mode=dark".to_string()),
                fragment: Some("1,2".to_string()),
            }
        );
        assert_eq!(
            resolve("lx://userdata/map.html#anchor", false).unwrap(),
            ResolvedEmbeddedSource::LocalFile {
                author_base: "lx://userdata/map.html".to_string(),
                file_path: tmp.clone(),
                query: None,
                fragment: Some("anchor".to_string()),
            }
        );

        assert_eq!(
            resolve("https://embed-1.lx-local.invalid/doc", false)
                .unwrap_err()
                .code,
            EmbeddedErrorCode::Source
        );
        assert!(!allowed("https://embed-1.lx-local.invalid/doc", None, true));

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn pinned_local_file_serves_only_its_own_url_and_maps_public_fragment() {
        let pinned = PinnedLocalFile {
            scheme: LocalOriginScheme::Lx,
            embed_host: "embed-7".to_string(),
            file_path: PathBuf::from("/tmp/map.html"),
            mime_type: "text/html".to_string(),
            author_base: "pages/pick/map.html?k=1".to_string(),
        };
        let url = pinned.internal_url(Some("k=1"), Some("31,121"));
        assert_eq!(url, "lx://embed-7/doc?k=1#31,121");
        let uri: http::Uri = "lx://embed-7/doc?k=1".parse().unwrap();
        assert!(pinned.matches_uri(&uri));
        assert!(!pinned.matches_uri(&"lx://embed-7/other.html".parse().unwrap()));
        assert!(!pinned.matches_uri(&"lx://embed-8/doc".parse().unwrap()));
        assert!(!pinned.matches_uri(&"lx://lxapp/pages/pick/map.html".parse().unwrap()));
        assert_eq!(
            pinned.public_url("lx://embed-7/doc?k=1#99,88"),
            "pages/pick/map.html?k=1#99,88"
        );

        let android_pinned = PinnedLocalFile {
            scheme: LocalOriginScheme::HttpsInvalid,
            embed_host: "embed-7".to_string(),
            file_path: PathBuf::from("/tmp/map.html"),
            mime_type: "text/html; charset=utf-8".to_string(),
            author_base: "embed/demo.html".to_string(),
        };
        assert_eq!(
            android_pinned.internal_url(None, Some("sec")),
            "https://embed-7.lx-local.invalid/doc#sec"
        );
        assert!(android_pinned.matches_url("https://embed-7.lx-local.invalid/doc#sec"));
        assert!(!android_pinned.matches_url("https://embed-7.lx-local.invalid/other"));
        assert!(!android_pinned.matches_url("https://embed-8.lx-local.invalid/doc"));
        assert_eq!(
            android_pinned.public_url("https://embed-7.lx-local.invalid/doc#sec"),
            "embed/demo.html#sec"
        );
    }

    #[test]
    fn permission_origin_requires_matching_top_level_allowed_origin() {
        let allow = |origin: &str, main: Option<bool>, top: Option<&str>| {
            permission_origin_allowed(origin, main, top, |u| u.starts_with("https://maps.example"))
        };
        assert!(allow(
            "https://maps.example:443",
            Some(true),
            Some("https://maps.example")
        ));
        assert!(allow(
            "https://maps.example",
            None,
            Some("https://maps.example")
        ));
        // Cross-origin iframe or subframe is always denied.
        assert!(!allow(
            "https://maps.example",
            Some(false),
            Some("https://maps.example")
        ));
        assert!(!allow(
            "https://ads.example",
            None,
            Some("https://maps.example")
        ));
        // Missing origin is denied.
        assert!(!allow("", Some(true), Some("https://maps.example")));
        assert!(!allow("null", Some(true), Some("https://maps.example")));
        // Pinned local file matches only its own isolated lx://embed-<id> origin.
        assert!(allow("lx://embed-5", Some(true), Some("lx://embed-5")));
        assert!(!allow("lx://embed-6", Some(true), Some("lx://embed-5")));
    }
}
