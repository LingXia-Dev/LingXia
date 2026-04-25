//! File dialog, file-manager, download, and upload helpers scoped to an
//! [`crate::LxApp`].

use serde::Serialize;
use std::path::{Path, PathBuf};

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

/// Request builder for [`download`].
#[derive(Debug, Clone)]
pub struct DownloadRequest {
    url: String,
    headers: Vec<(String, String)>,
}

impl DownloadRequest {
    /// Creates a download request for `url`.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            headers: Vec::new(),
        }
    }

    /// Adds an HTTP request header.
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Replaces the full HTTP request header list.
    pub fn headers<I, K, V>(mut self, headers: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        self.headers = headers
            .into_iter()
            .map(|(name, value)| (name.into(), value.into()))
            .collect();
        self
    }
}

impl From<&str> for DownloadRequest {
    fn from(url: &str) -> Self {
        Self::new(url)
    }
}

impl From<String> for DownloadRequest {
    fn from(url: String) -> Self {
        Self::new(url)
    }
}

/// A downloaded file returned by [`download`].
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadedFile {
    path: PathBuf,
    file_name: String,
    mime_type: Option<String>,
    size: u64,
}

impl DownloadedFile {
    /// Returns the local file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Converts this result into the local file path.
    pub fn into_path(self) -> PathBuf {
        self.path
    }

    /// Returns the suggested or resolved file name.
    pub fn file_name(&self) -> &str {
        &self.file_name
    }

    /// Returns the MIME type when the transfer layer could determine one.
    pub fn mime_type(&self) -> Option<&str> {
        self.mime_type.as_deref()
    }

    /// Returns the number of bytes written to disk.
    pub fn size(&self) -> u64 {
        self.size
    }
}

fn downloaded_file_from_result(
    result: lingxia_transfer::user_cache::UserCacheDownloadResult,
) -> DownloadedFile {
    DownloadedFile {
        path: result.temp_path,
        file_name: result.file_name,
        mime_type: result.mime_type,
        size: result.size,
    }
}

/// Downloads a file to the app's managed user cache.
pub async fn download(
    app: &crate::LxApp,
    request: impl Into<DownloadRequest>,
) -> crate::Result<DownloadedFile> {
    let request = request.into();
    let user_request = lingxia_transfer::user_cache::UserCacheDownloadRequest {
        url: request.url,
        headers: request.headers,
    };
    lingxia_transfer::user_cache::download_to_user_cache(
        None,
        &app.user_cache_dir,
        user_request,
        None,
        |_| {},
    )
    .await
    .map(downloaded_file_from_result)
    .map_err(map_download_error)
}

/// Request builder for [`upload`].
#[derive(Debug, Clone)]
pub struct UploadRequest {
    url: String,
    file_path: PathBuf,
    field_name: String,
    file_name: Option<String>,
    mime_type: Option<String>,
    headers: Vec<(String, String)>,
    form_fields: Vec<(String, String)>,
}

impl UploadRequest {
    /// Creates a multipart file upload request.
    pub fn new(url: impl Into<String>, file_path: impl Into<PathBuf>) -> Self {
        Self {
            url: url.into(),
            file_path: file_path.into(),
            field_name: "file".to_string(),
            file_name: None,
            mime_type: None,
            headers: Vec::new(),
            form_fields: Vec::new(),
        }
    }

    /// Sets the multipart file field name. Defaults to `file`.
    pub fn field_name(mut self, field_name: impl Into<String>) -> Self {
        self.field_name = field_name.into();
        self
    }

    /// Sets the multipart file name.
    pub fn file_name(mut self, file_name: impl Into<String>) -> Self {
        self.file_name = Some(file_name.into());
        self
    }

    /// Sets the uploaded file MIME type.
    pub fn mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = Some(mime_type.into());
        self
    }

    /// Adds an HTTP request header.
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Replaces the full HTTP request header list.
    pub fn headers<I, K, V>(mut self, headers: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        self.headers = headers
            .into_iter()
            .map(|(name, value)| (name.into(), value.into()))
            .collect();
        self
    }

    /// Adds a non-file multipart form field.
    pub fn form_field(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.form_fields.push((name.into(), value.into()));
        self
    }

    /// Replaces the full multipart non-file form field list.
    pub fn form_fields<I, K, V>(mut self, form_fields: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        self.form_fields = form_fields
            .into_iter()
            .map(|(name, value)| (name.into(), value.into()))
            .collect();
        self
    }

    fn into_transfer(self) -> lingxia_transfer::UploadRequest {
        lingxia_transfer::UploadRequest {
            url: self.url,
            method: lingxia_transfer::UploadMethod::Post,
            file_path: self.file_path,
            field_name: self.field_name,
            file_name: self.file_name,
            mime_type: self.mime_type,
            headers: self.headers,
            form_fields: self.form_fields,
            user_agent: None,
        }
    }
}

