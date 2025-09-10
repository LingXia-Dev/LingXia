use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::{ModalOptions, ModalResult, ToastOptions, UserFeedback};
use jni::objects::{JClass, JObject, JValue};
use std::collections::HashMap;

impl UserFeedback for Platform {
    fn show_toast(&self, options: ToastOptions) -> Result<(), PlatformError> {
        match || -> Result<(), Box<dyn std::error::Error>> {
            let mut env = lingxia_webview::get_env()?;

            // Get the LxApp class
            let lxapp_class: &JClass = super::get_lxapp_class()?.as_obj().into();

            // Convert parameters to Java objects
            let title_jstring = env.new_string(&options.title)?;
            let title_obj: JObject = title_jstring.into();

            let image_obj: JObject = if let Some(image) = &options.image {
                env.new_string(image)?.into()
            } else {
                JObject::null()
            };

            // Call the static showToast method on LxApp
            env.call_static_method(
                lxapp_class,
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

            // Get the LxApp class
            let lxapp_class: &JClass = super::get_lxapp_class()?.as_obj().into();

            // Call the static hideToast method on LxApp
            env.call_static_method(lxapp_class, "hideToast", "()V", &[])?;
            Ok(())
        }() {
            Ok(_) => Ok(()),
            Err(e) => Err(PlatformError::Platform(format!(
                "Failed to hide toast: {}",
                e
            ))),
        }
    }

    fn show_modal(&self, options: ModalOptions) -> Result<ModalResult, PlatformError> {
        match || -> Result<ModalResult, Box<dyn std::error::Error>> {
            let mut env = lingxia_webview::get_env()?;

            // Get the LxApp class
            let lxapp_class: &JClass = super::get_lxapp_class()?.as_obj().into();

            // Create parameters map
            let mut params = HashMap::new();
            params.insert("title", options.title.as_str());
            params.insert("content", options.content.as_str());
            params.insert("cancelText", options.cancel_text.as_str());
            params.insert("confirmText", options.confirm_text.as_str());
            params.insert("placeholderText", options.placeholder_text.as_str());

            if let Some(ref cancel_color) = options.cancel_color {
                params.insert("cancelColor", cancel_color.as_str());
            }
            if let Some(ref confirm_color) = options.confirm_color {
                params.insert("confirmColor", confirm_color.as_str());
            }

            // Create HashMap object
            let hashmap_class = env.find_class("java/util/HashMap")?;
            let hashmap = env.new_object(hashmap_class, "()V", &[])?;

            // Put string values
            for (key, value) in params {
                let key_jstring = env.new_string(key)?;
                let value_jstring = env.new_string(value)?;
                env.call_method(
                    &hashmap,
                    "put",
                    "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                    &[
                        JValue::Object(&key_jstring.into()),
                        JValue::Object(&value_jstring.into()),
                    ],
                )?;
            }

            // Put boolean values
            let show_cancel_key = env.new_string("showCancel")?;
            let show_cancel_value = env.call_static_method(
                "java/lang/Boolean",
                "valueOf",
                "(Z)Ljava/lang/Boolean;",
                &[JValue::Bool(options.show_cancel as u8)],
            )?;
            env.call_method(
                &hashmap,
                "put",
                "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                &[
                    JValue::Object(&show_cancel_key.into()),
                    show_cancel_value.borrow(),
                ],
            )?;

            let editable_key = env.new_string("editable")?;
            let editable_value = env.call_static_method(
                "java/lang/Boolean",
                "valueOf",
                "(Z)Ljava/lang/Boolean;",
                &[JValue::Bool(options.editable as u8)],
            )?;
            env.call_method(
                &hashmap,
                "put",
                "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                &[
                    JValue::Object(&editable_key.into()),
                    editable_value.borrow(),
                ],
            )?;

            // Call the static showModal method on LxApp
            let result = env.call_static_method(
                lxapp_class,
                "showModal",
                "(Ljava/util/Map;)Lcom/lingxia/lxapp/APIs/ModalResult;",
                &[JValue::Object(&hashmap)],
            )?;

            // Extract result fields
            let result_obj = result.l()?;

            let confirm_field = env.get_field(&result_obj, "confirm", "Z")?;
            let cancel_field = env.get_field(&result_obj, "cancel", "Z")?;
            let content_field = env.get_field(&result_obj, "content", "Ljava/lang/String;")?;

            let confirm = confirm_field.z()?;
            let cancel = cancel_field.z()?;
            let content_jstring = content_field.l()?;
            let content = if content_jstring.is_null() {
                String::new()
            } else {
                env.get_string(&content_jstring.into())?.into()
            };

            Ok(ModalResult {
                confirm,
                cancel,
                content,
            })
        }() {
            Ok(result) => Ok(result),
            Err(e) => Err(PlatformError::Platform(format!(
                "Failed to show modal: {}",
                e
            ))),
        }
    }
}
