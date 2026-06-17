use std::future::Future;
use std::path::PathBuf;
use std::process::Command;

use lingxia_webview::{FileChooserFile, FileChooserResponse};

use super::Platform;
use crate::error::PlatformError;
use crate::traits::file::{
    ChooseDirectoryRequest, ChooseFileRequest, FileDialogFilter, FileDialogResult, FileService,
    OpenFileRequest, RevealInFileManagerRequest,
};

impl Platform {
    pub fn file_chooser_handler(
        &self,
    ) -> impl Fn(
        lingxia_webview::FileChooserRequest,
    ) -> std::pin::Pin<Box<dyn Future<Output = FileChooserResponse> + Send>>
    + Clone
    + Send
    + Sync
    + 'static {
        let platform = self.clone();
        move |request| {
            let platform = platform.clone();
            Box::pin(async move { platform.handle_file_chooser(request).await })
        }
    }

    async fn handle_file_chooser(
        &self,
        request: lingxia_webview::FileChooserRequest,
    ) -> FileChooserResponse {
        if request.capture {
            return FileChooserResponse::Error(
                "capture file chooser is not supported on Windows yet".to_string(),
            );
        }

        let result = if request.allow_directories {
            self.choose_directory(ChooseDirectoryRequest {
                title: Some(crate::i18n::dialog_title(
                    "file_chooser.select_folder",
                    "Select folder",
                )),
                default_path: None,
            })
            .await
        } else {
            self.choose_file(ChooseFileRequest {
                multiple: request.allow_multiple,
                filters: filters_from_accept_types(&request.accept_types),
                title: Some(crate::i18n::dialog_title(
                    "file_chooser.select_file",
                    "Select file",
                )),
                default_path: None,
            })
            .await
        };

        match result {
            Ok(result) if result.canceled => FileChooserResponse::Cancel,
            Ok(result) => FileChooserResponse::Files(
                result
                    .paths
                    .into_iter()
                    .map(|path| FileChooserFile {
                        path: Some(path),
                        uri: None,
                    })
                    .collect(),
            ),
            Err(err) => FileChooserResponse::Error(err.to_string()),
        }
    }
}

impl FileService for Platform {
    fn review_file(
        &self,
        request: OpenFileRequest,
    ) -> impl Future<Output = Result<(), PlatformError>> + Send {
        async move { crate::rt::blocking(move || open_with_shell(&request.path)).await }
    }

    fn open_external(
        &self,
        request: OpenFileRequest,
    ) -> impl Future<Output = Result<(), PlatformError>> + Send {
        async move { crate::rt::blocking(move || open_with_shell(&request.path)).await }
    }

    fn reveal_in_file_manager(
        &self,
        request: RevealInFileManagerRequest,
    ) -> impl Future<Output = Result<(), PlatformError>> + Send {
        async move {
            // explorer /select, requires backslashes and exits non-zero even on
            // success, so only spawn failures are reported.
            let path = request.path.replace('/', "\\");
            Command::new("explorer")
                .arg(format!("/select,{path}"))
                .spawn()
                .map(drop)
                .map_err(|err| PlatformError::Platform(format!("failed to start explorer: {err}")))
        }
    }

    fn choose_file(
        &self,
        request: ChooseFileRequest,
    ) -> impl Future<Output = Result<FileDialogResult, PlatformError>> + Send {
        crate::desktop::file_dialog::choose_file_desktop(request)
    }

    fn choose_directory(
        &self,
        request: ChooseDirectoryRequest,
    ) -> impl Future<Output = Result<FileDialogResult, PlatformError>> + Send {
        crate::desktop::file_dialog::choose_directory_desktop(request)
    }
}

fn filters_from_accept_types(accept_types: &[String]) -> Vec<FileDialogFilter> {
    let mut extensions: Vec<String> = Vec::new();
    for accept in accept_types {
        let accept = accept.trim().to_ascii_lowercase();
        let mapped: Vec<String> = if let Some(ext) = accept.strip_prefix('.') {
            vec![ext.to_string()]
        } else {
            match accept.as_str() {
                "image/*" => ["png", "jpg", "jpeg", "gif", "bmp", "webp"]
                    .map(String::from)
                    .to_vec(),
                "video/*" => ["mp4", "mov", "avi", "mkv", "webm"]
                    .map(String::from)
                    .to_vec(),
                "audio/*" => ["mp3", "wav", "m4a", "flac", "ogg"]
                    .map(String::from)
                    .to_vec(),
                _ => accept
                    .split_once('/')
                    .and_then(|(_, subtype)| subtype.split('+').next())
                    .map(|ext| vec![ext.to_string()])
                    .unwrap_or_default(),
            }
        };
        for ext in mapped {
            if !ext.is_empty() && !ext.contains('*') && !extensions.contains(&ext) {
                extensions.push(ext);
            }
        }
    }
    if extensions.is_empty() {
        Vec::new()
    } else {
        vec![FileDialogFilter {
            name: None,
            extensions,
        }]
    }
}

/// Blocking shell open; call via `crate::rt::blocking` from async contexts.
pub(super) fn open_with_shell(target: &str) -> Result<(), PlatformError> {
    let status = Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", target])
        .status()
        .map_err(|err| PlatformError::Platform(format!("failed to launch shell open: {err}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(PlatformError::Platform(format!(
            "shell open exited with status {status}"
        )))
    }
}

/// Shell open for sync trait methods that cannot offload to a blocking task:
/// launch without waiting so the executor thread never blocks on the child.
pub(super) fn open_with_shell_detached(target: &str) -> Result<(), PlatformError> {
    Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", target])
        .spawn()
        .map(drop)
        .map_err(|err| PlatformError::Platform(format!("failed to launch shell open: {err}")))
}

pub(super) fn normalize_file_uri(uri: &str) -> Result<PathBuf, PlatformError> {
    let trimmed = uri.trim();
    if trimmed.is_empty() {
        return Err(PlatformError::InvalidParameter(
            "file uri is empty".to_string(),
        ));
    }
    let path = if let Some(rest) = trimmed.strip_prefix("file://") {
        if let Some(local) = rest.strip_prefix('/') {
            percent_decode(local)
        } else if rest.is_empty() {
            return Err(PlatformError::InvalidParameter(format!(
                "file uri has no path: {uri}"
            )));
        } else {
            // file://server/share -> \\server\share UNC path.
            format!("\\\\{}", percent_decode(rest))
        }
    } else {
        trimmed.to_string()
    };
    Ok(PathBuf::from(path.replace('/', "\\")))
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
