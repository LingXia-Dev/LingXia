use std::future::Future;

use crate::error::PlatformError;

#[derive(Debug, Clone)]
pub struct OpenDocumentRequest {
    pub file_path: String,
    pub mime_type: Option<String>,
    pub show_menu: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct FileDialogFilter {
    pub name: Option<String>,
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ChooseFileRequest {
    pub multiple: bool,
    pub filters: Vec<FileDialogFilter>,
    pub title: Option<String>,
    pub default_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ChooseDirectoryRequest {
    pub title: Option<String>,
    pub default_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FileDialogResult {
    pub canceled: bool,
    pub paths: Vec<String>,
}

pub trait FileInteraction: Send + Sync + 'static {
    fn open_document(
        &self,
        request: OpenDocumentRequest,
    ) -> impl Future<Output = Result<(), PlatformError>> + Send;

    fn choose_file(
        &self,
        _request: ChooseFileRequest,
    ) -> impl Future<Output = Result<FileDialogResult, PlatformError>> + Send {
        async {
            Err(PlatformError::NotSupported(
                "choose_file is not supported on this platform".into(),
            ))
        }
    }

    fn choose_directory(
        &self,
        _request: ChooseDirectoryRequest,
    ) -> impl Future<Output = Result<FileDialogResult, PlatformError>> + Send {
        async {
            Err(PlatformError::NotSupported(
                "choose_directory is not supported on this platform".into(),
            ))
        }
    }
}
