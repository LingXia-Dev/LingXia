use super::{
    network_security, normalize_relative_path,
    storage::{self, StorageQuotaError},
};
use crate::i18n::{
    js_error_from_business_code_with_detail, js_error_from_lxapp_error, js_internal_error,
    js_invalid_parameter_error,
};
use futures::channel::{mpsc, oneshot};
use futures::lock::Mutex;
use futures::{SinkExt, StreamExt};
use lingxia_transfer::user_cache::{
    self, DownloadBehavior, DownloadEvent as TransferDownloadEvent, DownloadFailure,
    DownloadFailureKind,
};
use lxapp::{LxApp, LxAppSecurityPrivilege, lx};
use rong::{
    HostError, IntoJSObj, JSContext, JSFunc, JSObject, JSResult, JSSymbol, JSValue, Promise,
    function::Optional,
};
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
struct ParsedDownloadOptions {
    url: String,
    headers: Vec<(String, String)>,
    timeout_ms: Option<u64>,
    file_path: Option<String>,
    destination: DownloadDestination,
    signal: Option<JSObject>,
}

#[derive(Debug, Clone, IntoJSObj)]
struct JSDownloadResult {
    #[rename = "tempFilePath"]
    temp_file_path: Option<String>,
    #[rename = "filePath"]
    file_path: Option<String>,
    #[rename = "mimeType"]
    mime_type: Option<String>,
    size: u64,
}

#[derive(Debug, Clone, IntoJSObj)]
struct JSDownloadEvent {
    kind: String,
    #[rename = "downloadedBytes"]
    downloaded_bytes: Option<u64>,
    #[rename = "totalBytes"]
    total_bytes: Option<u64>,
    progress: Option<f64>,
    result: Option<JSDownloadResult>,
}

