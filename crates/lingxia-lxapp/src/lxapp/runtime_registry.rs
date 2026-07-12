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
        LxAppError::Runtime(
            "LxApps manager singleton had been initialized by another instance".to_string(),
        )
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

/// User override for the product display language (the settings-page
/// "Language" choice). `None` follows the system locale.
static DISPLAY_LANGUAGE: Mutex<Option<String>> = Mutex::new(None);

/// Set (or clear) the display-language override. The shell that owns the
/// language setting seeds this at startup and updates it on change so every
/// `get_locale` consumer — native chrome i18n included — follows the user's
/// choice without re-reading the settings store.
pub fn set_display_language(language: Option<String>) {
    let normalized = language.filter(|value| !value.trim().is_empty());
    *DISPLAY_LANGUAGE.lock().unwrap_or_else(|e| e.into_inner()) = normalized;
}

/// Get the product display language: the user override when set, else the
/// system locale. Returns "en-US" if the SDK has not been initialized.
pub fn get_locale() -> String {
    if let Some(language) = DISPLAY_LANGUAGE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
    {
        return language;
    }
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
