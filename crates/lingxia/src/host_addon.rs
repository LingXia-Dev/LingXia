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
    /// Picks the campaign screen shown after this cold start's launch face,
    /// with a countdown the user can skip.
    ///
    /// Runs once the runtime is up, not on the cold-start path, so reading a
    /// file or checking a clock here costs the launch nothing — but an answer
    /// that arrives after the launch face lifts is dropped, so this is not
    /// the place to wait on a network. Choose only among assets already on
    /// disk; hand any downloading to [`crate::spawn`], which lands it in time
    /// for a later launch.
    fn select_campaign(&self, _launch: &crate::splash::Launch) -> crate::splash::CampaignChoice {
        crate::splash::CampaignChoice::default()
    }

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

/// Whether any addon is installed — lets startup-path work skip entirely.
pub(crate) fn any_registered() -> bool {
    !snapshot_host_addons().is_empty()
}

/// First addon that names a campaign wins. The screen has one writer by
/// construction — a second opinion would just be a race for the same pixels.
pub(crate) fn run_select_campaign(launch: &crate::splash::Launch) -> crate::splash::CampaignChoice {
    let installed = snapshot_host_addons();
    for addon in installed.iter() {
        let choice = addon.select_campaign(launch);
        if choice != crate::splash::CampaignChoice::default() {
            return choice;
        }
    }
    crate::splash::CampaignChoice::default()
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
