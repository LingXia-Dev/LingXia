use http::{Request, Response, StatusCode, Uri};
use lingxia_webview::WebResourceResponse;
use std::fs;
use std::path::PathBuf;

use crate::error;
use crate::error::LxAppError;
use crate::lxapp::LxApp;

impl LxApp {
    /// Handler for lx:// scheme requests to access static app assets (images, CSS, JS, etc.)
    /// HTML files are handled separately through generate_page_html and load_data
    pub(crate) fn lingxia_handler(&self, req: Request<Vec<u8>>) -> Option<WebResourceResponse> {
        let uri = req.uri();
        let asset_path = match self.resolve_lx_uri(uri) {
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
        Some(WebResourceResponse::new(parts, asset_path))
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

    /// Handler for HTTPS requests to check domain whitelist and restrict to static resources
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

            // Check if this is likely an API request based on request headers
            let is_api_request = self.is_api_request(&req);
            if is_api_request {
                return Some(self.create_error_response(
                    StatusCode::FORBIDDEN,
                    "API Request Blocked",
                    &format!(
                        "API requests are not allowed. Domain: {}, Path: {}",
                        host,
                        uri.path()
                    ),
                ));
            }

            // Check if the request is for an allowed resource type based on URL
            let is_allowed_resource = self.is_allowed_resource_by_url(uri.path());

            if !is_allowed_resource {
                // Check if this looks like a typical web page request (no extension)
                let path = uri.path();
                let has_extension = path.rfind('.').is_some();

                if has_extension {
                    // Has extension but not in our allowed list
                    return Some(self.create_error_response(
                        StatusCode::FORBIDDEN,
                        "Resource Type Not Allowed",
                        &format!(
                            "The requested resource type is not allowed. Domain: {}, Path: {}",
                            host, path
                        ),
                    ));
                }
            }

            // Resource type is allowed or undetermined, let the request proceed
            return None;
        }

        // URI doesn't have a host component
        Some(self.create_error_response(
            StatusCode::BAD_REQUEST,
            "Invalid URL",
            "The URL is missing a host component and cannot be processed.",
        ))
    }

    /// Check if a request is likely an API request based on headers
    fn is_api_request(&self, req: &Request<Vec<u8>>) -> bool {
        // Check Content-Type for POST/PUT requests
        if let Some(content_type) = req
            .headers()
            .get("Content-Type")
            .and_then(|h| h.to_str().ok())
        {
            if content_type.contains("application/json") || content_type.contains("application/xml")
            {
                return true;
            }
        }

        // Check for common API path patterns
        let path = req.uri().path().to_lowercase();
        if path.contains("/api/") || path.contains("/rest/") || path.contains("/graphql") {
            return true;
        }

        false
    }

    /// Check if a URL path represents an allowed static resource
    fn is_allowed_resource_by_url(&self, path: &str) -> bool {
        let extension = path.rfind('.').map(|pos| path[pos + 1..].to_lowercase());

        match extension.as_ref() {
            Some(ext) => {
                matches!(
                    ext.as_str(),
                    // Images
                    "jpg" | "jpeg" | "png" | "gif" | "svg" | "webp" | "ico" | "bmp" | "tiff" |
                    // Audio
                    "mp3" | "wav" | "ogg" | "aac" | "flac" | "m4a" |
                    // Video
                    "mp4" | "webm" | "ogv" | "avi" | "mov" | "wmv" | "flv" |
                    // Multimedia playlist
                    "m3u" | "m3u8" | "pls" |
                    // Fonts
                    "ttf" | "woff" | "woff2" | "eot" | "otf" |
                    // Scripts and styles (from trusted domains)
                    "js" | "css" | "mjs" |
                    // Documents and archives (common static files)
                    "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" |
                    "zip" | "rar" | "7z" | "tar" | "gz" |
                    // Text files
                    "txt" | "md" | "csv" |
                    // Data files
                    "json" | "xml" | "yaml" | "yml"
                )
            }
            None => false, // No extension - could be anything
        }
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
        WebResourceResponse::new(parts, file_path)
    }
}
