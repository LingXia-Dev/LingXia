use std::sync::{Arc, Mutex, OnceLock};

pub trait HostAddon: Send + Sync {
    fn before_init(&self) {}
    #[cfg(feature = "js-lxapp")]
    fn install_logic_extensions(&self) {}
    fn install_host_apis(&self) {}
    fn after_init(&self) {}
    fn start_services(&self) {}
}

static HOST_ADDONS: OnceLock<Mutex<Vec<Arc<dyn HostAddon>>>> = OnceLock::new();

fn host_addons() -> &'static Mutex<Vec<Arc<dyn HostAddon>>> {
    HOST_ADDONS.get_or_init(|| Mutex::new(Vec::new()))
}

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
    #[cfg(feature = "js-lxapp")]
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
