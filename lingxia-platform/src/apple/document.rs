use super::app::Platform;
use super::ffi::open_document;
use crate::error::PlatformError;
use crate::traits::{DocumentInteraction, OpenDocumentRequest};

impl DocumentInteraction for Platform {
    fn open_document(&self, request: OpenDocumentRequest) -> Result<(), PlatformError> {
        let mime = request.mime_type.unwrap_or_default();
        let show_menu = request.show_menu.unwrap_or(true); // Default to true for backward compatibility
        if open_document(&request.file_path, &mime, show_menu) {
            Ok(())
        } else {
            Err(PlatformError::Platform(
                "Failed to open document on Apple platform".to_string(),
            ))
        }
    }
}
