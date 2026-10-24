//! OS adapters. Each one bridges surfaces, input, IME and accessibility only.
#[cfg(target_os = "android")]
pub mod android;
