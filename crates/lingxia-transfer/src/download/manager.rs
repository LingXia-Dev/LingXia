use bytes::Bytes;
use dashmap::DashMap;
use dashmap::mapref::entry::Entry as DashEntry;
use http::Request as HttpRequest;
use http::header;
use http_body_util::{BodyExt, Full};
use lingxia_webview::DownloadRequest;
use ring::digest::{SHA256, digest};
use rong_rt::http::{self as host_http, HttpBody, RequestOptions};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::Error as IoError;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::sync::watch;

const DOWNLOAD_PROGRESS_INTERVAL_BYTES: u64 = 64 * 1024;
const DOWNLOAD_PROGRESS_INTERVAL_MILLIS: u128 = 120;
const DOWNLOAD_RESUME_METADATA_INTERVAL_BYTES: u64 = 256 * 1024;
const DOWNLOAD_SMALL_BODY_LIMIT: usize = 128 * 1024;
const DOWNLOAD_DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(90);
const DOWNLOAD_DEFAULT_MAX_RETRIES: u32 = 3;
const DOWNLOAD_DEFAULT_RETRY_DELAY: Duration = Duration::from_millis(750);
const DOWNLOAD_MAX_RETRY_DELAY: Duration = Duration::from_secs(5);
pub(crate) const DOWNLOAD_CANCELED_ERROR: &str = "Download canceled";
pub(crate) const DOWNLOAD_PAUSED_ERROR: &str = "Download paused";
static USER_CACHE_DOWNLOADS: OnceLock<DashMap<String, SharedDownloadRegistration>> =
    OnceLock::new();
static TARGET_PATH_DOWNLOADS: OnceLock<DashMap<String, SharedTargetPathDownload>> = OnceLock::new();

const BROWSER_DOWNLOAD_EVENT_STARTED: &str = "BrowserDownloadStarted";
const BROWSER_DOWNLOAD_EVENT_PROGRESS: &str = "BrowserDownloadProgress";
const BROWSER_DOWNLOAD_EVENT_COMPLETED: &str = "BrowserDownloadCompleted";
const BROWSER_DOWNLOAD_EVENT_FAILED: &str = "BrowserDownloadFailed";
const BROWSER_DOWNLOAD_EVENT_PAUSED: &str = "BrowserDownloadPaused";

/// A single HTTP(S) download job with resumable semantics.
#[derive(Debug, Clone)]
pub struct DownloadTask {
    request: DownloadRequest,
    root_dir: PathBuf,
    fallback_user_agent: Option<String>,
    request_headers: Vec<(String, String)>,
    target_path: Option<PathBuf>,
    persistence: Option<DownloadPersistence>,
    behavior: DownloadBehavior,
    reuse_existing_target: bool,
}

/// Tuning knobs for how a single `DownloadTask` behaves under weak-network
/// conditions: per-request timeout, optional connect timeout, and the
/// retry-with-resume policy used by `run_download_task`.
///
/// `max_retries` is the **number of additional attempts** after the initial
/// one, so the total attempt count is `1 + max_retries`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadBehavior {
    pub request_timeout: Duration,
    pub connect_timeout: Option<Duration>,
    pub max_retries: u32,
    pub retry_delay: Duration,
}

