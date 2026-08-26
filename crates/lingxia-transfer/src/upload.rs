use bytes::Bytes;
use http::Request as HttpRequest;
use http::header;
use http_body_util::{BodyExt, channel::Channel};
use ring::digest::{SHA256, digest};
use rong_rt::http::{self as host_http, RequestOptions};
use std::io::Error as IoError;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::sync::{mpsc, oneshot};

const UPLOAD_PROGRESS_INTERVAL_BYTES: u64 = 32 * 1024;
const UPLOAD_PROGRESS_INTERVAL_MILLIS: u128 = 120;
const UPLOAD_DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(90);
const UPLOAD_DEFAULT_CHUNK_SIZE: usize = 64 * 1024;
pub(crate) const UPLOAD_CANCELED_ERROR: &str = "Upload canceled";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadMethod {
    Post,
    Put,
    Patch,
}

impl UploadMethod {
    fn as_str(self) -> &'static str {
        match self {
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
        }
    }
}

/// How the file bytes are framed in the request body.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UploadBodyMode {
    /// `multipart/form-data`, the file carried as one part next to the text
    /// form fields. The default, and the only mode that uses `field_name`,
    /// `file_name`, and `form_fields`.
    #[default]
    Multipart,
    /// The file bytes are the entire request body, unframed. What presigned
    /// `PUT` endpoints (S3, OSS, Azure Blob) expect.
    Raw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UploadBehavior {
    pub request_timeout: Duration,
    pub connect_timeout: Option<Duration>,
    pub chunk_size: usize,
}

