use std::sync::{Arc, Mutex, OnceLock};

/// Host lifecycle extension points that can register additional runtime behavior.
pub trait HostAddon: Send + Sync {
    /// Runs before LingXia initialization begins.
    fn before_init(&self) {}
    /// Registers JS logic extensions when the `standard` feature is enabled.
    #[cfg(feature = "standard")]
    fn install_logic_extensions(&self) {}
    /// Registers native host APIs before the runtime starts serving requests.
    fn install_host_apis(&self) {}
    /// Runs after LingXia initialization succeeds.
    fn after_init(&self) {}
    /// Starts long-lived services after the host runtime is warmed up.
    fn start_services(&self) {}
}

static HOST_ADDONS: OnceLock<Mutex<Vec<Arc<dyn HostAddon>>>> = OnceLock::new();

fn host_addons() -> &'static Mutex<Vec<Arc<dyn HostAddon>>> {
    HOST_ADDONS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Registers a host addon for future LingXia initialization cycles.
pub fn register_host_addon(addon: Box<dyn HostAddon>) {
    let mut installed = host_addons()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    installed.push(Arc::from(addon));
}

fn snapshot_host_addons() -> Vec<Arc<dyn HostAddon>> {
    host_addons()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

pub(crate) fn run_before_init() {
    let installed = snapshot_host_addons();
    for addon in installed.iter() {
        addon.before_init();
    }
}

pub(crate) fn run_install_logic_extensions() {
    #[cfg(feature = "standard")]
    {
        let installed = snapshot_host_addons();
        for addon in installed.iter() {
            addon.install_logic_extensions();
        }
    }
}

pub(crate) fn run_install_host_apis() {
    let installed = snapshot_host_addons();
    for addon in installed.iter() {
        addon.install_host_apis();
    }
}

pub(crate) fn run_after_init() {
    let installed = snapshot_host_addons();
    for addon in installed.iter() {
        addon.after_init();
    }
}

pub(crate) fn run_start_services() {
    let installed = snapshot_host_addons();
    for addon in installed.iter() {
        addon.start_services();
    }
}
