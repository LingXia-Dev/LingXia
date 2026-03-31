use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::ui::{ModalOptions, ToastOptions, UserFeedback};
use jni::objects::{JClass, JObject, JValue};
use jni::{jni_sig, jni_str};
use lingxia_webview::platform::android::with_env;

impl UserFeedback for Platform {
    fn show_toast(&self, options: ToastOptions) -> Result<(), PlatformError> {
        let toast_class: &JClass = super::get_cached_class(super::CachedClass::LxAppToast)
            .map_err(|e| PlatformError::Platform(e.to_string()))?;

        with_env(|env| -> Result<(), PlatformError> {
            let title_jstring = env.new_string(&options.title)?;
            let title_obj: JObject = title_jstring.into();

            let image_obj: JObject = if let Some(image) = &options.image {
                env.new_string(image)?.into()
            } else {
                JObject::null()
            };

            env.call_static_method(
                toast_class,
                jni_str!("showToast"),
                jni_sig!("(Ljava/lang/String;ILjava/lang/String;DZI)V"),
                &[
                    JValue::Object(&title_obj),
                    JValue::Int(options.icon as i32),
                    JValue::Object(&image_obj),
                    JValue::Double(options.duration),
                    JValue::Bool(options.mask),
                    JValue::Int(options.position as i32),
                ],
            )?;
            Ok(())
        })
        .map_err(|e| PlatformError::Platform(format!("Failed to show toast: {}", e)))
    }

    fn hide_toast(&self) -> Result<(), PlatformError> {
        let toast_class: &JClass = super::get_cached_class(super::CachedClass::LxAppToast)
            .map_err(|e| PlatformError::Platform(e.to_string()))?;

        with_env(|env| -> Result<(), PlatformError> {
            env.call_static_method(toast_class, jni_str!("hideToast"), jni_sig!("()V"), &[])?;
            Ok(())
        })
        .map_err(|e| PlatformError::Platform(format!("Failed to hide toast: {}", e)))
    }

    async fn show_modal(&self, options: ModalOptions) -> Result<String, PlatformError> {
        crate::rt::native_call(|callback_id| {
            let modal_class: &JClass = super::get_cached_class(super::CachedClass::LxAppModal)
                .map_err(|e| PlatformError::Platform(e.to_string()))?;

            with_env(|env| -> Result<(), PlatformError> {
                let title_jstring = env.new_string(&options.title)?;
                let content_jstring = env.new_string(&options.content)?;
                let cancel_text_jstring = env.new_string(&options.cancel_text)?;
                let confirm_text_jstring = env.new_string(&options.confirm_text)?;

                let cancel_color_obj: JObject = options
                    .cancel_color
                    .as_ref()
                    .map(|s| env.new_string(s).map(|js| js.into()))
                    .transpose()?
                    .unwrap_or(JObject::null());

                let confirm_color_obj: JObject = options
                    .confirm_color
                    .as_ref()
                    .map(|s| env.new_string(s).map(|js| js.into()))
                    .transpose()?
                    .unwrap_or(JObject::null());

                env.call_static_method(
                    modal_class,
                    jni_str!("showModal"),
                    jni_sig!("(Ljava/lang/String;Ljava/lang/String;ZLjava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;J)V"),
                    &[
                        JValue::Object(&title_jstring),
                        JValue::Object(&content_jstring),
                        JValue::Bool(options.show_cancel),
                        JValue::Object(&cancel_text_jstring),
                        JValue::Object(&cancel_color_obj),
                        JValue::Object(&confirm_text_jstring),
                        JValue::Object(&confirm_color_obj),
                        JValue::Long(callback_id as i64),
                    ],
                )?;
                Ok(())
            })
            .map_err(|e| PlatformError::Platform(format!("Failed to show modal: {}", e)))
        }).await
    }

    async fn show_action_sheet(
        &self,
        options: Vec<String>,
        cancel_text: String,
        item_color: String,
    ) -> Result<String, PlatformError> {
        crate::rt::native_call(|callback_id| {
            let action_sheet_class: &JClass =
                super::get_cached_class(super::CachedClass::LxAppActionSheet)
                    .map_err(|e| PlatformError::Platform(e.to_string()))?;

            with_env(|env| -> Result<(), PlatformError> {
                let string_class = env.find_class(jni_str!("java/lang/String"))?;
                let options_array =
                    env.new_object_array(options.len() as i32, string_class, JObject::null())?;
                for (idx, option) in options.iter().enumerate() {
                    let option_jstring = env.new_string(option)?;
                    options_array.set_element(env, idx, &option_jstring)?;
                }
                let options_array_obj: JObject = options_array.into();

                let cancel_text_jstring = env.new_string(&cancel_text)?;
                let item_color_jstring = env.new_string(&item_color)?;

                env.call_static_method(
                    action_sheet_class,
                    jni_str!("showActionSheet"),
                    jni_sig!("([Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;J)V"),
                    &[
                        JValue::Object(&options_array_obj),
                        JValue::Object(&cancel_text_jstring),
                        JValue::Object(&item_color_jstring),
                        JValue::Long(callback_id as i64),
                    ],
                )?;
                Ok(())
            })
            .map_err(|e| PlatformError::Platform(format!("Failed to show action sheet: {}", e)))
        })
        .await
    }
}
