use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::ui::{PopupPresenter, PopupRequest};
use jni::objects::{JClass, JObject, JValue};
use jni::{jni_sig, jni_str};

impl PopupPresenter for Platform {
    fn show_popup(&self, request: PopupRequest) -> Result<(), PlatformError> {
        let PopupRequest {
            app_id,
            path,
            width_ratio,
            height_ratio,
            position,
        } = request;
        let popup_class: &JClass = super::get_cached_class(super::CachedClass::LxAppPopup)
            .map_err(|e| PlatformError::Platform(e.to_string()))?;

        lingxia_webview::platform::android::with_env(|env| -> Result<(), PlatformError> {
            let app_id_jstring = env.new_string(&app_id)?;
            let app_id_obj: JObject = app_id_jstring.into();

            let path_jstring = env.new_string(&path)?;
            let path_obj: JObject = path_jstring.into();

            env.call_static_method(
                popup_class,
                jni_str!("showPopup"),
                jni_sig!("(Ljava/lang/String;Ljava/lang/String;DDI)V"),
                &[
                    JValue::Object(&app_id_obj),
                    JValue::Object(&path_obj),
                    JValue::Double(width_ratio),
                    JValue::Double(height_ratio),
                    JValue::Int(position as i32),
                ],
            )?;
            Ok(())
        })
        .map_err(|e| PlatformError::Platform(format!("Failed to show popup: {}", e)))
    }

    fn hide_popup(&self, app_id: &str) -> Result<(), PlatformError> {
        let popup_class: &JClass = super::get_cached_class(super::CachedClass::LxAppPopup)
            .map_err(|e| PlatformError::Platform(e.to_string()))?;

        lingxia_webview::platform::android::with_env(|env| -> Result<(), PlatformError> {
            let app_id_jstring = env.new_string(app_id)?;
            let app_id_obj: JObject = app_id_jstring.into();

            env.call_static_method(
                popup_class,
                jni_str!("hidePopup"),
                jni_sig!("(Ljava/lang/String;)V"),
                &[JValue::Object(&app_id_obj)],
            )?;

            Ok(())
        })
        .map_err(|e| PlatformError::Platform(format!("Failed to hide popup: {}", e)))
    }
}
