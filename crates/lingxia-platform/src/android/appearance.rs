use jni::objects::JValue;
use jni::{jni_sig, jni_str};

use crate::error::PlatformError;
use crate::traits::appearance::{Appearance, AppearancePreference, AppearanceState};

use super::Platform;

fn with_lxapp<T>(
    call: impl FnOnce(&mut jni::Env, &jni::objects::JClass) -> Result<T, jni::errors::Error>,
) -> Result<T, PlatformError> {
    let class = super::get_cached_class(super::CachedClass::LxApp)
        .map_err(|error| PlatformError::Platform(error.to_string()))?;
    super::with_env(|env| call(env, class))
        .map_err(|error| PlatformError::Platform(format!("appearance JNI call failed: {error}")))
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
        let preference = with_lxapp(|env, class| {
            env.call_static_method(
                class,
                jni_str!("getHostAppearancePreference"),
                jni_sig!("()I"),
                &[],
            )?
            .i()
        })?;
        let preference = match preference {
            0 => AppearancePreference::System,
            1 => AppearancePreference::Light,
            2 => AppearancePreference::Dark,
            raw => {
                return Err(PlatformError::Platform(format!(
                    "invalid Android appearance preference: {raw}"
                )));
            }
        };
        let effective_dark = with_lxapp(|env, class| {
            env.call_static_method(
                class,
                jni_str!("getHostAppearanceEffectiveDark"),
                jni_sig!("()Z"),
                &[],
            )?
            .z()
        })?;
        Ok(AppearanceState {
            preference,
            effective_dark,
        })
    }

    fn set_appearance(
        &self,
        preference: AppearancePreference,
    ) -> Result<AppearanceState, PlatformError> {
        let applied = with_lxapp(|env, class| {
            env.call_static_method(
                class,
                jni_str!("setHostAppearance"),
                jni_sig!("(I)Z"),
                &[JValue::Int(preference_raw(preference))],
            )?
            .z()
        })?;
        if !applied {
            return Err(PlatformError::Platform(
                "Android rejected host appearance".into(),
            ));
        }
        self.get_appearance()
    }

    fn add_appearance_change_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        with_lxapp(|env, class| {
            env.call_static_method(
                class,
                jni_str!("addHostAppearanceChangeListener"),
                jni_sig!("(J)V"),
                &[JValue::Long(callback_id as i64)],
            )?;
            Ok(())
        })
    }

    fn remove_appearance_change_listener(&self, callback_id: u64) -> Result<(), PlatformError> {
        with_lxapp(|env, class| {
            env.call_static_method(
                class,
                jni_str!("removeHostAppearanceChangeListener"),
                jni_sig!("(J)V"),
                &[JValue::Long(callback_id as i64)],
            )?;
            Ok(())
        })
    }
}
