pub(crate) mod schemehandler;
pub mod tsfn;
mod webview;

pub(crate) use webview::apply_http_proxy;
pub use webview::{
    WebViewInner, check_navigation_policy, complete_pending_screenshot_request, on_download_start,
    on_file_chooser_requested, on_load_error, webview_controller_created,
    webview_controller_destroyed,
};
