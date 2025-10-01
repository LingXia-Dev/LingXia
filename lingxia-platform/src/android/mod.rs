use jni::objects::GlobalRef;
use std::sync::OnceLock;

mod app;
mod device;
mod location;
mod media;
mod popup;
mod ui_update;
mod user_feedback;
pub use app::Platform;

/// Enumerates the cacheable Java classes we keep as global references.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum CachedClass {
    LxApp = 0,
    LxAppMedia = 1,
    PreviewMediaPayload = 2,
    LxAppDevice = 3,
    LxAppLocation = 4,
    LxAppPopup = 5,
    LxAppToast = 6,
    LxAppModal = 7,
    LxAppActionSheet = 8,
    LxAppPicker = 9,
}

impl CachedClass {
    const COUNT: usize = 10;

    fn missing_message(self) -> &'static str {
        match self {
            CachedClass::LxApp => "Global LxApp class reference not available",
            CachedClass::PreviewMediaPayload => {
                "Global PreviewMediaPayload class reference not available"
            }
            CachedClass::LxAppMedia => "Global LxAppMedia class reference not available",
            CachedClass::LxAppDevice => "Global LxAppDevice class reference not available",
            CachedClass::LxAppLocation => "Global LxAppLocation class reference not available",
            CachedClass::LxAppPopup => "Global LxAppPopup class reference not available",
            CachedClass::LxAppToast => "Global LxAppToast class reference not available",
            CachedClass::LxAppModal => "Global LxAppModal class reference not available",
            CachedClass::LxAppActionSheet => "Global LxAppActionSheet class reference not available",
            CachedClass::LxAppPicker => "Global LxAppPicker class reference not available",
        }
    }
}

fn cached_slot(kind: CachedClass) -> &'static OnceLock<GlobalRef> {
    static CLASS_CACHE: [OnceLock<GlobalRef>; CachedClass::COUNT] = [
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
    ];
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
