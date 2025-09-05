//! LingXia Library
//!
//! This is the main library crate that provides FFI interfaces for different platforms.
//! It generates the native library (liblingxia.so on Android, liblingxia.a on iOS, etc.)

#[cfg(target_os = "android")]
mod android;

#[cfg(any(target_os = "ios", target_os = "macos"))]
mod apple;

#[cfg(target_env = "ohos")]
mod harmony;