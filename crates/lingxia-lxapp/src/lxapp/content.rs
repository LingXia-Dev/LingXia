use crate::ControlDocumentBootstrap;
use crate::error;
use crate::error::LxAppError;
use crate::info;
use crate::lxapp::LxApp;

impl LxApp {
    /// Emitted ahead of everything else injected into `<head>`, so the parser
    /// settles on UTF-8 while it is still sniffing for an encoding.
    const CHARSET_META: &'static str = r#"<meta charset="utf-8" />"#;

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

    /// Generate one native-owned control document with its V3 bootstrap.
    ///
    /// The bootstrap is consumed so a caller cannot accidentally reuse its
    /// secret for a different document. The base URL supplied to WebView is
    /// still only resource location; the native document registry performs
    /// admission separately.
    #[doc(hidden)]
    pub fn generate_page_html_with_bridge_bootstrap(
        &self,
        path: &str,
        bridge_nonce: Option<&str>,
        bootstrap: ControlDocumentBootstrap,
    ) -> Result<Vec<u8>, LxAppError> {
        // A missing root is never converted into a bootstrap-bearing 404.
        let data = self.read_bytes(path).map_err(|_| {
            LxAppError::ResourceNotFound(format!(
                "trusted control document root is unavailable: {path}"
            ))
        })?;
        let mut injected_data = self.inject_content_security_policy(&data);
        injected_data =
            inject_bridge_config_with_bootstrap(&injected_data, bridge_nonce, Some(bootstrap))?;
        if let Ok(app_css_data) = self.read_bytes("lxapp.css") {
            injected_data = self.inject_css(&injected_data, &app_css_data, path)?;
        }
        Ok(injected_data)
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
            // Charset first. Everything injected here lands ahead of the page's
            // own `<meta charset>`, and the CSP plus the bridge config easily
            // exceed the prescan window the parser uses to find an encoding —
            // past it the parser falls back to a legacy default and every
            // non-ASCII character in the markup renders as mojibake.
            return format!("{}\n{}\n{}\n{}", before, Self::CHARSET_META, meta, after).into_bytes();
        }

        if let Some(html_pos) = find_ascii_case_insensitive(&html_str, "<html")
            && let Some(html_end) = html_str[html_pos..].find('>')
        {
            let insert_pos = html_pos + html_end + 1;
            let (before, after) = html_str.split_at(insert_pos);
            return format!(
                "{}\n<head>\n{}\n{}\n</head>\n{}",
                before,
                Self::CHARSET_META,
                meta,
                after
            )
            .into_bytes();
        }

