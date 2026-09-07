//! Host-wide light/dark appearance: the value an lxapp resolves `auto`
//! against, and the scheme the host's own chrome follows.
//!
//! The platform reports what the operating system is set to. This layer adds
//! the user's own choice on top: `auto` follows the system, `light` and `dark`
//! pin it for the whole product. An lxapp that pinned its own scheme keeps it.

use super::page_chrome::{AppearancePreference, ResolvedAppearance};
use super::runtime_registry::get_platform;
use crate::error::LxAppError;
use lingxia_platform::traits::app_runtime::AppRuntime;
use lingxia_platform::traits::ui::UIUpdate;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock, RwLock};
use tokio::sync::mpsc;

/// What the host currently follows, and how it resolves right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostAppearanceState {
    /// The user's choice: `auto` follows the system.
    pub preference: AppearancePreference,
    /// What that choice resolves to at this moment.
    pub resolved: ResolvedAppearance,
}

/// A revisioned update, so a subscriber that arrives mid-transition can drop
/// what its initial snapshot already covered.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostAppearanceUpdate {
    pub revision: u64,
    pub state: HostAppearanceState,
}

fn preference_slot() -> &'static RwLock<AppearancePreference> {
    static PREFERENCE: OnceLock<RwLock<AppearancePreference>> = OnceLock::new();
    PREFERENCE.get_or_init(|| RwLock::new(AppearancePreference::Auto))
}

fn revision_counter() -> &'static AtomicU64 {
    static REVISION: AtomicU64 = AtomicU64::new(1);
    &REVISION
}

fn subscribers() -> &'static Mutex<Vec<mpsc::UnboundedSender<HostAppearanceUpdate>>> {
    static SUBSCRIBERS: OnceLock<Mutex<Vec<mpsc::UnboundedSender<HostAppearanceUpdate>>>> =
        OnceLock::new();
    SUBSCRIBERS.get_or_init(|| Mutex::new(Vec::new()))
}

fn preference() -> AppearancePreference {
    *preference_slot()
        .read()
        .unwrap_or_else(|error| error.into_inner())
}

fn system_dark() -> bool {
    get_platform().is_some_and(|platform| platform.host_appearance_dark())
}

/// The light/dark value the host is in. Every `auto` resolution reads this
/// instead of the raw platform value, so one preference moves the product.
pub fn host_appearance_dark() -> bool {
    match preference() {
        AppearancePreference::Light => false,
        AppearancePreference::Dark => true,
        AppearancePreference::Auto => system_dark(),
    }
}

pub fn host_appearance_state() -> HostAppearanceState {
    HostAppearanceState {
        preference: preference(),
        resolved: if host_appearance_dark() {
            ResolvedAppearance::Dark
        } else {
            ResolvedAppearance::Light
        },
    }
}

fn host_appearance_update() -> HostAppearanceUpdate {
    HostAppearanceUpdate {
        revision: revision_counter().load(Ordering::Acquire),
        state: host_appearance_state(),
    }
}

/// Subscribe without an initial-snapshot race: the snapshot and the channel
/// are taken under one lock, and later updates carry a higher revision.
#[doc(hidden)]
pub fn subscribe_host_appearance() -> (
    HostAppearanceUpdate,
    mpsc::UnboundedReceiver<HostAppearanceUpdate>,
) {
    let mut registered = subscribers()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let (sender, receiver) = mpsc::unbounded_channel();
    registered.retain(|subscriber| !subscriber.is_closed());
    registered.push(sender);
    (host_appearance_update(), receiver)
}

/// Load the persisted preference during bootstrap. The host chrome is told
/// about a pinned scheme here; `auto` leaves the platform's own value alone.
pub fn initialize_host_appearance() {
    let Some(platform) = get_platform() else {
        return;
    };
    let stored = lingxia_service::settings::host_appearance(&platform.app_data_dir())
        .ok()
        .flatten()
        .and_then(|value| value.parse::<AppearancePreference>().ok())
        .unwrap_or(AppearancePreference::Auto);
    *preference_slot()
        .write()
        .unwrap_or_else(|error| error.into_inner()) = stored;
    publish_host_color_mode(&platform, stored);
}

/// Pin the whole host to `light`/`dark`, or follow the system with `auto`.
pub fn set_host_appearance_preference(
    preference: AppearancePreference,
) -> Result<HostAppearanceState, LxAppError> {
    let platform = get_platform()
        .ok_or_else(|| LxAppError::Runtime("platform runtime is not initialized".to_string()))?;
    let changed = {
        let mut slot = preference_slot()
            .write()
            .unwrap_or_else(|error| error.into_inner());
        let changed = *slot != preference;
        *slot = preference;
        changed
    };
    // `auto` clears the stored override rather than pinning today's answer:
    // the next launch must follow the system as it is then.
    let stored = match preference {
        AppearancePreference::Auto => None,
        other => Some(other.as_str()),
    };
    lingxia_service::settings::set_host_appearance(&platform.app_data_dir(), stored)
        .map_err(|error| LxAppError::Runtime(error.to_string()))?;
    if changed {
        revision_counter().fetch_add(1, Ordering::AcqRel);
        publish_host_color_mode(&platform, preference);
        // Every lxapp still on `auto` resolves against the new host value.
        // That call also broadcasts the new state to live Logic workers.
        super::runtime_ops::refresh_auto_appearances();
    }
    Ok(host_appearance_state())
}

/// The system flipped underneath us. Only `auto` moves with it, but the
/// resolved value it reports changes either way.
pub fn refresh_host_appearance_system() {
    revision_counter().fetch_add(1, Ordering::AcqRel);
    notify();
}

fn notify() {
    let update = host_appearance_update();
    let mut registered = subscribers()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    registered.retain(|subscriber| subscriber.send(update).is_ok());
}

fn publish_host_color_mode(
    platform: &lingxia_platform::Platform,
    preference: AppearancePreference,
) {
    let dark = match preference {
        AppearancePreference::Auto => None,
        AppearancePreference::Light => Some(false),
        AppearancePreference::Dark => Some(true),
    };
    platform.set_host_color_mode(dark);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pinned_preference_resolves_without_the_platform() {
        *preference_slot()
            .write()
            .unwrap_or_else(|error| error.into_inner()) = AppearancePreference::Dark;
        assert!(host_appearance_dark());
        let state = host_appearance_state();
        assert_eq!(state.preference, AppearancePreference::Dark);
        assert_eq!(state.resolved, ResolvedAppearance::Dark);
        assert!(host_appearance_update().revision >= 1);

        *preference_slot()
            .write()
            .unwrap_or_else(|error| error.into_inner()) = AppearancePreference::Light;
        assert!(!host_appearance_dark());
        assert_eq!(host_appearance_state().resolved, ResolvedAppearance::Light);

        // `auto` with no platform registered falls back to light rather than
        // guessing dark.
        *preference_slot()
            .write()
            .unwrap_or_else(|error| error.into_inner()) = AppearancePreference::Auto;
        assert_eq!(host_appearance_dark(), system_dark());
    }
}
