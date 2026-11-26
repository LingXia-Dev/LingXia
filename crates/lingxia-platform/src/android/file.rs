use super::app::Platform;
use super::with_env;
use crate::traits::file::{
    FileDialogResult, FileService, OpenFileRequest, PickDirectoryRequest, PickFileRequest,
};
use jni::objects::{JObject, JString, JValue};
use jni::{jni_sig, jni_str};
use lingxia_error::Error;
use serde::Deserialize;

impl FileService for Platform {
    async fn review_file(&self, request: OpenFileRequest) -> Result<(), Error> {
        crate::rt::blocking(move || review_file_sync(request)).await
    }

    async fn open_external(&self, request: OpenFileRequest) -> Result<(), Error> {
        crate::rt::blocking(move || open_external_sync(request)).await
    }

    async fn pick_file(&self, request: PickFileRequest) -> Result<FileDialogResult, Error> {
        let payload =
            crate::rt::native_call(|callback_id| pick_file_impl(&request, callback_id)).await?;
        parse_file_dialog_result(&payload)
    }

    async fn pick_directory(
        &self,
        request: PickDirectoryRequest,
    ) -> Result<FileDialogResult, Error> {
        let payload =
            crate::rt::native_call(|callback_id| pick_directory_impl(&request, callback_id))
                .await
                .map_err(|err| {
                    if err.is(lingxia_error::code::NOT_SUPPORTED) {
                        Error::not_supported(
                            "pickDirectory on Android needs a defaultPath in lxapp storage: \
                             a system folder pick is a SAF tree URI no file API can read"
                                .to_string(),
                        )
                    } else {
                        err
                    }
                })?;
        parse_file_dialog_result(&payload)
    }
}

#[derive(Deserialize)]
struct AndroidFileDialogResult {
    canceled: bool,
    paths: Vec<String>,
    #[serde(default)]
    names: Option<Vec<Option<String>>>,
    #[serde(default)]
    sizes: Option<Vec<Option<u64>>>,
}

fn review_file_sync(request: OpenFileRequest) -> Result<(), Error> {
    let show_menu = request.show_menu.unwrap_or(true);
    match review_document_impl(&request.path, request.mime_type.as_deref(), show_menu) {
        Ok(true) => Ok(()),
        Ok(false) => Err(Error::internal(
            "Failed to review file on Android platform".to_string(),
        )),
        Err(err) => Err(Error::internal(format!(
            "Failed to review file on Android platform: {}",
            err
        ))),
    }
}

fn open_external_sync(request: OpenFileRequest) -> Result<(), Error> {
    let show_menu = request.show_menu.unwrap_or(true);
    match open_document_external_impl(&request.path, request.mime_type.as_deref(), show_menu) {
        Ok(true) => Ok(()),
        Ok(false) => Err(Error::internal(
            "Failed to open file externally on Android platform".to_string(),
        )),
        Err(err) => Err(Error::internal(format!(
            "Failed to open file externally on Android platform: {}",
            err
        ))),
    }
}

fn review_document_impl(
    file_path: &str,
    mime_type: Option<&str>,
    show_menu: bool,
) -> Result<bool, Box<dyn std::error::Error>> {
    with_env(|env| {
        let class = super::get_cached_class(super::CachedClass::LxAppFile)?;

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
) -> Result<bool, Box<dyn std::error::Error>> {
    with_env(|env| {
        let class = super::get_cached_class(super::CachedClass::LxAppFile)?;

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

fn pick_file_impl(request: &PickFileRequest, callback_id: u64) -> Result<(), Error> {
    with_env(|env| -> Result<(), Error> {
        let class = super::get_cached_class(super::CachedClass::LxAppFile)
            .map_err(|e| Error::internal(e.to_string()))?;
        let title = env.new_string(request.title.clone().unwrap_or_default())?;
        let default_path = env.new_string(request.default_path.clone().unwrap_or_default())?;
        let filters_json = serde_json::to_string(
            &request
                .filters
                .iter()
                .flat_map(|filter| filter.extensions.iter().cloned())
                .collect::<Vec<String>>(),
        )
        .map_err(|e| Error::internal(format!("serialize filters failed: {e}")))?;
        let filters_json = env.new_string(filters_json)?;

        let accepted = env
            .call_static_method(
                class,
                jni_str!("chooseFile"),
                jni_sig!("(ZLjava/lang/String;Ljava/lang/String;Ljava/lang/String;J)Z"),
                &[
                    JValue::Bool(request.multiple),
                    JValue::Object(&JObject::from(title)),
                    JValue::Object(&JObject::from(default_path)),
                    JValue::Object(&JObject::from(filters_json)),
                    JValue::Long(callback_id as i64),
                ],
            )?
            .z()?;
        if !accepted {
            return Err(Error::internal("failed to launch Android file picker"));
        }
        Ok(())
    })
}

fn pick_directory_impl(request: &PickDirectoryRequest, callback_id: u64) -> Result<(), Error> {
    with_env(|env| -> Result<(), Error> {
        let class = super::get_cached_class(super::CachedClass::LxAppFile)
            .map_err(|e| Error::internal(e.to_string()))?;
        let title = env.new_string(request.title.clone().unwrap_or_default())?;
        let default_path = env.new_string(request.default_path.clone().unwrap_or_default())?;

        let accepted = env
            .call_static_method(
                class,
                jni_str!("chooseDirectory"),
                jni_sig!("(Ljava/lang/String;Ljava/lang/String;J)Z"),
                &[
                    JValue::Object(&JObject::from(title)),
                    JValue::Object(&JObject::from(default_path)),
                    JValue::Long(callback_id as i64),
                ],
            )?
            .z()?;
        if !accepted {
            return Err(Error::internal("failed to launch Android directory picker"));
        }
        Ok(())
    })
}

fn parse_file_dialog_result(payload: &str) -> Result<FileDialogResult, Error> {
    let parsed: AndroidFileDialogResult = serde_json::from_str(payload)
        .map_err(|e| Error::internal(format!("parse file dialog result failed: {e}")))?;
    FileDialogResult::from_wire(parsed.canceled, parsed.paths, parsed.names)?
        .with_sizes(parsed.sizes)
}
