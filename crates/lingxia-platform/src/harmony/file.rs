use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::file::{
    ChooseDirectoryRequest, ChooseFileRequest, FileDialogResult, FileService, OpenFileRequest,
};
use serde::Deserialize;

impl FileService for Platform {
    async fn review_file(&self, request: OpenFileRequest) -> Result<(), PlatformError> {
        if !request.is_pdf_like() {
            return Err(PlatformError::NotSupported(
                "review_file is only supported for PDF on HarmonyOS".to_string(),
            ));
        }
        crate::rt::blocking(move || review_file_sync(request)).await
    }

    async fn open_external(&self, request: OpenFileRequest) -> Result<(), PlatformError> {
        crate::rt::blocking(move || open_external_sync(request)).await
    }

    async fn choose_file(
        &self,
        request: ChooseFileRequest,
    ) -> Result<FileDialogResult, PlatformError> {
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

    async fn choose_directory(
        &self,
        request: ChooseDirectoryRequest,
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
        .await?;
        parse_file_dialog_result(&payload)
    }
}

#[derive(Deserialize)]
struct HarmonyFileDialogResult {
    canceled: bool,
    paths: Vec<String>,
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
    Ok(FileDialogResult {
        canceled: parsed.canceled,
        paths: parsed.paths,
    })
}
