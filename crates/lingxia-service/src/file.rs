pub use lingxia_platform::PlatformError;
pub use lingxia_platform::traits::file::{
    ChooseDirectoryRequest, ChooseFileRequest, FileDialogFilter, FileDialogResult, FileService,
    OpenFileRequest, RevealInFileManagerRequest,
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

pub async fn choose_file(
    runtime: &(impl FileService + ?Sized),
    request: ChooseFileRequest,
) -> Result<FileDialogResult> {
    runtime.choose_file(request).await
}

pub async fn choose_directory(
    runtime: &(impl FileService + ?Sized),
    request: ChooseDirectoryRequest,
) -> Result<FileDialogResult> {
    runtime.choose_directory(request).await
}
