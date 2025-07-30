//! LingXia Library - Cross-platform MiniApp Runtime
//!
//! This is the main library crate that provides FFI interfaces for different platforms.
//! It generates the native library (liblingxia.so on Android, liblingxia.a on iOS, etc.)

#[cfg(target_os = "android")]
pub mod android;

#[cfg(target_os = "ios")]
pub mod ios;

#[cfg(target_env = "harmony")]
pub mod harmony;

