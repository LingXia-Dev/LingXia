use crate::error;
use crate::error::LxAppError;
use crate::info;
use crate::lxapp::LxApp;

impl LxApp {
    /// Generate processed HTML content for a page
    ///
    /// This reads the HTML file. If it cannot be read, returns a 404 page.
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

        let mut injected_data = self.inject_bridge_config(&data);

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
        let escaped_path = escape_js_string(failed_path);
        let bridge_script = build_bridge_config_script();
        let html = format!(
            r#"<!DOCTYPE html>
<html>
  <head>
    <meta charset="UTF-8" />
    <title>404</title>
  </head>
  <body>
    {}
    <script>
      window.__LX_RUNTIME_CONFIG = {{
        error: {{ failedPath: "{}", reason: "not_found" }}
      }};
    </script>
    <script src="lx://assets/runtime.js"></script>
  </body>
</html>"#,
            bridge_script, escaped_path
        );
        html.into_bytes()
    }

    fn inject_bridge_config(&self, html_data: &[u8]) -> Vec<u8> {
        let html_str = String::from_utf8_lossy(html_data);
        if html_str.contains("__LX_BRIDGE_CFG") {
            return html_data.to_vec();
        }

        let script_tag = build_bridge_config_script();

        let lower = html_str.to_lowercase();
        if let Some(src_pos) = lower.find("lx://assets/runtime.js") {
            if let Some(script_start) = lower[..src_pos].rfind("<script") {
                let (before, after) = html_str.split_at(script_start);
                return format!("{}{}\n{}", before, script_tag, after).into_bytes();
            }
        }
        if let Some(head_pos) = lower.find("</head>") {
            let (before, after) = html_str.split_at(head_pos);
            return format!("{}{}\n{}", before, script_tag, after).into_bytes();
        }
        if let Some(body_pos) = lower.find("<body") {
            if let Some(body_end) = html_str[body_pos..].find('>') {
                let insert_pos = body_pos + body_end + 1;
                let (before, after) = html_str.split_at(insert_pos);
                return format!("{}{}\n{}", before, script_tag, after).into_bytes();
            }
        }

        format!("{}\n{}", script_tag, html_str).into_bytes()
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

fn build_bridge_config_script() -> String {
    #[cfg(any(target_os = "ios", target_os = "macos"))]
    let bridge_os = if cfg!(target_os = "macos") {
        "macOS"
    } else {
        "iOS"
    };
    #[cfg(target_os = "android")]
    let bridge_os = "Android";
    #[cfg(all(target_os = "linux", target_env = "ohos"))]
    let bridge_os = "Harmony";
    #[cfg(not(any(
        target_os = "ios",
        target_os = "macos",
        target_os = "android",
        all(target_os = "linux", target_env = "ohos"),
    )))]
    let bridge_os = "unknown";

    format!(
        r#"<script>window.__LX_BRIDGE_CFG={{os:"{}"}};</script>"#,
        bridge_os
    )
}

fn escape_js_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
