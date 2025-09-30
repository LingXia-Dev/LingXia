use jni::objects::GlobalRef;
use std::sync::OnceLock;

mod app;
mod device;
mod location;
mod popup;
mod ui_update;
mod user_feedback;
pub use app::Platform;

/// Enumerates the cacheable Java classes we keep as global references.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum CachedClass {
    LxApp = 0,
}

impl CachedClass {
    const COUNT: usize = 2;

    fn missing_message(self) -> &'static str {
        match self {
            CachedClass::LxApp => "Global LxApp class reference not available",
        }
    }
}

fn cached_slot(kind: CachedClass) -> &'static OnceLock<GlobalRef> {
    static CLASS_CACHE: [OnceLock<GlobalRef>; CachedClass::COUNT] =
        [OnceLock::new(), OnceLock::new()];
    &CLASS_CACHE[kind as usize]
}

/// Initialize a cached Java class reference (called from JNI_OnLoad)
pub fn init_cached_class(kind: CachedClass, global_ref: GlobalRef) {
    let _ = cached_slot(kind).set(global_ref);
}

/// Fetch a cached Java class reference
pub(crate) fn get_cached_class(kind: CachedClass) -> Result<&'static GlobalRef, &'static str> {
    cached_slot(kind).get().ok_or(kind.missing_message())
}