impl Default for UploadBehavior {
    fn default() -> Self {
        Self {
            request_timeout: UPLOAD_DEFAULT_REQUEST_TIMEOUT,
            connect_timeout: None,
            chunk_size: UPLOAD_DEFAULT_CHUNK_SIZE,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadRequest {
    pub url: String,
    pub method: UploadMethod,
    pub body_mode: UploadBodyMode,
    pub file_path: PathBuf,
    pub field_name: String,
    pub file_name: Option<String>,
    pub mime_type: Option<String>,
    pub headers: Vec<(String, String)>,
    pub form_fields: Vec<(String, String)>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadResult {
    pub status_code: u16,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum UploadEvent {
    Started {
        url: String,
        file_name: String,
        uploaded_bytes: u64,
        total_bytes: u64,
    },
    Progress {
        url: String,
        uploaded_bytes: u64,
        total_bytes: u64,
    },
    Completed {
        url: String,
        status_code: u16,
        uploaded_bytes: u64,
        total_bytes: u64,
    },
    Failed {
        url: String,
        error: String,
        uploaded_bytes: u64,
        total_bytes: u64,
    },
    Canceled {
        url: String,
        uploaded_bytes: u64,
        total_bytes: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadFailureKind {
    InvalidRequest,
    InvalidFile,
    Timeout,
    NetworkUnavailable,
    Server,
    Connection,
    AccessDenied,
    Canceled,
    Internal,
}

#[derive(Debug, Clone)]
pub struct UploadFailure {
    pub kind: UploadFailureKind,
    pub url: String,
    pub error: String,
    pub uploaded_bytes: u64,
    pub total_bytes: u64,
}

impl UploadFailure {
    fn new(
        kind: UploadFailureKind,
        url: String,
        error: impl Into<String>,
        uploaded_bytes: u64,
        total_bytes: u64,
    ) -> Self {
        let error = error.into();
        Self {
            kind,
            url,
            error,
            uploaded_bytes,
            total_bytes,
        }
    }
}

fn classify_transport_upload_failure(error: &str) -> UploadFailureKind {
    let lower = error.trim().to_ascii_lowercase();
    if lower == "aborted" || lower == UPLOAD_CANCELED_ERROR.to_ascii_lowercase() {
        return UploadFailureKind::Canceled;
    }
    if lower.contains("timeout") {
        return UploadFailureKind::Timeout;
    }
    if lower.contains("dns")
        || lower.contains("unreachable")
        || lower.contains("network unavailable")
        || lower.contains("no route")
    {
        return UploadFailureKind::NetworkUnavailable;
    }
    if lower.contains("connection")
        || lower.contains("connect")
        || lower.contains("broken pipe")
        || lower.contains("tls")
        || lower.contains("unexpected eof")
        || lower.contains("early eof")
    {
        return UploadFailureKind::Connection;
    }
    if lower.starts_with("http status ") {
        return UploadFailureKind::Server;
    }
    if lower.contains("access denied") || lower.contains("not allowed") {
        return UploadFailureKind::AccessDenied;
    }
    UploadFailureKind::Internal
}

/// Turns a transport error into a failure the caller can act on. A denied host
/// is not a network glitch, and only the error's own kind can tell them apart.
fn http_failure(
    error: &host_http::HttpError,
    url: String,
    uploaded_bytes: u64,
    total_bytes: u64,
) -> UploadFailure {
    let message = error.to_string();
    let kind = if error.kind() == host_http::HttpErrorKind::AccessDenied {
        UploadFailureKind::AccessDenied
    } else {
        classify_transport_upload_failure(&message)
    };
    UploadFailure::new(kind, url, message, uploaded_bytes, total_bytes)
}

fn upload_request_options(
    behavior: UploadBehavior,
    abort_rx: oneshot::Receiver<()>,
) -> RequestOptions {
    let options = RequestOptions::new()
        .with_request_timeout(behavior.request_timeout)
        .with_abort(abort_rx);
    if let Some(connect_timeout) = behavior.connect_timeout {
        options.with_connect_timeout(connect_timeout)
    } else {
        options
    }
}

fn should_forward_header(name: &str, body_mode: UploadBodyMode) -> bool {
    if name == "content-type" {
        // A raw body lets the caller pin the type: presigned upload signatures
        // usually cover `Content-Type`, so silently rewriting it breaks them.
        // Multipart owns the header instead -- it carries the boundary.
        return body_mode == UploadBodyMode::Raw;
    }
    !matches!(
        name,
        "content-length" | "host" | "referer" | "transfer-encoding" | "user-agent"
    )
}

fn escape_multipart_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn invalid_multipart_value(request: &UploadRequest, file_name: &str) -> Option<&'static str> {
    let contains_control = |value: &str| value.chars().any(char::is_control);

    if contains_control(&request.field_name) {
        return Some("upload fieldName cannot contain control characters");
    }
    if request.file_name.as_deref().is_some_and(contains_control) || contains_control(file_name) {
        return Some("upload fileName cannot contain control characters");
    }
    if request.mime_type.as_deref().is_some_and(contains_control) {
        return Some("upload mimeType cannot contain control characters");
    }
    for (name, value) in &request.form_fields {
        if contains_control(name) {
            return Some("upload form field names cannot contain control characters");
        }
        if contains_control(value) {
            return Some("upload form field values cannot contain control characters");
        }
    }
    None
}

fn invalid_raw_value(request: &UploadRequest) -> Option<&'static str> {
    if !request.form_fields.is_empty() {
        return Some("raw upload does not support form fields");
    }
    if request
        .mime_type
        .as_deref()
        .is_some_and(|value| value.chars().any(char::is_control))
    {
        return Some("upload mimeType cannot contain control characters");
    }
    None
}

/// Content type for a raw body. An explicit `content-type` request header still
/// wins over this -- it is applied after the builder sets the default.
fn raw_content_type(request: &UploadRequest) -> String {
    request
        .mime_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "application/octet-stream".to_string())
}

fn file_name_for_request(request: &UploadRequest) -> String {
    request
        .file_name
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            request
                .file_path
                .file_name()
                .and_then(|value| value.to_str())
                .map(|value| value.to_string())
        })
        .unwrap_or_else(|| "upload.bin".to_string())
}

fn multipart_boundary(request: &UploadRequest) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or_default();
    let seed = format!(
        "{}:{}:{}:{}",
        request.url,
        request.file_path.display(),
        request.field_name,
        nanos
    );
    let digest = digest(&SHA256, seed.as_bytes());
    let mut encoded = String::with_capacity(digest.as_ref().len() * 2);
    for byte in digest.as_ref() {
        encoded.push_str(&format!("{byte:02x}"));
    }
    format!("lingxia-{encoded}")
}

fn build_multipart_parts(
    request: &UploadRequest,
    boundary: &str,
    file_name: &str,
) -> (Vec<u8>, Vec<u8>) {
    let mut prefix = Vec::new();
    for (name, value) in &request.form_fields {
        prefix.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        prefix.extend_from_slice(
            format!(
                "Content-Disposition: form-data; name=\"{}\"\r\n\r\n",
                escape_multipart_value(name)
            )
            .as_bytes(),
        );
        prefix.extend_from_slice(value.as_bytes());
        prefix.extend_from_slice(b"\r\n");
    }

    prefix.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    prefix.extend_from_slice(
        format!(
            "Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\n",
            escape_multipart_value(&request.field_name),
            escape_multipart_value(file_name)
        )
        .as_bytes(),
    );
    let mime = request
        .mime_type
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("application/octet-stream");
    prefix.extend_from_slice(format!("Content-Type: {mime}\r\n\r\n").as_bytes());

    let suffix = format!("\r\n--{boundary}--\r\n").into_bytes();
    (prefix, suffix)
}

fn user_agent_for_request(request: &UploadRequest) -> String {
    request
        .user_agent
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(rong::get_user_agent)
}

async fn collect_response_body(body: rong_rt::http::HttpBody) -> Result<Vec<u8>, UploadFailure> {
    host_http::collect_body(body)
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|err| {
            let error = err.to_string();
            UploadFailure::new(
                classify_transport_upload_failure(&error),
                String::new(),
                error,
                0,
                0,
            )
        })
}

