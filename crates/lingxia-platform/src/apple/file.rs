use super::app::Platform;
#[cfg(target_os = "macos")]
use super::ffi::reveal_in_file_manager;
#[cfg(target_os = "ios")]
use super::ffi::{choose_directory, choose_file};
use super::ffi::{open_document_external, review_document};
use crate::error::PlatformError;
use crate::traits::file::{
    ChooseDirectoryRequest, ChooseFileRequest, FileDialogResult, FileService, OpenFileRequest,
    RevealInFileManagerRequest,
};
#[cfg(target_os = "ios")]
use serde::Deserialize;

fn review_file_sync(request: OpenFileRequest) -> Result<(), PlatformError> {
    let mime = request.mime_type.unwrap_or_default();
    let show_menu = request.show_menu.unwrap_or(true);
    if review_document(&request.path, &mime, show_menu) {
        Ok(())
    } else {
        Err(PlatformError::Platform(
            "Failed to review file on Apple platform".to_string(),
        ))
    }
}

fn open_external_sync(request: OpenFileRequest) -> Result<(), PlatformError> {
    let mime = request.mime_type.unwrap_or_default();
    let show_menu = request.show_menu.unwrap_or(true);
    if open_document_external(&request.path, &mime, show_menu) {
        Ok(())
    } else {
        Err(PlatformError::Platform(
            "Failed to open file externally on Apple platform".to_string(),
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

#[cfg(target_os = "ios")]
#[derive(Deserialize)]
struct AppleFileDialogResult {
    canceled: bool,
    paths: Vec<String>,
}

#[cfg(target_os = "ios")]
fn parse_file_dialog_result(payload: &str) -> Result<FileDialogResult, PlatformError> {
    let parsed: AppleFileDialogResult = serde_json::from_str(payload)
        .map_err(|e| PlatformError::Platform(format!("parse file dialog result failed: {e}")))?;
    Ok(FileDialogResult {
        canceled: parsed.canceled,
        paths: parsed.paths,
    })
}

#[cfg(target_os = "ios")]
async fn choose_file_ios(request: ChooseFileRequest) -> Result<FileDialogResult, PlatformError> {
    let payload = crate::rt::native_call(|callback_id| {
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
        if choose_file(
            &title,
            &default_path,
            request.multiple,
            &filters_json,
            callback_id,
        ) {
            Ok(())
        } else {
            Err(PlatformError::Platform(
                "Failed to start choose_file on Apple platform".to_string(),
            ))
        }
    })
    .await?;
    parse_file_dialog_result(&payload)
}

#[cfg(target_os = "ios")]
async fn choose_directory_ios(
    request: ChooseDirectoryRequest,
) -> Result<FileDialogResult, PlatformError> {
    let payload = crate::rt::native_call(|callback_id| {
        let title = request.title.clone().unwrap_or_default();
        let default_path = request.default_path.clone().unwrap_or_default();
        if choose_directory(&title, &default_path, callback_id) {
            Ok(())
        } else {
            Err(PlatformError::Platform(
                "Failed to start choose_directory on Apple platform".to_string(),
            ))
        }
    })
    .await?;
    parse_file_dialog_result(&payload)
}

impl FileService for Platform {
    async fn review_file(&self, request: OpenFileRequest) -> Result<(), PlatformError> {
        crate::rt::blocking(move || review_file_sync(request)).await
    }

    async fn open_external(&self, request: OpenFileRequest) -> Result<(), PlatformError> {
        crate::rt::blocking(move || open_external_sync(request)).await
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

    #[cfg(target_os = "ios")]
    async fn choose_file(
        &self,
        request: ChooseFileRequest,
    ) -> Result<FileDialogResult, PlatformError> {
        choose_file_ios(request).await
    }

    #[cfg(target_os = "macos")]
    async fn choose_directory(
        &self,
        request: ChooseDirectoryRequest,
    ) -> Result<FileDialogResult, PlatformError> {
        crate::desktop::file_dialog::choose_directory_desktop(request).await
    }

    #[cfg(target_os = "ios")]
    async fn choose_directory(
        &self,
        request: ChooseDirectoryRequest,
    ) -> Result<FileDialogResult, PlatformError> {
        choose_directory_ios(request).await
    }
}
