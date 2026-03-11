use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::document::{DocumentInteraction, OpenDocumentRequest};
use jni::objects::{JObject, JString, JValue};
use jni::{jni_sig, jni_str};
use lingxia_webview::platform::android::with_env;
use std::error::Error;

impl DocumentInteraction for Platform {
    fn open_document(&self, request: OpenDocumentRequest) -> Result<(), PlatformError> {
        let show_menu = request.show_menu.unwrap_or(true); // Default to true for backward compatibility
        match open_document_impl(&request.file_path, request.mime_type.as_deref(), show_menu) {
            Ok(true) => Ok(()),
            Ok(false) => Err(PlatformError::Platform(
                "Failed to open document on Android platform".to_string(),
            )),
            Err(err) => Err(PlatformError::Platform(format!(
                "Failed to open document on Android platform: {}",
                err
            ))),
        }
    }
}

fn open_document_impl(
    file_path: &str,
    mime_type: Option<&str>,
    show_menu: bool,
) -> Result<bool, Box<dyn Error>> {
    with_env(|env| {
        let class = super::get_cached_class(super::CachedClass::LxAppDocument)?;

        let path_java: JString = env.new_string(file_path)?;
        let path_obj: JObject = path_java.into();

        let mime_obj = match mime_type.filter(|m| !m.is_empty()) {
            Some(mime) => {
                let mime_java: JString = env.new_string(mime)?;
                mime_java.into()
            }
            None => JObject::null(),
        };

        let result = env.call_static_method(
            class,
            jni_str!("openDocument"),
            jni_sig!("(Ljava/lang/String;Ljava/lang/String;Z)Z"),
            &[
                JValue::Object(&path_obj),
                JValue::Object(&mime_obj),
                JValue::Bool(show_menu),
            ],
        )?;

        if env.exception_check() {
            env.exception_clear();
            return Ok(false);
        }

        Ok(result.z()?)
    })
}
