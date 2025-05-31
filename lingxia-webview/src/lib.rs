#[cfg(target_os = "android")]
mod android;

#[cfg(any(target_os = "ios", target_os = "macos"))]
mod apple;

mod controller;
mod webview;

#[cfg(target_os = "android")]
pub use android::{App, WebViewInner};

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub use apple::{App, WebViewInner};
