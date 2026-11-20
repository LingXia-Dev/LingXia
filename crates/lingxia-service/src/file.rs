pub use lingxia_platform::PlatformError;
pub use lingxia_platform::traits::file::{
    FileDialogFilter, FileDialogResult, FileService, OpenFileRequest, PickDirectoryRequest,
    PickFileRequest, PickedPath, RevealInFileManagerRequest,
};

pub type Result<T> = std::result::Result<T, PlatformError>;

pub async fn review_file(
    runtime: &(impl FileService + ?Sized),
    request: OpenFileRequest,
) -> Result<()> {
    runtime.review_file(request).await
}

pub async fn open_external(
    runtime: &(impl FileService + ?Sized),
    request: OpenFileRequest,
) -> Result<()> {
    runtime.open_external(request).await
}

pub async fn reveal_in_file_manager(
    runtime: &(impl FileService + ?Sized),
    request: RevealInFileManagerRequest,
) -> Result<()> {
    runtime.reveal_in_file_manager(request).await
}

pub async fn pick_file(
    runtime: &(impl FileService + ?Sized),
    request: PickFileRequest,
) -> Result<FileDialogResult> {
    runtime.pick_file(request).await
}

pub async fn pick_directory(
    runtime: &(impl FileService + ?Sized),
    request: PickDirectoryRequest,
) -> Result<FileDialogResult> {
    runtime.pick_directory(request).await
}
