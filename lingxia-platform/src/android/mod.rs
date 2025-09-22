use jni::objects::GlobalRef;
use std::sync::OnceLock;

mod app;
mod device;
mod ui_update;
mod user_feedback;
pub use app::Platform;

/// Global reference to LxApp class for worker threads
static LXAPP_CLASS: OnceLock<GlobalRef> = OnceLock::new();

/// Initialize LxApp class global reference (called from JNI_OnLoad)
pub fn init_lxapp_class(global_ref: GlobalRef) {
    let _ = LXAPP_CLASS.set(global_ref);
}

/// Get the global LxApp class reference
pub(crate) fn get_lxapp_class() -> Result<&'static GlobalRef, &'static str> {
    LXAPP_CLASS
        .get()
        .ok_or("Global LxApp class reference not available")
}
