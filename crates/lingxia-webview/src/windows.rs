mod webview;

pub(crate) use webview::WebViewInner;
pub use webview::{hide_webview_window, set_webview_close_handler, show_webview_window};
