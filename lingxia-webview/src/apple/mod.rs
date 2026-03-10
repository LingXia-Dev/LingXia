mod schemehandler;
mod webview;

pub(crate) use webview::WebViewInner;
pub(crate) use webview::apply_http_proxy;

#[cfg(target_os = "macos")]
pub fn toggle_webview_devtools_by_swift_ptr(swift_ptr: usize, detached: bool) -> bool {
    webview::toggle_devtools_by_swift_ptr(swift_ptr, detached)
}
