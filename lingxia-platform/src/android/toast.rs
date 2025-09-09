use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::{Toast, ToastOptions};
use jni::objects::{JClass, JObject, JValue};

impl Toast for Platform {
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
}
