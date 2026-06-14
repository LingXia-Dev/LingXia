//! Windows host-window API re-export.
//!
//! The cycle-free bridge lives in `lingxia-platform`; this crate is the
//! Windows host entry point and re-exports that API for host applications.

pub use lingxia_platform::windows::webview_host::*;
