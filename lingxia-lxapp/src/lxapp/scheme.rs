use crate::info;
use http::{Method, Request, Response, StatusCode, Uri};
use lingxia_webview::{SystemPipeReader, WebResourceResponse};
use rong::net;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use urlencoding::decode;

use crate::error;
use crate::error::LxAppError;
use crate::lxapp::LxApp;

impl LxApp {
    const PROXY_SEGMENT: &'static str = "_LINGXIA_";

    /// Handler for lx:// scheme requests to access static app assets (images, CSS, JS, etc.)
    /// HTML files are handled separately through generate_page_html and load_data
    pub(crate) fn lingxia_handler(&self, req: Request<Vec<u8>>) -> Option<WebResourceResponse> {
        let uri = req.uri().clone();

        if let Some(target_uri) = Self::extract_proxy_target(&uri) {
            return self.handle_lingxia_proxy(target_uri);
        }

        let asset_path = match self.resolve_lx_uri(&uri) {
            Ok(path) => path,
            Err(e) => {
                error!("Asset not found: {} - {}", uri.path(), e).with_appid(self.appid.clone());
                return Some(self.create_error_response(
                    StatusCode::NOT_FOUND,
                    "Asset Not Found",
                    &format!("The requested asset '{}' could not be found.", uri.path()),
                ));
            }
        };

        let metadata = match fs::metadata(&asset_path) {
            Ok(meta) => meta,
            Err(e) => {
                error!(
                    "Failed to read asset metadata: {} - {}",
                    asset_path.display(),
                    e
                )
                .with_appid(self.appid.clone());
                return Some(self.create_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Asset Error",
                    "Failed to read asset metadata.",
                ));
            }
        };

        let file_len = metadata.len();
        let mime_type = Self::infer_mime_type(uri.path());

        let mut builder = Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", mime_type)
            .header("Access-Control-Allow-Origin", "null");

        if let Ok(value) = http::HeaderValue::from_str(&file_len.to_string()) {
            builder = builder.header("Content-Length", value);
        }

