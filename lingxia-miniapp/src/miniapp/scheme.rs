use http::{Request, Response, StatusCode};

use crate::miniapp::MiniApp;

impl MiniApp {
    /// Handler for lingxia:// scheme requests to access app assets
    pub(crate) fn lingxia_handler(&self, req: Request<Vec<u8>>) -> Option<Response<Vec<u8>>> {
        let uri = req.uri();

        // Get the path part after lingxia://
        let path = uri.path().trim_start_matches('/');

        // Try to read the asset from app directory
        let file_result = self.read_bytes(path);

        let response = match file_result {
            Ok(data) => {
                // Determine MIME type based on file extension
                let mime_type = if path.ends_with(".html") {
                    "text/html"
                } else if path.ends_with(".js") {
                    "application/javascript"
                } else if path.ends_with(".css") {
                    "text/css"
                } else if path.ends_with(".png") {
                    "image/png"
                } else if path.ends_with(".jpg") || path.ends_with(".jpeg") {
                    "image/jpeg"
                } else if path.ends_with(".svg") {
                    "image/svg+xml"
                } else if path.ends_with(".json") {
                    "application/json"
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
            Err(_) => {
                // Return a 404 Not Found response
                Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .header("Content-Type", "text/html")
                    .body(match self.controller.read_asset("404.html") {
                        Ok(mut reader) => {
                            let mut data = Vec::new();
                            if reader.read_to_end(&mut data).is_ok() {
                                data
                            } else {
                                "Not Found".as_bytes().to_vec()
                            }
                        }
                        Err(_) => "Not Found".as_bytes().to_vec(),
                    })
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
            if !self.network_security.is_domain_allowed(host) && !self.home_miniapp {
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

            // Allow only media resource types (images, audio, video, fonts)
            let is_allowed_resource = match extension {
                Some(ext) => matches!(
                    ext.as_str(),
                    // Images
                    "jpg" | "jpeg" | "png" | "gif" | "svg" | "webp" | "ico" |
                    // Audio
                    "mp3" | "wav" | "ogg" |
                    // Video
                    "mp4" | "webm" | "ogv" |
                    // Fonts
                    "ttf" | "woff" | "woff2" | "eot" |
                    // Other allowed static resources
                    "css" | "js"
                ),
                None => false, // No extension, likely not a static resource
            };

            // Check content type in the Accept header if available
            let accept_header = req.headers().get("Accept").and_then(|h| h.to_str().ok());
            let is_api_request = match accept_header {
                Some(accept) => {
                    accept.contains("application/json")
                        || accept.contains("application/xml")
                        || accept.contains("application/")
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
                            "Only static resources (images, audio, video, fonts) are allowed"
                                .as_bytes()
                                .to_vec(),
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