        format!(
            "<head>\n{}\n{}\n</head>\n{}",
            Self::CHARSET_META,
            meta,
            html_str
        )
        .into_bytes()
    }

    fn content_security_policy_meta(&self) -> String {
        format!(
            r#"<meta http-equiv="Content-Security-Policy" content="{}">"#,
            escape_html_attr(&self.content_security_policy())
        )
    }

    fn content_security_policy(&self) -> String {
        build_content_security_policy()
    }

    fn inject_bridge_config(&self, html_data: &[u8], bridge_nonce: Option<&str>) -> Vec<u8> {
        inject_bridge_config_with_bootstrap(html_data, bridge_nonce, None)
            .expect("ordinary bridge config injection has a safe fallback")
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

fn inject_bridge_config_with_bootstrap(
    html_data: &[u8],
    bridge_nonce: Option<&str>,
    bootstrap: Option<ControlDocumentBootstrap>,
) -> Result<Vec<u8>, LxAppError> {
    let html_str = String::from_utf8_lossy(html_data);
    let script_tag = build_bridge_config_script(bridge_nonce);

    if let Some(script_start) = find_bridge_runtime_script_start(&html_str, bootstrap.is_some()) {
        let (before, after) = html_str.split_at(script_start);
        let bootstrap_tag = bootstrap
            .map(build_control_document_bootstrap_script)
            .unwrap_or_default();
        return Ok(format!("{}{}\n{}\n{}", before, bootstrap_tag, script_tag, after).into_bytes());
    }
    if bootstrap.is_some() {
        return Err(LxAppError::WebView(
            "trusted control document is missing the bridge runtime script".to_string(),
        ));
    }
    if let Some(head_pos) = find_ascii_case_insensitive(&html_str, "</head>") {
        let (before, after) = html_str.split_at(head_pos);
        return Ok(format!("{}{}\n{}", before, script_tag, after).into_bytes());
    }
    if let Some(body_pos) = find_ascii_case_insensitive(&html_str, "<body")
        && let Some(body_end) = html_str[body_pos..].find('>')
    {
        let insert_pos = body_pos + body_end + 1;
        let (before, after) = html_str.split_at(insert_pos);
        return Ok(format!("{}{}\n{}", before, script_tag, after).into_bytes());
    }

    Ok(format!("{}\n{}", script_tag, html_str).into_bytes())
}

fn build_control_document_bootstrap_script(bootstrap: ControlDocumentBootstrap) -> String {
    let (session_id, secret) = bootstrap.take_binding();
    format!(
        r#"<script>(function(){{let secret="{}";let used=false;Object.defineProperty(window,"__LingXiaTakeControlBootstrap",{{enumerable:false,configurable:true,value:function(){{if(used){{return undefined;}}used=true;let result={{requiredProtocol:3,publicSessionId:"{}",secret:secret}};secret="";delete window.__LingXiaTakeControlBootstrap;return result;}}}});}})();</script>"#,
        escape_js_string(&secret),
        escape_js_string(&session_id),
    )
}

/// The build pipeline emits this exact tag for its bridge runtime. Trusted
/// bootstrap injection deliberately requires it: handwritten or ambiguous
/// runtime tags must not receive a control-document secret.
const TRUSTED_BRIDGE_RUNTIME_TAG: &str =
    r#"<script data-lingxia-bridge-runtime="v3-bootstrap" src="lx://assets/bridge-runtime.js">"#;
const LEGACY_BRIDGE_RUNTIME_TAG: &str = r#"<script src="lx://assets/bridge-runtime.js">"#;

/// Locate the structured bridge-runtime element emitted by the build
/// pipeline. This is intentionally a small HTML tokenizer rather than a
/// substring search: comments, attributes, and raw script/style text can all
/// contain a convincing-looking literal which the browser will not execute.
fn find_bridge_runtime_script_start(html: &str, require_trusted_sentinel: bool) -> Option<usize> {
    let mut cursor = 0;
    while let Some(relative) = html[cursor..].find('<') {
        let start = cursor + relative;
        let remaining = &html[start..];
        if remaining.starts_with("<!--") {
            let end = remaining.find("-->")?;
            cursor = start + end + 3;
            continue;
        }

        let tag_end_relative = remaining.find('>')?;
        let tag_end = start + tag_end_relative;
        let opener = &html[start..=tag_end];
        if opener == TRUSTED_BRIDGE_RUNTIME_TAG
            || (!require_trusted_sentinel && opener == LEGACY_BRIDGE_RUNTIME_TAG)
        {
            return Some(start);
        }

        // HTML parses script and style contents as raw text. Skip them as a
        // browser would, so a literal sentinel in a string cannot be used as
        // an injection point.
        if opener
            .as_bytes()
            .get(..7)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"<script"))
            || opener
                .as_bytes()
                .get(..6)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"<style"))
        {
            let closing = if opener
                .as_bytes()
                .get(..7)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"<script"))
            {
                "</script"
            } else {
                "</style"
            };
            let closing_relative = find_ascii_case_insensitive(&html[tag_end + 1..], closing)?;
            let closing_start = tag_end + 1 + closing_relative;
            let closing_end = html[closing_start..].find('>')? + closing_start;
            cursor = closing_end + 1;
        } else {
            cursor = tag_end + 1;
        }
    }
    None
}

