use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::{ModalOptions, ToastOptions, UserFeedback};
use jni::objects::{JClass, JObject, JValue};

impl UserFeedback for Platform {
    fn show_toast(&self, options: ToastOptions) -> Result<(), PlatformError> {
        match || -> Result<(), Box<dyn std::error::Error>> {
            let mut env = lingxia_webview::get_env()?;
            let toast_class: &JClass = super::get_cached_class(super::CachedClass::LxAppToast)?
                .as_obj()
                .into();

            let title_jstring = env.new_string(&options.title)?;
            let title_obj: JObject = title_jstring.into();

            let image_obj: JObject = if let Some(image) = &options.image {
                env.new_string(image)?.into()
            } else {
                JObject::null()
            };

            env.call_static_method(
                toast_class,
                "showToast",
                "(Ljava/lang/String;ILjava/lang/String;DZI)V",
                &[
                    JValue::Object(&title_obj),
                    JValue::Int(options.icon as i32),
                    JValue::Object(&image_obj),
                    JValue::Double(options.duration),
                    JValue::Bool(options.mask as u8),
                    JValue::Int(options.position as i32),
                ],
            )?;

            Ok(())
        }() {
            Ok(_) => Ok(()),
            Err(e) => Err(PlatformError::Platform(format!(
                "Failed to show toast: {}",
                e
            ))),
        }
    }

    fn hide_toast(&self) -> Result<(), PlatformError> {
        match || -> Result<(), Box<dyn std::error::Error>> {
            let mut env = lingxia_webview::get_env()?;

            let toast_class: &JClass = super::get_cached_class(super::CachedClass::LxAppToast)?
                .as_obj()
                .into();

            env.call_static_method(toast_class, "hideToast", "()V", &[])?;
            Ok(())
        }() {
            Ok(_) => Ok(()),
            Err(e) => Err(PlatformError::Platform(format!(
                "Failed to hide toast: {}",
                e
            ))),
        }
    }

    fn show_modal(&self, options: ModalOptions, callback_id: u64) -> Result<(), PlatformError> {
        match || -> Result<(), Box<dyn std::error::Error>> {
            let mut env = lingxia_webview::get_env()?;
            let modal_class: &JClass = super::get_cached_class(super::CachedClass::LxAppModal)?
                .as_obj()
                .into();

            let title_jstring = env.new_string(&options.title)?;
            let content_jstring = env.new_string(&options.content)?;
            let cancel_text_jstring = env.new_string(&options.cancel_text)?;
            let confirm_text_jstring = env.new_string(&options.confirm_text)?;

            let cancel_color_obj: JObject = options
                .cancel_color
                .as_ref()
                .map(|s| env.new_string(s).map(|js| js.into()))
                .transpose()? // Result<Option<JObject>>
                .unwrap_or(JObject::null());

            let confirm_color_obj: JObject = options
                .confirm_color
                .as_ref()
                .map(|s| env.new_string(s).map(|js| js.into()))
                .transpose()? // Result<Option<JObject>>
                .unwrap_or(JObject::null());

            env.call_static_method(
                modal_class,
                "showModal",
                "(Ljava/lang/String;Ljava/lang/String;ZLjava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;J)V",
                &[
                    JValue::Object(&title_jstring.into()),
                    JValue::Object(&content_jstring.into()),
                    JValue::Bool(options.show_cancel as u8),
                    JValue::Object(&cancel_text_jstring.into()),
                    JValue::Object(&cancel_color_obj),
                    JValue::Object(&confirm_text_jstring.into()),
                    JValue::Object(&confirm_color_obj),
                    JValue::Long(callback_id as i64),
                ],
            )?;

            Ok(())
        }() {
            Ok(()) => Ok(()),
            Err(e) => Err(PlatformError::Platform(format!(
                "Failed to show modal: {}",
                e
            ))),
        }
    }

    fn show_action_sheet(
        &self,
        options: Vec<String>,
        cancel_text: String,
        item_color: String,
        callback_id: u64,
    ) -> Result<(), PlatformError> {
        match || -> Result<(), Box<dyn std::error::Error>> {
            let mut env = lingxia_webview::get_env()?;

            let action_sheet_class: &JClass =
                super::get_cached_class(super::CachedClass::LxAppActionSheet)?
                    .as_obj()
                    .into();

            let string_class = env.find_class("java/lang/String")?;
            let options_array =
                env.new_object_array(options.len() as i32, string_class, JObject::null())?;
            for (idx, option) in options.iter().enumerate() {
                let option_jstring = env.new_string(option)?;
                env.set_object_array_element(&options_array, idx as i32, option_jstring)?;
            }
            let options_array_obj: JObject = options_array.into();

            let cancel_text_jstring = env.new_string(&cancel_text)?;
            let item_color_jstring = env.new_string(&item_color)?;

            env.call_static_method(
                action_sheet_class,
                "showActionSheet",
                "([Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;J)V",
                &[
                    JValue::Object(&options_array_obj),
                    JValue::Object(&cancel_text_jstring.into()),
                    JValue::Object(&item_color_jstring.into()),
                    JValue::Long(callback_id as i64),
                ],
            )?;

            Ok(())
        }() {
            Ok(()) => Ok(()),
            Err(e) => Err(PlatformError::Platform(format!(
                "Failed to show action sheet: {}",
                e
            ))),
        }
    }
}
