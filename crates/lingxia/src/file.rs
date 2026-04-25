//! File dialog and file-manager helpers scoped to an [`crate::LxApp`].

pub use lingxia_service::file::{
    ChooseDirectoryRequest, ChooseFileRequest, FileDialogFilter, FileDialogResult, OpenFileRequest,
    RevealInFileManagerRequest,
};

/// Opens a file for in-app review using the platform's preview UI.
pub async fn review(app: &crate::LxApp, request: OpenFileRequest) -> crate::Result<()> {
    lingxia_service::file::review_file(&*app.runtime, request)
        .await
        .map_err(crate::Error::from)
}

/// Opens a file or URL in an external app chosen by the host platform.
pub async fn open_external(app: &crate::LxApp, request: OpenFileRequest) -> crate::Result<()> {
    lingxia_service::file::open_external(&*app.runtime, request)
        .await
        .map_err(crate::Error::from)
}

/// Reveals a file or directory in the host platform's file manager.
pub async fn reveal_in_file_manager(
    app: &crate::LxApp,
    request: RevealInFileManagerRequest,
) -> crate::Result<()> {
    lingxia_service::file::reveal_in_file_manager(&*app.runtime, request)
        .await
        .map_err(crate::Error::from)
}

/// Presents a file picker and returns the selected entries.
pub async fn choose_file(
    app: &crate::LxApp,
    request: ChooseFileRequest,
) -> crate::Result<FileDialogResult> {
    lingxia_service::file::choose_file(&*app.runtime, request)
        .await
        .map_err(crate::Error::from)
}

/// Presents a directory picker and returns the selected entry.
pub async fn choose_directory(
    app: &crate::LxApp,
    request: ChooseDirectoryRequest,
) -> crate::Result<FileDialogResult> {
    lingxia_service::file::choose_directory(&*app.runtime, request)
        .await
        .map_err(crate::Error::from)
}
