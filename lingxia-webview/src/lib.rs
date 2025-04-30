#[cfg(target_os = "android")]
mod android;

#[cfg(any(target_os = "ios", target_os = "macos"))]
mod apple;

mod controller;

#[cfg(target_os = "android")]
pub use android::WebView;

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub use apple::WebView;