        let response = builder.body(()).unwrap_or_else(|_| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(())
                .expect("Failed to build fallback empty response")
        });

        let (parts, _) = response.into_parts();
        Some((parts, asset_path).into())
    }

    fn handle_lingxia_proxy(&self, target_uri: Uri) -> Option<WebResourceResponse> {
        let target_str = target_uri.to_string();

        //info!("lingxia_proxy: forwarding request to {}", target_str).with_appid(self.appid.clone());

        if target_uri.scheme_str() != Some("https") {
            return Some(self.create_error_response(
                StatusCode::BAD_REQUEST,
                "Unsupported Scheme",
                "Only https URLs are allowed in proxy requests.",
            ));
        }

        let host = match target_uri.host() {
            Some(host) => host,
            None => {
                return Some(self.create_error_response(
                    StatusCode::BAD_REQUEST,
                    "Invalid URL",
                    "Proxy target is missing host component.",
                ));
            }
        };

        if !self
            .state
            .lock()
            .unwrap()
            .network_security
            .is_domain_allowed(host)
        {
            return Some(self.create_error_response(
                StatusCode::FORBIDDEN,
                "Domain Access Denied",
                &format!(
                    "Access to domain '{}' is not allowed by the security policy.",
                    host
                ),
            ));
        }

        let proxy_request = match Request::builder()
            .method(Method::GET)
            .uri(target_uri)
            .body(Vec::new())
        {
            Ok(req) => req,
            Err(e) => {
                error!(
                    "lingxia_proxy: failed to build proxy request {}: {}",
                    target_str, e
                )
                .with_appid(self.appid.clone());

                return Some(self.create_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Proxy Request Error",
                    &format!("Failed to prepare proxy request: {}", e),
                ));
            }
        };

        self.https_handler(proxy_request)
    }

    fn extract_proxy_target(uri: &Uri) -> Option<Uri> {
        if !uri.path().contains(Self::PROXY_SEGMENT) {
            return None;
        }

        let query = uri.query()?;
        for pair in query.split('&') {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next()?;
            let value = parts.next()?;

            if key != "url" {
                continue;
            }

            let decoded = decode(value).ok()?;
            let decoded_str = decoded.trim();

            if !decoded_str.starts_with("https://") {
                return None;
            }

            return Uri::from_str(decoded_str).ok();
        }

        None
    }

    fn resolve_lx_uri(&self, uri: &Uri) -> Result<PathBuf, LxAppError> {
        if uri.scheme_str() != Some("lx") {
            return Err(LxAppError::InvalidParameter(format!(
                "unsupported scheme: {}",
                uri
            )));
        }

        if uri.host() != Some(self.appid.as_str()) {
            return Err(LxAppError::ResourceNotFound(uri.to_string()));
        }

        let path_str = uri.path();
        let raw_path = path_str.trim_start_matches('/');
        if raw_path.is_empty() {
            return Err(LxAppError::ResourceNotFound(uri.to_string()));
        }

        self.resolve_accessible_path(raw_path)
    }

    /// Handler for HTTPS requests:
    /// - Only accepts GET; other methods are rejected
    /// - All GETs to allowed domains are treated as downloadable resources
    ///   with per-app cache; miss -> stream via system pipe while saving
    pub(crate) fn https_handler(&self, req: Request<Vec<u8>>) -> Option<WebResourceResponse> {
        let uri = req.uri();

        // Check if the domain is allowed
        if let Some(host) = uri.host() {
            // First check domain whitelist
            if !self
                .state
                .lock()
                .unwrap()
                .network_security
                .is_domain_allowed(host)
            {
                return Some(self.create_error_response(
                    StatusCode::FORBIDDEN,
                    "Domain Access Denied",
                    &format!(
                        "Access to domain '{}' is not allowed by the security policy.",
                        host
                    ),
                ));
            }

            // Only accept GET; reject others
            if req.method() != http::Method::GET {
                return Some(self.create_error_response(
                    StatusCode::METHOD_NOT_ALLOWED,
                    "Method Not Allowed",
                    &format!(
                        "Only GET is allowed in WebView: method={} {}",
                        req.method(),
                        uri
                    ),
                ));
            }

            let url_str = uri.to_string();
            // Decide extension from URL or query
            let ext_opt = url_ext_from_uri(uri);
            let ext = ext_opt.as_deref().unwrap_or("bin");
            match self.cache().resolve_path_with_ext(&url_str, ext) {
                crate::cache::ResolveResult::Exists(file_path) => {
                    // Cached: serve file path
                    let mime_type = ext_opt
                        .as_deref()
                        .map(Self::infer_mime_type_ext)
                        .unwrap_or_else(|| Self::infer_mime_type(uri.path()));
                    let builder = http::Response::builder()
                        .status(StatusCode::OK)
                        .header("Content-Type", mime_type)
                        .header("Access-Control-Allow-Origin", "null");
                    let response = builder.body(()).unwrap_or_else(|_| {
                        http::Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .body(())
                            .expect("Failed to build cached file response")
                    });
                    let (parts, _) = response.into_parts();
                    return Some((parts, file_path).into());
                }
                crate::cache::ResolveResult::NonExists(dest_path) => {
                    #[cfg(unix)]
                    {
                        match create_pipe_sink() {
                            Ok((reader, sink)) => {
                                // info!(
                                //     "https_handler: cache miss, start pipe download -> {}",
                                //     dest_path.display()
                                // )
                                // .with_appid(self.appid.clone());

                                let mime_type = ext_opt
                                    .as_deref()
                                    .map(Self::infer_mime_type_ext)
                                    .unwrap_or_else(|| Self::infer_mime_type(uri.path()));
                                let builder = http::Response::builder()
                                    .status(StatusCode::OK)
                                    .header("Content-Type", mime_type)
                                    .header("Access-Control-Allow-Origin", "null");

                                let response = builder.body(()).unwrap_or_else(|_| {
                                    http::Response::builder()
                                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                                        .body(())
                                        .expect("Failed to build pipe response")
                                });
                                let (parts, _) = response.into_parts();

                                match net::request_download(
                                    url_str.clone(),
                                    dest_path.clone(),
                                    None,
                                    Some(sink),
                                ) {
                                    Ok(_rx) => {
                                        info!("https_handler: pipe ready for {}", uri.path())
                                            .with_appid(self.appid.clone());
                                        return Some((parts, reader).into());
                                    }
                                    Err(e) => {
                                        error!(
                                            "https_handler: failed to start download task: {}",
                                            e
                                        )
                                        .with_appid(self.appid.clone());
                                        return Some(self.create_error_response(
                                            StatusCode::BAD_GATEWAY,
                                            "Download Failed",
                                            &format!("Failed to start download: {}", e),
                                        ));
                                    }
                                }
                            }
                            Err(e) => {
                                error!("https_handler: failed to create pipe sink: {}", e)
                                    .with_appid(self.appid.clone());
                                return Some(self.create_error_response(
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    "Pipe Creation Failed",
                                    &e,
                                ));
                            }
                        }
                    }
                    #[cfg(not(unix))]
                    {
                        // Fallback: no pipe support here; return 501
                        warn!(
                            "https_handler: pipe unsupported on this platform for {}",
                            uri
                        )
                        .with_appid(self.appid.clone());
                        return Some(self.create_error_response(
                            StatusCode::NOT_IMPLEMENTED,
                            "Pipe Unsupported",
                            "Streaming via pipe is not supported on this platform.",
                        ));
                    }
                }
            }
        }

        // URI doesn't have a host component
        Some(self.create_error_response(
            StatusCode::BAD_REQUEST,
            "Invalid URL",
            "The URL is missing a host component and cannot be processed.",
        ))
    }

    fn infer_mime_type(path: &str) -> &'static str {
        if path.ends_with(".js") {
            "application/javascript"
        } else if path.ends_with(".css") {
            "text/css"
        } else if path.ends_with(".png") {
            "image/png"
        } else if path.ends_with(".jpg") || path.ends_with(".jpeg") {
            "image/jpeg"
        } else if path.ends_with(".gif") {
            "image/gif"
        } else if path.ends_with(".svg") {
            "image/svg+xml"
        } else if path.ends_with(".webp") {
            "image/webp"
        } else if path.ends_with(".ico") {
            "image/x-icon"
        } else if path.ends_with(".json") {
            "application/json"
        } else if path.ends_with(".woff") {
            "font/woff"
        } else if path.ends_with(".woff2") {
            "font/woff2"
        } else if path.ends_with(".ttf") {
            "font/ttf"
        } else if path.ends_with(".mp3") {
            "audio/mpeg"
        } else if path.ends_with(".wav") {
            "audio/wav"
        } else if path.ends_with(".mp4") {
            "video/mp4"
        } else {
            "application/octet-stream"
        }
    }

    fn infer_mime_type_ext(ext: &str) -> &'static str {
        match ext.to_ascii_lowercase().as_str() {
            "js" => "application/javascript",
            "css" => "text/css",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "svg" => "image/svg+xml",
            "webp" => "image/webp",
            "ico" => "image/x-icon",
            "json" => "application/json",
            "woff" => "font/woff",
            "woff2" => "font/woff2",
            "ttf" => "font/ttf",
            "mp3" => "audio/mpeg",
            "wav" => "audio/wav",
            "mp4" => "video/mp4",
            _ => "application/octet-stream",
        }
    }

    fn sanitize_for_filename(value: &str) -> String {
        value
            .chars()
            .map(|c| match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' => c,
                _ => '_',
            })
            .collect()
    }
    /// Create a simple centered error response
    pub(crate) fn create_error_response(
        &self,
        status: StatusCode,
        title: &str,
        message: &str,
    ) -> WebResourceResponse {
        let html_content = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>{}</title>
    <style>
        body {{ font-family: system-ui, sans-serif; margin: 0; padding: 20px; background: #f5f5f5; display: flex; justify-content: center; align-items: center; min-height: 100vh; }}
        .error {{ background: white; border-radius: 8px; padding: 40px; text-align: center; max-width: 500px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }}
        .code {{ font-size: 48px; font-weight: bold; color: #e74c3c; margin-bottom: 16px; }}
        .title {{ font-size: 24px; font-weight: 600; color: #2c3e50; margin-bottom: 16px; }}
        .message {{ font-size: 16px; color: #7f8c8d; line-height: 1.5; }}
    </style>
</head>
<body>
    <div class="error">
        <div class="code">{}</div>
        <div class="title">{}</div>
        <div class="message">{}</div>
    </div>
</body>
</html>"#,
            title,
            status.as_u16(),
            title,
            message
        );

        let mut target_dir = self.user_cache_dir.join("webview_errors");
        if let Err(e) = fs::create_dir_all(&target_dir) {
            error!("Failed to prepare error directory: {}", e).with_appid(self.appid.clone());
            target_dir = std::env::temp_dir().join("lingxia-webview-errors");
            let _ = fs::create_dir_all(&target_dir);
        }

        let file_name = format!(
            "{}_{}.html",
            status.as_u16(),
            Self::sanitize_for_filename(title)
        );
        let mut file_path = target_dir.join(file_name);

        if let Err(e) = fs::write(&file_path, html_content.as_bytes()) {
            error!(
                "Failed to write error response file ({}): {}",
                file_path.display(),
                e
            )
            .with_appid(self.appid.clone());
            let unique_suffix = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            file_path = target_dir.join(format!(
                "{}_{}_{}.html",
                status.as_u16(),
                Self::sanitize_for_filename(title),
                unique_suffix
            ));
            let _ = fs::write(&file_path, html_content.as_bytes());
        }

        let file_len = fs::metadata(&file_path)
            .map(|meta| meta.len())
            .unwrap_or_else(|_| html_content.as_bytes().len() as u64);

        let mut builder = Response::builder()
            .status(status)
            .header("Content-Type", "text/html; charset=utf-8");

        if let Ok(value) = http::HeaderValue::from_str(&file_len.to_string()) {
            builder = builder.header("Content-Length", value);
        }

        let response = builder.body(()).unwrap_or_else(|_| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(())
                .expect("Failed to build fallback error response")
        });

        let (parts, _) = response.into_parts();
        (parts, file_path).into()
    }
}

fn url_ext_from_uri(uri: &Uri) -> Option<String> {
    // Try path first
    if let Some(ext) = ext_from_segment(uri.path()) {
        return Some(ext.to_string());
    }
    // Then try query (e.g., id=...UHD.jpg)
    if let Some(q) = uri.query() {
        let lower = q.to_ascii_lowercase();
        if let Some(pos) = lower.rfind('.') {
            let tail = &lower[pos + 1..];
            let end: String = tail
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            if !end.is_empty() && end.len() <= 8 {
                return Some(end);
            }
        }
    }
    None
}

fn ext_from_segment(path: &str) -> Option<&str> {
    let seg = path.rsplit('/').next().unwrap_or(path);
    let dot = seg.rfind('.')?;
    let ext = &seg[dot + 1..];
    if ext.len() >= 1 && ext.len() <= 8 {
        Some(ext)
    } else {
        None
    }
}

#[cfg(unix)]
struct PipeBodySink {
    writer: std::os::unix::net::UnixStream,
}

#[cfg(unix)]
impl rong::net::BodySink for PipeBodySink {
    fn write(&mut self, chunk: &[u8]) -> Result<(), String> {
        use std::io::Write;
        self.writer
            .write_all(chunk)
            .map_err(|e| format!("pipe write: {}", e))
    }

    fn close(&mut self, _result: &Result<(), String>) {
        use std::net::Shutdown;
        let _ = self.writer.shutdown(Shutdown::Write);
    }
}

#[cfg(unix)]
fn create_pipe_sink() -> Result<(SystemPipeReader, Box<dyn rong::net::BodySink + Send>), String> {
    use std::os::fd::IntoRawFd;
    use std::os::unix::net::UnixStream;

    let (read_end, write_end) = UnixStream::pair().map_err(|e| format!("pipe: {}", e))?;
    let read_fd = read_end.into_raw_fd();
    let reader = unsafe { SystemPipeReader::from_raw_fd(read_fd) };
    let sink: Box<dyn rong::net::BodySink + Send> = Box::new(PipeBodySink { writer: write_end });
    Ok((reader, sink))
}
