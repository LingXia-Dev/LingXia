use jni::{
    Env, JavaVM,
    errors::Error as JniError,
    objects::{Global, JClass, JObject},
};
use std::sync::OnceLock;

mod app;
mod device;
mod file;
mod location;
mod media;
mod network;
mod pull_to_refresh;
mod screenshot;
mod surface;
mod ui_update;
mod update;
mod user_feedback;
mod video_player;
mod wifi;
pub use app::Platform;
pub use device::{
    get_android_id, get_api_level, get_system_property, has_telephony_feature,
    read_external_storage_text, write_external_storage_text,
};

static JAVA_VM: OnceLock<JavaVM> = OnceLock::new();
static APPLICATION_CONTEXT: OnceLock<Global<JObject<'static>>> = OnceLock::new();

pub fn initialize_jni(vm: JavaVM) {
    let _ = JAVA_VM.set(vm);
}

/// Register the host application's `android.content.Context` exactly once,
/// at platform initialization. The platform crate uses this to fulfill any
/// API that internally needs a Context (display metrics, Settings.Secure,
/// resources). Hosts that integrate this crate directly — not just lingxia
/// SDK apps — provide their own Context here without dragging in the SDK's
/// LxApp class.
pub fn set_application_context(env: &mut Env, ctx: &JObject) -> Result<(), String> {
    let global = env
        .new_global_ref(ctx)
        .map_err(|e| format!("Failed to create global ref for application context: {}", e))?;
    APPLICATION_CONTEXT
        .set(global)
        .map_err(|_| "Application context already registered".to_string())
}

/// Borrow the registered application context as a local reference.
/// Returns `None` if [`set_application_context`] has not been called yet.
pub(crate) fn application_context<'a>(env: &mut Env<'a>) -> Option<JObject<'a>> {
    let global = APPLICATION_CONTEXT.get()?;
    env.new_local_ref(global).ok()
}

pub(crate) fn with_env<T, E>(f: impl FnOnce(&mut Env) -> Result<T, E>) -> Result<T, E>
where
    E: From<JniError>,
{
    let vm = JAVA_VM
        .get()
        .ok_or_else(|| E::from(JniError::UninitializedJavaVM))?;
    vm.attach_current_thread(f)
}

/// Enumerates the cacheable Java classes we keep as global references.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum CachedClass {
    Lingxia = 0,
    LxAppMedia = 1,
    PreviewMediaPayload = 2,
    LxAppDevice = 3,
    LxAppLocation = 4,
    LxAppSurface = 5,
    LxAppToast = 6,
    LxAppModal = 7,
    LxAppActionSheet = 8,
    LxAppPicker = 9,
    LxAppFile = 10,
    ComponentRouter = 11,
    LxAppPullToRefresh = 12,
    UpdateManager = 13,
    LxAppCapsule = 14,
    LxAppWifi = 15,
    LxAppNetwork = 16,
    AppScreenshot = 17,
    LxApp = 18,
}

impl CachedClass {
    const COUNT: usize = 19;

    pub const fn class_path(self) -> &'static str {
        match self {
            CachedClass::Lingxia => "com/lingxia/app/Lingxia",
            CachedClass::LxApp => "com/lingxia/lxapp/LxApp",
            CachedClass::LxAppMedia => "com/lingxia/lxapp/APIs/LxAppMedia",
            CachedClass::PreviewMediaPayload => "com/lingxia/lxapp/APIs/media/PreviewMediaPayload",
            CachedClass::LxAppDevice => "com/lingxia/lxapp/APIs/LxAppDevice",
            CachedClass::LxAppLocation => "com/lingxia/lxapp/APIs/LxAppLocation",
            CachedClass::LxAppSurface => "com/lingxia/lxapp/APIs/LxAppSurface",
            CachedClass::LxAppToast => "com/lingxia/lxapp/APIs/LxAppToast",
            CachedClass::LxAppModal => "com/lingxia/lxapp/APIs/LxAppModal",
            CachedClass::LxAppActionSheet => "com/lingxia/lxapp/APIs/LxAppActionSheet",
            CachedClass::LxAppPicker => "com/lingxia/lxapp/APIs/LxAppPicker",
            CachedClass::LxAppFile => "com/lingxia/lxapp/APIs/LxAppFile",
            CachedClass::ComponentRouter => "com/lingxia/lxapp/NativeComponents/ComponentRouter",
            CachedClass::LxAppPullToRefresh => "com/lingxia/lxapp/APIs/LxAppPullToRefresh",
            CachedClass::UpdateManager => "com/lingxia/app/UpdateManager",
            CachedClass::LxAppWifi => "com/lingxia/lxapp/APIs/LxAppWifi",
            CachedClass::LxAppCapsule => "com/lingxia/lxapp/APIs/LxAppCapsule",
            CachedClass::LxAppNetwork => "com/lingxia/lxapp/APIs/LxAppNetwork",
            CachedClass::AppScreenshot => "com/lingxia/app/AppScreenshot",
        }
    }

    fn missing_message(self) -> &'static str {
        match self {
            CachedClass::Lingxia => concat!(
                "Global class reference not found: ",
                "com/lingxia/app/Lingxia"
            ),
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
            CachedClass::LxAppSurface => concat!(
                "Global class reference not found: ",
                "com/lingxia/lxapp/APIs/LxAppSurface"
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
            CachedClass::LxAppFile => concat!(
                "Global class reference not found: ",
                "com/lingxia/lxapp/APIs/LxAppFile"
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
                "com/lingxia/app/UpdateManager"
            ),
            CachedClass::LxAppWifi => concat!(
                "Global class reference not found: ",
                "com/lingxia/lxapp/APIs/LxAppWifi"
            ),
            CachedClass::LxAppCapsule => concat!(
                "Global class reference not found: ",
                "com/lingxia/lxapp/APIs/LxAppCapsule"
            ),
            CachedClass::LxAppNetwork => concat!(
                "Global class reference not found: ",
                "com/lingxia/lxapp/APIs/LxAppNetwork"
            ),
            CachedClass::AppScreenshot => concat!(
                "Global class reference not found: ",
                "com/lingxia/app/AppScreenshot"
            ),
        }
    }
}

fn cached_slot(kind: CachedClass) -> &'static OnceLock<Global<JClass<'static>>> {
    static CLASS_CACHE: [OnceLock<Global<JClass<'static>>>; CachedClass::COUNT] = [
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
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
    ];
    &CLASS_CACHE[kind as usize]
}

/// Initialize a cached Java class reference (called from JNI_OnLoad)
pub fn init_cached_class(kind: CachedClass, global_ref: Global<JClass<'static>>) {
    let _ = cached_slot(kind).set(global_ref);
}

/// Fetch a cached Java class reference
pub(crate) fn get_cached_class(
    kind: CachedClass,
) -> Result<&'static Global<JClass<'static>>, &'static str> {
    cached_slot(kind).get().ok_or(kind.missing_message())
}
