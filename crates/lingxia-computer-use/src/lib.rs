//! Local desktop OS automation backend for LingXia devtools.
//!
//! This crate is linked directly into the process that drives the desktop
//! (`lxdev.exe` for the CLI; a future in-process JS binding). It is
//! session-less: it calls the local OS APIs directly. Every operation returns
//! typed DTOs ([`model`]) that serialize to the `desktop` command contract's
//! JSON, and a single [`Error`] taxonomy that maps to stable exit codes.

pub mod error;
pub mod model;

pub use error::{Error, ErrorCode, Result};
pub use model::{
    Capabilities, Capture, CaptureTarget, Display, Doctor, Pixel, Rect, Window, WindowQuery,
};

#[cfg(target_os = "windows")]
#[path = "win/mod.rs"]
mod backend;

#[cfg(not(target_os = "windows"))]
#[path = "stub.rs"]
mod backend;

/// Backend + capability report (`desktop doctor`).
pub fn doctor() -> Doctor {
    backend::doctor()
}

/// Enumerate monitors (`desktop displays`).
pub fn displays() -> Result<Vec<Display>> {
    backend::displays()
}

/// Enumerate top-level OS windows, optionally filtered (`desktop windows`).
pub fn windows(query: &WindowQuery) -> Result<Vec<Window>> {
    backend::windows(query)
}

/// Capture a display/window/region (`desktop screenshot`).
pub fn screenshot(target: CaptureTarget) -> Result<Capture> {
    backend::screenshot(target)
}

/// Read a single pixel's color (`desktop pixel`).
pub fn pixel(x: i32, y: i32) -> Result<Pixel> {
    backend::pixel(x, y)
}
