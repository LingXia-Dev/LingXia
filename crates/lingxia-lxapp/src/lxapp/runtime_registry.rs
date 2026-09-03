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

/// Re-serve every open page after the simulated host class changed.
///
/// A page reads its class from the bridge config the host writes into the
/// document at load, so a page that is already rendering carries the old one.
/// Only the runner ever changes the class, and only when the developer picks a
/// device frame of a different shape — the same gesture that reloads the page
/// in a browser's device mode, and the same result as picking a different
/// simulator. A shipped host is one machine for its whole life and never
/// reaches this.
///
/// `load_html` rather than `WebView::reload`, for the reason the in-place
/// restart gives: these pages come from `loadHTMLString` with a logical base
/// URL, and reloading requests that URL's raw source, dropping the injected
/// config entirely.
pub(crate) fn reload_pages_for_host_class_change() {
    let Some(manager) = get_lxapps_manager() else {
        return;
    };
    // Collect first: looking the appid up in this same map while a writer is
    // queued deadlocks the caller.
    let apps: Vec<Arc<LxApp>> = manager
        .lxapps
        .iter()
        .map(|entry| entry.value().clone())
        .collect();
    for app in apps {
        for page in app.live_page_instances() {
            // Only documents that are actually on screen. A page that left the
            // stack is parked awaiting re-entry and a headless or LRU-detached
            // one has no WebView at all; serving either would un-park it and
            // run its code off-screen against a service that is gone.
            if page.webview().is_none() || page.document_is_departing() {
                continue;
            }
            // The old document's in-flight view calls and channels belong to a
            // page that is about to be replaced.
            page.cancel_bridge_work();
            if let Err(error) = page.load_html() {
                warn!("failed to re-serve page after a host class change: {error}")
                    .with_appid(app.appid.clone());
            }
        }
    }
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
