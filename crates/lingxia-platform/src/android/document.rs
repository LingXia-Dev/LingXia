use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::file::{FileService, OpenFileRequest};
use jni::objects::{JObject, JString, JValue};
use jni::{jni_sig, jni_str};
use lingxia_webview::platform::android::with_env;
use std::error::Error;

impl FileService for Platform {
    async fn review_file(&self, request: OpenFileRequest) -> Result<(), PlatformError> {
        if !request.is_pdf_like() {
            return Err(PlatformError::NotSupported(
                "review_file is only supported for PDF on Android".to_string(),
            ));
        }
        crate::rt::blocking(move || review_file_sync(request)).await
    }

    async fn open_external(&self, request: OpenFileRequest) -> Result<(), PlatformError> {
        crate::rt::blocking(move || open_external_sync(request)).await
    }
}

fn review_file_sync(request: OpenFileRequest) -> Result<(), PlatformError> {
    let show_menu = request.show_menu.unwrap_or(true);
    match review_document_impl(&request.path, request.mime_type.as_deref(), show_menu) {
        Ok(true) => Ok(()),
        Ok(false) => Err(PlatformError::Platform(
            "Failed to review file on Android platform".to_string(),
        )),
        Err(err) => Err(PlatformError::Platform(format!(
            "Failed to review file on Android platform: {}",
            err
        ))),
    }
}

fn open_external_sync(request: OpenFileRequest) -> Result<(), PlatformError> {
    let show_menu = request.show_menu.unwrap_or(true);
    match open_document_external_impl(&request.path, request.mime_type.as_deref(), show_menu) {
        Ok(true) => Ok(()),
        Ok(false) => Err(PlatformError::Platform(
            "Failed to open file externally on Android platform".to_string(),
        )),
        Err(err) => Err(PlatformError::Platform(format!(
            "Failed to open file externally on Android platform: {}",
            err
        ))),
    }
}

fn review_document_impl(
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
            jni_str!("reviewDocument"),
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

fn open_document_external_impl(
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
            jni_str!("openDocumentExternal"),
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
