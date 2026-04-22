pub use lingxia_service::file::{
    ChooseDirectoryRequest, ChooseFileRequest, FileDialogFilter, FileDialogResult, OpenFileRequest,
    RevealInFileManagerRequest,
};

pub async fn review(app: &crate::LxApp, request: OpenFileRequest) -> crate::Result<()> {
    lingxia_service::file::review_file(&*app.runtime, request)
        .await
        .map_err(crate::Error::from)
}

pub async fn open_external(app: &crate::LxApp, request: OpenFileRequest) -> crate::Result<()> {
    lingxia_service::file::open_external(&*app.runtime, request)
        .await
        .map_err(crate::Error::from)
}

pub async fn reveal_in_file_manager(
    app: &crate::LxApp,
    request: RevealInFileManagerRequest,
) -> crate::Result<()> {
    lingxia_service::file::reveal_in_file_manager(&*app.runtime, request)
        .await
        .map_err(crate::Error::from)
}

pub async fn choose_file(
    app: &crate::LxApp,
    request: ChooseFileRequest,
) -> crate::Result<FileDialogResult> {
    lingxia_service::file::choose_file(&*app.runtime, request)
        .await
        .map_err(crate::Error::from)
}

pub async fn choose_directory(
    app: &crate::LxApp,
    request: ChooseDirectoryRequest,
) -> crate::Result<FileDialogResult> {
    lingxia_service::file::choose_directory(&*app.runtime, request)
        .await
        .map_err(crate::Error::from)
}
