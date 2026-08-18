//! Sessionless desktop visual capture engine.
//!
//! Snapshot and the desktop realtime adapter share this. Frames stay crate-
//! private so the public API never grows an RGBA type.

use crate::error::Result;
use crate::model::{CaptureTarget, Rect};

pub(crate) struct EngineFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// Global source rect and scale feed realtime geometry; snapshot ignores
    /// them, and the stub backend constructs no frame at all.
    #[cfg_attr(not(feature = "realtime-capture-provider"), allow(dead_code))]
    pub source: Rect,
    #[cfg_attr(not(feature = "realtime-capture-provider"), allow(dead_code))]
    pub scale: f64,
    #[cfg_attr(not(feature = "snapshot"), allow(dead_code))]
    pub backend: &'static str,
    #[cfg_attr(not(feature = "snapshot"), allow(dead_code))]
    pub occlusion_independent: bool,
}

pub(crate) fn capture_frame(target: &CaptureTarget) -> Result<EngineFrame> {
    crate::backend::capture_frame(target)
}

#[cfg(any(feature = "snapshot", feature = "realtime-capture-provider"))]
pub(crate) fn encode_png(width: u32, height: u32, rgba: Vec<u8>) -> Result<Vec<u8>> {
    let img = image::RgbaImage::from_raw(width, height, rgba)
        .ok_or_else(|| crate::error::Error::Failed("bitmap buffer size mismatch".into()))?;
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| crate::error::Error::Failed(format!("PNG encode failed: {e}")))?;
    Ok(png)
}