pub async fn upload_file_with_behavior(
    request: UploadRequest,
    behavior: UploadBehavior,
    abort_rx: oneshot::Receiver<()>,
    mut on_event: impl FnMut(UploadEvent) + Send + 'static,
) -> Result<UploadResult, UploadFailure> {
    let url = request.url.trim().to_string();
    if url.is_empty() {
        return Err(UploadFailure::new(
            UploadFailureKind::InvalidRequest,
            url,
            "upload url cannot be empty",
            0,
            0,
        ));
    }

    let file_name = file_name_for_request(&request);
    let invalid_value = match request.body_mode {
        UploadBodyMode::Multipart => invalid_multipart_value(&request, &file_name),
        UploadBodyMode::Raw => invalid_raw_value(&request),
    };
    if let Some(error) = invalid_value {
        return Err(UploadFailure::new(
            UploadFailureKind::InvalidRequest,
            url.clone(),
            error,
            0,
            0,
        ));
    }

    let mut file = File::open(&request.file_path).await.map_err(|err| {
        UploadFailure::new(
            UploadFailureKind::InvalidFile,
            url.clone(),
            format!("open upload file failed: {err}"),
            0,
            0,
        )
    })?;
    let file_meta = file.metadata().await.map_err(|err| {
        UploadFailure::new(
            UploadFailureKind::InvalidFile,
            url.clone(),
            format!("read upload file metadata failed: {err}"),
            0,
            0,
        )
    })?;
    if !file_meta.is_file() {
        return Err(UploadFailure::new(
            UploadFailureKind::InvalidFile,
            url.clone(),
            "upload filePath must point to a regular file",
            0,
            0,
        ));
    }

    let (content_type, prefix, suffix) = match request.body_mode {
        UploadBodyMode::Multipart => {
            let boundary = multipart_boundary(&request);
            let (prefix, suffix) = build_multipart_parts(&request, &boundary, &file_name);
            (
                format!("multipart/form-data; boundary={boundary}"),
                prefix,
                suffix,
            )
        }
        // No framing: the file bytes are the whole body, so `total_bytes`
        // collapses to the file size and progress tracks it exactly.
        UploadBodyMode::Raw => (raw_content_type(&request), Vec::new(), Vec::new()),
    };
    let file_size = file_meta.len();
    let total_bytes = prefix.len() as u64 + file_size + suffix.len() as u64;

    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<UploadEvent>();
    let forwarder = rong::RongExecutor::global().spawn(async move {
        while let Some(event) = event_rx.recv().await {
            on_event(event);
        }
    });

    let (mut body_tx, body) = Channel::<Bytes, IoError>::new(8);
    let mut request_builder = HttpRequest::builder()
        .method(request.method.as_str())
        .uri(&url)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, total_bytes.to_string())
        .header(header::ACCEPT, "*/*");

    if let Some(headers) = request_builder.headers_mut() {
        if let Ok(ua_value) = http::HeaderValue::from_str(&user_agent_for_request(&request)) {
            headers.insert(header::USER_AGENT, ua_value);
        }
        for (name, value) in &request.headers {
            let normalized = name.trim().to_ascii_lowercase();
            if !should_forward_header(&normalized, request.body_mode) {
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
    }

    let request_obj = request_builder.body(body.boxed()).map_err(|err| {
        UploadFailure::new(
            UploadFailureKind::Internal,
            url.clone(),
            format!("build upload request failed: {err}"),
            0,
            total_bytes,
        )
    })?;

    let url_for_writer = url.clone();
    let event_tx_for_writer = event_tx.clone();
    let chunk_size = behavior.chunk_size.max(16 * 1024);
    let writer = rong::RongExecutor::global().spawn(async move {
        let mut uploaded_bytes = 0u64;
        let _ = event_tx_for_writer.send(UploadEvent::Started {
            url: url_for_writer.clone(),
            file_name: file_name.clone(),
            uploaded_bytes: 0,
            total_bytes,
        });

        if !prefix.is_empty() {
            body_tx.send_data(Bytes::from(prefix)).await.map_err(|_| {
                UploadFailure::new(
                    UploadFailureKind::Connection,
                    url_for_writer.clone(),
                    "upload request body closed before prefix was sent",
                    uploaded_bytes,
                    total_bytes,
                )
            })?;
            uploaded_bytes += (total_bytes - file_size - suffix.len() as u64)
                .min(total_bytes.saturating_sub(uploaded_bytes));
            let _ = event_tx_for_writer.send(UploadEvent::Progress {
                url: url_for_writer.clone(),
                uploaded_bytes,
                total_bytes,
            });
        }

        let mut buffer = vec![0u8; chunk_size];
        let mut last_emitted = uploaded_bytes;
        let mut last_emit_at = Instant::now();
        let mut remaining_file_bytes = file_size;

        while remaining_file_bytes > 0 {
            let read_limit = remaining_file_bytes.min(buffer.len() as u64) as usize;
            let read = file.read(&mut buffer[..read_limit]).await.map_err(|err| {
                UploadFailure::new(
                    UploadFailureKind::InvalidFile,
                    url_for_writer.clone(),
                    format!("read upload file failed: {err}"),
                    uploaded_bytes,
                    total_bytes,
                )
            })?;
            if read == 0 {
                return Err(UploadFailure::new(
                    UploadFailureKind::InvalidFile,
                    url_for_writer.clone(),
                    format!(
                        "upload file was truncated during transfer with {remaining_file_bytes} bytes remaining"
                    ),
                    uploaded_bytes,
                    total_bytes,
                ));
            }
            body_tx
                .send_data(Bytes::copy_from_slice(&buffer[..read]))
                .await
                .map_err(|_| {
                    UploadFailure::new(
                        UploadFailureKind::Connection,
                        url_for_writer.clone(),
                        "upload request body closed during file transfer",
                        uploaded_bytes,
                        total_bytes,
                    )
                })?;
            uploaded_bytes += read as u64;
            remaining_file_bytes -= read as u64;
            let should_emit = uploaded_bytes.saturating_sub(last_emitted)
                >= UPLOAD_PROGRESS_INTERVAL_BYTES
                || last_emit_at.elapsed().as_millis() >= UPLOAD_PROGRESS_INTERVAL_MILLIS;
            if should_emit {
                last_emitted = uploaded_bytes;
                last_emit_at = Instant::now();
                let _ = event_tx_for_writer.send(UploadEvent::Progress {
                    url: url_for_writer.clone(),
                    uploaded_bytes,
                    total_bytes,
                });
            }
        }

        if !suffix.is_empty() {
            body_tx.send_data(Bytes::from(suffix)).await.map_err(|_| {
                UploadFailure::new(
                    UploadFailureKind::Connection,
                    url_for_writer.clone(),
                    "upload request body closed before trailer was sent",
                    uploaded_bytes,
                    total_bytes,
                )
            })?;
            uploaded_bytes = total_bytes;
        }

        if uploaded_bytes != last_emitted {
            let _ = event_tx_for_writer.send(UploadEvent::Progress {
                url: url_for_writer,
                uploaded_bytes,
                total_bytes,
            });
        }

        Ok::<u64, UploadFailure>(uploaded_bytes)
    });

    let mut response =
        Some(host_http::send(request_obj, upload_request_options(behavior, abort_rx)).await);

    let uploaded_bytes = match writer.await {
        Ok(Ok(uploaded_bytes)) => uploaded_bytes,
        Ok(Err(err)) => {
            // The writer trips on a closed body whenever the request ends early,
            // so its error is the symptom. Whatever the request itself settled
            // on is the cause and wins: a status from a server that answered and
            // hung up mid-body -- how a presigned PUT refuses a signature -- or
            // a transport error such as a host the lxapp never trusted.
            let failure = match response.take() {
                Some(Ok(response_value)) if !response_value.status.is_success() => {
                    let status_code = response_value.status.as_u16();
                    let body = collect_response_body(response_value.body)
                        .await
                        .unwrap_or_default();
                    let error = String::from_utf8_lossy(&body).trim().to_string();
                    UploadFailure::new(
                        UploadFailureKind::Server,
                        request.url.clone(),
                        if error.is_empty() {
                            format!("http status {status_code}")
                        } else {
                            format!("http status {status_code}: {error}")
                        },
                        err.uploaded_bytes,
                        total_bytes,
                    )
                }
                Some(Err(transport_error)) => http_failure(
                    &transport_error,
                    request.url.clone(),
                    err.uploaded_bytes,
                    total_bytes,
                ),
                _ => err,
            };
            let event = if failure.kind == UploadFailureKind::Canceled {
                UploadEvent::Canceled {
                    url: request.url.clone(),
                    uploaded_bytes: failure.uploaded_bytes,
                    total_bytes,
                }
            } else {
                UploadEvent::Failed {
                    url: request.url.clone(),
                    error: failure.error.clone(),
                    uploaded_bytes: failure.uploaded_bytes,
                    total_bytes,
                }
            };
            let _ = event_tx.send(event);
            drop(event_tx);
            let _ = forwarder.await;
            return Err(failure);
        }
        Err(err) => {
            let failure = UploadFailure::new(
                UploadFailureKind::Internal,
                request.url.clone(),
                format!("upload writer task failed: {err}"),
                0,
                total_bytes,
            );
            let _ = event_tx.send(UploadEvent::Failed {
                url: request.url.clone(),
                error: failure.error.clone(),
                uploaded_bytes: 0,
                total_bytes,
            });
            drop(event_tx);
            let _ = forwarder.await;
            return Err(failure);
        }
    };

    match response.take().unwrap() {
        Ok(response) => {
            let status_code = response.status.as_u16();
            let body = collect_response_body(response.body)
                .await
                .map_err(|mut err| {
                    err.url = request.url.clone();
                    err.uploaded_bytes = uploaded_bytes;
                    err.total_bytes = total_bytes;
                    err
                })?;

            if !(200..300).contains(&status_code) {
                let error = String::from_utf8_lossy(&body).trim().to_string();
                let failure = UploadFailure::new(
                    UploadFailureKind::Server,
                    request.url.clone(),
                    if error.is_empty() {
                        format!("http status {status_code}")
                    } else {
                        format!("http status {status_code}: {error}")
                    },
                    uploaded_bytes,
                    total_bytes,
                );
                let _ = event_tx.send(UploadEvent::Failed {
                    url: request.url.clone(),
                    error: failure.error.clone(),
                    uploaded_bytes,
                    total_bytes,
                });
                drop(event_tx);
                let _ = forwarder.await;
                return Err(failure);
            }

            let _ = event_tx.send(UploadEvent::Completed {
                url: request.url.clone(),
                status_code,
                uploaded_bytes,
                total_bytes,
            });
            drop(event_tx);
            let _ = forwarder.await;
            Ok(UploadResult { status_code, body })
        }
        Err(err) => {
            let failure = http_failure(&err, request.url.clone(), uploaded_bytes, total_bytes);
            let event = if failure.kind == UploadFailureKind::Canceled {
                UploadEvent::Canceled {
                    url: request.url.clone(),
                    uploaded_bytes,
                    total_bytes,
                }
            } else {
                UploadEvent::Failed {
                    url: request.url.clone(),
                    error: failure.error.clone(),
                    uploaded_bytes,
                    total_bytes,
                }
            };
            let _ = event_tx.send(event);
            drop(event_tx);
            let _ = forwarder.await;
            Err(failure)
        }
    }
}

pub fn resolve_upload_file_name(path: &Path, override_name: Option<&str>) -> String {
    override_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            path.file_name()
                .and_then(|value| value.to_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "upload.bin".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn upload_request() -> UploadRequest {
        UploadRequest {
            url: "https://example.com/upload".to_string(),
            method: UploadMethod::Post,
            body_mode: UploadBodyMode::Multipart,
            file_path: PathBuf::from("upload.bin"),
            field_name: "file".to_string(),
            file_name: Some("upload.bin".to_string()),
            mime_type: Some("application/octet-stream".to_string()),
            headers: Vec::new(),
            form_fields: vec![("description".to_string(), "safe value".to_string())],
            user_agent: None,
        }
    }

    #[test]
    fn multipart_values_reject_control_characters() {
        let mut request = upload_request();
        assert_eq!(invalid_multipart_value(&request, "upload.bin"), None);

        request.file_name = Some("upload\r\nX-Injected: true".to_string());
        assert_eq!(
            invalid_multipart_value(&request, request.file_name.as_deref().unwrap()),
            Some("upload fileName cannot contain control characters")
        );

        request = upload_request();
        request.form_fields[0].1 = "safe\r\nunsafe".to_string();
        assert_eq!(
            invalid_multipart_value(&request, "upload.bin"),
            Some("upload form field values cannot contain control characters")
        );
    }

    #[test]
    fn multipart_header_values_still_escape_quotes_and_backslashes() {
        assert_eq!(escape_multipart_value("a\\b\"c"), "a\\\\b\\\"c");
    }

    #[test]
    fn raw_uploads_reject_form_fields_and_keep_the_caller_mime_type() {
        let mut request = upload_request();
        request.body_mode = UploadBodyMode::Raw;
        assert_eq!(
            invalid_raw_value(&request),
            Some("raw upload does not support form fields")
        );

        request.form_fields.clear();
        assert_eq!(invalid_raw_value(&request), None);
        assert_eq!(raw_content_type(&request), "application/octet-stream");

        request.mime_type = Some("video/mp4".to_string());
        assert_eq!(raw_content_type(&request), "video/mp4");

        request.mime_type = Some("   ".to_string());
        assert_eq!(raw_content_type(&request), "application/octet-stream");
    }

    #[test]
    fn only_raw_uploads_may_override_content_type() {
        assert!(!should_forward_header(
            "content-type",
            UploadBodyMode::Multipart
        ));
        assert!(should_forward_header("content-type", UploadBodyMode::Raw));
        assert!(!should_forward_header(
            "content-length",
            UploadBodyMode::Raw
        ));
        assert!(should_forward_header("x-amz-acl", UploadBodyMode::Raw));
    }
}

/// Request-shape tests against a throwaway loopback server. The showcase
/// automation covers uploads end to end, but only on macOS -- these hold the
/// wire format on every platform CI builds.
#[cfg(test)]
mod wire_tests {
    use super::*;
    use std::net::SocketAddr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::task::JoinHandle;

    const OK_RESPONSE: &[u8] = b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nok";

    /// Accepts one request, reads until the client stops, and answers `OK`.
    /// Returns the raw bytes the client sent.
    async fn serve_once() -> (SocketAddr, JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut seen = Vec::new();
            let mut buf = vec![0u8; 64 * 1024];
            loop {
                let read = socket.read(&mut buf).await.unwrap_or(0);
                if read == 0 {
                    break;
                }
                seen.extend_from_slice(&buf[..read]);
                // Headers plus the whole body have arrived once the client goes
                // quiet; every fixture here sends a known, small payload.
                if seen.len() >= 128 && socket.try_write(b"").is_ok() {
                    let idle =
                        tokio::time::timeout(Duration::from_millis(120), socket.read(&mut buf))
                            .await;
                    match idle {
                        Ok(Ok(0)) | Err(_) => break,
                        Ok(Ok(read)) => seen.extend_from_slice(&buf[..read]),
                        Ok(Err(_)) => break,
                    }
                }
            }
            let _ = socket.write_all(OK_RESPONSE).await;
            let _ = socket.flush().await;
            seen
        });
        (addr, handle)
    }

    /// Answers with `status` while the body is still arriving, then hangs up --
    /// how a presigned endpoint refuses a signature it disagrees with.
    ///
    /// It absorbs a slice of the body between answering and closing. Closing a
    /// socket that still holds unread data resets the connection, which would
    /// discard the answer in flight and leave an ordinary connection error in
    /// its place; draining a little first lets the client read the status,
    /// while leaving far too much body outstanding for the upload to finish.
    async fn refuse_once(status: u16) -> (SocketAddr, JoinHandle<()>) {
        const DRAIN_AFTER_ANSWER: usize = 256 * 1024;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 64 * 1024];
            let _ = socket.read(&mut buf).await;
            let response =
                format!("HTTP/1.1 {status} Forbidden\r\ncontent-length: 9\r\n\r\nrefused!!");
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.flush().await;

            let mut drained = 0usize;
            while drained < DRAIN_AFTER_ANSWER {
                match socket.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(read) => drained += read,
                }
            }
            drop(socket);
        });
        (addr, handle)
    }

    fn temp_file(name: &str, bytes: usize) -> PathBuf {
        let path = std::env::temp_dir().join(format!("lingxia-upload-{name}.bin"));
        std::fs::write(&path, vec![7u8; bytes]).unwrap();
        path
    }

    fn request_for(addr: SocketAddr, file: &Path, body_mode: UploadBodyMode) -> UploadRequest {
        UploadRequest {
            url: format!("http://{addr}/target"),
            method: match body_mode {
                UploadBodyMode::Raw => UploadMethod::Put,
                UploadBodyMode::Multipart => UploadMethod::Post,
            },
            body_mode,
            file_path: file.to_path_buf(),
            field_name: "asset".to_string(),
            file_name: Some("clip.mp4".to_string()),
            mime_type: Some("video/mp4".to_string()),
            headers: Vec::new(),
            form_fields: Vec::new(),
            user_agent: Some("lingxia-test/1.0".to_string()),
        }
    }

    async fn send(request: UploadRequest) -> Result<UploadResult, UploadFailure> {
        let (_abort_tx, abort_rx) = oneshot::channel();
        upload_file_with_behavior(request, UploadBehavior::default(), abort_rx, |_| {}).await
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_raw_body_is_the_file_and_nothing_else() {
        let (addr, server) = serve_once().await;
        let file = temp_file("raw", 128);

        let result = send(request_for(addr, &file, UploadBodyMode::Raw)).await;
        let seen = String::from_utf8_lossy(&server.await.unwrap()).to_string();
        let _ = std::fs::remove_file(&file);

        assert_eq!(result.unwrap().status_code, 200);
        assert!(seen.starts_with("PUT /target "), "request line: {seen}");
        let lower = seen.to_lowercase();
        assert!(lower.contains("content-type: video/mp4"), "{seen}");
        assert!(lower.contains("content-length: 128"), "{seen}");
        // No envelope: a presigned endpoint would store these bytes verbatim.
        assert!(!lower.contains("multipart/form-data"), "{seen}");
        assert!(!seen.contains("Content-Disposition"), "{seen}");
        assert!(seen.ends_with(&"\u{7}".repeat(128)), "body was reframed");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_multipart_body_still_carries_its_envelope() {
        let (addr, server) = serve_once().await;
        let file = temp_file("multipart", 128);
        let mut request = request_for(addr, &file, UploadBodyMode::Multipart);
        request.form_fields = vec![("note".to_string(), "spec".to_string())];

        let result = send(request).await;
        let seen = String::from_utf8_lossy(&server.await.unwrap()).to_string();
        let _ = std::fs::remove_file(&file);

        assert_eq!(result.unwrap().status_code, 200);
        assert!(seen.starts_with("POST /target "), "request line: {seen}");
        assert!(
            seen.to_lowercase()
                .contains("content-type: multipart/form-data; boundary="),
            "{seen}"
        );
        assert!(
            seen.contains(r#"name="asset"; filename="clip.mp4""#),
            "{seen}"
        );
        assert!(
            seen.contains(r#"name="note""#) && seen.contains("spec"),
            "{seen}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn only_a_raw_body_lets_the_caller_state_content_type() {
        let (raw_addr, raw_server) = serve_once().await;
        let file = temp_file("pinned", 128);

        // A presigned signature covers Content-Type, so the caller's value has
        // to reach the wire untouched.
        let mut raw = request_for(raw_addr, &file, UploadBodyMode::Raw);
        raw.headers = vec![
            ("Content-Type".to_string(), "image/avif".to_string()),
            ("User-Agent".to_string(), "spoofed/1.0".to_string()),
            ("Content-Length".to_string(), "1".to_string()),
        ];
        assert_eq!(send(raw).await.unwrap().status_code, 200);
        let seen = String::from_utf8_lossy(&raw_server.await.unwrap()).to_string();
        let lower = seen.to_lowercase();
        assert!(lower.contains("content-type: image/avif"), "{seen}");
        assert!(
            !lower.contains("video/mp4"),
            "mimeType should have lost: {seen}"
        );
        // The runtime owns its identity and the derived length regardless.
        assert!(!lower.contains("spoofed/1.0"), "{seen}");
        assert!(lower.contains("lingxia-test/1.0"), "{seen}");
        assert!(lower.contains("content-length: 128"), "{seen}");

        // Multipart owns the header instead: it carries the boundary the
        // server parses by, so a caller value must not replace it.
        let (multipart_addr, multipart_server) = serve_once().await;
        let mut multipart = request_for(multipart_addr, &file, UploadBodyMode::Multipart);
        multipart.headers = vec![("Content-Type".to_string(), "text/plain".to_string())];
        assert_eq!(send(multipart).await.unwrap().status_code, 200);
        let seen = String::from_utf8_lossy(&multipart_server.await.unwrap()).to_string();
        let _ = std::fs::remove_file(&file);
        assert!(
            seen.to_lowercase()
                .contains("content-type: multipart/form-data; boundary="),
            "{seen}"
        );
        assert!(!seen.to_lowercase().contains("text/plain"), "{seen}");
    }

    /// Accepts the connection and hangs up without answering at all.
    async fn drop_once() -> (SocketAddr, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            drop(socket);
        });
        (addr, handle)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_transport_error_beats_the_writers_broken_pipe() {
        let (addr, server) = drop_once().await;
        let file = temp_file("dropped", 32 * 1024 * 1024);

        let failure = send(request_for(addr, &file, UploadBodyMode::Raw))
            .await
            .expect_err("a connection that never answers is not a success");
        server.await.unwrap();
        let _ = std::fs::remove_file(&file);

        // Both sides fail here, and only the request's own error carries why.
        // The lxapp-facing codes are derived from it, so a denied host reaching
        // the caller as a closed body would read as a flaky network.
        assert!(
            !failure.error.contains("upload request body closed"),
            "the writer's symptom reached the caller: {}",
            failure.error
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_refusal_mid_body_reports_the_status_not_the_broken_pipe() {
        let (addr, server) = refuse_once(403).await;
        // Far more than the server drains, so the body is guaranteed to still
        // be outstanding when the connection dies.
        let file = temp_file("refused", 32 * 1024 * 1024);

        let failure = send(request_for(addr, &file, UploadBodyMode::Raw))
            .await
            .expect_err("a 403 must not read as success");
        server.await.unwrap();
        let _ = std::fs::remove_file(&file);

        // Without the status, a rejected signature is indistinguishable from a
        // flaky network, which is the one thing the caller must be able to tell.
        assert_eq!(failure.kind, UploadFailureKind::Server, "{}", failure.error);
        assert!(failure.error.contains("403"), "{}", failure.error);
        assert!(failure.error.contains("refused!!"), "{}", failure.error);
    }
}
