use jni::objects::GlobalRef;
use std::sync::OnceLock;

mod app;
mod device;
mod document;
mod location;
mod media;
mod popup;
mod pull_to_refresh;
mod ui_update;
mod update;
mod user_feedback;
mod video_player;
pub use app::Platform;
pub use device::get_android_id;

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
    LxAppDocument = 10,
    ComponentRouter = 11,
    LxAppPullToRefresh = 12,
    UpdateManager = 13,
    LxAppCapsule = 14,
}

impl CachedClass {
    const COUNT: usize = 15;

    pub const fn class_path(self) -> &'static str {
        match self {
            CachedClass::LxApp => "com/lingxia/lxapp/LxApp",
            CachedClass::LxAppMedia => "com/lingxia/lxapp/APIs/LxAppMedia",
            CachedClass::PreviewMediaPayload => "com/lingxia/lxapp/APIs/media/PreviewMediaPayload",
            CachedClass::LxAppDevice => "com/lingxia/lxapp/APIs/LxAppDevice",
            CachedClass::LxAppLocation => "com/lingxia/lxapp/APIs/LxAppLocation",
            CachedClass::LxAppPopup => "com/lingxia/lxapp/APIs/LxAppPopup",
            CachedClass::LxAppToast => "com/lingxia/lxapp/APIs/LxAppToast",
            CachedClass::LxAppModal => "com/lingxia/lxapp/APIs/LxAppModal",
            CachedClass::LxAppActionSheet => "com/lingxia/lxapp/APIs/LxAppActionSheet",
            CachedClass::LxAppPicker => "com/lingxia/lxapp/APIs/LxAppPicker",
            CachedClass::LxAppDocument => "com/lingxia/lxapp/APIs/LxAppDocument",
            CachedClass::ComponentRouter => "com/lingxia/lxapp/NativeComponents/ComponentRouter",
            CachedClass::LxAppPullToRefresh => "com/lingxia/lxapp/APIs/LxAppPullToRefresh",
            CachedClass::UpdateManager => "com/lingxia/lxapp/UpdateManager",
            CachedClass::LxAppCapsule => "com/lingxia/lxapp/APIs/LxAppCapsule",
        }
    }

    fn missing_message(self) -> &'static str {
        match self {
            CachedClass::LxApp => concat!(
                "Global class reference not found: ",
                "com/lingxia/lxapp/LxApp"
            ),
            CachedClass::LxAppMedia => concat!(
                "Global class reference not found: ",
                "com/lingxia/lxapp/APIs/LxAppMedia"
            ),
            CachedClass::PreviewMediaPayload => concat!(
                "Global class reference not found: ",
                "com/lingxia/lxapp/APIs/media/PreviewMediaPayload"
            ),
            CachedClass::LxAppDevice => concat!(
                "Global class reference not found: ",
                "com/lingxia/lxapp/APIs/LxAppDevice"
            ),
            CachedClass::LxAppLocation => concat!(
                "Global class reference not found: ",
                "com/lingxia/lxapp/APIs/LxAppLocation"
            ),
            CachedClass::LxAppPopup => concat!(
                "Global class reference not found: ",
                "com/lingxia/lxapp/APIs/LxAppPopup"
            ),
            CachedClass::LxAppToast => concat!(
                "Global class reference not found: ",
                "com/lingxia/lxapp/APIs/LxAppToast"
            ),
            CachedClass::LxAppModal => concat!(
                "Global class reference not found: ",
                "com/lingxia/lxapp/APIs/LxAppModal"
            ),
            CachedClass::LxAppActionSheet => concat!(
                "Global class reference not found: ",
                "com/lingxia/lxapp/APIs/LxAppActionSheet"
            ),
            CachedClass::LxAppPicker => concat!(
                "Global class reference not found: ",
                "com/lingxia/lxapp/APIs/LxAppPicker"
            ),
            CachedClass::LxAppDocument => concat!(
                "Global class reference not found: ",
                "com/lingxia/lxapp/APIs/LxAppDocument"
            ),
            CachedClass::ComponentRouter => concat!(
                "Global class reference not found: ",
                "com/lingxia/lxapp/NativeComponents/ComponentRouter"
            ),
            CachedClass::LxAppPullToRefresh => concat!(
                "Global class reference not found: ",
                "com/lingxia/lxapp/APIs/LxAppPullToRefresh"
            ),
            CachedClass::UpdateManager => concat!(
                "Global class reference not found: ",
                "com/lingxia/lxapp/UpdateManager"
            ),
            CachedClass::LxAppCapsule => concat!(
                "Global class reference not found: ",
                "com/lingxia/lxapp/APIs/LxAppCapsule"
            ),
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
