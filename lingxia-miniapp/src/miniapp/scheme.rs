use http::{Request, Response, StatusCode};

use crate::error;
use crate::miniapp::LxApp;

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

            // Then check if the request is for an allowed resource type
            let path = uri.path();
            let extension = path.rfind('.').map(|pos| path[pos + 1..].to_lowercase());

            // Allow media resources and JavaScript/CSS from trusted domain
            let is_allowed_resource = match extension.as_ref() {
                Some(ext) => {
                    let is_media_resource = matches!(
                        ext.as_str(),
                        // Images
                        "jpg" | "jpeg" | "png" | "gif" | "svg" | "webp" | "ico" |
                        // Audio
                        "mp3" | "wav" | "ogg" |
                        // Video
                        "mp4" | "webm" | "ogv" |
                        // multimedia playlist
                        "m3u" | "m3u8"|
                        // Fonts
                        "ttf" | "woff" | "woff2" | "eot"
                    );

                    let is_script_or_style = matches!(ext.as_str(), "js" | "css");

                    // Allow media resources always, and scripts/styles only from trusted domain
                    is_media_resource || is_script_or_style
                }
                None => false, // No extension, likely not a static resource
            };

            // Check content type in the Accept header if available
            let accept_header = req.headers().get("Accept").and_then(|h| h.to_str().ok());
            let is_api_request = match accept_header {
                Some(accept) => {
                    accept.contains("application/json")
                        || accept.contains("application/xml")
                        || (accept.contains("application/") && !accept.contains("javascript"))
                }
                None => false,
            };

            // Block API requests or non-allowed resource types
            if is_api_request || !is_allowed_resource {
                return Some(
                    Response::builder()
                        .status(StatusCode::FORBIDDEN)
                        .header("Content-Type", "text/plain")
                        .body(
                            format!("Only static resources are allowed. Domain: {}, Path: {}, Extension: {:?}",
                                   host, path, extension).as_bytes().to_vec(),
                        )
                        .unwrap(),
                );
            }

            // Resource type is allowed, let the request proceed
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
}
