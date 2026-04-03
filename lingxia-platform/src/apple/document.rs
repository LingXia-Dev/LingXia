use super::app::Platform;
use super::ffi::open_document;
#[cfg(target_os = "macos")]
use super::ffi::reveal_in_file_manager;
use crate::error::PlatformError;
#[cfg(target_os = "macos")]
use crate::traits::file::{ChooseDirectoryRequest, ChooseFileRequest, FileDialogResult};
use crate::traits::file::{FileInteraction, OpenDocumentRequest, RevealInFileManagerRequest};

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

fn reveal_in_file_manager_sync(request: RevealInFileManagerRequest) -> Result<(), PlatformError> {
    #[cfg(target_os = "macos")]
    {
        if reveal_in_file_manager(&request.path) {
            Ok(())
        } else {
            Err(PlatformError::Platform(
                "Failed to reveal path in file manager on Apple platform".to_string(),
            ))
        }
    }
    #[cfg(target_os = "ios")]
    {
        let _ = request;
        Err(PlatformError::NotSupported(
            "reveal_in_file_manager is not supported on iOS".to_string(),
        ))
    }
}

impl FileInteraction for Platform {
    async fn open_document(&self, request: OpenDocumentRequest) -> Result<(), PlatformError> {
        crate::rt::blocking(move || open_document_sync(request)).await
    }

    async fn reveal_in_file_manager(
        &self,
        request: RevealInFileManagerRequest,
    ) -> Result<(), PlatformError> {
        crate::rt::blocking(move || reveal_in_file_manager_sync(request)).await
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
