use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::ui::UIUpdate;
use jni::objects::{JClass, JValue};
use jni::sys::jlong;
use jni::{jni_sig, jni_str};

impl UIUpdate for Platform {
    fn host_appearance_dark(&self) -> bool {
        let Ok(lxapp_class) = super::get_cached_class(super::CachedClass::LxApp) else {
            return false;
        };
        super::with_env(|env| {
            env.call_static_method(
                lxapp_class,
                jni_str!("hostAppearanceDark"),
                jni_sig!("()Z"),
                &[],
            )
            .and_then(|value| value.z())
        })
        .unwrap_or(false)
    }

    fn apply_lxapp_appearance(&self, appid: &str, dark: bool) -> Result<(), PlatformError> {
        let lxapp_class: &JClass = super::get_cached_class(super::CachedClass::LxApp)
            .map_err(|error| PlatformError::Platform(error.to_string()))?;
        super::with_env(|env| -> Result<(), PlatformError> {
            let appid = env.new_string(appid)?;
            let applied = env.call_static_method(
                lxapp_class,
                jni_str!("applyAppearance"),
                jni_sig!("(Ljava/lang/String;Z)Z"),
                &[JValue::Object(&appid), JValue::Bool(dark.into())],
            )?;
            if applied.z()? {
                Ok(())
            } else {
                Err(PlatformError::Platform(
                    "applyAppearance returned false".to_string(),
                ))
            }
        })
    }

    async fn measure_page_chrome_capsule(
        &self,
        appid: String,
    ) -> Result<Option<String>, PlatformError> {
        let payload = crate::rt::native_call(|callback_id| {
            let capsule_class: &JClass = super::get_cached_class(super::CachedClass::LxAppCapsule)
                .map_err(|error| PlatformError::Platform(error.to_string()))?;
            super::with_env(|env| -> Result<(), PlatformError> {
                let appid = env.new_string(&appid)?;
                env.call_static_method(
                    capsule_class,
                    jni_str!("getCapsuleRect"),
                    jni_sig!("(JLjava/lang/String;)V"),
                    &[JValue::Long(callback_id as jlong), JValue::Object(&appid)],
                )?;
                Ok(())
            })
        })
        .await?;
        Ok((payload != "null").then_some(payload))
    }

    fn update_navbar_ui(&self, appid: String) -> Result<(), PlatformError> {
        let lxapp_class: &JClass = super::get_cached_class(super::CachedClass::LxApp)
            .map_err(|e| PlatformError::Platform(e.to_string()))?;

        super::with_env(|env| -> Result<(), PlatformError> {
            let appid_jstring = env.new_string(&appid)?;
            let result = env.call_static_method(
                lxapp_class,
                jni_str!("updateNavBarUI"),
                jni_sig!("(Ljava/lang/String;)Z"),
                &[JValue::Object(&appid_jstring)],
            )?;
            if result.z()? {
                Ok(())
            } else {
                Err(PlatformError::Platform(
                    "updateNavBarUI returned false".to_string(),
                ))
            }
        })
        .map_err(|e| {
            PlatformError::Platform(format!(
                "Failed to update NavigationBar UI for appId: {}: {}",
                appid, e
            ))
        })
    }

    fn update_tabbar_ui(&self, appid: String) -> Result<(), PlatformError> {
        let lxapp_class: &JClass = super::get_cached_class(super::CachedClass::LxApp)
            .map_err(|e| PlatformError::Platform(e.to_string()))?;

        super::with_env(|env| -> Result<(), PlatformError> {
            let appid_jstring = env.new_string(&appid)?;
            let result = env.call_static_method(
                lxapp_class,
                jni_str!("updateTabBarUI"),
                jni_sig!("(Ljava/lang/String;)Z"),
                &[JValue::Object(&appid_jstring)],
            )?;
            if result.z()? {
                Ok(())
            } else {
                Err(PlatformError::Platform(
                    "updateTabBarUI returned false".to_string(),
                ))
            }
        })
        .map_err(|e| {
            PlatformError::Platform(format!(
                "Failed to update TabBar UI for appId: {}: {}",
                appid, e
            ))
        })
    }

    async fn update_tabbar_ui_async(&self, appid: String) -> Result<(), PlatformError> {
        crate::rt::native_call_ui(|callback_id| {
            let lxapp_class: &JClass = super::get_cached_class(super::CachedClass::LxApp)
                .map_err(|e| PlatformError::Platform(e.to_string()))?;

            super::with_env(|env| -> Result<(), PlatformError> {
                let appid_jstring = env.new_string(&appid)?;
                env.call_static_method(
                    lxapp_class,
                    jni_str!("updateTabBarUIAsync"),
                    jni_sig!("(JLjava/lang/String;)V"),
                    &[
                        JValue::Long(callback_id as jlong),
                        JValue::Object(&appid_jstring),
                    ],
                )?;
                Ok(())
            })
            .map_err(|e| {
                PlatformError::Platform(format!(
                    "Failed to update TabBar UI for appId: {}: {}",
                    appid, e
                ))
            })
        })
        .await
    }

    fn update_orientation_ui(&self, appid: String) -> Result<(), PlatformError> {
        let lxapp_class: &JClass = super::get_cached_class(super::CachedClass::LxApp)
            .map_err(|e| PlatformError::Platform(e.to_string()))?;

        super::with_env(|env| -> Result<(), PlatformError> {
            let appid_jstring = env.new_string(&appid)?;
            let result = env.call_static_method(
                lxapp_class,
                jni_str!("updateOrientationUI"),
                jni_sig!("(Ljava/lang/String;)Z"),
                &[JValue::Object(&appid_jstring)],
            )?;
            if result.z()? {
                Ok(())
            } else {
                Err(PlatformError::Platform(
                    "updateOrientationUI returned false".to_string(),
                ))
            }
        })
        .map_err(|e| {
            PlatformError::Platform(format!(
                "Failed to update orientation UI for appId: {}: {}",
                appid, e
            ))
        })
    }
}
