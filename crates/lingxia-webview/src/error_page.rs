/// Localized content for the built-in WebView load-error document.
#[derive(Debug, Clone, Copy)]
pub struct LoadErrorPage<'a> {
    pub title: &'a str,
    pub message: &'a str,
    pub retry_label: &'a str,
    pub retry_url: &'a str,
}

/// Render the one cross-platform load-error document used by managed WebViews
/// and native URL surfaces.
pub fn render_load_error_page(content: LoadErrorPage<'_>) -> String {
    let title = html_text(content.title);
    let message = html_text(content.message);
    let retry_label = html_text(content.retry_label);
    let retry_url = html_attribute(content.retry_url);
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="color-scheme" content="light dark"><style>html,body{{height:100%;margin:0}}body{{display:flex;align-items:center;justify-content:center;padding:24px;font-family:system-ui,sans-serif;background:#f7f8fb;color:#1c2233}}main{{max-width:320px;text-align:center}}h1{{font-size:18px;margin:0 0 8px}}p{{font-size:14px;line-height:1.5;color:#70788f;margin:0 0 20px}}button{{border:0;border-radius:10px;padding:11px 28px;background:#2f6bff;color:white;font:600 15px system-ui;cursor:pointer}}@media (prefers-color-scheme:dark){{body{{background:#16181d;color:#e8eaf0}}p{{color:#9aa2b5}}}}</style></head><body><main><h1>{title}</h1><p>{message}</p><button id="retry" data-url="{retry_url}">{retry_label}</button></main><script>const retry=document.getElementById('retry');retry.addEventListener('click',()=>location.replace(retry.dataset.url));</script></body></html>"#
    )
}

fn html_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn html_attribute(value: &str) -> String {
    html_text(value)
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_localized_content_and_retry_url() {
        let html = render_load_error_page(LoadErrorPage {
            title: "<offline>",
            message: "A & B",
            retry_label: "Retry",
            retry_url: "https://example.test/?q=\"'><script>",
        });
        assert!(html.contains("&lt;offline&gt;"));
        assert!(html.contains("A &amp; B"));
        assert!(html.contains("&quot;&#39;&gt;&lt;script&gt;"));
    }
}
