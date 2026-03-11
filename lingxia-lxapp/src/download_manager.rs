use bytes::Bytes;
use http::Request as HttpRequest;
use http::header;
use http_body_util::{BodyExt, Full};
use lingxia_webview::DownloadRequest;
use rong_http::{DEFAULT_BLOCKING_BODY_LIMIT, HttpBody, send_request};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::Error as IoError;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;

const DOWNLOAD_PROGRESS_INTERVAL_BYTES: u64 = 256 * 1024;
static DOWNLOAD_ROOT_OVERRIDE: OnceLock<RwLock<Option<PathBuf>>> = OnceLock::new();

pub const BROWSER_DOWNLOAD_EVENT_STARTED: &str = "BrowserDownloadStarted";
pub const BROWSER_DOWNLOAD_EVENT_PROGRESS: &str = "BrowserDownloadProgress";
pub const BROWSER_DOWNLOAD_EVENT_COMPLETED: &str = "BrowserDownloadCompleted";
pub const BROWSER_DOWNLOAD_EVENT_FAILED: &str = "BrowserDownloadFailed";

#[derive(Debug, Clone)]
pub enum DownloadConfigError {
    InvalidParameter(String),
    Runtime(String),
}

impl std::fmt::Display for DownloadConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidParameter(msg) | Self::Runtime(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for DownloadConfigError {}

/// A single HTTP(S) download job with resumable semantics.
#[derive(Debug, Clone)]
pub struct DownloadTask {
    pub request: DownloadRequest,
    pub root_dir: PathBuf,
    pub fallback_user_agent: Option<String>,
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
}

/// Terminal success result for a download task.
#[derive(Debug, Clone)]
pub struct DownloadSuccess {
    pub url: String,
    pub file_name: String,
    pub path: PathBuf,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
}

/// Terminal failure result for a download task.
#[derive(Debug, Clone)]
pub struct DownloadFailure {
    pub url: String,
    pub error: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ResumeMetadata {
    etag: Option<String>,
    last_modified: Option<String>,
    downloaded: u64,
    total: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
struct DownloadProgress {
    downloaded: u64,
    last_reported: u64,
    last_percent: i32,
}

fn download_root_store() -> &'static RwLock<Option<PathBuf>> {
    DOWNLOAD_ROOT_OVERRIDE.get_or_init(|| RwLock::new(None))
}

pub fn set_download_root_override(path: impl Into<PathBuf>) -> Result<(), DownloadConfigError> {
    let path = path.into();
    if path.as_os_str().is_empty() {
        return Err(DownloadConfigError::InvalidParameter(
            "download root cannot be empty".to_string(),
        ));
    }
    let mut guard = download_root_store().write().map_err(|_| {
        DownloadConfigError::Runtime("download root override lock poisoned".to_string())
    })?;
    *guard = Some(path);
    Ok(())
}

pub fn clear_download_root_override() -> Result<(), DownloadConfigError> {
    let mut guard = download_root_store().write().map_err(|_| {
        DownloadConfigError::Runtime("download root override lock poisoned".to_string())
    })?;
    *guard = None;
    Ok(())
}

pub fn download_root_override() -> Option<PathBuf> {
    download_root_store()
        .read()
        .ok()
        .and_then(|guard| guard.clone())
}

pub fn resolve_download_root(default_root: impl Into<PathBuf>) -> PathBuf {
    download_root_override().unwrap_or_else(|| default_root.into())
}

pub fn browser_download_root(app_data_dir: &Path) -> PathBuf {
    resolve_download_root(app_data_dir.join("browser").join("downloads"))
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

fn meta_path_for(target: &Path) -> PathBuf {
    target.with_extension("resume.json")
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

async fn load_resume_metadata(path: &Path) -> Option<ResumeMetadata> {
    let bytes = fs::read(path).await.ok()?;
    serde_json::from_slice::<ResumeMetadata>(&bytes).ok()
}

async fn save_resume_metadata(path: &Path, meta: &ResumeMetadata) -> Result<(), String> {
    let content =
        serde_json::to_vec(meta).map_err(|e| format!("serialize resume metadata: {e}"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("create resume metadata dir: {e}"))?;
    }
    fs::write(path, content)
        .await
        .map_err(|e| format!("write resume metadata: {e}"))?;
    Ok(())
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
    file: &mut tokio::fs::File,
    chunk: &[u8],
    progress: &mut DownloadProgress,
    total_bytes: Option<u64>,
    resume_meta: &mut ResumeMetadata,
    meta_path: &Path,
    url: &str,
    on_event: &mut impl FnMut(DownloadEvent),
) -> Result<(), String> {
    file.write_all(chunk)
        .await
        .map_err(|e| format!("write chunk failed: {}", e))?;
    progress.downloaded += chunk.len() as u64;
    resume_meta.downloaded = progress.downloaded;

    let should_report = if let Some(total) = total_bytes {
        if total == 0 {
            false
        } else {
            let percent = ((progress.downloaded as f64 / total as f64) * 100.0) as i32;
            if percent > progress.last_percent {
                progress.last_percent = percent;
                true
            } else {
                false
            }
        }
    } else {
        progress.downloaded.saturating_sub(progress.last_reported)
            >= DOWNLOAD_PROGRESS_INTERVAL_BYTES
    };

    if should_report {
        progress.last_reported = progress.downloaded;
        let _ = save_resume_metadata(meta_path, resume_meta).await;
        on_event(DownloadEvent::Progress {
            url: url.to_string(),
            downloaded_bytes: progress.downloaded,
            total_bytes,
        });
    }
    Ok(())
}

pub async fn run_download_task(
    task: DownloadTask,
    mut on_event: impl FnMut(DownloadEvent),
) -> Result<DownloadSuccess, DownloadFailure> {
    let request = task.request;
    let filename = suggest_filename(&request);
    let target_path = resolve_unique_download_path(&task.root_dir, &filename);
    let part_path = part_path_for(&target_path);
    let meta_path = meta_path_for(&target_path);
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

    let mut resume_meta = load_resume_metadata(&meta_path).await.unwrap_or_default();
    let mut resume_offset = fs::metadata(&part_path)
        .await
        .ok()
        .map(|meta| meta.len())
        .unwrap_or(0);

    let has_resume_validator = resume_meta.etag.is_some() || resume_meta.last_modified.is_some();
    if resume_offset > 0 && !has_resume_validator {
        let _ = fs::remove_file(&part_path).await;
        resume_offset = 0;
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
            return Err(emit_failed(
                &mut on_event,
                url,
                format!("build request failed: {}", e),
                resume_offset,
                None,
            ));
        }
    };

    let response = match send_request(request_obj, DEFAULT_BLOCKING_BODY_LIMIT, None).await {
        Ok(response) => response,
        Err(e) => {
            return Err(emit_failed(&mut on_event, url, e, resume_offset, None));
        }
    };

    if !response.status.is_success() {
        return Err(emit_failed(
            &mut on_event,
            url,
            format!("http status {}", response.status.as_u16()),
            resume_offset,
            None,
        ));
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
        .open(&part_path)
        .await
    {
        Ok(file) => file,
        Err(e) => {
            return Err(emit_failed(
                &mut on_event,
                request.url,
                format!("open temp file failed: {}", e),
                resume_offset,
                None,
            ));
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
    let _ = save_resume_metadata(&meta_path, &resume_meta).await;

    on_event(DownloadEvent::Started {
        url: request.url.clone(),
        file_name: filename.clone(),
        target_path: target_path.clone(),
        mime_type: request.mime_type.clone(),
        total_bytes,
        resumed_bytes: resume_offset,
    });

    let mut progress = DownloadProgress {
        downloaded: resume_offset,
        last_reported: resume_offset,
        last_percent: if let Some(total) = total_bytes {
            if total > 0 {
                ((resume_offset as f64 / total as f64) * 100.0) as i32
            } else {
                0
            }
        } else {
            0
        },
    };

    let stream_result = match response.body {
        HttpBody::Empty => Ok(()),
        HttpBody::Small(bytes) => {
            write_chunk(
                &mut file,
                bytes.as_ref(),
                &mut progress,
                total_bytes,
                &mut resume_meta,
                &meta_path,
                &request.url,
                &mut on_event,
            )
            .await
        }
        HttpBody::Stream(mut rx) => {
            let mut result = Ok(());
            while let Some(chunk) = rx.recv().await {
                match chunk {
                    Ok(bytes) => {
                        if let Err(e) = write_chunk(
                            &mut file,
                            bytes.as_ref(),
                            &mut progress,
                            total_bytes,
                            &mut resume_meta,
                            &meta_path,
                            &request.url,
                            &mut on_event,
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

    if progress.downloaded > progress.last_reported {
        progress.last_reported = progress.downloaded;
        let _ = save_resume_metadata(&meta_path, &resume_meta).await;
        on_event(DownloadEvent::Progress {
            url: request.url.clone(),
            downloaded_bytes: progress.downloaded,
            total_bytes,
        });
    }

    if let Err(e) = stream_result {
        let _ = file.flush().await;
        let _ = save_resume_metadata(&meta_path, &resume_meta).await;
        return Err(emit_failed(
            &mut on_event,
            request.url,
            e,
            progress.downloaded,
            total_bytes,
        ));
    }

    if let Err(e) = file.flush().await {
        let _ = save_resume_metadata(&meta_path, &resume_meta).await;
        return Err(emit_failed(
            &mut on_event,
            request.url,
            format!("flush failed: {}", e),
            progress.downloaded,
            total_bytes,
        ));
    }
    drop(file);

    if let Err(e) = fs::rename(&part_path, &target_path).await {
        let _ = save_resume_metadata(&meta_path, &resume_meta).await;
        return Err(emit_failed(
            &mut on_event,
            request.url,
            format!("rename failed: {}", e),
            progress.downloaded,
            total_bytes,
        ));
    }

    let _ = fs::remove_file(&meta_path).await;
    let success = DownloadSuccess {
        url: request.url.clone(),
        file_name: filename.clone(),
        path: target_path.clone(),
        downloaded_bytes: progress.downloaded,
        total_bytes,
    };
    on_event(DownloadEvent::Completed {
        url: request.url,
        file_name: filename,
        path: target_path,
        downloaded_bytes: progress.downloaded,
        total_bytes,
    });
    Ok(success)
}

fn map_browser_download_event(
    task_id: &str,
    tab_id: &str,
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
    }
}

pub async fn run_browser_download_task(
    task: DownloadTask,
    task_id: &str,
    tab_id: &str,
    mut on_event: impl FnMut(&'static str, serde_json::Value),
) -> Result<DownloadSuccess, DownloadFailure> {
    let task_id = task_id.to_string();
    let tab_id = tab_id.to_string();
    run_download_task(task, |event| {
        let (event_name, payload) = map_browser_download_event(&task_id, &tab_id, event);
        on_event(event_name, payload);
    })
    .await
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
}
