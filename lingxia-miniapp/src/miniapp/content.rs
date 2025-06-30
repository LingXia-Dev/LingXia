use crate::error::MiniAppError;
use crate::miniapp::MiniApp;
use crate::{error, info};
use std::io::Read;

impl MiniApp {
    /// Generate processed HTML content for a page with script and CSS injection
    ///
    /// This reads the HTML file and injects necessary scripts and styles
    /// If the file cannot be read, returns a 404 page for better user experience
    ///
    /// # Arguments
    /// * `path` - The page path (e.g., "pages/home/index.html")
    ///
    /// # Returns
    /// * `Vec<u8>` - Processed HTML content or 404 page
    pub(crate) fn generate_page_html(&self, path: &str) -> Vec<u8> {
        // Try to read the file
        let data = match self.read_bytes(path) {
            Ok(data) => data,
            Err(_) => {
                return self.get_404_page(path);
            }
        };

        // Inject bridge script
        let mut injected_data = match self.inject_bridge_script(&data) {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to inject bridge script: {}", e).with_appid(self.appid.clone());
                data
            }
        };

        // Inject global app.css if it exists (optional)
        if let Ok(app_css_data) = self.read_bytes("app.css") {
            info!("Injecting global app.css").with_appid(self.appid.clone());
            injected_data = self
                .inject_css(&injected_data, &app_css_data, path)
                .unwrap_or_else(|e| {
                    error!("Failed to inject global CSS: {}, skipping", e)
                        .with_appid(self.appid.clone());
                    injected_data
                });
        }

        // Inject page-specific CSS if it exists (optional)
        if let Some(css_path) = path.strip_suffix(".html") {
            let css_full_path = format!("{}.css", css_path);
            if let Ok(css_data) = self.read_bytes(&css_full_path) {
                info!("Found and injecting matching CSS file: {}", css_full_path)
                    .with_appid(self.appid.clone());
                injected_data = self
                    .inject_css(&injected_data, &css_data, path)
                    .unwrap_or_else(|e| {
                        error!("Failed to inject page CSS: {}, skipping", e)
                            .with_appid(self.appid.clone());
                        injected_data
                    });
            }
        }

