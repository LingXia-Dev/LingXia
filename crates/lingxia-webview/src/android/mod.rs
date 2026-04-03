mod ffi;
mod jni_env;
mod webview;

pub(crate) use webview::{WebViewInner, apply_http_proxy};

// Re-export JNI utilities
pub use jni_env::{initialize_jni, with_env};
