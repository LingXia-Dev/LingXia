use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::file::{FileService, OpenFileRequest};

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
