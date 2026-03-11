use super::app::Platform;
use super::ffi::open_document;
use crate::error::PlatformError;
#[cfg(target_os = "macos")]
use crate::traits::file::{ChooseDirectoryRequest, ChooseFileRequest, FileDialogResult};
use crate::traits::file::{FileInteraction, OpenDocumentRequest};

fn open_document_sync(request: OpenDocumentRequest) -> Result<(), PlatformError> {
    let mime = request.mime_type.unwrap_or_default();
    let show_menu = request.show_menu.unwrap_or(true);
    if open_document(&request.file_path, &mime, show_menu) {
        Ok(())
    } else {
        Err(PlatformError::Platform(
            "Failed to open document on Apple platform".to_string(),
        ))
    }
}

impl FileInteraction for Platform {
    async fn open_document(&self, request: OpenDocumentRequest) -> Result<(), PlatformError> {
        crate::bg_runtime::blocking(move || open_document_sync(request)).await
    }

    #[cfg(target_os = "macos")]
    async fn choose_file(
        &self,
        request: ChooseFileRequest,
    ) -> Result<FileDialogResult, PlatformError> {
        crate::desktop::file_dialog::choose_file_desktop(request).await
    }

    #[cfg(target_os = "macos")]
    async fn choose_directory(
        &self,
        request: ChooseDirectoryRequest,
    ) -> Result<FileDialogResult, PlatformError> {
        crate::desktop::file_dialog::choose_directory_desktop(request).await
    }
}
