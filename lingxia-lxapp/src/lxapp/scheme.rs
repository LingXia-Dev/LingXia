use http::{Request, Response, StatusCode};

use crate::error;
use crate::lxapp::LxApp;

impl LxApp {
    /// Handler for lx:// scheme requests to access static app assets (images, CSS, JS, etc.)
    /// HTML files are handled separately through generate_page_html and load_data
    pub(crate) fn lingxia_handler(&self, req: Request<Vec<u8>>) -> Option<Response<Vec<u8>>> {
        let uri = req.uri();
        let path = uri.path().trim_start_matches('/');

        // Try to read the static asset from app directory
        let file_result = self.read_bytes(path);

        let response = match file_result {
            Ok(data) => {
                // Determine MIME type based on file extension
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
                error!("Static asset not found: {} - {}", path, e).with_appid(self.appid.clone());

                // Return a 404 Not Found response for static assets
                Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .header("Content-Type", "text/plain")
                    .body(
                        format!("Static asset not found: {}", path)
                            .as_bytes()
                            .to_vec(),
                    )
                    .unwrap()
            }
        };

        Some(response)
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
                return Some(
                    Response::builder()
                        .status(StatusCode::FORBIDDEN)
                        .header("Content-Type", "text/plain")
                        .body(format!("Access to domain '{}' is not allowed", host).into_bytes())
                        .unwrap(),
                );
            }

            // Check if this is likely an API request based on request headers
            let is_api_request = self.is_api_request(&req);
            if is_api_request {
                return Some(
                    Response::builder()
                        .status(StatusCode::FORBIDDEN)
                        .header("Content-Type", "text/plain")
                        .body(
                            format!(
                                "API requests are not allowed. Domain: {}, Path: {}",
                                host,
                                uri.path()
                            )
                            .as_bytes()
                            .to_vec(),
                        )
                        .unwrap(),
                );
            }

            // Check if the request is for an allowed resource type based on URL
            let is_allowed_resource = self.is_allowed_resource_by_url(uri.path());

            if !is_allowed_resource {
                // Check if this looks like a typical web page request (no extension)
                let path = uri.path();
                let has_extension = path.rfind('.').is_some();

                if has_extension {
                    // Has extension but not in our allowed list
                    return Some(
                        Response::builder()
                            .status(StatusCode::FORBIDDEN)
                            .header("Content-Type", "text/plain")
                            .body(
                                format!(
                                    "Resource type not allowed. Domain: {}, Path: {}",
                                    host, path
                                )
                                .as_bytes()
                                .to_vec(),
                            )
                            .unwrap(),
                    );
                }
            }

            // Resource type is allowed or undetermined, let the request proceed
            return None;
        }

        // URI doesn't have a host component
        Some(
            Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header("Content-Type", "text/plain")
                .body("Invalid URL: missing host".as_bytes().to_vec())
                .unwrap(),
        )
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
}
