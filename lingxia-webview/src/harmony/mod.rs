pub(crate) mod schemehandler;
pub mod tsfn;
mod webview;

pub(crate) use webview::apply_http_proxy;
pub use webview::{
    WebViewInner, check_navigation_policy, on_download_start, on_load_error,
    webview_controller_created, webview_controller_destroyed,
};
