use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::file::{
    FileDialogResult, FileService, OpenFileRequest, PickDirectoryRequest, PickFileRequest,
};
use serde::Deserialize;
use std::path::Path;

impl FileService for Platform {
    async fn review_file(&self, request: OpenFileRequest) -> Result<(), PlatformError> {
        if !supports_native_review(&request) {
            return Err(PlatformError::NotSupported(
                "review_file is only supported for PDF and images on HarmonyOS".to_string(),
            ));
        }
        crate::rt::blocking(move || review_file_sync(request)).await
    }

    async fn open_external(&self, request: OpenFileRequest) -> Result<(), PlatformError> {
        crate::rt::blocking(move || open_external_sync(request)).await
    }

    async fn pick_file(&self, request: PickFileRequest) -> Result<FileDialogResult, PlatformError> {
        let payload = crate::rt::native_call(|callback_id| {
            let callback_id = callback_id.to_string();
            let multiple = if request.multiple { "1" } else { "0" };
            let title = request.title.clone().unwrap_or_default();
            let default_path = request.default_path.clone().unwrap_or_default();
            let filters_json = serde_json::to_string(
                &request
                    .filters
                    .iter()
                    .flat_map(|filter| filter.extensions.iter().cloned())
                    .collect::<Vec<String>>(),
            )
            .map_err(|e| PlatformError::Platform(format!("serialize filters failed: {e}")))?;
            lingxia_webview::platform::harmony::tsfn::call_arkts(
                "chooseFile",
                &[&callback_id, multiple, &title, &default_path, &filters_json],
            )
            .map_err(|e| PlatformError::Platform(format!("Failed to choose file: {}", e)))
        })
        .await?;
        parse_file_dialog_result(&payload)
    }

    async fn pick_directory(
        &self,
        request: PickDirectoryRequest,
    ) -> Result<FileDialogResult, PlatformError> {
        let payload = crate::rt::native_call(|callback_id| {
            let callback_id = callback_id.to_string();
            let title = request.title.clone().unwrap_or_default();
            let default_path = request.default_path.clone().unwrap_or_default();
            lingxia_webview::platform::harmony::tsfn::call_arkts(
                "chooseDirectory",
                &[&callback_id, &title, &default_path],
            )
            .map_err(|e| PlatformError::Platform(format!("Failed to choose directory: {}", e)))
        })
        .await
        .map_err(|err| match err {
            PlatformError::BusinessError(6000) => PlatformError::NotSupported(
                "pickDirectory on this HarmonyOS device needs a defaultPath in lxapp storage: \
                 the system folder picker is 2-in-1 only"
                    .to_string(),
            ),
            other => other,
        })?;
        parse_file_dialog_result(&payload)
    }
}

#[derive(Deserialize)]
struct HarmonyFileDialogResult {
    canceled: bool,
    paths: Vec<String>,
    #[serde(default)]
    names: Option<Vec<Option<String>>>,
}

fn supports_native_review(request: &OpenFileRequest) -> bool {
    if request.is_pdf_like() {
        return true;
    }

    if let Some(mime) = request.mime_type.as_deref()
        && mime.to_ascii_lowercase().starts_with("image/")
    {
        return true;
    }

    Path::new(&request.path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "webp" | "gif" | "bmp" | "heic" | "heif"
            )
        })
        .unwrap_or(false)
}

fn review_file_sync(request: OpenFileRequest) -> Result<(), PlatformError> {
    let mime = request.mime_type.unwrap_or_default();
    let show_menu_flag = request.show_menu.unwrap_or(true);
    let show_menu = if show_menu_flag { "1" } else { "0" };
    lingxia_webview::platform::harmony::tsfn::call_arkts(
        "reviewDocument",
        &[request.path.as_str(), mime.as_str(), show_menu],
    )
    .map(|_| ())
    .map_err(|e| PlatformError::Platform(format!("Failed to review file on HarmonyOS: {}", e)))
}

fn open_external_sync(request: OpenFileRequest) -> Result<(), PlatformError> {
    let mime = request.mime_type.unwrap_or_default();
    let show_menu_flag = request.show_menu.unwrap_or(true);
    let show_menu = if show_menu_flag { "1" } else { "0" };
    lingxia_webview::platform::harmony::tsfn::call_arkts(
        "openDocumentExternal",
        &[request.path.as_str(), mime.as_str(), show_menu],
    )
    .map(|_| ())
    .map_err(|e| PlatformError::Platform(format!("Failed to open file externally: {}", e)))
}

fn parse_file_dialog_result(payload: &str) -> Result<FileDialogResult, PlatformError> {
    let parsed: HarmonyFileDialogResult = serde_json::from_str(payload)
        .map_err(|e| PlatformError::Platform(format!("parse file dialog result failed: {e}")))?;
    // The system picker reports URIs whose tail is the percent-encoded file
    // name; the in-app chooser overlay reports plain paths, named by their tail
    // in the logic layer.
    let names = parsed.names.or_else(|| {
        Some(
            parsed
                .paths
                .iter()
                .map(|path| uri_tail_name(path))
                .collect(),
        )
    });
    FileDialogResult::from_wire(parsed.canceled, parsed.paths, names)
}

fn uri_tail_name(uri: &str) -> Option<String> {
    if !uri.contains("://") {
        return None;
    }
    let tail = uri.trim_end_matches('/').rsplit('/').next()?;
    let decoded = percent_encoding::percent_decode_str(tail)
        .decode_utf8()
        .ok()?;
    (!decoded.is_empty()).then(|| decoded.into_owned())
}
