use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::ui::UIUpdate;
use jni::objects::{JClass, JValue};
use jni::sys::jlong;
use jni::{jni_sig, jni_str};

impl UIUpdate for Platform {
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
