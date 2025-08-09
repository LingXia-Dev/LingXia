#[cfg(target_os = "android")]
mod android;

#[cfg(any(target_os = "ios", target_os = "macos"))]
mod apple;

#[cfg(all(target_os = "linux", target_env = "ohos"))]
mod harmony;

#[cfg(target_os = "android")]
pub use android::{WebViewInner, get_env, initialize_jni};

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub use apple::WebViewInner;

#[cfg(all(target_os = "linux", target_env = "ohos"))]
pub use harmony::{WebViewInner, tsfn, schemehandler::register_custom_schemes};

mod webview;
pub use webview::{create_webview, find_webview, find_webview_by_tag, init_webview_manager};