impl Default for DownloadBehavior {
    fn default() -> Self {
        Self {
            request_timeout: DOWNLOAD_DEFAULT_REQUEST_TIMEOUT,
            connect_timeout: None,
            max_retries: DOWNLOAD_DEFAULT_MAX_RETRIES,
            retry_delay: DOWNLOAD_DEFAULT_RETRY_DELAY,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DownloadOwnerKind {
    #[default]
    #[serde(alias = "unknown")]
    Browser,
    LxApp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DownloadOwner {
    #[serde(default)]
    pub kind: DownloadOwnerKind,
    #[serde(default)]
    pub appid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DownloadPersistence {
    pub app_data_dir: PathBuf,
    pub task_id: String,
    pub owner: DownloadOwner,
    pub track_record: bool,
}

impl DownloadPersistence {
    pub fn new(
        app_data_dir: PathBuf,
        task_id: impl Into<String>,
        owner: DownloadOwner,
        track_record: bool,
    ) -> Self {
        Self {
            app_data_dir,
            task_id: task_id.into(),
            owner,
            track_record,
        }
    }
}

/// Streaming updates emitted while a download task is running.
#[derive(Debug, Clone)]
pub enum DownloadEvent {
    Started {
        url: String,
        file_name: String,
        target_path: PathBuf,
        mime_type: Option<String>,
        total_bytes: Option<u64>,
        resumed_bytes: u64,
    },
    Progress {
        url: String,
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    },
    Completed {
        url: String,
        file_name: String,
        path: PathBuf,
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    },
    Failed {
        url: String,
        error: String,
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    },
    Paused {
        url: String,
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    },
}

/// Terminal success result for a download task.
#[derive(Debug, Clone)]
struct DownloadSuccess {
    file_name: String,
    path: PathBuf,
    downloaded_bytes: u64,
}

/// Terminal failure result for a download task.
#[derive(Debug, Clone)]
pub struct DownloadFailure {
    pub url: String,
    pub error: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserCacheDownloadRequest {
    pub url: String,
    pub headers: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserCacheDownloadResult {
    pub temp_path: PathBuf,
    pub file_name: String,
    pub mime_type: Option<String>,
    pub size: u64,
}

#[derive(Debug, Clone)]
enum SharedDownloadState {
    InProgress,
    Completed(DownloadSuccess),
    Failed(DownloadFailure),
}

#[derive(Debug, Clone)]
struct SharedDownloadRegistration {
    signature: String,
    tx: watch::Sender<SharedDownloadState>,
}

#[derive(Debug, Clone)]
struct SharedTargetPathDownload {
    signature: String,
    tx: watch::Sender<SharedDownloadState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ResumeMetadata {
    etag: Option<String>,
    last_modified: Option<String>,
    downloaded: u64,
    total: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
struct DownloadProgress {
    downloaded: u64,
    last_emitted: u64,
    last_persisted: u64,
    last_percent: i32,
    emitted_once: bool,
    last_emit_at: Instant,
}

#[derive(Debug, Clone)]
struct DownloadAttemptFailure {
    error: String,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    retryable: bool,
}

fn resolve_download_root(app_data_dir: &Path, default_root: impl Into<PathBuf>) -> PathBuf {
    lingxia_settings::get_download_dir(app_data_dir)
        .ok()
        .flatten()
        .unwrap_or_else(|| default_root.into())
}

fn default_download_root(app_data_dir: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            let candidate = PathBuf::from(home).join("Downloads");
            if !candidate.as_os_str().is_empty() {
                return candidate;
            }
        }
    }

    app_data_dir.join("downloads")
}

pub(crate) fn download_root(app_data_dir: &Path) -> PathBuf {
    resolve_download_root(app_data_dir, default_download_root(app_data_dir))
}

pub fn browser_download_root(app_data_dir: &Path) -> PathBuf {
    download_root(app_data_dir)
}

fn user_cache_download_root(user_cache_dir: &Path) -> PathBuf {
    user_cache_dir.to_path_buf()
}

impl DownloadTask {
    pub fn for_browser(
        request: DownloadRequest,
        root_dir: PathBuf,
        fallback_user_agent: Option<String>,
    ) -> Self {
        Self {
            request,
            root_dir,
            fallback_user_agent,
            request_headers: Vec::new(),
            target_path: None,
            persistence: None,
            behavior: DownloadBehavior::default(),
            reuse_existing_target: true,
        }
    }

    pub fn with_target_path(mut self, target_path: PathBuf) -> Self {
        self.target_path = Some(target_path);
        self
    }

    pub(crate) fn with_persistence(mut self, persistence: DownloadPersistence) -> Self {
        self.persistence = Some(persistence);
        self
    }

    pub fn with_browser_persistence(
        mut self,
        app_data_dir: PathBuf,
        task_id: impl Into<String>,
    ) -> Self {
        self.persistence = Some(DownloadPersistence {
            app_data_dir,
            task_id: task_id.into(),
            owner: DownloadOwner {
                kind: DownloadOwnerKind::Browser,
                ..DownloadOwner::default()
            },
            track_record: false,
        });
        self
    }

    pub fn with_behavior(mut self, behavior: DownloadBehavior) -> Self {
        self.behavior = behavior;
        self
    }

    fn with_reuse_existing_target(mut self, reuse_existing_target: bool) -> Self {
        self.reuse_existing_target = reuse_existing_target;
        self
    }
}

fn download_request_options(behavior: DownloadBehavior) -> RequestOptions {
    let options = RequestOptions::new().with_request_timeout(behavior.request_timeout);
    if let Some(connect_timeout) = behavior.connect_timeout {
        options.with_connect_timeout(connect_timeout)
    } else {
        options
    }
}

fn retry_delay_for_attempt(base: Duration, attempt: u32) -> Duration {
    let factor = 1u32 << attempt.min(3);
    base.checked_mul(factor)
        .unwrap_or(DOWNLOAD_MAX_RETRY_DELAY)
        .min(DOWNLOAD_MAX_RETRY_DELAY)
}

fn is_retryable_http_status(status: u16) -> bool {
    matches!(status, 408 | 425 | 429 | 500 | 502 | 503 | 504)
}

/// Heuristic classification of transport-level error strings returned from
/// `rong_rt` / `hyper` / `hyper-util`. rong surfaces errors as opaque
/// `String`s (see `rong_rt::client::send_request_with_coalesce` and
/// `process_request`), so matching against substrings is the only option
/// short of restructuring the upstream error types. Revisit this list when
/// bumping `rong` or `hyper` — a wording change there silently turns a
/// retryable blip into a hard failure.
fn is_retryable_transport_error(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    // Explicit user/app cancellation must never be retried, even if the
    // underlying transport would otherwise look transient.
    if lower == DOWNLOAD_CANCELED_ERROR.to_ascii_lowercase()
        || lower == DOWNLOAD_PAUSED_ERROR.to_ascii_lowercase()
        || lower.contains("download canceled")
        || lower.contains("download paused")
        || lower.contains("operation canceled")
        || lower.contains("operation cancelled")
        || lower.contains("request canceled")
        || lower.contains("request cancelled")
        || lower.contains("user canceled")
        || lower.contains("user cancelled")
    {
        return false;
    }

    // Timeouts from rong_rt: "request timeout", "read timeout".
    lower.contains("timeout")
        // Generic "temporary failure" phrasing from DNS resolvers etc.
        || lower.contains("tempor")
        // TCP-level disconnects while a stream is in flight. Note: we cannot
        // match "connection aborted" here because the cancel guard above
        // swallows any string containing "aborted".
        || lower.contains("connection reset")
        || lower.contains("connection aborted")
        || lower.contains("connection closed")
        || lower.contains("connect error")
        || lower.contains("broken pipe")
        // hyper body errors when the peer closes mid-stream.
        || lower.contains("unexpected eof")
        || lower.contains("early eof")
        || lower.contains("body stream")
        || lower.contains("incomplete message")
        // DNS / routing failures, often transient on cellular.
        || lower.contains("dns")
        || lower.contains("unreachable")
        || lower.contains("no route")
        // TLS handshake hiccups on flaky networks.
        || lower.contains("tls")
        || lower.contains("handshake")
        // HTTP/2 stream-level retry signals.
        || lower.contains("refused stream")
        || lower.contains("goaway")
        // Catch-all for "network" phrasing in upstream errors.
        || lower.contains("network")
}

async fn sleep_for_retry(
    cancel_rx: Option<&mut watch::Receiver<crate::download::ActiveDownloadCommand>>,
    delay: Duration,
) -> Result<(), String> {
    if delay.is_zero() {
        return Ok(());
    }

    if let Some(cancel) = cancel_rx {
        let initial_command = *cancel.borrow();
        if initial_command != crate::download::ActiveDownloadCommand::None {
            return Err(match initial_command {
                crate::download::ActiveDownloadCommand::Pause => DOWNLOAD_PAUSED_ERROR,
                crate::download::ActiveDownloadCommand::Cancel => DOWNLOAD_CANCELED_ERROR,
                crate::download::ActiveDownloadCommand::None => unreachable!(),
            }
            .to_string());
        }

        let sleep = tokio::time::sleep(delay);
        tokio::pin!(sleep);

        loop {
            tokio::select! {
                _ = &mut sleep => return Ok(()),
                changed = cancel.changed() => {
                    if changed.is_err() {
                        return Ok(());
                    }
                    match *cancel.borrow() {
                        crate::download::ActiveDownloadCommand::Pause => {
                            return Err(DOWNLOAD_PAUSED_ERROR.to_string());
                        }
                        crate::download::ActiveDownloadCommand::Cancel => {
                            return Err(DOWNLOAD_CANCELED_ERROR.to_string());
                        }
                        crate::download::ActiveDownloadCommand::None => {}
                    }
                }
            }
        }
    }

    tokio::time::sleep(delay).await;
    Ok(())
}

fn user_cache_downloads() -> &'static DashMap<String, SharedDownloadRegistration> {
    USER_CACHE_DOWNLOADS.get_or_init(DashMap::new)
}

fn target_path_downloads() -> &'static DashMap<String, SharedTargetPathDownload> {
    TARGET_PATH_DOWNLOADS.get_or_init(DashMap::new)
}

fn sanitize_filename(raw: &str) -> String {
    let mut sanitized = raw
        .trim()
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => ch,
        })
        .collect::<String>();
    while sanitized.ends_with('.') {
        sanitized.pop();
    }
    if sanitized.is_empty() || sanitized == "." || sanitized == ".." {
        "download.bin".to_string()
    } else {
        sanitized
    }
}

fn percent_decode_lossy(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = bytes[i + 1];
            let lo = bytes[i + 2];
            let hex = [hi, lo];
            if let Ok(hex_str) = std::str::from_utf8(&hex)
                && let Ok(value) = u8::from_str_radix(hex_str, 16)
            {
                out.push(value);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn parse_content_disposition_filename(value: &str) -> Option<String> {
    let segments: Vec<&str> = value.split(';').map(str::trim).collect();

    for segment in &segments {
        let lower = segment.to_ascii_lowercase();
        if lower.starts_with("filename*=") {
            let encoded = segment.split_once('=').map(|(_, rhs)| rhs.trim())?;
            let encoded = encoded.trim_matches('"');
            let payload = encoded.split("''").nth(1).unwrap_or(encoded);
            let decoded = percent_decode_lossy(payload);
            let candidate = sanitize_filename(&decoded);
            if !candidate.is_empty() {
                return Some(candidate);
            }
        }
    }

    for segment in &segments {
        let lower = segment.to_ascii_lowercase();
        if lower.starts_with("filename=") {
            let value = segment.split_once('=').map(|(_, rhs)| rhs.trim())?;
            let candidate = sanitize_filename(value.trim_matches('"'));
            if !candidate.is_empty() {
                return Some(candidate);
            }
        }
    }

    None
}

fn filename_from_url(url: &str) -> Option<String> {
    let parsed: http::Uri = url.parse().ok()?;
    let path = parsed.path();
    let last = path.rsplit('/').next()?;
    if last.is_empty() {
        return None;
    }
    Some(sanitize_filename(last))
}

fn suggest_filename(request: &DownloadRequest) -> String {
    if let Some(name) = request
        .suggested_filename
        .as_ref()
        .map(|value| sanitize_filename(value))
        .filter(|value| !value.is_empty())
    {
        return name;
    }
    if let Some(disposition) = request.content_disposition.as_deref()
        && let Some(name) = parse_content_disposition_filename(disposition)
    {
        return name;
    }
    if let Some(name) = filename_from_url(&request.url) {
        return name;
    }
    "download.bin".to_string()
}

fn normalize_request_headers(headers: &[(String, String)]) -> Vec<(String, String)> {
    let mut normalized: Vec<(String, String)> = headers
        .iter()
        .filter_map(|(name, value)| {
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim().to_string();
            if name.is_empty() || value.is_empty() {
                return None;
            }
            Some((name, value))
        })
        .collect();
    normalized.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    normalized
}

fn should_forward_header(name: &str) -> bool {
    !matches!(
        name,
        "accept" | "range" | "if-range" | "referer" | "user-agent" | "cookie"
    )
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn user_cache_download_key(request: &UserCacheDownloadRequest) -> String {
    let mut material = String::new();
    material.push_str(request.url.trim());
    for (name, value) in normalize_request_headers(&request.headers) {
        material.push('\n');
        material.push_str(&name);
        material.push(':');
        material.push_str(&value);
    }
    let digest = digest(&SHA256, material.as_bytes());
    hex_encode(digest.as_ref())
}

fn shared_download_signature(
    request: &UserCacheDownloadRequest,
    behavior: DownloadBehavior,
) -> String {
    let mut material = user_cache_download_key(request);
    material.push('\n');
    material.push_str(&format!(
        "rt={};ct={};mr={};rd={}",
        behavior.request_timeout.as_millis(),
        behavior
            .connect_timeout
            .map(|value| value.as_millis().to_string())
            .unwrap_or_else(|| "none".to_string()),
        behavior.max_retries,
        behavior.retry_delay.as_millis()
    ));
    material
}

pub fn download_request_task_id(request: &UserCacheDownloadRequest) -> String {
    user_cache_download_key(request)
}

fn build_user_cache_download_request(url: String) -> DownloadRequest {
    DownloadRequest {
        url,
        user_agent: None,
        content_disposition: None,
        mime_type: None,
        content_length: None,
        suggested_filename: None,
        source_page_url: None,
        cookie: None,
    }
}

fn stable_target_path_for_key(root_dir: &Path, key: &str, file_name: &str) -> PathBuf {
    let sanitized = sanitize_filename(file_name);
    let ext = Path::new(&sanitized)
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty());
    match ext {
        Some(ext) => root_dir.join(format!("{key}.{ext}")),
        None => root_dir.join(key),
    }
}

fn split_filename(filename: &str) -> (String, Option<String>) {
    if let Some((stem, ext)) = filename.rsplit_once('.')
        && !stem.is_empty()
        && !ext.is_empty()
    {
        return (stem.to_string(), Some(ext.to_string()));
    }
    (filename.to_string(), None)
}

fn resolve_unique_download_path(root: &Path, filename: &str) -> PathBuf {
    let sanitized = sanitize_filename(filename);
    let (stem, ext) = split_filename(&sanitized);
    let mut idx = 0u32;
    loop {
        let candidate_name = if idx == 0 {
            sanitized.clone()
        } else if let Some(ext) = ext.as_ref() {
            format!("{stem} ({idx}).{ext}")
        } else {
            format!("{stem} ({idx})")
        };
        let candidate = root.join(candidate_name);
        if !candidate.exists() {
            return candidate;
        }
        idx += 1;
    }
}

fn part_path_for(target: &Path) -> PathBuf {
    target.with_extension("part")
}

fn parse_content_range_total(value: &str) -> Option<u64> {
    let (_, total) = value.split_once('/')?;
    if total == "*" {
        None
    } else {
        total.parse::<u64>().ok()
    }
}

fn header_to_string(headers: &http::HeaderMap, name: http::header::HeaderName) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string())
}

async fn load_resume_metadata(task: &DownloadTask) -> ResumeMetadata {
    let Some(persistence) = task.persistence.as_ref() else {
        return ResumeMetadata::default();
    };
    crate::download::load_resume_metadata(&persistence.app_data_dir, &persistence.task_id)
        .ok()
        .flatten()
        .unwrap_or_default()
}

async fn save_resume_metadata(task: &DownloadTask, meta: &ResumeMetadata) {
    let Some(persistence) = task.persistence.as_ref() else {
        return;
    };
    let _ = crate::download::save_resume_metadata(
        &persistence.app_data_dir,
        &persistence.task_id,
        meta.clone(),
    );
}

async fn clear_resume_metadata(task: &DownloadTask) {
    let Some(persistence) = task.persistence.as_ref() else {
        return;
    };
    let _ = crate::download::clear_resume_metadata(&persistence.app_data_dir, &persistence.task_id);
}

fn track_persistent_record(task: &DownloadTask) -> Option<&DownloadPersistence> {
    task.persistence
        .as_ref()
        .filter(|persistence| persistence.track_record)
}

fn record_started_for_persistence(
    task: &DownloadTask,
    file_name: &str,
    target_path: &Path,
    mime_type: Option<&str>,
    total_bytes: Option<u64>,
    resumed_bytes: u64,
) {
    let Some(persistence) = track_persistent_record(task) else {
        return;
    };
    if let Err(err) = crate::download::record_managed_download_started(
        &persistence.app_data_dir,
        &persistence.task_id,
        persistence.owner.clone(),
        &task.request.url,
        file_name,
        target_path,
        mime_type,
        total_bytes,
        resumed_bytes,
        task.request_headers.clone(),
        task.fallback_user_agent.clone(),
        task.behavior,
    ) {
        log::warn!(
            "[DownloadManager] failed to persist started task_id={} url={} error={}",
            persistence.task_id,
            task.request.url,
            err
        );
    }
}

fn record_paused_for_persistence(
    task: &DownloadTask,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
) {
    let Some(persistence) = track_persistent_record(task) else {
        return;
    };
    if let Err(err) = crate::download::record_managed_download_paused(
        &persistence.app_data_dir,
        &persistence.task_id,
        downloaded_bytes,
        total_bytes,
    ) {
        log::warn!(
            "[DownloadManager] failed to persist paused task_id={} url={} error={}",
            persistence.task_id,
            task.request.url,
            err
        );
    }
}

fn record_progress_for_persistence(
    task: &DownloadTask,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
) {
    let Some(persistence) = track_persistent_record(task) else {
        return;
    };
    if let Err(err) = crate::download::record_managed_download_progress(
        &persistence.app_data_dir,
        &persistence.task_id,
        downloaded_bytes,
        total_bytes,
    ) {
        log::warn!(
            "[DownloadManager] failed to persist progress task_id={} url={} error={}",
            persistence.task_id,
            task.request.url,
            err
        );
    }
}

fn record_completed_for_persistence(
    task: &DownloadTask,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
) {
    let Some(persistence) = track_persistent_record(task) else {
        return;
    };
    if let Err(err) = crate::download::record_managed_download_completed(
        &persistence.app_data_dir,
        &persistence.task_id,
        downloaded_bytes,
        total_bytes,
    ) {
        log::warn!(
            "[DownloadManager] failed to persist completed task_id={} url={} error={}",
            persistence.task_id,
            task.request.url,
            err
        );
    }
}

fn record_failed_for_persistence(
    task: &DownloadTask,
    error: &str,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
) {
    let Some(persistence) = track_persistent_record(task) else {
        return;
    };
    if let Err(err) = crate::download::record_managed_download_failed(
        &persistence.app_data_dir,
        &persistence.task_id,
        error,
        downloaded_bytes,
        total_bytes,
    ) {
        log::warn!(
            "[DownloadManager] failed to persist failed task_id={} url={} error={}",
            persistence.task_id,
            task.request.url,
            err
        );
    }
}

fn emit_failed(
    on_event: &mut impl FnMut(DownloadEvent),
    url: String,
    error: String,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
) -> DownloadFailure {
    let failure = DownloadFailure {
        url: url.clone(),
        error: error.clone(),
        downloaded_bytes,
        total_bytes,
    };
    on_event(DownloadEvent::Failed {
        url,
        error,
        downloaded_bytes,
        total_bytes,
    });
    failure
}

async fn write_chunk(
    task: &DownloadTask,
    file: &mut tokio::fs::File,
    chunk: &[u8],
    progress: &mut DownloadProgress,
    total_bytes: Option<u64>,
    resume_meta: &mut ResumeMetadata,
    url: &str,
    on_event: &mut impl FnMut(DownloadEvent),
) -> Result<(), String> {
    file.write_all(chunk)
        .await
        .map_err(|e| format!("write chunk failed: {}", e))?;
    progress.downloaded += chunk.len() as u64;
    resume_meta.downloaded = progress.downloaded;

    let mut should_emit = !progress.emitted_once;
    if let Some(total) = total_bytes
        && total > 0
    {
        let percent = ((progress.downloaded as f64 / total as f64) * 100.0) as i32;
        if percent > progress.last_percent {
            progress.last_percent = percent;
            should_emit = true;
        }
    }

    let now = Instant::now();
    let advanced_since_emit = progress.downloaded.saturating_sub(progress.last_emitted);
    if !should_emit
        && (advanced_since_emit >= DOWNLOAD_PROGRESS_INTERVAL_BYTES
            || now.duration_since(progress.last_emit_at).as_millis()
                >= DOWNLOAD_PROGRESS_INTERVAL_MILLIS)
    {
        should_emit = true;
    }

    if should_emit {
        progress.emitted_once = true;
        progress.last_emitted = progress.downloaded;
        progress.last_emit_at = now;
        on_event(DownloadEvent::Progress {
            url: url.to_string(),
            downloaded_bytes: progress.downloaded,
            total_bytes,
        });
        record_progress_for_persistence(task, progress.downloaded, total_bytes);
    }

    if progress.downloaded.saturating_sub(progress.last_persisted)
        >= DOWNLOAD_RESUME_METADATA_INTERVAL_BYTES
    {
        progress.last_persisted = progress.downloaded;
        save_resume_metadata(task, resume_meta).await;
    }
    Ok(())
}

async fn run_download_attempt(
    task: &DownloadTask,
    filename: &str,
    target_path: &Path,
    part_path: &Path,
    cancel_rx: &mut Option<watch::Receiver<crate::download::ActiveDownloadCommand>>,
    started: &mut bool,
    on_event: &mut impl FnMut(DownloadEvent),
) -> Result<DownloadSuccess, DownloadAttemptFailure> {
    let request = task.request.clone();
    let request_headers = task.request_headers.clone();

    let mut resume_meta = load_resume_metadata(task).await;
    let mut resume_offset = fs::metadata(part_path)
        .await
        .ok()
        .map(|meta| meta.len())
        .unwrap_or(0);

    let has_resume_validator = resume_meta.etag.is_some() || resume_meta.last_modified.is_some();
    if resume_offset > 0 && !has_resume_validator {
        let _ = fs::remove_file(part_path).await;
        resume_offset = 0;
        resume_meta = ResumeMetadata::default();
    }

    let mut request_builder = HttpRequest::builder()
        .method("GET")
        .uri(&request.url)
        .header(header::ACCEPT, "*/*");

    if let Some(headers) = request_builder.headers_mut() {
        let ua = request
            .user_agent
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .or(task
                .fallback_user_agent
                .as_deref()
                .filter(|value| !value.trim().is_empty()));
        if let Some(ua) = ua
            && let Ok(ua_value) = http::HeaderValue::from_str(ua)
        {
            headers.insert(header::USER_AGENT, ua_value);
        }
        if let Some(cookie) = request
            .cookie
            .as_ref()
            .filter(|value| !value.trim().is_empty())
            && let Ok(cookie_value) = http::HeaderValue::from_str(cookie)
        {
            headers.insert(header::COOKIE, cookie_value);
        }
        for (name, value) in request_headers {
            let normalized = name.trim().to_ascii_lowercase();
            if !should_forward_header(&normalized) {
                continue;
            }
            let header_name = match http::header::HeaderName::from_bytes(normalized.as_bytes()) {
                Ok(name) => name,
                Err(_) => continue,
            };
            let header_value = match http::HeaderValue::from_str(value.trim()) {
                Ok(value) => value,
                Err(_) => continue,
            };
            headers.insert(header_name, header_value);
        }
        if resume_offset > 0 {
            if let Ok(range_value) = http::HeaderValue::from_str(&format!("bytes={resume_offset}-"))
            {
                headers.insert(header::RANGE, range_value);
            }
            if let Some(validator) = resume_meta
                .etag
                .as_ref()
                .or(resume_meta.last_modified.as_ref())
                && let Ok(if_range) = http::HeaderValue::from_str(validator)
            {
                headers.insert(header::IF_RANGE, if_range);
            }
        }
    }

    let body = Full::new(Bytes::new())
        .map_err(|_| IoError::other("body error"))
        .boxed();
    let request_obj = match request_builder.body(body) {
        Ok(req) => req,
        Err(e) => {
            return Err(DownloadAttemptFailure {
                error: format!("build request failed: {}", e),
                downloaded_bytes: resume_offset,
                total_bytes: None,
                retryable: false,
            });
        }
    };

    let response = match host_http::send_with_small_body_limit(
        request_obj,
        DOWNLOAD_SMALL_BODY_LIMIT,
        download_request_options(task.behavior),
    )
    .await
    {
        Ok(response) => response,
        Err(e) => {
            let error = e.to_string();
            return Err(DownloadAttemptFailure {
                retryable: is_retryable_transport_error(&error),
                error,
                downloaded_bytes: resume_offset,
                total_bytes: None,
            });
        }
    };

    if !response.status.is_success() {
        let status = response.status.as_u16();
        return Err(DownloadAttemptFailure {
            error: format!("http status {status}"),
            downloaded_bytes: resume_offset,
            total_bytes: None,
            retryable: is_retryable_http_status(status),
        });
    }

    let append_mode = resume_offset > 0 && response.status.as_u16() == 206;
    if !append_mode {
        resume_offset = 0;
    }

    let mut file = match OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(!append_mode)
        .append(append_mode)
        .open(part_path)
        .await
    {
        Ok(file) => file,
        Err(e) => {
            return Err(DownloadAttemptFailure {
                error: format!("open temp file failed: {}", e),
                downloaded_bytes: resume_offset,
                total_bytes: None,
                retryable: false,
            });
        }
    };

    let content_range_total = header_to_string(&response.headers, header::CONTENT_RANGE)
        .and_then(|value| parse_content_range_total(&value));
    let content_len_header = header_to_string(&response.headers, header::CONTENT_LENGTH)
        .and_then(|value| value.parse::<u64>().ok());
    let total_bytes = content_range_total.or_else(|| {
        content_len_header.map(|value| {
            if append_mode {
                resume_offset + value
            } else {
                value
            }
        })
    });
    let etag = header_to_string(&response.headers, header::ETAG);
    let last_modified = header_to_string(&response.headers, header::LAST_MODIFIED);

    resume_meta.etag = etag;
    resume_meta.last_modified = last_modified;
    resume_meta.total = total_bytes;
    resume_meta.downloaded = resume_offset;
    save_resume_metadata(task, &resume_meta).await;

    // Started must fire exactly once per top-level download call, even if the
    // stream reconnects several times under the hood. Upper layers (browser JS,
    // lxapp UI) treat a second Started as a new download and would reset their
    // progress indicators. Subsequent attempts only surface progress events.
    if !*started {
        *started = true;
        on_event(DownloadEvent::Started {
            url: request.url.clone(),
            file_name: filename.to_string(),
            target_path: target_path.to_path_buf(),
            mime_type: request.mime_type.clone(),
            total_bytes,
            resumed_bytes: resume_offset,
        });
        record_started_for_persistence(
            task,
            filename,
            target_path,
            request.mime_type.as_deref(),
            total_bytes,
            resume_offset,
        );
    }

    let mut progress = DownloadProgress {
        downloaded: resume_offset,
        last_emitted: resume_offset,
        last_persisted: resume_offset,
        last_percent: if let Some(total) = total_bytes {
            if total > 0 {
                ((resume_offset as f64 / total as f64) * 100.0) as i32
            } else {
                0
            }
        } else {
            0
        },
        emitted_once: false,
        last_emit_at: Instant::now(),
    };

    let stream_result = match response.body {
        HttpBody::Empty => Ok(()),
        HttpBody::Small(bytes) => {
            write_chunk(
                task,
                &mut file,
                bytes.as_ref(),
                &mut progress,
                total_bytes,
                &mut resume_meta,
                &request.url,
                on_event,
            )
            .await
        }
        HttpBody::Stream(mut rx) => {
            let mut result = Ok(());
            loop {
                let next_chunk = if let Some(cancel) = cancel_rx.as_mut() {
                    match *cancel.borrow() {
                        crate::download::ActiveDownloadCommand::Pause => {
                            result = Err(DOWNLOAD_PAUSED_ERROR.to_string());
                            break;
                        }
                        crate::download::ActiveDownloadCommand::Cancel => {
                            result = Err(DOWNLOAD_CANCELED_ERROR.to_string());
                            break;
                        }
                        crate::download::ActiveDownloadCommand::None => {}
                    }
                    tokio::select! {
                        biased;
                        changed = cancel.changed() => {
                            if changed.is_ok() {
                                match *cancel.borrow() {
                                    crate::download::ActiveDownloadCommand::Pause => {
                                        result = Err(DOWNLOAD_PAUSED_ERROR.to_string());
                                        break;
                                    }
                                    crate::download::ActiveDownloadCommand::Cancel => {
                                        result = Err(DOWNLOAD_CANCELED_ERROR.to_string());
                                        break;
                                    }
                                    crate::download::ActiveDownloadCommand::None => {}
                                }
                            }
                            continue;
                        }
                        chunk = rx.recv() => chunk,
                    }
                } else {
                    rx.recv().await
                };

                let Some(chunk) = next_chunk else {
                    break;
                };
                match chunk {
                    Ok(bytes) => {
                        if let Err(e) = write_chunk(
                            task,
                            &mut file,
                            bytes.as_ref(),
                            &mut progress,
                            total_bytes,
                            &mut resume_meta,
                            &request.url,
                            on_event,
                        )
                        .await
                        {
                            result = Err(e);
                            break;
                        }
                    }
                    Err(e) => {
                        result = Err(e);
                        break;
                    }
                }
            }
            result
        }
    };

    if progress.downloaded > progress.last_emitted {
        progress.last_emitted = progress.downloaded;
        progress.last_emit_at = Instant::now();
        save_resume_metadata(task, &resume_meta).await;
        on_event(DownloadEvent::Progress {
            url: request.url.clone(),
            downloaded_bytes: progress.downloaded,
            total_bytes,
        });
        record_progress_for_persistence(task, progress.downloaded, total_bytes);
    }

    if let Err(error) = stream_result {
        let _ = file.flush().await;
        save_resume_metadata(task, &resume_meta).await;
        return Err(DownloadAttemptFailure {
            retryable: error != DOWNLOAD_CANCELED_ERROR
                && error != DOWNLOAD_PAUSED_ERROR
                && is_retryable_transport_error(&error),
            error,
            downloaded_bytes: progress.downloaded,
            total_bytes,
        });
    }

    if let Err(e) = file.flush().await {
        save_resume_metadata(task, &resume_meta).await;
        return Err(DownloadAttemptFailure {
            error: format!("flush failed: {}", e),
            downloaded_bytes: progress.downloaded,
            total_bytes,
            retryable: false,
        });
    }
    drop(file);

    if !task.reuse_existing_target
        && fs::try_exists(target_path).await.unwrap_or(false)
        && let Err(e) = fs::remove_file(target_path).await
        && e.kind() != std::io::ErrorKind::NotFound
    {
        save_resume_metadata(task, &resume_meta).await;
        return Err(DownloadAttemptFailure {
            error: format!("remove existing target failed: {}", e),
            downloaded_bytes: progress.downloaded,
            total_bytes,
            retryable: false,
        });
    }

    if let Err(e) = fs::rename(part_path, target_path).await {
        save_resume_metadata(task, &resume_meta).await;
        return Err(DownloadAttemptFailure {
            error: format!("rename failed: {}", e),
            downloaded_bytes: progress.downloaded,
            total_bytes,
            retryable: false,
        });
    }

    clear_resume_metadata(task).await;
    record_completed_for_persistence(task, progress.downloaded, total_bytes);
    let success = DownloadSuccess {
        file_name: filename.to_string(),
        path: target_path.to_path_buf(),
        downloaded_bytes: progress.downloaded,
    };
    on_event(DownloadEvent::Completed {
        url: request.url,
        file_name: filename.to_string(),
        path: target_path.to_path_buf(),
        downloaded_bytes: progress.downloaded,
        total_bytes,
    });
    Ok(success)
}

async fn run_download_task(
    task: DownloadTask,
    mut cancel_rx: Option<watch::Receiver<crate::download::ActiveDownloadCommand>>,
    mut on_event: impl FnMut(DownloadEvent),
) -> Result<DownloadSuccess, DownloadFailure> {
    let request = task.request.clone();
    let filename = suggest_filename(&request);
    let target_path = task
        .target_path
        .clone()
        .unwrap_or_else(|| resolve_unique_download_path(&task.root_dir, &filename));
    let part_path = part_path_for(&target_path);
    let url = request.url.clone();

    if let Err(e) = fs::create_dir_all(&task.root_dir).await {
        return Err(emit_failed(
            &mut on_event,
            url,
            format!("create download dir failed: {}", e),
            0,
            None,
        ));
    }

    if task.reuse_existing_target
        && fs::try_exists(&target_path).await.unwrap_or(false)
        && !fs::try_exists(&part_path).await.unwrap_or(false)
    {
        let size = fs::metadata(&target_path)
            .await
            .ok()
            .map(|meta| meta.len())
            .unwrap_or(0);
        return Ok(DownloadSuccess {
            file_name: filename.clone(),
            path: target_path.clone(),
            downloaded_bytes: size,
        });
    }

    let mut started = false;
    let mut attempt = 0u32;

    loop {
        match run_download_attempt(
            &task,
            &filename,
            &target_path,
            &part_path,
            &mut cancel_rx,
            &mut started,
            &mut on_event,
        )
        .await
        {
            Ok(success) => return Ok(success),
            Err(failure) => {
                if failure.error == DOWNLOAD_PAUSED_ERROR {
                    record_paused_for_persistence(
                        &task,
                        failure.downloaded_bytes,
                        failure.total_bytes,
                    );
                    on_event(DownloadEvent::Paused {
                        url: request.url.clone(),
                        downloaded_bytes: failure.downloaded_bytes,
                        total_bytes: failure.total_bytes,
                    });
                    return Err(DownloadFailure {
                        url: request.url.clone(),
                        error: failure.error,
                        downloaded_bytes: failure.downloaded_bytes,
                        total_bytes: failure.total_bytes,
                    });
                }

                let should_retry = failure.retryable && attempt < task.behavior.max_retries;
                if should_retry {
                    let delay = retry_delay_for_attempt(task.behavior.retry_delay, attempt);
                    attempt += 1;
                    log::warn!(
                        "[DownloadManager] retrying download attempt={} url={} downloaded_bytes={} reason={}",
                        attempt,
                        request.url,
                        failure.downloaded_bytes,
                        failure.error
                    );
                    if let Err(error) = sleep_for_retry(cancel_rx.as_mut(), delay).await {
                        if error == DOWNLOAD_PAUSED_ERROR {
                            record_paused_for_persistence(
                                &task,
                                failure.downloaded_bytes,
                                failure.total_bytes,
                            );
                            on_event(DownloadEvent::Paused {
                                url: request.url.clone(),
                                downloaded_bytes: failure.downloaded_bytes,
                                total_bytes: failure.total_bytes,
                            });
                            return Err(DownloadFailure {
                                url: request.url.clone(),
                                error,
                                downloaded_bytes: failure.downloaded_bytes,
                                total_bytes: failure.total_bytes,
                            });
                        }
                        record_failed_for_persistence(
                            &task,
                            &error,
                            failure.downloaded_bytes,
                            failure.total_bytes,
                        );
                        return Err(emit_failed(
                            &mut on_event,
                            request.url.clone(),
                            error,
                            failure.downloaded_bytes,
                            failure.total_bytes,
                        ));
                    }
                    continue;
                }

                record_failed_for_persistence(
                    &task,
                    &failure.error,
                    failure.downloaded_bytes,
                    failure.total_bytes,
                );
                return Err(emit_failed(
                    &mut on_event,
                    request.url.clone(),
                    failure.error,
                    failure.downloaded_bytes,
                    failure.total_bytes,
                ));
            }
        }
    }
}

fn map_browser_download_event(
    task_id: &str,
    tab_id: &str,
    request: &DownloadRequest,
    behavior: DownloadBehavior,
    event: DownloadEvent,
) -> (&'static str, serde_json::Value) {
    match event {
        DownloadEvent::Started {
            url,
            file_name,
            target_path,
            mime_type,
            total_bytes,
            resumed_bytes,
        } => (
            BROWSER_DOWNLOAD_EVENT_STARTED,
            json!({
                "taskId": task_id,
                "tabId": tab_id,
                "url": url,
                "fileName": file_name,
                "targetPath": target_path.to_string_lossy(),
                "mimeType": mime_type,
                "totalBytes": total_bytes,
                "resumedBytes": resumed_bytes,
                "userAgent": request.user_agent,
                "suggestedFilename": request.suggested_filename,
                "sourcePageUrl": request.source_page_url,
                "cookie": request.cookie,
                "behavior": behavior,
            }),
        ),
        DownloadEvent::Progress {
            url,
            downloaded_bytes,
            total_bytes,
        } => (
            BROWSER_DOWNLOAD_EVENT_PROGRESS,
            json!({
                "taskId": task_id,
                "tabId": tab_id,
                "url": url,
                "downloadedBytes": downloaded_bytes,
                "totalBytes": total_bytes,
            }),
        ),
        DownloadEvent::Completed {
            url,
            file_name,
            path,
            downloaded_bytes,
            total_bytes,
        } => (
            BROWSER_DOWNLOAD_EVENT_COMPLETED,
            json!({
                "taskId": task_id,
                "tabId": tab_id,
                "url": url,
                "path": path.to_string_lossy(),
                "fileName": file_name,
                "downloadedBytes": downloaded_bytes,
                "totalBytes": total_bytes,
            }),
        ),
        DownloadEvent::Failed {
            url,
            error,
            downloaded_bytes,
            total_bytes,
        } => (
            BROWSER_DOWNLOAD_EVENT_FAILED,
            json!({
                "taskId": task_id,
                "tabId": tab_id,
                "url": url,
                "error": error,
                "downloadedBytes": downloaded_bytes,
                "totalBytes": total_bytes,
            }),
        ),
        DownloadEvent::Paused {
            url,
            downloaded_bytes,
            total_bytes,
        } => (
            BROWSER_DOWNLOAD_EVENT_PAUSED,
            json!({
                "taskId": task_id,
                "tabId": tab_id,
                "url": url,
                "downloadedBytes": downloaded_bytes,
                "totalBytes": total_bytes,
            }),
        ),
    }
}

pub async fn run_browser_download_task(
    task: DownloadTask,
    task_id: &str,
    tab_id: &str,
    cancel_rx: watch::Receiver<crate::download::ActiveDownloadCommand>,
    mut on_event: impl FnMut(&'static str, serde_json::Value),
) -> Result<(), DownloadFailure> {
    let task_id = task_id.to_string();
    let tab_id = tab_id.to_string();
    let request = task.request.clone();
    let behavior = task.behavior;
    run_download_task(task, Some(cancel_rx), |event| {
        let (event_name, payload) =
            map_browser_download_event(&task_id, &tab_id, &request, behavior, event);
        on_event(event_name, payload);
    })
    .await
    .map(|_| ())
}

fn wait_shared_download_error(url: &str) -> DownloadFailure {
    DownloadFailure {
        url: url.to_string(),
        error: "download channel closed unexpectedly".to_string(),
        downloaded_bytes: 0,
        total_bytes: None,
    }
}

async fn wait_shared_download(
    mut rx: watch::Receiver<SharedDownloadState>,
    url: &str,
) -> Result<DownloadSuccess, DownloadFailure> {
    loop {
        match rx.borrow().clone() {
            SharedDownloadState::InProgress => {}
            SharedDownloadState::Completed(success) => return Ok(success),
            SharedDownloadState::Failed(failure) => return Err(failure),
        }
        if rx.changed().await.is_err() {
            return Err(wait_shared_download_error(url));
        }
    }
}

fn sanitize_header_list(headers: Vec<(String, String)>) -> Vec<(String, String)> {
    normalize_request_headers(&headers)
        .into_iter()
        .map(|(name, value)| (name, value.trim().to_string()))
        .collect()
}

struct SharedDownloadLeaderGuard {
    key: String,
    active: bool,
}

impl SharedDownloadLeaderGuard {
    fn new(key: String) -> Self {
        Self { key, active: true }
    }

    fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for SharedDownloadLeaderGuard {
    fn drop(&mut self) {
        if self.active {
            user_cache_downloads().remove(&self.key);
        }
    }
}

struct SharedTargetPathDownloadLeaderGuard {
    key: String,
    active: bool,
}

impl SharedTargetPathDownloadLeaderGuard {
    fn new(key: String) -> Self {
        Self { key, active: true }
    }

    fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for SharedTargetPathDownloadLeaderGuard {
    fn drop(&mut self) {
        if self.active {
            target_path_downloads().remove(&self.key);
        }
    }
}

pub async fn download_to_user_cache(
    persistence: Option<DownloadPersistence>,
    user_cache_dir: &Path,
    user_request: UserCacheDownloadRequest,
    fallback_user_agent: Option<String>,
    on_event: impl FnMut(DownloadEvent) + Send,
) -> Result<UserCacheDownloadResult, DownloadFailure> {
    download_to_user_cache_with_behavior(
        persistence,
        user_cache_dir,
        user_request,
        fallback_user_agent,
        DownloadBehavior::default(),
        on_event,
    )
    .await
}

pub async fn download_to_user_cache_with_behavior(
    persistence: Option<DownloadPersistence>,
    user_cache_dir: &Path,
    user_request: UserCacheDownloadRequest,
    fallback_user_agent: Option<String>,
    behavior: DownloadBehavior,
    mut on_event: impl FnMut(DownloadEvent) + Send,
) -> Result<UserCacheDownloadResult, DownloadFailure> {
    let url = user_request.url.trim().to_string();
    if url.is_empty() {
        return Err(DownloadFailure {
            url,
            error: "download url cannot be empty".to_string(),
            downloaded_bytes: 0,
            total_bytes: None,
        });
    }

    let key = user_cache_download_key(&user_request);
    let root_dir = user_cache_download_root(user_cache_dir);
    let request = build_user_cache_download_request(url.clone());
    let file_name = suggest_filename(&request);
    let target_path = stable_target_path_for_key(&root_dir, &key, &file_name);
    let part_path = part_path_for(&target_path);
    if fs::try_exists(&target_path).await.unwrap_or(false)
        && !fs::try_exists(&part_path).await.unwrap_or(false)
    {
        let size = fs::metadata(&target_path)
            .await
            .ok()
            .map(|meta| meta.len())
            .unwrap_or(0);
        return Ok(UserCacheDownloadResult {
            temp_path: target_path,
            file_name,
            mime_type: None,
            size,
        });
    }

    let share_signature = shared_download_signature(&user_request, behavior);
    let should_share = persistence.is_none();
    let tracker = user_cache_downloads();
    let mut leader = false;
    let rx = match tracker.entry(key.clone()) {
        DashEntry::Occupied(entry) => {
            if !should_share {
                return Err(DownloadFailure {
                    url,
                    error: "download is already active".to_string(),
                    downloaded_bytes: 0,
                    total_bytes: None,
                });
            }
            if entry.get().signature != share_signature {
                return Err(DownloadFailure {
                    url,
                    error: "download is already active with different behavior".to_string(),
                    downloaded_bytes: 0,
                    total_bytes: None,
                });
            }
            Some(entry.get().tx.subscribe())
        }
        DashEntry::Vacant(entry) => {
            let (tx, rx) = watch::channel(SharedDownloadState::InProgress);
            entry.insert(SharedDownloadRegistration {
                signature: share_signature,
                tx,
            });
            leader = true;
            if should_share { Some(rx) } else { None }
        }
    };

    if should_share && !leader {
        let shared = wait_shared_download(
            rx.expect("shared user-cache download receiver missing"),
            &url,
        )
        .await?;
        return Ok(UserCacheDownloadResult {
            temp_path: shared.path,
            file_name: shared.file_name,
            mime_type: None,
            size: shared.downloaded_bytes,
        });
    }

    let mut leader_guard = Some(SharedDownloadLeaderGuard::new(key.clone()));

    let persistence_task_id = persistence.as_ref().map(|value| value.task_id.clone());
    let cancel_rx = persistence_task_id
        .as_deref()
        .map(crate::download::register_active_download);

    let mut task = DownloadTask::for_browser(request, root_dir, fallback_user_agent)
        .with_target_path(target_path);
    task.request_headers = sanitize_header_list(user_request.headers);
    task = task.with_behavior(behavior);
    if let Some(persistence) = persistence {
        task = task.with_persistence(persistence);
    }
    let result = run_download_task(task, cancel_rx, |event| {
        on_event(event);
    })
    .await;

    if let Some(task_id) = persistence_task_id.as_deref() {
        crate::download::unregister_active_download(task_id);
    }

    if should_share && let Some(entry) = tracker.get(&key) {
        match &result {
            Ok(success) => {
                let _ = entry
                    .tx
                    .send(SharedDownloadState::Completed(success.clone()));
            }
            Err(failure) => {
                let _ = entry.tx.send(SharedDownloadState::Failed(failure.clone()));
            }
        }
    }
    tracker.remove(&key);
    if let Some(guard) = leader_guard.as_mut() {
        guard.disarm();
    }

    result.map(|success| UserCacheDownloadResult {
        temp_path: success.path,
        file_name: success.file_name,
        mime_type: None,
        size: success.downloaded_bytes,
    })
}

pub async fn download_to_path_with_behavior(
    persistence: Option<DownloadPersistence>,
    target_path: PathBuf,
    user_request: UserCacheDownloadRequest,
    fallback_user_agent: Option<String>,
    behavior: DownloadBehavior,
    mut on_event: impl FnMut(DownloadEvent) + Send,
) -> Result<UserCacheDownloadResult, DownloadFailure> {
    let url = user_request.url.trim().to_string();
    if url.is_empty() {
        return Err(DownloadFailure {
            url,
            error: "download url cannot be empty".to_string(),
            downloaded_bytes: 0,
            total_bytes: None,
        });
    }

    let root_dir = target_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let share_signature = shared_download_signature(&user_request, behavior);
    let path_key = target_path.to_string_lossy().into_owned();
    let tracker = target_path_downloads();
    let should_share = persistence.is_none();
    let mut leader = false;
    let rx = match tracker.entry(path_key.clone()) {
        DashEntry::Occupied(entry) => {
            if !should_share || entry.get().signature != share_signature {
                return Err(DownloadFailure {
                    url,
                    error: format!(
                        "download target path is already in use: {}",
                        target_path.display()
                    ),
                    downloaded_bytes: 0,
                    total_bytes: None,
                });
            }
            Some(entry.get().tx.subscribe())
        }
        DashEntry::Vacant(entry) => {
            let (tx, rx) = watch::channel(SharedDownloadState::InProgress);
            entry.insert(SharedTargetPathDownload {
                signature: share_signature,
                tx,
            });
            leader = true;
            if should_share { Some(rx) } else { None }
        }
    };

    if should_share && !leader {
        let shared = wait_shared_download(
            rx.expect("shared target-path download receiver missing"),
            &url,
        )
        .await?;
        return Ok(UserCacheDownloadResult {
            temp_path: shared.path,
            file_name: shared.file_name,
            mime_type: None,
            size: shared.downloaded_bytes,
        });
    }

    let mut leader_guard = Some(SharedTargetPathDownloadLeaderGuard::new(path_key.clone()));
    let request = build_user_cache_download_request(url.clone());
    let mut task = DownloadTask::for_browser(request, root_dir, fallback_user_agent)
        .with_target_path(target_path)
        .with_reuse_existing_target(false);
    task.request_headers = sanitize_header_list(user_request.headers);
    task = task.with_behavior(behavior);
    let persistence_task_id = persistence.as_ref().map(|value| value.task_id.clone());
    let cancel_rx = persistence_task_id
        .as_deref()
        .map(crate::download::register_active_download);
    if let Some(persistence) = persistence {
        task = task.with_persistence(persistence);
    }

    let result = run_download_task(task, cancel_rx, |event| {
        on_event(event);
    })
    .await;

    if let Some(task_id) = persistence_task_id.as_deref() {
        crate::download::unregister_active_download(task_id);
    }

    if should_share && let Some(entry) = tracker.get(&path_key) {
        match &result {
            Ok(success) => {
                let _ = entry
                    .tx
                    .send(SharedDownloadState::Completed(success.clone()));
            }
            Err(failure) => {
                let _ = entry.tx.send(SharedDownloadState::Failed(failure.clone()));
            }
        }
    }
    tracker.remove(&path_key);
    if let Some(guard) = leader_guard.as_mut() {
        guard.disarm();
    }

    result.map(|success| UserCacheDownloadResult {
        temp_path: success.path,
        file_name: success.file_name,
        mime_type: None,
        size: success.downloaded_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_filename_rejects_dot_names() {
        assert_eq!(sanitize_filename(""), "download.bin");
        assert_eq!(sanitize_filename("."), "download.bin");
        assert_eq!(sanitize_filename(".."), "download.bin");
        assert_eq!(sanitize_filename("a/b:c"), "a_b_c");
    }

    #[test]
    fn parse_content_disposition_filename_works() {
        let star = "attachment; filename*=UTF-8''hello%20world.txt";
        assert_eq!(
            parse_content_disposition_filename(star),
            Some("hello world.txt".to_string())
        );

        let normal = "attachment; filename=\"demo.bin\"";
        assert_eq!(
            parse_content_disposition_filename(normal),
            Some("demo.bin".to_string())
        );
    }

    #[test]
    fn retryable_transport_errors_cover_weak_network_cases() {
        assert!(is_retryable_transport_error("request timeout"));
        assert!(is_retryable_transport_error("read timeout"));
        assert!(is_retryable_transport_error(
            "request failed: connection reset by peer"
        ));
        assert!(is_retryable_transport_error("read frame: unexpected eof"));
        assert!(is_retryable_transport_error(
            "request failed: connect error: tcp connect error"
        ));
        assert!(is_retryable_transport_error(
            "request failed: tls handshake eof"
        ));
        assert!(is_retryable_transport_error(
            "read frame: body stream closed"
        ));
        assert!(is_retryable_transport_error("http2 refused stream"));
        assert!(is_retryable_transport_error("http2 goaway received"));
        assert!(is_retryable_transport_error("dns lookup failed"));
        assert!(!is_retryable_transport_error("download canceled"));
        assert!(!is_retryable_transport_error("http status 404"));
        assert!(is_retryable_transport_error("connection aborted"));
    }

    #[test]
    fn retryable_http_statuses_match_transient_failures() {
        assert!(is_retryable_http_status(408));
        assert!(is_retryable_http_status(429));
        assert!(is_retryable_http_status(503));
        assert!(!is_retryable_http_status(404));
    }

    #[test]
    fn download_task_with_behavior_overrides_defaults() {
        let request = build_user_cache_download_request("https://example.com/file".to_string());
        let task = DownloadTask::for_browser(request, PathBuf::from("/tmp"), None);
        assert_eq!(task.behavior, DownloadBehavior::default());

        let custom = DownloadBehavior {
            request_timeout: Duration::from_secs(30),
            connect_timeout: Some(Duration::from_secs(5)),
            max_retries: 0,
            retry_delay: Duration::from_millis(100),
        };
        let tuned = task.with_behavior(custom);
        assert_eq!(tuned.behavior, custom);
        assert_eq!(tuned.behavior.max_retries, 0);
    }

    #[test]
    fn retry_delay_is_exponential_and_capped() {
        assert_eq!(
            retry_delay_for_attempt(Duration::from_millis(750), 0),
            Duration::from_millis(750)
        );
        assert_eq!(
            retry_delay_for_attempt(Duration::from_millis(750), 1),
            Duration::from_millis(1500)
        );
        assert_eq!(
            retry_delay_for_attempt(Duration::from_millis(750), 3),
            Duration::from_millis(5000)
        );
        assert_eq!(
            retry_delay_for_attempt(Duration::from_millis(750), 6),
            Duration::from_millis(5000)
        );
    }
}