        injected_data
    }

    /// Get 404 page content with path injection
    fn get_404_page(&self, failed_path: &str) -> Vec<u8> {
        self.runtime
            .read_asset("404.html")
            .and_then(|mut r| {
                let mut data = Vec::new();
                r.read_to_end(&mut data)
                    .map_err(|e| MiniAppError::IoError(e.to_string()))
                    .map(|_| {
                        // Replace placeholder with actual failed path
                        let html_str = String::from_utf8_lossy(&data);
                        let updated_html = html_str.replace("{{FAILED_PATH}}", failed_path);
                        updated_html.into_bytes()
                    })
            })
            .unwrap_or_else(|_| {
                format!(
                    "<!DOCTYPE html><html><head><title>404</title></head><body><h1>404 - Page Not Found</h1><p>Path: {}</p></body></html>",
                    failed_path
                ).as_bytes().to_vec()
            })
    }

    /// Inject WebView bridge script and framework integration into HTML content
    fn inject_bridge_script(&self, html_data: &[u8]) -> Result<Vec<u8>, MiniAppError> {
        // Load the bridge script from assets
        let bridge_script = match self.runtime.read_asset("webview-bridge.js") {
            Ok(mut reader) => {
                let mut script_data = Vec::new();
                reader.read_to_end(&mut script_data).map_err(|e| {
                    MiniAppError::IoError(format!("Failed to read bridge script: {}", e))
                })?;
                String::from_utf8_lossy(&script_data).to_string()
            }
            Err(e) => {
                return Err(MiniAppError::IoError(format!(
                    "Failed to open bridge script: {}",
                    e
                )));
            }
        };

        // Load the framework integration script from assets (optional)
        let framework_script = match self.runtime.read_asset("framework.js") {
            Ok(mut reader) => {
                let mut script_data = Vec::new();
                reader.read_to_end(&mut script_data).map_err(|e| {
                    MiniAppError::IoError(format!("Failed to read framework script: {}", e))
                })?;
                Some(String::from_utf8_lossy(&script_data).to_string())
            }
            Err(e) => {
                info!("Framework integration script not found (optional): {}", e)
                    .with_appid(self.appid.clone());
                None
            }
        };

        // Convert HTML content to string
        let html_str = String::from_utf8_lossy(html_data);

        // Create script tags - first bridge script, then framework script
        let mut script_tags = format!("<script>\n{}\n</script>", bridge_script);

        if let Some(framework_content) = framework_script {
            script_tags.push_str(&format!("\n<script>\n{}\n</script>", framework_content));
        }

        // Try to insert before </head> tag (preferred location)
        if let Some(head_pos) = html_str.to_lowercase().find("</head>") {
            let (before, after) = html_str.split_at(head_pos);
            info!("Injected scripts before </head>").with_appid(self.appid.clone());
            return Ok(format!("{}{}{}", before, script_tags, after).into_bytes());
        }
        // If no </head> tag, try to insert at the beginning of <body> tag
        else if let Some(body_pos) = html_str.to_lowercase().find("<body") {
            if let Some(body_end) = html_str[body_pos..].find('>') {
                let insert_pos = body_pos + body_end + 1;
                let (before, after) = html_str.split_at(insert_pos);
                info!("Injected scripts after <body>").with_appid(self.appid.clone());
                return Ok(format!("{}{}{}", before, script_tags, after).into_bytes());
            }
        }
        // If neither tag is found, insert at the beginning of the HTML
        else {
            info!("Injected scripts at beginning of HTML (fallback)")
                .with_appid(self.appid.clone());
            return Ok(format!("{}{}", script_tags, html_str).into_bytes());
        }

        // If all injection attempts failed, return the original data
        error!("All injection attempts failed, returning original HTML")
            .with_appid(self.appid.clone());
        Ok(html_data.to_vec())
    }

    /// Inject CSS into HTML content
    fn inject_css(
        &self,
        html_data: &[u8],
        css_data: &[u8],
        path: &str,
    ) -> Result<Vec<u8>, MiniAppError> {
        // Convert CSS content to string
        let css_content = String::from_utf8_lossy(css_data);
        let style_tag = format!("<style>\n{}\n</style>", css_content);

        // Convert HTML content to string
        let html_str = String::from_utf8_lossy(html_data);

        // Try to insert before </head> tag (preferred location for styles)
        if let Some(head_pos) = html_str.to_lowercase().find("</head>") {
            let (before, after) = html_str.split_at(head_pos);
            info!("Injected CSS before </head> in {}", path).with_appid(self.appid.clone());
            return Ok(format!("{}{}{}", before, style_tag, after).into_bytes());
        }
        // If no </head> tag, try to insert at the beginning of <body> tag
        else if let Some(body_pos) = html_str.to_lowercase().find("<body") {
            if let Some(body_end) = html_str[body_pos..].find('>') {
                let insert_pos = body_pos + body_end + 1;
                let (before, after) = html_str.split_at(insert_pos);
                info!("Injected CSS after <body> in {}", path).with_appid(self.appid.clone());
                return Ok(format!("{}{}{}", before, style_tag, after).into_bytes());
            }
        }
        // If neither tag is found, insert at the beginning of the HTML
        else {
            info!("Injected CSS at beginning of HTML in {} (fallback)", path)
                .with_appid(self.appid.clone());
            return Ok(format!("{}{}", style_tag, html_str).into_bytes());
        }

        // If all injection attempts failed, return the original data
        error!("CSS injection failed for {}, returning original HTML", path)
            .with_appid(self.appid.clone());
        Ok(html_data.to_vec())
    }
}
