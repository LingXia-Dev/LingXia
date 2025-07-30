//! LingXia Library - Cross-platform MiniApp Runtime
//!
//! This is the main library crate that provides FFI interfaces for different platforms.
//! It generates the native library (liblingxia.so on Android, liblingxia.a on iOS, etc.)

#[cfg(target_os = "android")]
mod android;

#[cfg(target_os = "ios")]
mod ios;

#[cfg(target_env = "ohos")]
mod harmony;

mod runtime;

#[cfg(target_os = "android")]
pub(crate) use android::App;

#[cfg(target_os = "ios")]
pub(crate) use ios::App;

#[cfg(target_env = "ohos")]
pub(crate) use harmony::App;