#[derive(Debug, Clone, IntoJSObj)]
struct JSDownloadIteratorStep {
    done: bool,
    value: Option<JSDownloadEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DownloadTaskStatus {
    Running,
    Paused,
    Canceled,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DownloadDestination {
    App,
    Downloads,
}

impl DownloadTaskStatus {
    fn is_terminal(self) -> bool {
        matches!(
            self,
            DownloadTaskStatus::Canceled
                | DownloadTaskStatus::Succeeded
                | DownloadTaskStatus::Failed
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequestedStop {
    Pause,
    Cancel,
}

#[derive(Clone)]
struct DownloadTaskConfig {
    lxapp: Arc<LxApp>,
    task_id: String,
    app_data_dir: PathBuf,
    user_data_dir: PathBuf,
    user_cache_dir: PathBuf,
    temp_dir: PathBuf,
    owner: user_cache::DownloadOwner,
    request: user_cache::UserCacheDownloadRequest,
    user_agent: Option<String>,
    behavior: DownloadBehavior,
    staging_path: PathBuf,
    output_path: Option<(PathBuf, DownloadPathKind)>,
    reservation_key: Option<String>,
}

#[derive(Debug, Clone)]
struct DownloadCompletion {
    path: PathBuf,
    path_kind: DownloadPathKind,
    mime_type: Option<String>,
    size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DownloadPathKind {
    Temp,
    UserData,
    Downloads,
}

struct DownloadIteratorState {
    receiver: Option<mpsc::Receiver<DownloadIteratorMessage>>,
    sender: mpsc::Sender<DownloadIteratorMessage>,
    pending_message: Option<DownloadIteratorMessage>,
    terminal_seen: bool,
    iteration_closed: bool,
    fallback_progress: f64,
    status: DownloadTaskStatus,
    stop_requested: Option<RequestedStop>,
    config: DownloadTaskConfig,
    completion: Option<oneshot::Sender<DownloadCompletionOutcome>>,
}

impl DownloadIteratorState {
    fn new(
        receiver: mpsc::Receiver<DownloadIteratorMessage>,
        sender: mpsc::Sender<DownloadIteratorMessage>,
        config: DownloadTaskConfig,
        completion: oneshot::Sender<DownloadCompletionOutcome>,
    ) -> Self {
        Self {
            receiver: Some(receiver),
            sender,
            pending_message: None,
            terminal_seen: false,
            iteration_closed: false,
            fallback_progress: 0.0,
            status: DownloadTaskStatus::Running,
            stop_requested: None,
            config,
            completion: Some(completion),
        }
    }
}

impl Drop for DownloadIteratorState {
    fn drop(&mut self) {
        release_output_reservation(self.config.reservation_key.take());
    }
}

enum DownloadCompletionOutcome {
    Success(DownloadCompletion),
    Failed(DownloadFailureReason),
    Canceled,
}

#[derive(Debug, Clone)]
enum DownloadFailureReason {
    Quota(StorageQuotaError),
    Network { code: u32, message: String },
    Internal(String),
}

impl DownloadFailureReason {
    fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    fn to_js_error(&self) -> rong::RongJSError {
        match self {
            Self::Quota(error) => storage::quota_error_to_js(*error),
            Self::Network { code, message } => {
                js_error_from_business_code_with_detail(*code, message)
            }
            Self::Internal(message) => js_internal_error(format!("download failed: {message}")),
        }
    }
}

fn download_failure_to_reason(error: DownloadFailure) -> DownloadFailureReason {
    let code = match error.kind {
        DownloadFailureKind::Timeout => Some(5002),
        DownloadFailureKind::NetworkUnavailable => Some(5001),
        DownloadFailureKind::Server => Some(5003),
        DownloadFailureKind::Connection => Some(5004),
        DownloadFailureKind::AccessDenied => Some(3000),
        DownloadFailureKind::Canceled
        | DownloadFailureKind::InvalidRequest
        | DownloadFailureKind::Conflict
        | DownloadFailureKind::Internal => None,
    };

    if let Some(code) = code {
        DownloadFailureReason::Network {
            code,
            message: error.error,
        }
    } else {
        DownloadFailureReason::internal(error.error)
    }
}

#[derive(Debug, Clone)]
enum DownloadIteratorMessage {
    Progress {
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    },
    Paused,
    Resumed,
    Canceled,
    Success(DownloadCompletion),
    Error(DownloadFailureReason),
}

#[derive(Default)]
struct Fnv64Hasher(u64);

static TEMP_DOWNLOAD_SEQ: AtomicU64 = AtomicU64::new(1);
static OUTPUT_RESERVATIONS: OnceLock<StdMutex<HashSet<String>>> = OnceLock::new();

impl Fnv64Hasher {
    fn new() -> Self {
        Self(0xcbf29ce484222325)
    }
}

impl Hasher for Fnv64Hasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        const FNV_PRIME: u64 = 0x00000100000001B3;
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(FNV_PRIME);
        }
    }
}

fn stable_hash(value: impl Hash) -> String {
    let mut hasher = Fnv64Hasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn stable_download_task_id(
    request: &user_cache::UserCacheDownloadRequest,
    output_path: Option<&(PathBuf, DownloadPathKind)>,
) -> String {
    let request_key = user_cache::download_request_task_id(request);
    match output_path {
        Some((path, kind)) => {
            let target_key = format!("{kind:?}:{}", path.to_string_lossy());
            format!("download_{}", stable_hash(target_key))
        }
        None => {
            let seq = TEMP_DOWNLOAD_SEQ.fetch_add(1, Ordering::Relaxed);
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0);
            format!("download_{request_key}_temp_{nonce}_{seq}")
        }
    }
}

fn output_reservations() -> &'static StdMutex<HashSet<String>> {
    OUTPUT_RESERVATIONS.get_or_init(|| StdMutex::new(HashSet::new()))
}

fn reserve_output_path(
    output_path: Option<&(PathBuf, DownloadPathKind)>,
) -> JSResult<Option<String>> {
    let Some((path, _kind)) = output_path else {
        return Ok(None);
    };
    let key = path.to_string_lossy().into_owned();
    let mut guard = output_reservations()
        .lock()
        .map_err(|_| js_internal_error("download output reservation lock poisoned"))?;
    if !guard.insert(key.clone()) {
        return Err(js_error_from_business_code_with_detail(
            1002,
            "downloadFile filePath is already in use",
        ));
    }
    Ok(Some(key))
}

fn release_output_reservation(key: Option<String>) {
    let Some(key) = key else {
        return;
    };
    if let Ok(mut guard) = output_reservations().lock() {
        guard.remove(&key);
    }
}

fn is_output_path_reserved(path: &Path) -> JSResult<bool> {
    let key = path.to_string_lossy().into_owned();
    let guard = output_reservations()
        .lock()
        .map_err(|_| js_internal_error("download output reservation lock poisoned"))?;
    Ok(guard.contains(&key))
}

fn progress_value(
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    fallback_progress: &mut f64,
) -> Option<f64> {
    if let Some(total) = total_bytes
        && total > 0
    {
        let precise = ((downloaded_bytes as f64) / (total as f64)).clamp(0.0, 1.0);
        *fallback_progress = precise;
        return Some(precise);
    }

    *fallback_progress = (downloaded_bytes as f64).max(*fallback_progress);
    None
}

fn install_async_iterator(ctx: &JSContext, iterator: &JSObject) -> JSResult<()> {
    let symbol = ctx
        .global()
        .get::<_, JSObject>("Symbol")?
        .get::<_, JSSymbol>("asyncIterator")?;
    iterator.set(
        symbol,
        JSFunc::new(ctx, move |this: rong::function::This<JSObject>| {
            (*this).clone()
        })?,
    )?;
    Ok(())
}

fn bind_abort_signal_to_iterator(
    ctx: &JSContext,
    signal: Option<JSObject>,
    iterator: &JSObject,
) -> JSResult<()> {
    let Some(signal) = signal else {
        return Ok(());
    };

    let target = iterator.clone();
    let cancel_fn = JSFunc::new(ctx, move || -> JSResult<()> {
        if let Ok(cancel) = target.get::<_, JSFunc>("cancel") {
            let _ = cancel.call::<_, JSObject>(Some(target.clone()), ());
        }
        Ok(())
    })?;

    if signal.get::<_, bool>("aborted").unwrap_or(false) {
        cancel_fn.call::<_, ()>(None, ())?;
        return Ok(());
    }

    let add_event_listener = signal
        .get::<_, JSFunc>("addEventListener")
        .map_err(|_| js_invalid_parameter_error("downloadFile signal must be an AbortSignal"))?;
    let listener_opts = JSObject::new(ctx);
    listener_opts.set("once", true)?;
    add_event_listener.call::<_, ()>(Some(signal), ("abort", cancel_fn, listener_opts))?;
    Ok(())
}

fn path_to_result_string(lxapp: &LxApp, path: &Path) -> String {
    lxapp
        .to_uri(path)
        .map(|value| value.into_string())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn to_js_download_result(
    ctx: &JSContext,
    result: &DownloadCompletion,
) -> JSResult<JSDownloadResult> {
    let lxapp = LxApp::from_ctx(ctx)?;
    let path = path_to_result_string(&lxapp, &result.path);
    let (temp_file_path, file_path) = match result.path_kind {
        DownloadPathKind::Temp => (Some(path), None),
        DownloadPathKind::UserData | DownloadPathKind::Downloads => (None, Some(path)),
    };
    Ok(JSDownloadResult {
        temp_file_path,
        file_path,
        mime_type: result.mime_type.clone(),
        size: result.size,
    })
}

fn simple_event(kind: &str) -> JSDownloadEvent {
    JSDownloadEvent {
        kind: kind.to_string(),
        downloaded_bytes: None,
        total_bytes: None,
        progress: None,
        result: None,
    }
}

fn js_abort_error(detail: impl AsRef<str>) -> rong::RongJSError {
    HostError::new(rong::error::E_ABORT, detail.as_ref()).into()
}

fn get_present_property(obj: &JSObject, field: &str) -> Option<JSValue> {
    obj.get::<_, JSValue>(field)
        .ok()
        .filter(|value| !value.is_undefined() && !value.is_null())
}

fn read_required_string_field(obj: &JSObject, field: &str, api_name: &str) -> JSResult<String> {
    let value = get_present_property(obj, field).ok_or_else(|| {
        js_error_from_business_code_with_detail(1002, format!("{api_name} requires {field}"))
    })?;
    if !value.is_string() {
        return Err(js_invalid_parameter_error(format!(
            "{api_name} {field} must be a string"
        )));
    }
    value
        .to_rust::<String>()
        .map_err(|_| js_invalid_parameter_error(format!("{api_name} {field} must be a string")))
}

fn read_optional_string_field(
    obj: &JSObject,
    field: &str,
    api_name: &str,
) -> JSResult<Option<String>> {
    let Some(value) = get_present_property(obj, field) else {
        return Ok(None);
    };
    if !value.is_string() {
        return Err(js_invalid_parameter_error(format!(
            "{api_name} {field} must be a string"
        )));
    }
    value
        .to_rust::<String>()
        .map(Some)
        .map_err(|_| js_invalid_parameter_error(format!("{api_name} {field} must be a string")))
}

fn read_optional_timeout_field(obj: &JSObject) -> JSResult<Option<u64>> {
    let Some(value) = get_present_property(obj, "timeout") else {
        return Ok(None);
    };
    if !value.is_number() {
        return Err(js_invalid_parameter_error(
            "downloadFile timeout must be a positive number",
        ));
    }
    let timeout = value.to_rust::<f64>().map_err(|_| {
        js_invalid_parameter_error("downloadFile timeout must be a positive number")
    })?;
    if !timeout.is_finite() || timeout <= 0.0 {
        return Err(js_invalid_parameter_error(
            "downloadFile timeout must be a positive number",
        ));
    }
    Ok(Some(timeout.round() as u64))
}

fn read_optional_signal(obj: &JSObject) -> JSResult<Option<JSObject>> {
    let Some(value) = get_present_property(obj, "signal") else {
        return Ok(None);
    };
    value
        .into_object()
        .map(Some)
        .ok_or_else(|| js_invalid_parameter_error("downloadFile signal must be an AbortSignal"))
}

fn read_optional_destination(obj: &JSObject) -> JSResult<DownloadDestination> {
    let Some(value) = get_present_property(obj, "destination") else {
        return Ok(DownloadDestination::App);
    };
    if !value.is_string() {
        return Err(js_invalid_parameter_error(
            "downloadFile destination must be 'app' or 'downloads'",
        ));
    }
    let destination = value.to_rust::<String>().map_err(|_| {
        js_invalid_parameter_error("downloadFile destination must be 'app' or 'downloads'")
    })?;
    parse_download_destination(&destination)
}

fn parse_download_destination(destination: &str) -> JSResult<DownloadDestination> {
    match destination.trim() {
        "app" => Ok(DownloadDestination::App),
        "downloads" => Ok(DownloadDestination::Downloads),
        _ => Err(js_invalid_parameter_error(
            "downloadFile destination must be 'app' or 'downloads'",
        )),
    }
}

fn read_header_entries(obj: &JSObject, field: &str) -> JSResult<Vec<(String, String)>> {
    let Some(value) = get_present_property(obj, field) else {
        return Ok(Vec::new());
    };
    let Some(header_obj) = value.into_object() else {
        return Err(js_invalid_parameter_error(format!(
            "downloadFile {field} must be an object"
        )));
    };
    let mut headers = Vec::new();
    for entry in header_obj.entries()? {
        let (key, value) = entry.try_into::<String, JSValue>()?;
        if value.is_undefined() || value.is_null() {
            continue;
        }
        if !value.is_string() {
            return Err(js_invalid_parameter_error(format!(
                "downloadFile {field}.{key} must be a string"
            )));
        }
        let parsed = value.to_rust::<String>().map_err(|_| {
            js_invalid_parameter_error(format!("downloadFile {field}.{key} must be a string"))
        })?;
        headers.push((key, parsed));
    }
    Ok(headers)
}

fn parse_download_options(options: JSValue) -> JSResult<ParsedDownloadOptions> {
    let Some(obj) = options.into_object() else {
        return Err(js_invalid_parameter_error(
            "downloadFile expects an options object",
        ));
    };
    Ok(ParsedDownloadOptions {
        url: read_required_string_field(&obj, "url", "downloadFile")?,
        headers: read_header_entries(&obj, "headers")?,
        timeout_ms: read_optional_timeout_field(&obj)?,
        file_path: read_optional_string_field(&obj, "filePath", "downloadFile")?,
        destination: read_optional_destination(&obj)?,
        signal: read_optional_signal(&obj)?,
    })
}

fn resolve_output_path(
    lxapp: &LxApp,
    file_path: Option<&str>,
) -> JSResult<Option<(PathBuf, DownloadPathKind)>> {
    let Some(file_path) = file_path.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    if file_path.starts_with("lx://") {
        let resolved = lxapp
            .resolve_accessible_path(file_path)
            .map_err(|err| js_error_from_lxapp_error(&err))?;
        if resolved.starts_with(&lxapp.user_cache_dir) {
            return Err(js_invalid_parameter_error(
                "downloadFile filePath must use lx://userdata or be omitted",
            ));
        }
        if !resolved.starts_with(&lxapp.user_data_dir) {
            return Err(js_invalid_parameter_error(
                "downloadFile filePath must use lx://userdata or be omitted",
            ));
        }
        if resolved == lxapp.user_data_dir {
            return Err(js_invalid_parameter_error(
                "downloadFile filePath must reference a file under lx://userdata",
            ));
        }
        return Ok(Some((resolved, DownloadPathKind::UserData)));
    }

    let relative = normalize_relative_path(file_path, "downloadFile", "filePath")?;

    Ok(Some((
        lxapp.user_data_dir.join(relative),
        DownloadPathKind::UserData,
    )))
}

fn ensure_downloads_privilege(lxapp: &LxApp) -> JSResult<()> {
    let privilege =
        LxAppSecurityPrivilege::new("downloads").map_err(|err| js_error_from_lxapp_error(&err))?;
    if lxapp.has_security_privilege(&privilege) {
        Ok(())
    } else {
        Err(js_error_from_business_code_with_detail(
            3005,
            "downloadFile destination 'downloads' requires security.privileges to include 'downloads'",
        ))
    }
}

fn file_name_hint(raw: Option<&str>) -> Option<String> {
    let raw = raw?.trim();
    if raw.is_empty() {
        return None;
    }
    let last = raw
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(raw)
        .trim()
        .trim_matches('.');
    if last.is_empty() || last == "." || last == ".." {
        None
    } else {
        Some(last.to_string())
    }
}

fn sanitize_downloads_filename(raw: &str) -> String {
    let mut sanitized = raw
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
            {
                '_'
            } else {
                ch
            }
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
            let hex = [bytes[i + 1], bytes[i + 2]];
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

fn file_name_from_url(url: &str) -> Option<String> {
    let without_query = url.split(['?', '#']).next().unwrap_or(url);
    let last = without_query.rsplit('/').next()?.trim();
    if last.is_empty() {
        return None;
    }
    Some(sanitize_downloads_filename(&percent_decode_lossy(last)))
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

fn unique_downloads_path(root: &Path, filename: &str) -> JSResult<PathBuf> {
    let sanitized = sanitize_downloads_filename(filename);
    let (stem, ext) = split_filename(&sanitized);
    let mut index = 0u32;
    loop {
        let candidate_name = if index == 0 {
            sanitized.clone()
        } else if let Some(ext) = ext.as_ref() {
            format!("{stem} ({index}).{ext}")
        } else {
            format!("{stem} ({index})")
        };
        let candidate = root.join(candidate_name);
        if !candidate.exists()
            && !candidate.with_extension("part").exists()
            && !is_output_path_reserved(&candidate)?
        {
            return Ok(candidate);
        }
        index += 1;
    }
}

fn resolve_downloads_output_path(
    lxapp: &LxApp,
    file_path: Option<&str>,
    url: &str,
) -> JSResult<(PathBuf, DownloadPathKind)> {
    ensure_downloads_privilege(lxapp)?;
    let root = lingxia_service::downloads::dir(&lxapp.app_data_dir());
    if let Ok(metadata) = std::fs::symlink_metadata(&root)
        && metadata.file_type().is_symlink()
    {
        return Err(js_error_from_business_code_with_detail(
            3005,
            "download directory must not be a symlink",
        ));
    }
    let filename = file_name_hint(file_path)
        .or_else(|| file_name_from_url(url))
        .unwrap_or_else(|| "download.bin".to_string());
    Ok((
        unique_downloads_path(&root, &filename)?,
        DownloadPathKind::Downloads,
    ))
}

fn ensure_no_symlink_ancestors(
    user_data_dir: &Path,
    destination: &Path,
) -> Result<(), DownloadFailureReason> {
    let Ok(relative) = destination.strip_prefix(user_data_dir) else {
        return Err(DownloadFailureReason::internal(
            "download output must stay inside userdata",
        ));
    };
    let mut current = user_data_dir.to_path_buf();
    let mut components = relative.components().peekable();
    while let Some(component) = components.next() {
        if components.peek().is_none() {
            break;
        }
        current.push(component.as_os_str());
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(DownloadFailureReason::internal(
                    "download output must not pass through a symlink",
                ));
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    Ok(())
}

async fn finalize_download_result(
    temp_dir: &Path,
    user_data_dir: &Path,
    user_cache_dir: &Path,
    source_url: &str,
    output_path: Option<&(PathBuf, DownloadPathKind)>,
    result: user_cache::UserCacheDownloadResult,
) -> Result<DownloadCompletion, DownloadFailureReason> {
    let Some((output_path, path_kind)) = output_path else {
        let download_temp_dir = temp_dir.join("download");
        std::fs::create_dir_all(&download_temp_dir).map_err(|e| {
            cleanup_staging_file(&result.temp_path);
            DownloadFailureReason::internal(format!("create temp dir failed: {e}"))
        })?;
        let ext = download_extension(source_url, result.mime_type.as_deref()).ok_or_else(|| {
            cleanup_staging_file(&result.temp_path);
            DownloadFailureReason::Network {
                code: 5003,
                message: "download response requires Content-Type or a URL file extension"
                    .to_string(),
            }
        })?;
        let mut filename = unique_temp_download_name(source_url, &result.temp_path, result.size);
        filename.push('.');
        filename.push_str(&ext);
        let temp_path = download_temp_dir.join(filename);
        storage::ensure_temp_quota(temp_dir, &temp_path, result.size).map_err(|err| {
            cleanup_staging_file(&result.temp_path);
            DownloadFailureReason::Quota(err)
        })?;
        storage::with_disk_pressure_recovery(user_cache_dir, result.size, &[], || {
            storage::move_file_atomic(&result.temp_path, &temp_path)
        })
        .map_err(|e| {
            cleanup_staging_file(&result.temp_path);
            DownloadFailureReason::internal(format!("move download to temp failed: {e}"))
        })?;
        return Ok(DownloadCompletion {
            path: temp_path,
            path_kind: DownloadPathKind::Temp,
            mime_type: result.mime_type,
            size: result.size,
        });
    };

    match *path_kind {
        DownloadPathKind::UserData => {
            ensure_no_symlink_ancestors(user_data_dir, output_path)?;
            if std::fs::symlink_metadata(output_path).is_ok() {
                cleanup_staging_file(&result.temp_path);
                return Err(DownloadFailureReason::Quota(
                    StorageQuotaError::DestinationExists,
                ));
            }
            storage::ensure_userdata_quota(user_data_dir, output_path, result.size).map_err(
                |err| {
                    cleanup_staging_file(&result.temp_path);
                    DownloadFailureReason::Quota(err)
                },
            )?;
            storage::ensure_app_storage_quota(
                user_data_dir,
                user_cache_dir,
                output_path,
                result.size,
            )
            .map_err(|err| {
                cleanup_staging_file(&result.temp_path);
                DownloadFailureReason::Quota(err)
            })?;
        }
        DownloadPathKind::Downloads | DownloadPathKind::Temp => {}
    }

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            cleanup_staging_file(&result.temp_path);
            DownloadFailureReason::internal(format!("create output dir failed: {e}"))
        })?;
    }
    if result.temp_path != *output_path {
        storage::with_disk_pressure_recovery(user_cache_dir, result.size, &[], || {
            storage::move_file_atomic(&result.temp_path, output_path)
        })
        .map_err(|e| {
            cleanup_staging_file(&result.temp_path);
            DownloadFailureReason::internal(format!("move download to filePath failed: {e}"))
        })?;
    }

    Ok(DownloadCompletion {
        path: output_path.clone(),
        path_kind: *path_kind,
        mime_type: result.mime_type,
        size: result.size,
    })
}

fn cleanup_staging_file(path: &Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(path.with_extension("part"));
}

fn cleanup_canceled_download_files(
    output_path: Option<&(PathBuf, DownloadPathKind)>,
    staging_path: &Path,
) {
    if matches!(output_path, Some((_, DownloadPathKind::Downloads))) {
        let _ = std::fs::remove_file(staging_path.with_extension("part"));
    } else {
        cleanup_staging_file(staging_path);
    }
}

fn cleanup_completed_canceled_download(
    output_path: Option<&(PathBuf, DownloadPathKind)>,
    success_path: &Path,
    staging_path: &Path,
) {
    let _ = std::fs::remove_file(success_path);
    cleanup_canceled_download_files(output_path, staging_path);
}

fn should_cleanup_staging_file(output_path: Option<&(PathBuf, DownloadPathKind)>) -> bool {
    !matches!(output_path, Some((_, DownloadPathKind::Downloads)))
}

fn unique_temp_download_name(source_url: &str, staging_path: &Path, size: u64) -> String {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let staging_path = staging_path.to_string_lossy();
    stable_hash((source_url, staging_path.as_ref(), size, nonce))
}

fn download_extension(url: &str, mime_type: Option<&str>) -> Option<String> {
    if let Some(ext) = Path::new(url.split(['?', '#']).next().unwrap_or(url))
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.trim().trim_start_matches('.').to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        && ext != "part"
    {
        return Some(ext);
    }
    let ext = match mime_type
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
    {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "video/mp4" => "mp4",
        "audio/mpeg" => "mp3",
        "application/pdf" => "pdf",
        _ => return None,
    };
    Some(ext.to_string())
}

fn spawn_download_worker(state: Arc<Mutex<DownloadIteratorState>>) {
    std::mem::drop(rong::RongExecutor::global().spawn(async move {
        let (mut progress_tx, config) = {
            let guard = state.lock().await;
            if guard.status.is_terminal() {
                return;
            }
            (guard.sender.clone(), guard.config.clone())
        };
        let downloads_target = config
            .output_path
            .as_ref()
            .and_then(|(path, kind)| (*kind == DownloadPathKind::Downloads).then(|| path.clone()));
        let download_target = downloads_target
            .clone()
            .unwrap_or_else(|| config.staging_path.clone());

        let persistence = user_cache::DownloadPersistence::new(
            config.app_data_dir.clone(),
            config.task_id.clone(),
            config.owner.clone(),
            downloads_target.is_some(),
        );
        let mut event_tx = progress_tx.clone();
        let on_event = move |event| match event {
            TransferDownloadEvent::Started {
                resumed_bytes,
                total_bytes,
                ..
            } => {
                let _ = event_tx.try_send(DownloadIteratorMessage::Progress {
                    downloaded_bytes: resumed_bytes,
                    total_bytes,
                });
            }
            TransferDownloadEvent::Progress {
                downloaded_bytes,
                total_bytes,
                ..
            } => {
                let _ = event_tx.try_send(DownloadIteratorMessage::Progress {
                    downloaded_bytes,
                    total_bytes,
                });
            }
            _ => {}
        };

        let download_result = network_security::scope_lxapp_network_access(
            config.lxapp.clone(),
            user_cache::download_to_path_with_behavior(
                Some(persistence),
                download_target,
                config.request.clone(),
                config.user_agent.clone(),
                config.behavior,
                on_event,
            ),
        )
        .await;

        let result: Result<DownloadCompletion, DownloadFailureReason> = match download_result {
            Ok(success) => {
                finalize_download_result(
                    &config.temp_dir,
                    &config.user_data_dir,
                    &config.user_cache_dir,
                    &config.request.url,
                    config.output_path.as_ref(),
                    success,
                )
                .await
            }
            Err(error) => Err(download_failure_to_reason(error)),
        };

        match result {
            Ok(success) => {
                let (completion, cancel_event) = {
                    let mut guard = state.lock().await;
                    let cancel_wins = matches!(guard.stop_requested, Some(RequestedStop::Cancel))
                        || guard.status == DownloadTaskStatus::Canceled;
                    if cancel_wins {
                        let should_emit_cancel = guard.status != DownloadTaskStatus::Canceled;
                        cleanup_completed_canceled_download(
                            guard.config.output_path.as_ref(),
                            &success.path,
                            &guard.config.staging_path,
                        );
                        release_output_reservation(guard.config.reservation_key.take());
                        guard.stop_requested = None;
                        guard.status = DownloadTaskStatus::Canceled;
                        guard.terminal_seen = false;
                        (guard.completion.take(), Some(should_emit_cancel))
                    } else {
                        guard.stop_requested = None;
                        if guard.status.is_terminal() {
                            release_output_reservation(guard.config.reservation_key.take());
                            return;
                        }
                        guard.status = DownloadTaskStatus::Succeeded;
                        release_output_reservation(guard.config.reservation_key.take());
                        (guard.completion.take(), None)
                    }
                };
                if let Some(should_emit_cancel) = cancel_event {
                    if let Some(completion) = completion {
                        let _ = completion.send(DownloadCompletionOutcome::Canceled);
                    }
                    if should_emit_cancel {
                        let _ = progress_tx.send(DownloadIteratorMessage::Canceled).await;
                    }
                    return;
                }
                if let Some(completion) = completion {
                    let _ = completion.send(DownloadCompletionOutcome::Success(success.clone()));
                }
                let _ = progress_tx
                    .send(DownloadIteratorMessage::Success(success))
                    .await;
            }
            Err(error) => {
                let (message, completion, pause_event) = {
                    let mut guard = state.lock().await;
                    match guard.stop_requested {
                        Some(RequestedStop::Pause) => {
                            guard.stop_requested = None;
                            guard.status = DownloadTaskStatus::Paused;
                            (None, None, Some(DownloadIteratorMessage::Paused))
                        }
                        Some(RequestedStop::Cancel) | None
                            if guard.status == DownloadTaskStatus::Canceled =>
                        {
                            cleanup_canceled_download_files(
                                guard.config.output_path.as_ref(),
                                &guard.config.staging_path,
                            );
                            release_output_reservation(guard.config.reservation_key.take());
                            guard.stop_requested = None;
                            return;
                        }
                        _ => {
                            guard.stop_requested = None;
                            guard.status = DownloadTaskStatus::Failed;
                            if should_cleanup_staging_file(guard.config.output_path.as_ref()) {
                                cleanup_staging_file(&guard.config.staging_path);
                            }
                            release_output_reservation(guard.config.reservation_key.take());
                            (Some(error.clone()), guard.completion.take(), None)
                        }
                    }
                };
                if let Some(event) = pause_event {
                    let _ = progress_tx.send(event).await;
                    return;
                }
                if let Some(error) = message {
                    if let Some(completion) = completion {
                        let _ = completion.send(DownloadCompletionOutcome::Failed(error.clone()));
                    }
                    let _ = progress_tx
                        .send(DownloadIteratorMessage::Error(error))
                        .await;
                }
            }
        }
    }));
}

fn install_promise_methods(ctx: &JSContext, iterator: &JSObject, promise: Promise) -> JSResult<()> {
    let then_promise = promise.clone();
    let then_ctx = ctx.clone();
    iterator.set(
        "then",
        JSFunc::new(
            ctx,
            move |on_fulfilled: Optional<JSValue>,
                  on_rejected: Optional<JSValue>|
                  -> JSResult<JSObject> {
                let then = then_promise.then()?;
                then.call(
                    Some(then_promise.clone().into_object()),
                    (
                        on_fulfilled
                            .0
                            .unwrap_or_else(|| JSValue::undefined(&then_ctx)),
                        on_rejected
                            .0
                            .unwrap_or_else(|| JSValue::undefined(&then_ctx)),
                    ),
                )
            },
        )?,
    )?;

    let catch_promise = promise.clone();
    let catch_ctx = ctx.clone();
    iterator.set(
        "catch",
        JSFunc::new(
            ctx,
            move |on_rejected: Optional<JSValue>| -> JSResult<JSObject> {
                let catch_fn = catch_promise.catch()?;
                catch_fn.call(
                    Some(catch_promise.clone().into_object()),
                    (on_rejected
                        .0
                        .unwrap_or_else(|| JSValue::undefined(&catch_ctx)),),
                )
            },
        )?,
    )?;

    let finally_promise = promise.clone();
    let finally_ctx = ctx.clone();
    iterator.set(
        "finally",
        JSFunc::new(
            ctx,
            move |on_finally: Optional<JSValue>| -> JSResult<JSObject> {
                let finally_fn = finally_promise.get::<_, JSFunc>("finally")?;
                finally_fn.call(
                    Some(finally_promise.clone().into_object()),
                    (on_finally
                        .0
                        .unwrap_or_else(|| JSValue::undefined(&finally_ctx)),),
                )
            },
        )?,
    )?;

    let wait_promise = promise;
    iterator.set("wait", JSFunc::new(ctx, move || wait_promise.clone())?)?;
    Ok(())
}

fn download_file(ctx: JSContext, options: JSValue) -> JSResult<JSObject> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let options = parse_download_options(options)?;
    let url = options.url.trim().to_string();
    if url.is_empty() {
        return Err(js_error_from_business_code_with_detail(
            1002,
            "downloadFile requires url",
        ));
    }

    let mut behavior = DownloadBehavior::default();
    if let Some(timeout_ms) = options.timeout_ms {
        behavior.request_timeout = Duration::from_millis(timeout_ms);
    }

    let output_path = match options.destination {
        DownloadDestination::App => resolve_output_path(&lxapp, options.file_path.as_deref())?,
        DownloadDestination::Downloads => Some(resolve_downloads_output_path(
            &lxapp,
            options.file_path.as_deref(),
            &url,
        )?),
    };
    if let Some((path, DownloadPathKind::UserData)) = output_path.as_ref()
        && std::fs::symlink_metadata(path).is_ok()
    {
        return Err(storage::quota_error_to_js(
            StorageQuotaError::DestinationExists,
        ));
    }
    if let Some((path, DownloadPathKind::UserData)) = output_path.as_ref() {
        ensure_no_symlink_ancestors(&lxapp.user_data_dir, path)
            .map_err(|reason| reason.to_js_error())?;
    }
    let reservation_key = reserve_output_path(output_path.as_ref())?;
    let request = user_cache::UserCacheDownloadRequest {
        url,
        headers: options.headers,
    };
    let task_id = stable_download_task_id(&request, output_path.as_ref());
    let staging_dir = lxapp.temp_dir.join(".download-staging");
    std::fs::create_dir_all(&staging_dir).map_err(|err| {
        release_output_reservation(reservation_key.clone());
        js_internal_error(format!("download staging dir failed: {err}"))
    })?;
    let staging_path = output_path
        .as_ref()
        .and_then(|(path, kind)| (*kind == DownloadPathKind::Downloads).then(|| path.clone()))
        .unwrap_or_else(|| staging_dir.join(&task_id));
    let (tx, rx) = mpsc::channel::<DownloadIteratorMessage>(64);
    let (completion_tx, completion_rx) = oneshot::channel::<DownloadCompletionOutcome>();
    let promise_ctx = ctx.clone();
    let final_promise = match Promise::from_future(&ctx, None, async move {
        match completion_rx.await {
            Ok(DownloadCompletionOutcome::Success(result)) => {
                to_js_download_result(&promise_ctx, &result)
            }
            Ok(DownloadCompletionOutcome::Failed(error)) => Err(error.to_js_error()),
            Ok(DownloadCompletionOutcome::Canceled) => Err(js_abort_error("downloadFile canceled")),
            Err(_) => Err(js_abort_error("downloadFile canceled")),
        }
    }) {
        Ok(promise) => promise,
        Err(err) => {
            release_output_reservation(reservation_key.clone());
            return Err(err);
        }
    };

    let state = Arc::new(Mutex::new(DownloadIteratorState::new(
        rx,
        tx.clone(),
        DownloadTaskConfig {
            lxapp: lxapp.clone(),
            task_id: task_id.clone(),
            app_data_dir: lxapp.app_data_dir(),
            user_data_dir: lxapp.user_data_dir.clone(),
            user_cache_dir: lxapp.user_cache_dir.clone(),
            temp_dir: lxapp.temp_dir.clone(),
            owner: user_cache::DownloadOwner {
                kind: user_cache::DownloadOwnerKind::LxApp,
                appid: lxapp.appid.clone(),
                page_path: None,
                tab_id: None,
            },
            request,
            user_agent: Some(rong::get_user_agent()),
            behavior,
            staging_path,
            output_path,
            reservation_key,
        },
        completion_tx,
    )));

    let iterator = JSObject::new(&ctx);

    let next_state = state.clone();
    iterator.set(
        "next",
        JSFunc::new(&ctx, move |ctx: JSContext| {
            let state = next_state.clone();
            async move { download_next_step(&ctx, &state).await }
        })?,
    )?;

    let return_state = state.clone();
    iterator.set(
        "return",
        JSFunc::new(&ctx, move || {
            let state = return_state.clone();
            async move {
                let mut guard = state.lock().await;
                guard.iteration_closed = true;
                guard.pending_message = None;
                guard.receiver = None;
                Ok(JSDownloadIteratorStep {
                    done: true,
                    value: None,
                })
            }
        })?,
    )?;

    let pause_state = state.clone();
    iterator.set(
        "pause",
        JSFunc::new(&ctx, move || {
            let state = pause_state.clone();
            async move { download_pause_task(&state).await }
        })?,
    )?;

    let resume_state = state.clone();
    iterator.set(
        "resume",
        JSFunc::new(&ctx, move || {
            let state = resume_state.clone();
            async move { download_resume_task(&state).await }
        })?,
    )?;

    let cancel_state = state.clone();
    let cancel_fn = JSFunc::new(&ctx, move || {
        let state = cancel_state.clone();
        async move { download_cancel_task(&state).await }
    })?;
    iterator.set("cancel", cancel_fn)?;

    let abort_state = state.clone();
    iterator.set(
        "abort",
        JSFunc::new(&ctx, move || {
            let state = abort_state.clone();
            async move { download_cancel_task(&state).await }
        })?,
    )?;

    install_promise_methods(&ctx, &iterator, final_promise)?;
    install_async_iterator(&ctx, &iterator)?;
    bind_abort_signal_to_iterator(&ctx, options.signal, &iterator)?;

    spawn_download_worker(state.clone());

    Ok(iterator)
}

async fn download_next_step(
    ctx: &JSContext,
    state: &Arc<Mutex<DownloadIteratorState>>,
) -> JSResult<JSDownloadIteratorStep> {
    let mut receiver = {
        let mut state_guard = state.lock().await;
        if let Some(message) = state_guard.pending_message.take() {
            drop(state_guard);
            return handle_download_message(ctx, state, message).await;
        }
        if state_guard.terminal_seen || state_guard.iteration_closed {
            return Ok(JSDownloadIteratorStep {
                done: true,
                value: None,
            });
        }

        state_guard
            .receiver
            .take()
            .ok_or_else(|| js_internal_error("download iterator receiver unexpectedly missing"))?
    };

    let message = receiver.next().await;

    let mut state_guard = state.lock().await;
    state_guard.receiver = Some(receiver);
    drop(state_guard);

    match message {
        Some(message) => handle_download_message(ctx, state, message).await,
        None => {
            let mut state_guard = state.lock().await;
            state_guard.terminal_seen = true;
            Ok(JSDownloadIteratorStep {
                done: true,
                value: None,
            })
        }
    }
}

async fn handle_download_message(
    ctx: &JSContext,
    state: &Arc<Mutex<DownloadIteratorState>>,
    message: DownloadIteratorMessage,
) -> JSResult<JSDownloadIteratorStep> {
    let mut state_guard = state.lock().await;

    match message {
        DownloadIteratorMessage::Progress {
            downloaded_bytes,
            total_bytes,
        } => {
            let progress = progress_value(
                downloaded_bytes,
                total_bytes,
                &mut state_guard.fallback_progress,
            );
            Ok(JSDownloadIteratorStep {
                done: false,
                value: Some(JSDownloadEvent {
                    kind: "progress".to_string(),
                    downloaded_bytes: Some(downloaded_bytes),
                    total_bytes,
                    progress,
                    result: None,
                }),
            })
        }
        DownloadIteratorMessage::Paused => Ok(JSDownloadIteratorStep {
            done: false,
            value: Some(simple_event("paused")),
        }),
        DownloadIteratorMessage::Resumed => Ok(JSDownloadIteratorStep {
            done: false,
            value: Some(simple_event("resumed")),
        }),
        DownloadIteratorMessage::Canceled => {
            state_guard.status = DownloadTaskStatus::Canceled;
            state_guard.terminal_seen = true;
            Ok(JSDownloadIteratorStep {
                done: false,
                value: Some(simple_event("canceled")),
            })
        }
        DownloadIteratorMessage::Success(result) => {
            state_guard.status = DownloadTaskStatus::Succeeded;
            state_guard.terminal_seen = true;
            let js_result = to_js_download_result(ctx, &result)?;
            Ok(JSDownloadIteratorStep {
                done: false,
                value: Some(JSDownloadEvent {
                    kind: "completed".to_string(),
                    downloaded_bytes: Some(result.size),
                    total_bytes: Some(result.size),
                    progress: Some(1.0),
                    result: Some(js_result),
                }),
            })
        }
        DownloadIteratorMessage::Error(error) => {
            state_guard.status = DownloadTaskStatus::Failed;
            state_guard.terminal_seen = true;
            Err(error.to_js_error())
        }
    }
}

async fn download_pause_task(state: &Arc<Mutex<DownloadIteratorState>>) -> JSResult<()> {
    let (app_data_dir, task_id) = {
        let mut guard = state.lock().await;
        if guard.status != DownloadTaskStatus::Running {
            return Ok(());
        }
        guard.stop_requested = Some(RequestedStop::Pause);
        (
            guard.config.app_data_dir.clone(),
            guard.config.task_id.clone(),
        )
    };

    match lingxia_service::downloads::pause(&app_data_dir, &task_id) {
        Ok(()) => Ok(()),
        Err(err) => {
            let mut guard = state.lock().await;
            guard.stop_requested = None;
            Err(js_internal_error(format!("download pause failed: {err}")))
        }
    }
}

async fn download_resume_task(state: &Arc<Mutex<DownloadIteratorState>>) -> JSResult<()> {
    {
        let mut guard = state.lock().await;
        if guard.status.is_terminal() || guard.status != DownloadTaskStatus::Paused {
            return Ok(());
        }
        guard.stop_requested = None;
        guard.status = DownloadTaskStatus::Running;
        if guard
            .sender
            .try_send(DownloadIteratorMessage::Resumed)
            .is_err()
        {
            guard.pending_message = Some(DownloadIteratorMessage::Resumed);
        }
    }

    spawn_download_worker(state.clone());

    Ok(())
}

async fn download_cancel_task(state: &Arc<Mutex<DownloadIteratorState>>) -> JSResult<()> {
    let (app_data_dir, task_id) = {
        let mut guard = state.lock().await;
        if guard.status.is_terminal() {
            return Ok(());
        }
        if guard.status == DownloadTaskStatus::Paused {
            let completion = guard.completion.take();
            let staging_path = guard.config.staging_path.clone();
            let output_path = guard.config.output_path.clone();
            guard.stop_requested = None;
            guard.status = DownloadTaskStatus::Canceled;
            guard.terminal_seen = false;
            cleanup_canceled_download_files(output_path.as_ref(), &staging_path);
            release_output_reservation(guard.config.reservation_key.take());
            if guard
                .sender
                .try_send(DownloadIteratorMessage::Canceled)
                .is_err()
            {
                guard.pending_message = Some(DownloadIteratorMessage::Canceled);
            }
            drop(guard);
            if let Some(completion) = completion {
                let _ = completion.send(DownloadCompletionOutcome::Canceled);
            }
            return Ok(());
        }
        guard.stop_requested = Some(RequestedStop::Cancel);
        (
            guard.config.app_data_dir.clone(),
            guard.config.task_id.clone(),
        )
    };

    match lingxia_service::downloads::cancel(&app_data_dir, &task_id) {
        Ok(()) => {
            let completion = {
                let mut guard = state.lock().await;
                if guard.status.is_terminal() {
                    guard.stop_requested = None;
                    return Ok(());
                }
                guard.status = DownloadTaskStatus::Canceled;
                guard.terminal_seen = false;
                if guard
                    .sender
                    .try_send(DownloadIteratorMessage::Canceled)
                    .is_err()
                {
                    guard.pending_message = Some(DownloadIteratorMessage::Canceled);
                }
                guard.completion.take()
            };
            if let Some(completion) = completion {
                let _ = completion.send(DownloadCompletionOutcome::Canceled);
            }
            Ok(())
        }
        Err(err) => {
            let mut guard = state.lock().await;
            guard.stop_requested = None;
            Err(js_internal_error(format!("download cancel failed: {err}")))
        }
    }
}

pub(super) fn init(ctx: &JSContext) -> JSResult<()> {
    let download_file_func = JSFunc::new(ctx, download_file)?;
    lx::register_js_api(ctx, "downloadFile", download_file_func)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_temp_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "lingxia-logic-download-{name}-{}-{nonce}",
            std::process::id()
        ))
    }

    #[test]
    fn downloads_file_name_hint_uses_last_segment() {
        assert_eq!(
            file_name_hint(Some("../nested/report.pdf")),
            Some("report.pdf".to_string())
        );
        assert_eq!(
            file_name_hint(Some(r"..\nested\report.pdf")),
            Some("report.pdf".to_string())
        );
        assert_eq!(file_name_hint(Some(" . ")), None);
        assert_eq!(file_name_hint(Some(" .. ")), None);
    }

    #[test]
    fn downloads_file_name_from_url_decodes_and_sanitizes() {
        assert_eq!(
            file_name_from_url("https://example.com/files/a%20b.pdf?token=1#frag"),
            Some("a b.pdf".to_string())
        );
        assert_eq!(
            file_name_from_url("https://example.com/files/a%2Fb.txt"),
            Some("a_b.txt".to_string())
        );
        assert_eq!(file_name_from_url("https://example.com/files/"), None);
    }

    #[test]
    fn download_destination_rejects_empty_string() {
        assert!(matches!(
            parse_download_destination("app"),
            Ok(DownloadDestination::App)
        ));
        assert!(matches!(
            parse_download_destination("downloads"),
            Ok(DownloadDestination::Downloads)
        ));
        assert!(parse_download_destination("").is_err());
        assert!(parse_download_destination("   ").is_err());
    }

    #[test]
    fn unique_downloads_path_avoids_existing_part_and_reserved_paths() {
        let root = test_temp_dir("unique-path");
        std::fs::create_dir_all(&root).expect("create test dir");
        std::fs::write(root.join("report.pdf"), b"done").expect("write existing file");
        std::fs::write(root.join("report (1).part"), b"partial").expect("write part file");

        let reserved_path = root.join("report (2).pdf");
        let reserved = (reserved_path.clone(), DownloadPathKind::Downloads);
        let reservation = reserve_output_path(Some(&reserved)).expect("reserve output path");

        let result = unique_downloads_path(&root, "report.pdf").expect("resolve unique path");
        release_output_reservation(reservation);
        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(result, root.join("report (3).pdf"));
    }

    #[test]
    fn cancel_cleanup_for_downloads_removes_part_without_deleting_target_name() {
        let root = test_temp_dir("cancel-cleanup");
        std::fs::create_dir_all(&root).expect("create test dir");
        let target = root.join("report.pdf");
        let part = target.with_extension("part");
        std::fs::write(&target, b"done").expect("write target file");
        std::fs::write(&part, b"partial").expect("write part file");

        cleanup_canceled_download_files(
            Some(&(target.clone(), DownloadPathKind::Downloads)),
            &target,
        );

        assert!(!part.exists());
        assert_eq!(std::fs::read(&target).expect("target remains"), b"done");
        let _ = std::fs::remove_dir_all(&root);
    }
}