fn build_content_security_policy() -> String {
    [
        "default-src 'self' lx: lingxia:".to_string(),
        // Images are passive, non-executing content: restricting their
        // origins buys little (worst case a tracking pixel) but breaks any
        // runtime-provided asset — e.g. tenant logos / user avatars from
        // `lx.auth` identities live on arbitrary CDNs an app cannot
        // predeclare in trustedDomains. All https images are therefore
        // allowed; network *requests* (fetch) remain gated by
        // security.network.trustedDomains in the Logic runtime.
        // no media-src: View media is rejected by lingxia build; leftovers use default-src.
        "img-src 'self' lx: lingxia: data: blob: https:".to_string(),
        build_connect_src_policy(),
        "script-src 'self' lx: lingxia: 'unsafe-inline'".to_string(),
        "style-src 'self' lx: lingxia: 'unsafe-inline'".to_string(),
        "font-src 'self' lx: lingxia: data:".to_string(),
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
    let bridge_os = lingxia_platform::os_label();
    let display_language = escape_js_string(&super::display_language());

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

    // Dev session = the `lingxia dev` runner (a devtool websocket is
    // configured). The bridge reads this to decide whether to surface its own
    // protocol/lifecycle trace: verbose in dev, quiet in shipped apps. Native
    // log capture forwards whatever the page emits, so this single flag governs
    // the framework's console noise across every platform.
    let dev_kv = if super::is_dev_session() {
        ",dev:true"
    } else {
        ""
    };

    // Runner marker: the bridge reads this to expose `platform.isRunner()` so
    // apps can hide Runner-unavailable surfaces (e.g. the terminal).
    let runner_kv = if super::is_runner() {
        ",runner:true"
    } else {
        ""
    };

    // Which kind of machine this is, mobile or desktop — the product
    // property a page branches on. Deliberately separate from `os` (which
    // names the actual system, and so the bridge transport) and from the
    // surface size class (which answers how much room there is): the runner
    // simulating a phone is still macOS, and a narrowed desktop window is
    // still a desktop.
    let host_class_kv = format!(
        r#",hostClass:"{}""#,
        super::host_class::host_class().as_str()
    );

    let generated_kv = format!(
        r#",displayLanguage:"{}"{}{}{}"#,
        display_language, host_class_kv, nonce_kv, apple_downstream_kv
    );

    // Merge rather than overwrite so developer-provided config can coexist.
    format!(
        r#"<script>(function(){{var c=window.__LX_BRIDGE_CFG||{{}}; window.__LX_BRIDGE_CFG=Object.assign({{}},c,{{os:"{}"{}{}{}}});}})();</script>"#,
        bridge_os, generated_kv, dev_kv, runner_kv
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

#[cfg(test)]
mod tests {
    use super::{
        LEGACY_BRIDGE_RUNTIME_TAG, TRUSTED_BRIDGE_RUNTIME_TAG, build_content_security_policy,
        build_control_document_bootstrap_script, find_bridge_runtime_script_start,
        inject_bridge_config_with_bootstrap, strip_content_security_policy_meta,
    };
    use crate::issue_control_document_bootstrap;
    use ring::rand::SystemRandom;

    #[test]
    fn csp_allows_all_https_images() {
        let csp = build_content_security_policy();

        // Images are passive content: https: is always allowed. Fetch stays
        // gated by trustedDomains in the Logic runtime, not by CSP.
        assert!(csp.contains("img-src 'self' lx: lingxia: data: blob: https:"));
        #[cfg(any(target_os = "ios", target_os = "macos"))]
        assert!(csp.contains("connect-src lx-apple:"));
        #[cfg(not(any(target_os = "ios", target_os = "macos")))]
        assert!(csp.contains("connect-src 'none'"));
        assert!(csp.contains("frame-src 'none'"));
        assert!(csp.contains("object-src 'none'"));
        assert!(csp.contains("child-src 'none'"));
        assert!(csp.contains("worker-src 'none'"));
        assert!(csp.contains("base-uri 'none'"));
        assert!(csp.contains("form-action 'none'"));
        assert!(!csp.contains("default-src 'self' lx: data:"));
    }

    #[test]
    fn csp_does_not_set_media_src() {
        let csp = build_content_security_policy();
        assert!(!csp.contains("media-src"));
        assert!(csp.contains("default-src 'self' lx: lingxia:"));
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

    #[test]
    fn control_bootstrap_is_hidden_one_shot_and_precedes_bridge_runtime() {
        let (bootstrap, _) = issue_control_document_bootstrap(
            &crate::NativeControlPlaneAuthority::for_test(),
            &SystemRandom::new(),
        )
        .unwrap();
        let script = build_control_document_bootstrap_script(bootstrap);
        assert!(script.contains(
            "Object.defineProperty(window,\"__LingXiaTakeControlBootstrap\",{enumerable:false"
        ));
        assert!(script.contains("requiredProtocol:3"));
        assert!(script.contains("publicSessionId:"));
        assert!(script.contains("secret=\"\";delete window.__LingXiaTakeControlBootstrap"));
    }

    #[test]
    fn bridge_runtime_locator_ignores_fake_strings_before_the_real_script() {
        let html = r#"<SCRIPT>let fake='<script data-lingxia-bridge-runtime="v3-bootstrap" src="lx://assets/bridge-runtime.js">';</SCRIPT><STYLE>/* <script data-lingxia-bridge-runtime="v3-bootstrap" src="lx://assets/bridge-runtime.js"> */</STYLE><!-- <script data-lingxia-bridge-runtime="v3-bootstrap" src="lx://assets/bridge-runtime.js"> --><div title='<script data-lingxia-bridge-runtime="v3-bootstrap" src="lx://assets/bridge-runtime.js">'></div><script data-src="lx://assets/bridge-runtime.js"></script><script data-lingxia-bridge-runtime="v3-bootstrap" src="lx://assets/bridge-runtime.js"></script>"#;
        let start = find_bridge_runtime_script_start(html, true).unwrap();
        assert_eq!(
            &html[start..start + super::TRUSTED_BRIDGE_RUNTIME_TAG.len()],
            super::TRUSTED_BRIDGE_RUNTIME_TAG
        );
        assert!(find_bridge_runtime_script_start("<html><head></head></html>", true).is_none());
        assert!(
            find_bridge_runtime_script_start(
                r#"<script src="lx://assets/bridge-runtime.js"></script>"#,
                true,
            )
            .is_none()
        );
        assert_eq!(
            find_bridge_runtime_script_start(
                r#"<script src="lx://assets/bridge-runtime.js"></script>"#,
                false,
            ),
            Some(0)
        );
    }

    #[test]
    fn trusted_bootstrap_requires_the_generated_runtime_tag_and_precedes_it() {
        let html = format!("<html><head>{TRUSTED_BRIDGE_RUNTIME_TAG}</script></head></html>");
        let (bootstrap, _) = issue_control_document_bootstrap(
            &crate::NativeControlPlaneAuthority::for_test(),
            &SystemRandom::new(),
        )
        .unwrap();
        let trusted =
            inject_bridge_config_with_bootstrap(html.as_bytes(), None, Some(bootstrap)).unwrap();
        let trusted = String::from_utf8(trusted).unwrap();
        let bootstrap_at = trusted
            .find("__LingXiaTakeControlBootstrap")
            .expect("trusted bootstrap must be emitted");
        let runtime_at = trusted.find(TRUSTED_BRIDGE_RUNTIME_TAG).unwrap();
        assert!(bootstrap_at < runtime_at);

        let (bootstrap, _) = issue_control_document_bootstrap(
            &crate::NativeControlPlaneAuthority::for_test(),
            &SystemRandom::new(),
        )
        .unwrap();
        assert!(inject_bridge_config_with_bootstrap(
            b"<html><head><script src=\"lx://assets/bridge-runtime.js\"></script></head></html>",
            None,
            Some(bootstrap),
        )
        .is_err());

        let ordinary =
            inject_bridge_config_with_bootstrap(b"<html><head></head></html>", None, None).unwrap();
        assert!(
            !String::from_utf8(ordinary)
                .unwrap()
                .contains("__LingXiaTakeControlBootstrap")
        );

        let ordinary = inject_bridge_config_with_bootstrap(
            b"<html><head><script src=\"lx://assets/bridge-runtime.js\"></script></head></html>",
            None,
            None,
        )
        .unwrap();
        let ordinary = String::from_utf8(ordinary).unwrap();
        assert!(
            ordinary.find("__LX_BRIDGE_CFG").unwrap()
                < ordinary.find(LEGACY_BRIDGE_RUNTIME_TAG).unwrap()
        );
    }
}
