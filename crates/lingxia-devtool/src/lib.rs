//! Native entry library for LingXia development tools.
//!
//! This crate owns devtool-specific bootstrap behavior for Apple hosts such as
//! LingXia Runner while reusing the shared LingXia runtime.

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub use lingxia::apple::*;

#[cfg(any(target_os = "ios", target_os = "macos"))]
#[unsafe(no_mangle)]
pub extern "C" fn lingxia_install_host_addon() {}
