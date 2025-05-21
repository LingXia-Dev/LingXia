use http::{Request, Response, StatusCode};

use crate::log::Logging;
use crate::miniapp::MiniApp;

impl MiniApp {
    /// Handler for lx:// scheme requests to access app assets
    pub(crate) fn lingxia_handler(&mut self, req: Request<Vec<u8>>) -> Option<Response<Vec<u8>>> {
        let uri = req.uri();

        // Get the path part after lx://
        let uri_str = uri.to_string();
        let path = uri_str.trim_start_matches("lx://").trim_start_matches('/');

        // Try to read the asset from app directory
        let file_result = self.read_bytes(path);

        let response = match file_result {
            Ok(data) => {
                // Determine MIME type based on file extension
                let is_html = path.ends_with(".html");
                let mime_type = if is_html {
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

                // If this is an HTML file, inject script and CSS
                let response_data = if is_html {
                    let is_script_injected = if let Some(page) = self.pages.get_page(path) {
                        page.is_script_injected()
                    } else {
                        false
                    };

                    // Inject both bridge script and CSS only if not already injected
                    let html_data = if is_script_injected {
                        self.info(
                            path,
                            format!("Page already injected, skipping injection for {}", path),
                        );
                        data
                    } else {
                        self.info(path, format!("Injecting bridge script to {}", path));
                        let mut injected_data = inject_bridge_script(&data, self);

                        // Also inject CSS when injecting script (both happen only on first load)
                        // First try to inject global app.css
                        if let Ok(app_css_data) = self.read_bytes("app.css") {
                            self.info(path, "Injecting global app.css".to_string());
                            injected_data = inject_css(&injected_data, &app_css_data, self, path);
                        }

                        // Then inject page-specific CSS if it exists
                        if let Some(css_path) = path.strip_suffix(".html") {
                            let css_full_path = format!("{}.css", css_path);
                            if let Ok(css_data) = self.read_bytes(&css_full_path) {
                                self.info(
                                    path,
                                    format!(
                                        "Found and injecting matching CSS file: {}",
                                        css_full_path
                                    ),
                                );
                                injected_data = inject_css(&injected_data, &css_data, self, path);
                            }
                        }

                        if let Some(page) = self.pages.get_page(path) {
                            page.mark_script_injected();
                        }

                        injected_data
                    };

                    html_data
                } else {
                    data
                };

                Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", mime_type)
                    .header("Content-Length", response_data.len().to_string())
                    .body(response_data)
                    .unwrap_or_else(|_| {
                        Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .body(Vec::new())
                            .unwrap()
                    })
            }
            Err(e) => {
                self.error("", format!("Fallback to reading 404.html due to {}", e));

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
                    "ttf" | "woff" | "woff2" | "eot"
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

/// Injects WebView bridge script into HTML content
fn inject_bridge_script(html_data: &[u8], app: &MiniApp) -> Vec<u8> {
    // First, load the bridge script from assets
    let bridge_script = match app.controller.read_asset("webview-bridge.js") {
        Ok(mut reader) => {
            let mut script_data = Vec::new();
            if reader.read_to_end(&mut script_data).is_ok() {
                String::from_utf8_lossy(&script_data).to_string()
            } else {
                app.error("inject_bridge", "Failed to read bridge script content");
                return html_data.to_vec();
            }
        }
        Err(e) => {
            app.error(
                "inject_bridge",
                format!("Failed to open bridge script: {}", e),
            );
            return html_data.to_vec();
        }
    };

    // Convert HTML content to string
    if let Ok(html_str) = String::from_utf8(html_data.to_vec()) {
        // Create script tag with the bridge script
        let script_tag = format!("<script>\n{}\n</script>", bridge_script);

        // Try to insert before </head> tag (preferred location for early initialization)
        if let Some(head_pos) = html_str.to_lowercase().find("</head>") {
            let (before, after) = html_str.split_at(head_pos);
            app.info("inject_bridge", "Injected script before </head>");
            return format!("{}{}{}", before, script_tag, after).into_bytes();
        }
        // If no </head> tag, try to insert at the beginning of <body> tag
        else if let Some(body_pos) = html_str.to_lowercase().find("<body") {
            if let Some(body_end) = html_str[body_pos..].find('>') {
                let insert_pos = body_pos + body_end + 1;
                let (before, after) = html_str.split_at(insert_pos);
                app.info("inject_bridge", "Injected script after <body>");
                return format!("{}{}{}", before, script_tag, after).into_bytes();
            }
        }
        // If neither tag is found, insert at the beginning of the HTML
        else {
            app.info(
                "inject_bridge",
                "Injected script at beginning of HTML (fallback)",
            );
            return format!("{}{}", script_tag, html_str).into_bytes();
        }
    }

    // If all injection attempts failed, return the original data
    app.error(
        "inject_bridge",
        "All injection attempts failed, returning original HTML",
    );
    html_data.to_vec()
}

/// Injects CSS into HTML content
fn inject_css(html_data: &[u8], css_data: &[u8], app: &MiniApp, path: &str) -> Vec<u8> {
    // Convert CSS content to string
    let css_content = String::from_utf8_lossy(css_data).to_string();
    let style_tag = format!("<style>\n{}\n</style>", css_content);

    // Convert HTML content to string
    if let Ok(html_str) = String::from_utf8(html_data.to_vec()) {
        // Try to insert before </head> tag (preferred location for styles)
        if let Some(head_pos) = html_str.to_lowercase().find("</head>") {
            let (before, after) = html_str.split_at(head_pos);
            app.info(
                "inject_css",
                format!("Injected CSS before </head> in {}", path),
            );
            return format!("{}{}{}", before, style_tag, after).into_bytes();
        }
        // If no </head> tag, try to insert at the beginning of <body> tag
        else if let Some(body_pos) = html_str.to_lowercase().find("<body") {
            if let Some(body_end) = html_str[body_pos..].find('>') {
                let insert_pos = body_pos + body_end + 1;
                let (before, after) = html_str.split_at(insert_pos);
                app.info(
                    "inject_css",
                    format!("Injected CSS after <body> in {}", path),
                );
                return format!("{}{}{}", before, style_tag, after).into_bytes();
            }
        }
        // If neither tag is found, insert at the beginning of the HTML
        else {
            app.info(
                "inject_css",
                format!("Injected CSS at beginning of HTML in {} (fallback)", path),
            );
            return format!("{}{}", style_tag, html_str).into_bytes();
        }
    }

    // If all injection attempts failed, return the original data
    app.error(
        "inject_css",
        format!("CSS injection failed for {}, returning original HTML", path),
    );
    html_data.to_vec()
}
