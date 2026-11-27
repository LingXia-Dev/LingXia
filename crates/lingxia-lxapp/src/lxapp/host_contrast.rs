//! Host-wide contrast preference: `auto` follows the system, `more` puts every View page in
//! high contrast whatever the system says.

use super::runtime_registry::{get_lxapps_manager, get_platform};
use lingxia_platform::traits::app_runtime::AppRuntime;
use std::fmt;
use std::str::FromStr;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

/// What the product's contrast is set to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ContrastPreference {
    /// Follow the system's contrast setting.
    #[default]
    Auto,
    /// High contrast even where the system asks for standard.
    More,
}

impl ContrastPreference {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::More => "more",
        }
    }
}

impl fmt::Display for ContrastPreference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ContrastPreference {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "auto" => Ok(Self::Auto),
            "more" => Ok(Self::More),
            other => Err(format!(
                "contrast preference is 'auto' or 'more', got {other:?}"
            )),
        }
    }
}

/// The preference with the revision it was set at, so a late reader drops what it already has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostContrastUpdate {
    pub revision: u64,
    pub preference: ContrastPreference,
}

static PREFERENCE: RwLock<ContrastPreference> = RwLock::new(ContrastPreference::Auto);
static REVISION: AtomicU64 = AtomicU64::new(1);

pub fn host_contrast_update() -> HostContrastUpdate {
    HostContrastUpdate {
        revision: REVISION.load(Ordering::Acquire),
        preference: *PREFERENCE.read().unwrap_or_else(|error| error.into_inner()),
    }
}

/// Load the persisted preference during bootstrap.
pub fn initialize_host_contrast(platform: &lingxia_platform::Platform) {
    let stored = lingxia_service::settings::host_contrast(&platform.app_data_dir())
        .ok()
        .flatten()
        .and_then(|value| value.parse::<ContrastPreference>().ok())
        .unwrap_or_default();
    *PREFERENCE.write().unwrap_or_else(|error| error.into_inner()) = stored;
}

/// Persist first, then publish: a caller told the write failed must not see the product in the
/// contrast it rejected.
pub fn set_host_contrast_preference(
    preference: ContrastPreference,
) -> Result<HostContrastUpdate, lingxia_error::Error> {
    let platform = get_platform()
        .ok_or_else(|| lingxia_error::Error::internal("platform runtime is not initialized"))?;
    lingxia_service::settings::set_host_contrast(
        &platform.app_data_dir(),
        Some(preference.as_str()),
    )
    .map_err(|error| lingxia_error::Error::internal(error.to_string()))?;
    let changed = {
        let mut slot = PREFERENCE.write().unwrap_or_else(|error| error.into_inner());
        std::mem::replace(&mut *slot, preference) != preference
    };
    if changed {
        REVISION.fetch_add(1, Ordering::AcqRel);
        publish(host_contrast_update());
    }
    Ok(host_contrast_update())
}

fn publish(update: HostContrastUpdate) {
    if let Some(layer) = crate::feedback::feedback_layer() {
        layer.contrast_changed();
    }
    let Some(manager) = get_lxapps_manager() else {
        return;
    };
    let payload = format!(
        "{{\"revision\":{},\"preference\":\"{}\"}}",
        update.revision,
        update.preference.as_str()
    );
    let apps: Vec<_> = manager
        .lxapps
        .iter()
        .map(|entry| (entry.key().clone(), entry.value().clone()))
        .collect();
    for (appid, app) in apps {
        // Native bars resolve their roles for the contrast in effect.
        app.sync_host_ui();
        for page in app.live_page_instances() {
            page.push_contrast();
        }
        crate::appservice::event_bus::publish_app_event(
            &appid,
            crate::HOST_CONTRAST_CHANGE_EVENT,
            Some(payload.clone()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_preference_parses_only_its_two_values() {
        for preference in [ContrastPreference::Auto, ContrastPreference::More] {
            assert_eq!(preference.as_str().parse(), Ok(preference));
        }
        assert!("high".parse::<ContrastPreference>().is_err());
    }
}
