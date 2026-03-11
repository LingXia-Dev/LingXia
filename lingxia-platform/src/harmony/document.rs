use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::document::{DocumentInteraction, OpenDocumentRequest};

impl DocumentInteraction for Platform {
    fn open_document(&self, request: OpenDocumentRequest) -> Result<(), PlatformError> {
        let mime = request.mime_type.unwrap_or_default();
        let show_menu_flag = request.show_menu.unwrap_or(true);
        let show_menu = if show_menu_flag { "1" } else { "0" };
        lingxia_webview::platform::harmony::tsfn::call_arkts(
            "openDocument",
            &[request.file_path.as_str(), mime.as_str(), show_menu],
        )
        .map(|_| ())
        .map_err(|e| PlatformError::Platform(format!("Failed to open document: {}", e)))
    }
}
