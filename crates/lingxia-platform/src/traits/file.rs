use std::future::Future;
use std::path::Path;

use crate::error::PlatformError;

#[derive(Debug, Clone)]
pub struct OpenFileRequest {
    pub path: String,
    pub mime_type: Option<String>,
    pub show_menu: Option<bool>,
}

impl OpenFileRequest {
    pub fn is_pdf_like(&self) -> bool {
        self.mime_type
            .as_deref()
            .map(|mime| mime.eq_ignore_ascii_case("application/pdf"))
            .unwrap_or(false)
            || Path::new(&self.path)
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("pdf"))
                .unwrap_or(false)
    }
}

#[derive(Debug, Clone)]
pub struct RevealInFileManagerRequest {
    pub path: String,
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

pub trait FileService: Send + Sync + 'static {
    fn review_file(
        &self,
        _request: OpenFileRequest,
    ) -> impl Future<Output = Result<(), PlatformError>> + Send;

    fn open_external(
        &self,
        _request: OpenFileRequest,
    ) -> impl Future<Output = Result<(), PlatformError>> + Send {
        async {
            Err(PlatformError::NotSupported(
                "open_external is not supported on this platform".into(),
            ))
        }
    }

    fn reveal_in_file_manager(
        &self,
        _request: RevealInFileManagerRequest,
    ) -> impl Future<Output = Result<(), PlatformError>> + Send {
        async {
            Err(PlatformError::NotSupported(
                "reveal_in_file_manager is not supported on this platform".into(),
            ))
        }
    }

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
