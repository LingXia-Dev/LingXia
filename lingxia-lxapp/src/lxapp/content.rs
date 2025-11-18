use crate::error::LxAppError;
use crate::lxapp::LxApp;
use crate::{error, info};
use lingxia_platform::AppRuntime;
use std::io::Read;

impl LxApp {
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
            Err(e) => {
                error!(
                    "Failed to read page HTML: {} (root: {}) => {}",
                    path,
                    self.lxapp_dir.display(),
                    e
                )
                .with_appid(self.appid.clone());
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
        if let Ok(app_css_data) = self.read_bytes("lxapp.css") {
            info!("Injecting global app.css").with_appid(self.appid.clone());
            injected_data = self
                .inject_css(&injected_data, &app_css_data, path)
                .unwrap_or_else(|e| {
                    error!("Failed to inject global CSS: {}, skipping", e)
                        .with_appid(self.appid.clone());
                    injected_data
                });
        }

        injected_data
    }

    /// Get 404 page content with path injection
    fn get_404_page(&self, failed_path: &str) -> Vec<u8> {
        match self.runtime.read_asset("404.html") {
            Ok(mut r) => {
                let mut data = Vec::new();
                match r.read_to_end(&mut data) {
                    Ok(_) => {
                        // Replace placeholder with actual failed path
                        let html_str = String::from_utf8_lossy(&data);
                        let updated_html = html_str.replace("{{FAILED_PATH}}", failed_path);
                        updated_html.into_bytes()
                    }
                    Err(_) => {
                        format!(
                            "<!DOCTYPE html><html><head><title>404</title></head><body><h1>404 - Page Not Found</h1><p>Path: {}</p></body></html>",
                            failed_path
                        ).as_bytes().to_vec()
                    }
                }
            }
            Err(_) => {
                format!(
                    "<!DOCTYPE html><html><head><title>404</title></head><body><h1>404 - Page Not Found</h1><p>Path: {}</p></body></html>",
                    failed_path
                ).as_bytes().to_vec()
            }
        }
    }

    /// Inject WebView bridge script and framework integration into HTML content
    fn inject_bridge_script(&self, html_data: &[u8]) -> Result<Vec<u8>, LxAppError> {
        // Load the bridge script from assets
        let bridge_script = match self.runtime.read_asset("webview-bridge.js") {
            Ok(mut reader) => {
                let mut script_data = Vec::new();
                reader.read_to_end(&mut script_data).map_err(|e| {
                    LxAppError::IoError(format!("Failed to read bridge script: {}", e))
                })?;
                String::from_utf8_lossy(&script_data).to_string()
            }
            Err(e) => {
                return Err(LxAppError::IoError(format!(
                    "Failed to open bridge script: {}",
                    e
                )));
            }
        };

        let html_str = String::from_utf8_lossy(html_data);

        // Decide bridge method by compile target
        #[cfg(any(target_os = "ios", target_os = "macos"))]
        let bridge_method = "webkit";
        #[cfg(target_os = "android")]
        let bridge_method = "messageport";
        #[cfg(all(target_os = "linux", target_env = "ohos"))]
        let bridge_method = "messageport";
        #[cfg(not(any(
            target_os = "ios",
            target_os = "macos",
            target_os = "android",
            all(target_os = "linux", target_env = "ohos")
        )))]
        let bridge_method = "unknown";

        // Create script tags: host provides communication method first, then the bridge
        let prelude = format!(
            "<script>window.__LX_BRIDGE_METHOD='{}';</script>",
            bridge_method
        );
        let script_tags = format!("{}\n<script>\n{}\n</script>", prelude, bridge_script);

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
    ) -> Result<Vec<u8>, LxAppError> {
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
