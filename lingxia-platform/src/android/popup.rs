use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::{PopupPresenter, PopupRequest};
use jni::objects::{JClass, JObject, JValue};

impl PopupPresenter for Platform {
    fn show_popup(&self, request: PopupRequest) -> Result<(), PlatformError> {
        match || -> Result<(), Box<dyn std::error::Error>> {
            let mut env = lingxia_webview::get_env()?;

            let PopupRequest {
                app_id,
                path,
                width_ratio,
                height_ratio,
                position,
            } = request;

            let lxapp_class: &JClass = super::get_lxapp_class()?.as_obj().into();

            let app_id_jstring = env.new_string(&app_id)?;
            let app_id_obj: JObject = app_id_jstring.into();

            let path_jstring = env.new_string(&path)?;
            let path_obj: JObject = path_jstring.into();

            env.call_static_method(
                lxapp_class,
                "showPopup",
                "(Ljava/lang/String;Ljava/lang/String;DDI)V",
                &[
                    JValue::Object(&app_id_obj),
                    JValue::Object(&path_obj),
                    JValue::Double(width_ratio),
                    JValue::Double(height_ratio),
                    JValue::Int(position as i32),
                ],
            )?;

            Ok(())
        }() {
            Ok(()) => Ok(()),
            Err(e) => Err(PlatformError::Platform(format!(
                "Failed to show popup: {}",
                e
            ))),
        }
    }

    fn hide_popup(&self, app_id: &str) -> Result<(), PlatformError> {
        match || -> Result<(), Box<dyn std::error::Error>> {
            let mut env = lingxia_webview::get_env()?;

            let lxapp_class: &JClass = super::get_lxapp_class()?.as_obj().into();

            let app_id_jstring = env.new_string(app_id)?;
            let app_id_obj: JObject = app_id_jstring.into();

            env.call_static_method(
                lxapp_class,
                "hidePopup",
                "(Ljava/lang/String;)V",
                &[JValue::Object(&app_id_obj)],
            )?;

            Ok(())
        }() {
            Ok(()) => Ok(()),
            Err(e) => Err(PlatformError::Platform(format!(
                "Failed to hide popup: {}",
                e
            ))),
        }
    }
}
