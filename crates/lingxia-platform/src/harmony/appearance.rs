use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{LazyLock, Mutex};

use lingxia_webview::platform::harmony::tsfn::call_arkts;

use crate::error::PlatformError;
use crate::traits::appearance::{Appearance, AppearancePreference, AppearanceState};

use super::Platform;

static PREFERENCE: AtomicU8 = AtomicU8::new(0);
static EFFECTIVE_DARK: AtomicBool = AtomicBool::new(false);
static LISTENERS: LazyLock<Mutex<HashSet<u64>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

fn preference_raw(preference: AppearancePreference) -> u8 {
    match preference {
        AppearancePreference::System => 0,
        AppearancePreference::Light => 1,
        AppearancePreference::Dark => 2,
    }
}

fn preference_from_raw(raw: u8) -> Result<AppearancePreference, PlatformError> {
    match raw {
        0 => Ok(AppearancePreference::System),
        1 => Ok(AppearancePreference::Light),
        2 => Ok(AppearancePreference::Dark),
        _ => Err(PlatformError::Platform(format!(
            "invalid Harmony appearance preference: {raw}"
        ))),
    }
}

fn state() -> Result<AppearanceState, PlatformError> {
    Ok(AppearanceState {
        preference: preference_from_raw(PREFERENCE.load(Ordering::Acquire))?,
        effective_dark: EFFECTIVE_DARK.load(Ordering::Acquire),
    })
}

fn emit_to(callback_id: u64, state: AppearanceState) {
    let payload = serde_json::json!({
        "preference": state.preference.as_str(),
        "effective": state.effective(),
    })
    .to_string();
    let _ = lingxia_messaging::invoke_callback(callback_id, Ok(payload));
}

fn emit(state: AppearanceState) {
    let listeners = LISTENERS
        .lock()
        .map(|listeners| listeners.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default();
    for callback_id in listeners {
        emit_to(callback_id, state);
    }
}

/// Seed the process state before the Rust runtime applies the persisted host
/// preference. ArkTS calls this with the current system-effective color mode.
pub fn initialize_appearance(effective_dark: bool) {
    EFFECTIVE_DARK.store(effective_dark, Ordering::Release);
}

/// Apply a configuration update reported by the Harmony application context.
pub fn update_appearance(
    preference_raw: u8,
    effective_dark: bool,
) -> Result<AppearanceState, PlatformError> {
    let preference = preference_from_raw(preference_raw)?;
    let previous = state()?;
    PREFERENCE.store(preference_raw, Ordering::Release);
    EFFECTIVE_DARK.store(effective_dark, Ordering::Release);
    let current = AppearanceState {
        preference,
        effective_dark,
    };
    if current != previous {
        emit(current);
    }
    Ok(current)
}

impl Appearance for Platform {
    fn get_appearance(&self) -> Result<AppearanceState, PlatformError> {
        state()
    }

    fn set_appearance(
        &self,
        preference: AppearancePreference,
    ) -> Result<AppearanceState, PlatformError> {
        let raw = preference_raw(preference);
        let raw_string = raw.to_string();
        call_arkts("setHostAppearance", &[raw_string.as_str()]).map_err(|error| {
            PlatformError::Platform(format!("failed to apply Harmony appearance: {error}"))
        })?;

        let previous = state()?;
        PREFERENCE.store(raw, Ordering::Release);
        if preference == AppearancePreference::Light {
            EFFECTIVE_DARK.store(false, Ordering::Release);
        } else if preference == AppearancePreference::Dark {
            EFFECTIVE_DARK.store(true, Ordering::Release);
        }
        let current = state()?;
        if current != previous {
            emit(current);
        }
        Ok(current)
    }

    fn add_appearance_change_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        LISTENERS
            .lock()
            .map_err(|_| PlatformError::Platform("appearance listener lock poisoned".into()))?
            .insert(callback_id);
        emit_to(callback_id, state()?);
        Ok(())
    }

    fn remove_appearance_change_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        LISTENERS
            .lock()
            .map_err(|_| PlatformError::Platform("appearance listener lock poisoned".into()))?
            .remove(&callback_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_updates_preserve_preference_and_effective_mode() {
        initialize_appearance(false);
        let state = update_appearance(2, true).unwrap();
        assert_eq!(state.preference, AppearancePreference::Dark);
        assert!(state.effective_dark);
        let state = update_appearance(0, false).unwrap();
        assert_eq!(state.preference, AppearancePreference::System);
        assert!(!state.effective_dark);
    }
}
