//! Non-Windows fallback. The desktop backend is Windows-first; other platforms
//! report unsupported explicitly rather than pretending.

use crate::error::{Error, Result};
use crate::model::{Capabilities, Capture, CaptureTarget, Display, Doctor, Pixel, Window, WindowQuery};

fn unsupported<T>() -> Result<T> {
    Err(Error::Unsupported(
        "desktop automation backend is only implemented on Windows".to_string(),
    ))
}

pub fn doctor() -> Doctor {
    Doctor {
        backend: "unsupported".to_string(),
        os: std::env::consts::OS.to_string(),
        os_version: String::new(),
        capabilities: Capabilities::default(),
    }
}

pub fn displays() -> Result<Vec<Display>> {
    unsupported()
}

pub fn windows(_query: &WindowQuery) -> Result<Vec<Window>> {
    unsupported()
}

pub fn screenshot(_target: CaptureTarget) -> Result<Capture> {
    unsupported()
}

pub fn pixel(_x: i32, _y: i32) -> Result<Pixel> {
    unsupported()
}
