mod bridge_transport;
pub(crate) mod data_store;
mod schemehandler;
mod webview;

pub(crate) use webview::WebViewInner;
pub(crate) use webview::apply_http_proxy;

pub const BRIDGE_DOWNSTREAM_CSP_SOURCE: &str = bridge_transport::APPLE_BRIDGE_DOWNSTREAM_CSP_SOURCE;
pub const BRIDGE_DOWNSTREAM_URL: &str = bridge_transport::APPLE_BRIDGE_DOWNSTREAM_URL;

#[cfg(target_os = "macos")]
pub fn toggle_webview_devtools_by_swift_ptr(swift_ptr: usize, detached: bool) -> bool {
    webview::toggle_devtools_by_swift_ptr(swift_ptr, detached)
}
