//! Backends that rasterize a shared scene.
pub mod cpu;
pub(crate) mod cpu_paint;
#[cfg(feature = "gpu")]
pub mod gpu;
pub mod pool;
#[cfg(feature = "gpu")]
pub mod render_thread;
pub mod sentinel;
#[cfg(feature = "gpu")]
pub mod surfaces;

#[cfg(test)]
pub(crate) mod backdrop_tests;
#[cfg(test)]
mod composite_tests;

/// Video player costs for frame metrics; null without the GPU renderer.
pub fn video_stats() -> serde_json::Value {
    #[cfg(feature = "gpu")]
    return gpu::video::stats();
    #[cfg(not(feature = "gpu"))]
    serde_json::Value::Null
}

/// The page's scene asks player `key` to play `source`.
pub(crate) fn video_configure(
    key: &str,
    source: Result<crate::scene::video::Source, crate::scene::video::VideoError>,
) {
    #[cfg(feature = "gpu")]
    gpu::video::configure(key, source);
    #[cfg(not(feature = "gpu"))]
    let _ = (key, source);
}

/// Player `key`'s state; `None` until a frame draws it.
pub(crate) fn video_status(key: &str) -> Option<crate::scene::video::Status> {
    #[cfg(feature = "gpu")]
    return gpu::video::status(key);
    #[cfg(not(feature = "gpu"))]
    {
        let _ = key;
        None
    }
}

/// A handle's or Logic's command to player `key`.
pub(crate) fn video_command(
    key: &str,
    op: crate::scene::video::VideoOp,
) -> Result<(), crate::scene::video::VideoError> {
    #[cfg(feature = "gpu")]
    return gpu::video::command(key, op);
    #[cfg(not(feature = "gpu"))]
    {
        let _ = (key, op);
        Err(crate::scene::video::VideoError::new(
            "VIDEO_NO_PLAYER",
            "this renderer plays no video",
        ))
    }
}