impl<U, P> From<(U, P)> for UploadRequest
where
    U: Into<String>,
    P: Into<PathBuf>,
{
    fn from((url, file_path): (U, P)) -> Self {
        Self::new(url, file_path)
    }
}

/// Response returned by [`upload`].
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UploadResponse {
    status_code: u16,
    data: String,
}

impl UploadResponse {
    /// Returns the HTTP status code returned by the upload endpoint.
    pub fn status_code(&self) -> u16 {
        self.status_code
    }

    /// Returns the response body text.
    pub fn data(&self) -> &str {
        &self.data
    }

    /// Converts this response into the response body text.
    pub fn into_data(self) -> String {
        self.data
    }
}

fn upload_response_from_result(result: lingxia_transfer::UploadResult) -> UploadResponse {
    UploadResponse {
        status_code: result.status_code,
        data: String::from_utf8_lossy(&result.body).into_owned(),
    }
}

/// Uploads a local file as multipart form data.
pub async fn upload(
    _app: &crate::LxApp,
    request: impl Into<UploadRequest>,
) -> crate::Result<UploadResponse> {
    let request = request.into().into_transfer();
    let (_abort_tx, abort_rx) = tokio::sync::oneshot::channel();
    lingxia_transfer::upload_file_with_behavior(
        request,
        lingxia_transfer::UploadBehavior::default(),
        abort_rx,
        |_| {},
    )
    .await
    .map(upload_response_from_result)
    .map_err(map_upload_error)
}

fn map_download_error(failure: lingxia_transfer::user_cache::DownloadFailure) -> crate::Error {
    match failure.kind {
        lingxia_transfer::user_cache::DownloadFailureKind::InvalidRequest
        | lingxia_transfer::user_cache::DownloadFailureKind::Conflict => {
            crate::Error::invalid_request(failure.error)
        }
        lingxia_transfer::user_cache::DownloadFailureKind::Timeout
        | lingxia_transfer::user_cache::DownloadFailureKind::NetworkUnavailable
        | lingxia_transfer::user_cache::DownloadFailureKind::Server
        | lingxia_transfer::user_cache::DownloadFailureKind::Connection
        | lingxia_transfer::user_cache::DownloadFailureKind::Canceled => {
            crate::Error::platform(failure.error)
        }
        lingxia_transfer::user_cache::DownloadFailureKind::AccessDenied => {
            crate::Error::permission_denied(failure.error)
        }
        lingxia_transfer::user_cache::DownloadFailureKind::Internal => {
            crate::Error::internal(failure.error)
        }
    }
}

fn map_upload_error(failure: lingxia_transfer::UploadFailure) -> crate::Error {
    match failure.kind {
        lingxia_transfer::UploadFailureKind::InvalidRequest
        | lingxia_transfer::UploadFailureKind::InvalidFile => {
            crate::Error::invalid_request(failure.error)
        }
        lingxia_transfer::UploadFailureKind::Timeout
        | lingxia_transfer::UploadFailureKind::NetworkUnavailable
        | lingxia_transfer::UploadFailureKind::Server
        | lingxia_transfer::UploadFailureKind::Connection
        | lingxia_transfer::UploadFailureKind::Canceled => crate::Error::platform(failure.error),
        lingxia_transfer::UploadFailureKind::AccessDenied => {
            crate::Error::permission_denied(failure.error)
        }
        lingxia_transfer::UploadFailureKind::Internal => crate::Error::internal(failure.error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_request_accepts_url_directly() {
        let request = DownloadRequest::from("https://example.com/a.bin").header("x-token", "dev");

        assert_eq!(request.url, "https://example.com/a.bin");
        assert_eq!(request.headers, vec![("x-token".into(), "dev".into())]);
    }

    #[test]
    fn upload_request_defaults_to_multipart_post_file_field() {
        let request = UploadRequest::new("https://example.com/upload", "/tmp/a.bin")
            .field_name("package")
            .mime_type("application/octet-stream")
            .form_field("kind", "app");

        assert_eq!(request.url, "https://example.com/upload");
        assert_eq!(request.file_path, PathBuf::from("/tmp/a.bin"));
        assert_eq!(request.field_name, "package");
        assert_eq!(request.form_fields, vec![("kind".into(), "app".into())]);
    }

    #[test]
    fn transfer_results_serialize_for_native_handlers() {
        let file = DownloadedFile {
            path: PathBuf::from("/tmp/a.bin"),
            file_name: "a.bin".to_string(),
            mime_type: Some("application/octet-stream".to_string()),
            size: 7,
        };
        let json = serde_json::to_value(file).expect("serialize downloaded file");
        assert_eq!(json["fileName"], "a.bin");
        assert_eq!(json["size"], 7);

        let upload = upload_response_from_result(lingxia_transfer::UploadResult {
            status_code: 201,
            body: b"created".to_vec(),
        });
        let json = serde_json::to_value(upload).expect("serialize upload response");
        assert_eq!(json["statusCode"], 201);
        assert_eq!(json["data"], "created");
    }
}
