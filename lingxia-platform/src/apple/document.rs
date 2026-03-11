use super::app::Platform;
use super::ffi::open_document;
use crate::error::PlatformError;
#[cfg(target_os = "macos")]
use crate::traits::file::{ChooseDirectoryRequest, ChooseFileRequest};
use crate::traits::file::{FileInteraction, OpenDocumentRequest};

impl FileInteraction for Platform {
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

    #[cfg(target_os = "macos")]
    fn choose_file(&self, request: ChooseFileRequest) -> Result<(), PlatformError> {
        crate::desktop::file_dialog::choose_file_desktop(request)
    }

    #[cfg(target_os = "macos")]
    fn choose_directory(&self, request: ChooseDirectoryRequest) -> Result<(), PlatformError> {
        crate::desktop::file_dialog::choose_directory_desktop(request)
    }
}
