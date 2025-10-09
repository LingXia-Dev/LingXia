use http::{Request, Response, StatusCode, Uri};
use std::fs;
use std::path::PathBuf;

use crate::error;
use crate::error::LxAppError;
use crate::lxapp::LxApp;

impl LxApp {
    /// Handler for lx:// scheme requests to access static app assets (images, CSS, JS, etc.)
    /// HTML files are handled separately through generate_page_html and load_data
    pub(crate) fn lingxia_handler(&self, req: Request<Vec<u8>>) -> Option<Response<Vec<u8>>> {
        let uri = req.uri();
        let file_result = self.read_bytes_from_lx_uri(uri);

        let response = match file_result {
            Ok(data) => {
                // Determine MIME type based on file extension
                let path = uri.path();
                let mime_type = if path.ends_with(".js") {
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
                } else {
                    "application/octet-stream"
                };

                Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", mime_type)
                    .header("Content-Length", data.len().to_string())
                    .header("Access-Control-Allow-Origin", "null") // Solve CORS issues on the HarmonyOS platform with the Access-Control-Allow-Origin header
                    .body(data)
                    .unwrap_or_else(|_| {
                        Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .body(Vec::new())
                            .unwrap()
                    })
            }
            Err(e) => {
                error!("Asset not found: {} - {}", uri.path(), e).with_appid(self.appid.clone());

                // Return a styled 404 Not Found response
                Self::create_error_response(
                    StatusCode::NOT_FOUND,
                    "Asset Not Found",
                    &format!("The requested asset '{}' could not be found.", uri.path()),
                )
            }
        };

        Some(response)
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

    fn read_bytes_from_lx_uri(&self, uri: &Uri) -> Result<Vec<u8>, LxAppError> {
        let resolved_path = self.resolve_lx_uri(uri)?;

        fs::read(&resolved_path)
            .map_err(|e| LxAppError::ResourceNotFound(format!("{}:{}", uri.path(), e)))
    }

    /// Handler for HTTPS requests to check domain whitelist and restrict to static resources
    pub(crate) fn https_handler(&self, req: Request<Vec<u8>>) -> Option<Response<Vec<u8>>> {
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
                return Some(Self::create_error_response(
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
                return Some(Self::create_error_response(
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
                    return Some(Self::create_error_response(
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
        Some(Self::create_error_response(
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
    /// Create a simple centered error response
    fn create_error_response(status: StatusCode, title: &str, message: &str) -> Response<Vec<u8>> {
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

        Response::builder()
            .status(status)
            .header("Content-Type", "text/html; charset=utf-8")
            .body(html_content.into_bytes())
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header("Content-Type", "text/plain")
                    .body("Internal Server Error".as_bytes().to_vec())
                    .unwrap()
            })
    }
}
