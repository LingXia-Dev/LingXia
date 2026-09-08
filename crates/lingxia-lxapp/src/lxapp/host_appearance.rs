//! Host-wide light/dark appearance: the value an lxapp resolves `auto`
//! against, and the scheme the host's own chrome follows.
//!
//! The platform reports what the operating system is set to. This layer adds
//! the user's own choice on top: `auto` follows the system, `light` and `dark`
//! pin it for the whole product. An lxapp that pinned its own scheme keeps it.

use super::page_chrome::{AppearancePreference, ResolvedAppearance};
use super::runtime_registry::{get_lxapps_manager, get_platform};
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

/// The last state every subscriber has seen, so a system flip under a pinned
/// preference — which changes nothing observable — wakes nobody.
fn last_published() -> &'static Mutex<Option<HostAppearanceState>> {
    static LAST: OnceLock<Mutex<Option<HostAppearanceState>>> = OnceLock::new();
    LAST.get_or_init(|| Mutex::new(None))
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

/// Register before reading the snapshot so no update is lost. Consumers
/// discard queued updates already covered by the snapshot's revision.
#[doc(hidden)]
pub fn subscribe_host_appearance() -> (
    HostAppearanceUpdate,
    mpsc::UnboundedReceiver<HostAppearanceUpdate>,
) {
    subscribe_with_snapshot(subscribers(), host_appearance_update)
}

fn subscribe_with_snapshot(
    subscribers: &Mutex<Vec<mpsc::UnboundedSender<HostAppearanceUpdate>>>,
    snapshot: impl FnOnce() -> HostAppearanceUpdate,
) -> (
    HostAppearanceUpdate,
    mpsc::UnboundedReceiver<HostAppearanceUpdate>,
) {
    let (sender, receiver) = mpsc::unbounded_channel();
    {
        let mut registered = subscribers
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        registered.retain(|subscriber| !subscriber.is_closed());
        registered.push(sender);
    }
    // Resolving auto may synchronously query the native main thread. That
    // thread also publishes appearance changes and needs the subscriber lock.
    (snapshot(), receiver)
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
///
/// Persist first, then publish: a caller that was told the write failed must
/// not find the product already running in the scheme it rejected.
pub fn set_host_appearance_preference(
    preference: AppearancePreference,
) -> Result<HostAppearanceState, LxAppError> {
    let platform = get_platform()
        .ok_or_else(|| LxAppError::Runtime("platform runtime is not initialized".to_string()))?;
    // `auto` clears the stored override rather than pinning today's answer:
    // the next launch must follow the system as it is then.
    let stored = match preference {
        AppearancePreference::Auto => None,
        other => Some(other.as_str()),
    };
    lingxia_service::settings::set_host_appearance(&platform.app_data_dir(), stored)
        .map_err(|error| LxAppError::Runtime(error.to_string()))?;
    let changed = {
        let mut slot = preference_slot()
            .write()
            .unwrap_or_else(|error| error.into_inner());
        let changed = *slot != preference;
        *slot = preference;
        changed
    };
    if changed {
        revision_counter().fetch_add(1, Ordering::AcqRel);
        publish_host_color_mode(&platform, preference);
        // Every lxapp that follows the product re-resolves against the new
        // value, and each one tells its own Logic worker.
        super::runtime_ops::refresh_auto_appearances();
        notify();
    }
    Ok(host_appearance_state())
}

/// The system flipped underneath us. Only `auto` moves with it, so a pinned
/// product reports nothing: its state is unchanged.
pub fn refresh_host_appearance_system() {
    let state = host_appearance_state();
    {
        let mut last = last_published()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if *last == Some(state) {
            return;
        }
        *last = Some(state);
    }
    revision_counter().fetch_add(1, Ordering::AcqRel);
    notify();
}

fn notify() {
    let update = host_appearance_update();
    *last_published()
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = Some(update.state);
    {
        let mut registered = subscribers()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        registered.retain(|subscriber| subscriber.send(update).is_ok());
    }
    publish_to_logic(&update);
}

/// The product's own setting, for the Logic context that edits it. An lxapp's
/// resolved scheme travels separately, per lxapp, because a manifest pin can
/// hold it still while this moves.
fn publish_to_logic(update: &HostAppearanceUpdate) {
    let Some(manager) = get_lxapps_manager() else {
        return;
    };
    let payload = format!(
        "{{\"revision\":{},\"preference\":\"{}\"}}",
        update.revision,
        update.state.preference.as_str()
    );
    let appids: Vec<_> = manager
        .lxapps
        .iter()
        .map(|entry| entry.key().clone())
        .collect();
    for appid in appids {
        crate::appservice::event_bus::publish_app_event(
            &appid,
            crate::HOST_APPEARANCE_CHANGE_EVENT,
            Some(payload.clone()),
        );
    }
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
    fn snapshot_allows_native_publication_without_losing_updates() {
        let subscribers = Mutex::new(Vec::new());
        let update = HostAppearanceUpdate {
            revision: 2,
            state: HostAppearanceState {
                preference: AppearancePreference::Auto,
                resolved: ResolvedAppearance::Dark,
            },
        };
        let (initial, mut receiver) = subscribe_with_snapshot(&subscribers, || {
            // Model the main thread publishing while a background subscriber
            // waits for its native appearance query. This must not need a lock
            // retained by that waiting subscriber.
            let registered = subscribers
                .try_lock()
                .expect("snapshot holds subscriber lock");
            assert_eq!(registered.len(), 1);
            registered[0].send(update).unwrap();
            update
        });
        assert_eq!(initial.revision, update.revision);
        assert_eq!(initial.state, update.state);
        assert_eq!(receiver.try_recv().unwrap().revision, initial.revision);

        let later = HostAppearanceUpdate {
            revision: 3,
            state: HostAppearanceState {
                resolved: ResolvedAppearance::Light,
                ..update.state
            },
        };
        subscribers.lock().unwrap()[0].send(later).unwrap();
        let received = receiver.try_recv().unwrap();
        assert!(received.revision > initial.revision);
        assert_eq!(received.state, later.state);
    }

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
