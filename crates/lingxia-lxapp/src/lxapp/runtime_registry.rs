//! Process-wide runtime/manager registry and lookup helpers.

use super::*;

// Global instance of LxApps manager
static LXAPPS_MANAGER: OnceLock<Arc<LxApps>> = OnceLock::new();
// Global runtime available as soon as facade-driven runtime initialization starts.
static RUNTIME: OnceLock<Arc<Platform>> = OnceLock::new();

pub(crate) fn set_runtime(runtime: Arc<Platform>) {
    let _ = RUNTIME.set(runtime);
}

pub(crate) fn set_lxapps_manager(manager: Arc<LxApps>) -> Result<(), LxAppError> {
    LXAPPS_MANAGER.set(manager).map_err(|_| {
        LxAppError::Runtime("LxApps manager singleton had been initialized by another instance".to_string())
    })
}

/// Get access to the LxApps manager for navigation stack operations
pub(crate) fn get_lxapps_manager() -> Option<Arc<LxApps>> {
    LXAPPS_MANAGER.get().cloned()
}

/// Get the platform runtime instance.
/// Returns None if the SDK has not been initialized.
pub fn get_platform() -> Option<Arc<Platform>> {
    RUNTIME
        .get()
        .cloned()
        .or_else(|| LXAPPS_MANAGER.get().map(|manager| manager.runtime.clone()))
}

/// Get the system locale string.
/// Returns "en-US" as default if the SDK has not been initialized.
pub fn get_locale() -> String {
    RUNTIME
        .get()
        .map(|runtime| runtime.get_system_locale().to_string())
        .unwrap_or_else(|| "en-US".to_string())
}

/// Try to get a specific LxApp instance by lxappid
pub fn try_get(appid: &str) -> Option<Arc<LxApp>> {
    LXAPPS_MANAGER
        .get()
        .and_then(|manager| manager.lxapps.get(appid).map(|lxapp| lxapp.clone()))
}

pub fn find_page_by_instance_id(id: &str) -> Option<PageInstance> {
    LXAPPS_MANAGER.get().and_then(|manager| {
        manager
            .lxapps
            .iter()
            .find_map(|entry| entry.value().get_page_by_instance_id_str(id))
    })
}

/// Internal helper: get LxApp by appid, panics if not found.
/// Only for use within lingxia-lxapp where LxApp is known to exist.
pub(crate) fn get(appid: String) -> Arc<LxApp> {
    try_get(&appid).expect("LxApp not found")
}
