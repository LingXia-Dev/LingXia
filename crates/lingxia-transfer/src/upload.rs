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

fn should_forward_header(name: &str) -> bool {
    !matches!(
        name,
        "content-length" | "content-type" | "host" | "referer" | "transfer-encoding" | "user-agent"
    )
}

fn escape_multipart_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
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

    let file_meta = tokio::fs::metadata(&request.file_path)
        .await
        .map_err(|err| {
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

    let file_name = file_name_for_request(&request);
    let boundary = multipart_boundary(&request);
    let (prefix, suffix) = build_multipart_parts(&request, &boundary, &file_name);
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
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .header(header::CONTENT_LENGTH, total_bytes.to_string())
        .header(header::ACCEPT, "*/*");

    if let Some(headers) = request_builder.headers_mut() {
        if let Ok(ua_value) = http::HeaderValue::from_str(&user_agent_for_request(&request)) {
            headers.insert(header::USER_AGENT, ua_value);
        }
        for (name, value) in &request.headers {
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

    let file_path = request.file_path.clone();
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

        let mut file = File::open(&file_path).await.map_err(|err| {
            UploadFailure::new(
                UploadFailureKind::InvalidFile,
                url_for_writer.clone(),
                format!("open upload file failed: {err}"),
                uploaded_bytes,
                total_bytes,
            )
        })?;
        let mut buffer = vec![0u8; chunk_size];
        let mut last_emitted = uploaded_bytes;
        let mut last_emit_at = Instant::now();

        loop {
            let read = file.read(&mut buffer).await.map_err(|err| {
                UploadFailure::new(
                    UploadFailureKind::InvalidFile,
                    url_for_writer.clone(),
                    format!("read upload file failed: {err}"),
                    uploaded_bytes,
                    total_bytes,
                )
            })?;
            if read == 0 {
                break;
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
            if err.error == UPLOAD_CANCELED_ERROR {
                if let Some(Ok(response_value)) = response.take() {
                    if !response_value.status.is_success() {
                        let status_code = response_value.status.as_u16();
                        let body = collect_response_body(response_value.body)
                            .await
                            .unwrap_or_default();
                        let error = String::from_utf8_lossy(&body).trim().to_string();
                        let failure = UploadFailure::new(
                            UploadFailureKind::Server,
                            request.url.clone(),
                            if error.is_empty() {
                                format!("http status {status_code}")
                            } else {
                                format!("http status {status_code}: {error}")
                            },
                            err.uploaded_bytes,
                            total_bytes,
                        );
                        let _ = event_tx.send(UploadEvent::Failed {
                            url: request.url.clone(),
                            error: failure.error.clone(),
                            uploaded_bytes: failure.uploaded_bytes,
                            total_bytes,
                        });
                        drop(event_tx);
                        let _ = forwarder.await;
                        return Err(failure);
                    }
                }
            }
            let event = if err.error == UPLOAD_CANCELED_ERROR {
                UploadEvent::Canceled {
                    url: request.url.clone(),
                    uploaded_bytes: err.uploaded_bytes,
                    total_bytes,
                }
            } else {
                UploadEvent::Failed {
                    url: request.url.clone(),
                    error: err.error.clone(),
                    uploaded_bytes: err.uploaded_bytes,
                    total_bytes,
                }
            };
            let _ = event_tx.send(event);
            drop(event_tx);
            let _ = forwarder.await;
            return Err(err);
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
            let message = err.to_string();
            let kind = if err.kind() == host_http::HttpErrorKind::AccessDenied {
                UploadFailureKind::AccessDenied
            } else {
                classify_transport_upload_failure(&message)
            };
            let failure = UploadFailure::new(
                kind,
                request.url.clone(),
                message.clone(),
                uploaded_bytes,
                total_bytes,
            );
            let event = if message == "aborted" {
                UploadEvent::Canceled {
                    url: request.url.clone(),
                    uploaded_bytes,
                    total_bytes,
                }
            } else {
                UploadEvent::Failed {
                    url: request.url.clone(),
                    error: message,
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
