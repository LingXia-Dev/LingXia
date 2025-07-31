mod ffi;
mod jni_env;
mod webview;

pub use webview::WebViewInner;

// Re-export JNI utilities
pub use jni_env::{get_env, initialize_jni};
