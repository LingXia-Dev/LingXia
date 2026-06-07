use std::sync::{Arc, OnceLock};

use lingxia_platform::Platform;

static PLATFORM: OnceLock<Arc<Platform>> = OnceLock::new();

pub(crate) fn set_platform(platform: Arc<Platform>) {
    let _ = PLATFORM.set(platform);
}

pub(crate) fn platform() -> crate::Result<Arc<Platform>> {
    PLATFORM
        .get()
        .cloned()
        .ok_or_else(|| crate::Error::internal("LingXia native runtime is not initialized"))
}
