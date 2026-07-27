use crate::error::PlatformError;
use crate::traits::appearance::{Appearance, AppearancePreference, AppearanceState};

use super::Platform;

fn preference_from_raw(raw: i32) -> Result<AppearancePreference, PlatformError> {
    match raw {
        0 => Ok(AppearancePreference::System),
        1 => Ok(AppearancePreference::Light),
        2 => Ok(AppearancePreference::Dark),
        _ => Err(PlatformError::Platform(format!(
            "native host returned invalid appearance preference: {raw}"
        ))),
    }
}

fn preference_raw(preference: AppearancePreference) -> i32 {
    match preference {
        AppearancePreference::System => 0,
        AppearancePreference::Light => 1,
        AppearancePreference::Dark => 2,
    }
}

impl Appearance for Platform {
    fn get_appearance(&self) -> Result<AppearanceState, PlatformError> {
        Ok(AppearanceState {
            preference: preference_from_raw(super::ffi::host_appearance_preference())?,
            effective_dark: super::ffi::host_appearance_effective_dark(),
        })
    }

    fn set_appearance(
        &self,
        preference: AppearancePreference,
    ) -> Result<AppearanceState, PlatformError> {
        if !super::ffi::set_host_appearance(preference_raw(preference)) {
            return Err(PlatformError::Platform(
                "failed to apply host appearance".to_string(),
            ));
        }
        self.get_appearance()
    }

    fn add_appearance_change_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        super::ffi::add_host_appearance_change_listener(callback_id);
        Ok(())
    }

    fn remove_appearance_change_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        super::ffi::remove_host_appearance_change_listener(callback_id);
        Ok(())
    }
}
