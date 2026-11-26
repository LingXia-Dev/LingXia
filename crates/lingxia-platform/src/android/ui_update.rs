use super::app::Platform;
use crate::error::{chrome_callback_error, is_presenter_unavailable, presenter_unavailable};
use crate::traits::ui::{Appearance, LaunchFace, UIUpdate};
use jni::objects::{JClass, JValue};
use jni::sys::jlong;
use jni::{jni_sig, jni_str};
use lingxia_error::Error;

impl Appearance for Platform {
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

    fn set_host_color_mode(&self, dark: Option<bool>) {
        let Ok(lxapp_class) = super::get_cached_class(super::CachedClass::LxApp) else {
            return;
        };
        // -1 follows the system; 0 and 1 pin the app's own appearance.
        let mode = match dark {
            None => -1,
            Some(false) => 0,
            Some(true) => 1,
        };
        let _ = super::with_env(|env| {
            env.call_static_method(
                lxapp_class,
                jni_str!("setHostColorMode"),
                jni_sig!("(I)V"),
                &[JValue::Int(mode)],
            )
            .map(|_| ())
        });
    }

    fn apply_lxapp_appearance(&self, appid: &str, dark: bool) -> Result<(), Error> {
        let lxapp_class: &JClass = super::get_cached_class(super::CachedClass::LxApp)
            .map_err(|error| Error::internal(error.to_string()))?;
        super::with_env(|env| -> Result<(), Error> {
            let appid = env.new_string(appid)?;
            let applied = env.call_static_method(
                lxapp_class,
                jni_str!("applyAppearance"),
                jni_sig!("(Ljava/lang/String;Z)Z"),
                &[JValue::Object(&appid), JValue::Bool(dark)],
            )?;
            if applied.z()? {
                Ok(())
            } else {
                Err(Error::internal(
                    "applyAppearance returned false".to_string(),
                ))
            }
        })
    }
}

impl LaunchFace for Platform {
    fn notify_home_first_ready(&self) {
        let Ok(lxapp_class) = super::get_cached_class(super::CachedClass::LxApp) else {
            return;
        };
        let _ = super::with_env(|env| {
            env.call_static_method(
                lxapp_class,
                jni_str!("onHomeFirstReady"),
                jni_sig!("()V"),
                &[],
            )
            .map(|_| ())
        });
    }

    fn show_splash_campaign(&self, image_path: String, duration_ms: u32) {
        let Ok(lxapp_class) = super::get_cached_class(super::CachedClass::LxApp) else {
            return;
        };
        let _ = super::with_env(|env| {
            let path = env.new_string(&image_path)?;
            env.call_static_method(
                lxapp_class,
                jni_str!("showSplashCampaign"),
                jni_sig!("(Ljava/lang/String;I)V"),
                &[JValue::Object(&path), JValue::Int(duration_ms as i32)],
            )
            .map(|_| ())
        });
    }
}

impl UIUpdate for Platform {
    async fn measure_page_chrome_capsule(&self, appid: String) -> Result<Option<String>, Error> {
        let payload = crate::rt::native_call_with(
            |callback_id| {
                let capsule_class: &JClass =
                    super::get_cached_class(super::CachedClass::LxAppCapsule)
                        .map_err(|error| Error::internal(error.to_string()))?;
                super::with_env(|env| -> Result<(), Error> {
                    let appid = env.new_string(&appid)?;
                    env.call_static_method(
                        capsule_class,
                        jni_str!("getCapsuleRect"),
                        jni_sig!("(JLjava/lang/String;)V"),
                        &[JValue::Long(callback_id as jlong), JValue::Object(&appid)],
                    )?;
                    Ok(())
                })
            },
            chrome_callback_error,
        )
        .await?;
        Ok((payload != "null").then_some(payload))
    }

    fn update_navbar_ui(&self, appid: String) -> Result<(), Error> {
        let lxapp_class: &JClass = super::get_cached_class(super::CachedClass::LxApp)
            .map_err(|e| Error::internal(e.to_string()))?;

        super::with_env(|env| -> Result<(), Error> {
            let appid_jstring = env.new_string(&appid)?;
            let result = env.call_static_method(
                lxapp_class,
                jni_str!("updateNavBarUI"),
                jni_sig!("(Ljava/lang/String;)Z"),
                &[JValue::Object(&appid_jstring)],
            )?;
            // The Kotlin side answers false only when no activity is hosting
            // this appid; every other outcome is posted to the UI thread.
            if result.z()? {
                Ok(())
            } else {
                Err(presenter_unavailable())
            }
        })
        .map_err(|e| match e {
            e if is_presenter_unavailable(&e) => e,
            e => Error::internal(format!(
                "Failed to update NavigationBar UI for appId: {}: {}",
                appid, e
            )),
        })
    }

    fn update_tabbar_ui(&self, appid: String) -> Result<(), Error> {
        let lxapp_class: &JClass = super::get_cached_class(super::CachedClass::LxApp)
            .map_err(|e| Error::internal(e.to_string()))?;

        super::with_env(|env| -> Result<(), Error> {
            let appid_jstring = env.new_string(&appid)?;
            let result = env.call_static_method(
                lxapp_class,
                jni_str!("updateTabBarUI"),
                jni_sig!("(Ljava/lang/String;)Z"),
                &[JValue::Object(&appid_jstring)],
            )?;
            // False only when no activity hosts this appid yet; it reads the state when it mounts.
            if result.z()? {
                Ok(())
            } else {
                Err(presenter_unavailable())
            }
        })
        .map_err(|e| match e {
            e if is_presenter_unavailable(&e) => e,
            e => Error::internal(format!(
                "Failed to update TabBar UI for appId: {}: {}",
                appid, e
            )),
        })
    }

    async fn update_tabbar_ui_async(&self, appid: String) -> Result<(), Error> {
        crate::rt::native_call_ui_with(
            |callback_id| {
                let lxapp_class: &JClass = super::get_cached_class(super::CachedClass::LxApp)
                    .map_err(|e| Error::internal(e.to_string()))?;

                super::with_env(|env| -> Result<(), Error> {
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
                    Error::internal(format!(
                        "Failed to update TabBar UI for appId: {}: {}",
                        appid, e
                    ))
                })
            },
            chrome_callback_error,
        )
        .await
    }

    fn update_orientation_ui(&self, appid: String) -> Result<(), Error> {
        let lxapp_class: &JClass = super::get_cached_class(super::CachedClass::LxApp)
            .map_err(|e| Error::internal(e.to_string()))?;

        super::with_env(|env| -> Result<(), Error> {
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
                Err(Error::internal(
                    "updateOrientationUI returned false".to_string(),
                ))
            }
        })
        .map_err(|e| {
            Error::internal(format!(
                "Failed to update orientation UI for appId: {}: {}",
                appid, e
            ))
        })
    }
}
