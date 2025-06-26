#[cfg(target_os = "android")]
mod android;

#[cfg(any(target_os = "ios", target_os = "macos"))]
mod apple;

#[cfg(all(target_os = "linux", target_env = "ohos"))]
mod harmony;

mod controller;
mod webview;

#[cfg(target_os = "android")]
pub use android::{App, WebViewInner};

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub use apple::{App, WebViewInner};

#[cfg(all(target_os = "linux", target_env = "ohos"))]
pub use harmony::{App, WebViewInner};
