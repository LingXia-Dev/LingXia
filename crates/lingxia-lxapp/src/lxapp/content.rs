use crate::error;
use crate::error::LxAppError;
use crate::info;
use crate::lxapp::LxApp;

impl LxApp {
    fn trusted_domains_snapshot(&self) -> Vec<String> {
        self.state
            .lock()
            .unwrap()
            .network_security
            .domains_snapshot()
    }

    /// Generate processed HTML content for a page
    ///
    /// This reads the HTML file. If it cannot be read, returns a 404 page.
    ///
    /// # Arguments
    /// * `path` - The page path (e.g., "pages/home/index.html")
    /// * `bridge_nonce` - Optional per-page nonce used for bridge wiring validation
    ///
    /// # Returns
    /// * `Vec<u8>` - Processed HTML content or 404 page
    pub fn generate_page_html(&self, path: &str, bridge_nonce: Option<&str>) -> Vec<u8> {
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
                return self.get_404_page(path, bridge_nonce);
            }
        };

        let mut injected_data = self.inject_content_security_policy(&data);
        injected_data = self.inject_bridge_config(&injected_data, bridge_nonce);

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
    fn get_404_page(&self, failed_path: &str, bridge_nonce: Option<&str>) -> Vec<u8> {
        let escaped_path = escape_js_string(failed_path);
        let bridge_script = build_bridge_config_script(bridge_nonce);
        let csp_meta = self.content_security_policy_meta();
        let html = format!(
            r#"<!DOCTYPE html>
<html>
  <head>
    <meta charset="UTF-8" />
    {}
    <title>404</title>
  </head>
  <body>
    {}
    <script>
      window.__LX_RUNTIME_CONFIG = {{
        error: {{ failedPath: "{}", reason: "not_found" }}
      }};
    </script>
    <script src="lx://assets/bridge-runtime.js"></script>
  </body>
</html>"#,
            csp_meta, bridge_script, escaped_path
        );
        html.into_bytes()
    }

    fn inject_content_security_policy(&self, html_data: &[u8]) -> Vec<u8> {
        let html_str = String::from_utf8_lossy(html_data);
        let html_str = strip_content_security_policy_meta(&html_str);
        let meta = self.content_security_policy_meta();

        if let Some(head_pos) = find_ascii_case_insensitive(&html_str, "<head")
            && let Some(head_end) = html_str[head_pos..].find('>')
        {
            let insert_pos = head_pos + head_end + 1;
            let (before, after) = html_str.split_at(insert_pos);
            return format!("{}\n{}\n{}", before, meta, after).into_bytes();
        }

        if let Some(html_pos) = find_ascii_case_insensitive(&html_str, "<html")
            && let Some(html_end) = html_str[html_pos..].find('>')
        {
            let insert_pos = html_pos + html_end + 1;
            let (before, after) = html_str.split_at(insert_pos);
            return format!("{}\n<head>\n{}\n</head>\n{}", before, meta, after).into_bytes();
        }

        format!("<head>\n{}\n</head>\n{}", meta, html_str).into_bytes()
    }

    fn content_security_policy_meta(&self) -> String {
        format!(
            r#"<meta http-equiv="Content-Security-Policy" content="{}">"#,
            escape_html_attr(&self.content_security_policy())
        )
    }

    fn content_security_policy(&self) -> String {
        build_content_security_policy(&self.trusted_domains_snapshot())
    }

    fn inject_bridge_config(&self, html_data: &[u8], bridge_nonce: Option<&str>) -> Vec<u8> {
        let html_str = String::from_utf8_lossy(html_data);
        let script_tag = build_bridge_config_script(bridge_nonce);

        if let Some(src_pos) =
            find_ascii_case_insensitive(&html_str, "lx://assets/bridge-runtime.js")
            && let Some(script_start) =
                find_ascii_case_insensitive_rev(&html_str[..src_pos], "<script")
        {
            let (before, after) = html_str.split_at(script_start);
            return format!("{}{}\n{}", before, script_tag, after).into_bytes();
        }
        if let Some(head_pos) = find_ascii_case_insensitive(&html_str, "</head>") {
            let (before, after) = html_str.split_at(head_pos);
            return format!("{}{}\n{}", before, script_tag, after).into_bytes();
        }
        if let Some(body_pos) = find_ascii_case_insensitive(&html_str, "<body")
            && let Some(body_end) = html_str[body_pos..].find('>')
        {
            let insert_pos = body_pos + body_end + 1;
            let (before, after) = html_str.split_at(insert_pos);
            return format!("{}{}\n{}", before, script_tag, after).into_bytes();
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
        if let Some(head_pos) = find_ascii_case_insensitive(&html_str, "</head>") {
            let (before, after) = html_str.split_at(head_pos);
            info!("Injected CSS before </head> in {}", path).with_appid(self.appid.clone());
            return Ok(format!("{}{}{}", before, style_tag, after).into_bytes());
        }
        // If no </head> tag, try to insert at the beginning of <body> tag
        else if let Some(body_pos) = find_ascii_case_insensitive(&html_str, "<body") {
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

fn build_content_security_policy(trusted_domains: &[String]) -> String {
    let mut img_sources = vec![
        "'self'".to_string(),
        "lx:".to_string(),
        "lingxia:".to_string(),
        "data:".to_string(),
        "blob:".to_string(),
    ];

    if trusted_domains.iter().any(|domain| domain == "*") {
        img_sources.push("https:".to_string());
    } else {
        img_sources.extend(trusted_domains.iter().map(|domain| {
            if let Some(suffix) = domain.strip_prefix("*.") {
                format!("https://*.{suffix}")
            } else {
                format!("https://{domain}")
            }
        }));
    }

    [
        "default-src 'self' lx: lingxia:".to_string(),
        format!("img-src {}", img_sources.join(" ")),
        build_connect_src_policy(),
        "script-src 'self' lx: lingxia: 'unsafe-inline'".to_string(),
        "style-src 'self' lx: lingxia: 'unsafe-inline'".to_string(),
        "font-src 'self' lx: lingxia: data:".to_string(),
        "media-src 'none'".to_string(),
        "worker-src 'none'".to_string(),
        "child-src 'none'".to_string(),
        "frame-src 'none'".to_string(),
        "object-src 'none'".to_string(),
        "base-uri 'none'".to_string(),
        "form-action 'none'".to_string(),
    ]
    .join("; ")
}

fn build_connect_src_policy() -> String {
    #[cfg(any(target_os = "ios", target_os = "macos"))]
    {
        format!(
            "connect-src {}",
            lingxia_webview::platform::apple::BRIDGE_DOWNSTREAM_CSP_SOURCE
        )
    }

    #[cfg(not(any(target_os = "ios", target_os = "macos")))]
    {
        "connect-src 'none'".to_string()
    }
}

fn build_bridge_config_script(bridge_nonce: Option<&str>) -> String {
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

    #[cfg(any(target_os = "ios", target_os = "macos"))]
    let apple_downstream_url = Some(escape_js_string(
        lingxia_webview::platform::apple::BRIDGE_DOWNSTREAM_URL,
    ));
    #[cfg(not(any(target_os = "ios", target_os = "macos")))]
    let apple_downstream_url: Option<String> = None;
    let apple_downstream_kv = match apple_downstream_url {
        Some(url) if !url.is_empty() => format!(r#",appleDownstreamURL:"{}""#, url),
        _ => String::new(),
    };

    let nonce_json = bridge_nonce.map(escape_js_string);
    let nonce_kv = match nonce_json {
        Some(nonce) if !nonce.is_empty() => format!(r#",nonce:"{}""#, nonce),
        _ => String::new(),
    };

    let generated_kv = format!("{}{}", nonce_kv, apple_downstream_kv);

    // Merge rather than overwrite so developer-provided config can coexist.
    format!(
        r#"<script>(function(){{var c=window.__LX_BRIDGE_CFG||{{}}; window.__LX_BRIDGE_CFG=Object.assign({{}},c,{{os:"{}"{}}});}})();</script>"#,
        bridge_os, generated_kv
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

fn escape_html_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}

fn strip_content_security_policy_meta(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut cursor = 0;

    while let Some(rel_start) = find_ascii_case_insensitive(&html[cursor..], "<meta") {
        let start = cursor + rel_start;
        let Some(rel_end) = html[start..].find('>') else {
            break;
        };
        let end = start + rel_end + 1;
        let meta = &html[start..end];
        let meta_lower = meta.to_ascii_lowercase();

        if meta_lower.contains("http-equiv") && meta_lower.contains("content-security-policy") {
            out.push_str(&html[cursor..start]);
        } else {
            out.push_str(&html[cursor..end]);
        }
        cursor = end;
    }

    out.push_str(&html[cursor..]);
    out
}

fn find_ascii_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    let needle = needle.as_bytes();
    if needle.is_empty() {
        return Some(0);
    }
    haystack
        .as_bytes()
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle))
}

fn find_ascii_case_insensitive_rev(haystack: &str, needle: &str) -> Option<usize> {
    let needle = needle.as_bytes();
    if needle.is_empty() {
        return Some(haystack.len());
    }
    haystack
        .as_bytes()
        .windows(needle.len())
        .rposition(|window| window.eq_ignore_ascii_case(needle))
}

#[cfg(test)]
mod tests {
    use super::{build_content_security_policy, strip_content_security_policy_meta};

    #[test]
    fn csp_only_allows_trusted_https_images() {
        let csp = build_content_security_policy(&[
            "cdn.example.com".to_string(),
            "*.img.example.com".to_string(),
        ]);

        assert!(csp.contains(
            "img-src 'self' lx: lingxia: data: blob: https://cdn.example.com https://*.img.example.com"
        ));
        #[cfg(any(target_os = "ios", target_os = "macos"))]
        assert!(csp.contains("connect-src lx-apple:"));
        #[cfg(not(any(target_os = "ios", target_os = "macos")))]
        assert!(csp.contains("connect-src 'none'"));
        assert!(csp.contains("media-src 'none'"));
        assert!(csp.contains("frame-src 'none'"));
        assert!(!csp.contains("default-src 'self' lx: data:"));
    }

    #[test]
    fn csp_wildcard_trusted_domain_allows_https_images() {
        let csp = build_content_security_policy(&["*".to_string()]);

        assert!(csp.contains("img-src 'self' lx: lingxia: data: blob: https:"));
        assert!(!csp.contains("https://*"));
    }

    #[test]
    fn strips_page_owned_csp_before_runtime_injection() {
        let html = r#"<html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src *"><title>x</title></head></html>"#;
        let stripped = strip_content_security_policy_meta(html);

        assert!(stripped.contains(r#"<meta charset="utf-8">"#));
        assert!(
            !stripped
                .to_ascii_lowercase()
                .contains("content-security-policy")
        );
    }
}
