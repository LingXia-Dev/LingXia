//! File chooser support for browser tab WebViews: accept-type mapping and
//! routing chooser requests through the host file dialogs.

use crate::BUILTIN_BROWSER_APPID;
use lingxia_platform::traits::file::{
    ChooseDirectoryRequest, ChooseFileRequest, FileDialogFilter, FileService,
};
use lingxia_webview::{FileChooserFile, FileChooserRequest, FileChooserResponse};
use lxapp::{LxApp, publish_app_event};
use std::sync::Arc;

fn extensions_for_accept_token(value: &str) -> Vec<&'static str> {
    match value {
        "image/*" => vec![
            "png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "heic", "heif",
        ],
        "audio/*" => vec!["mp3", "wav", "aac", "m4a", "ogg", "flac"],
        "video/*" => vec!["mp4", "mov", "m4v", "webm", "mkv", "avi"],
        "text/*" => vec!["txt", "md", "csv", "log"],
        "application/pdf" => vec!["pdf"],
        "application/zip" => vec!["zip"],
        "application/json" => vec!["json"],
        "text/plain" => vec!["txt"],
        "text/csv" => vec!["csv"],
        "text/markdown" => vec!["md"],
        "application/msword" => vec!["doc"],
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => vec!["docx"],
        "application/vnd.ms-excel" => vec!["xls"],
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => vec!["xlsx"],
        "application/vnd.ms-powerpoint" => vec!["ppt"],
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => vec!["pptx"],
        _ => Vec::new(),
    }
}

fn file_filters_from_accept_types(accept_types: &[String]) -> Vec<FileDialogFilter> {
    let mut extensions: Vec<String> = accept_types
        .iter()
        .flat_map(|raw| raw.split(','))
        .map(str::trim)
        .filter_map(|value| {
            if value.is_empty() {
                return None;
            }
            if let Some(stripped) = value.strip_prefix('.') {
                return (!stripped.is_empty()).then(|| stripped.to_ascii_lowercase());
            }
            if value.contains('/') {
                return None;
            }
            Some(value.to_ascii_lowercase())
        })
        .collect();

    for accept_type in accept_types
        .iter()
        .flat_map(|raw| raw.split(','))
        .map(str::trim)
    {
        if accept_type.is_empty() {
            continue;
        }
        extensions.extend(
            extensions_for_accept_token(&accept_type.to_ascii_lowercase())
                .into_iter()
                .map(str::to_string),
        );
    }

    extensions.sort();
    extensions.dedup();

    if extensions.is_empty() {
        Vec::new()
    } else {
        vec![FileDialogFilter {
            name: Some("Files".to_string()),
            extensions,
        }]
    }
}

fn publish_browser_file_chooser_failed_event(request: &FileChooserRequest, error: &str) {
    let payload = serde_json::json!({
        "error": error,
        "acceptTypes": request.accept_types,
        "allowMultiple": request.allow_multiple,
        "allowDirectories": request.allow_directories,
        "capture": request.capture,
        "sourcePageUrl": request.source_page_url,
    });
    let _ = publish_app_event(
        BUILTIN_BROWSER_APPID,
        "FileChooserFailed",
        Some(payload.to_string()),
    );
}

pub(crate) async fn browser_choose_files(
    owner: Arc<LxApp>,
    request: FileChooserRequest,
) -> FileChooserResponse {
    if request.allow_directories {
        return match owner
            .runtime
            .choose_directory(ChooseDirectoryRequest {
                title: Some("Choose folder".to_string()),
                default_path: None,
            })
            .await
        {
            Ok(result) if !result.canceled && !result.paths.is_empty() => {
                FileChooserResponse::Files(
                    result
                        .paths
                        .into_iter()
                        .map(|value| FileChooserFile {
                            path: (!value.contains("://")).then_some(value.clone()),
                            uri: value.contains("://").then_some(value),
                        })
                        .collect(),
                )
            }
            Ok(_) => FileChooserResponse::Cancel,
            Err(err) => {
                publish_browser_file_chooser_failed_event(&request, &err.to_string());
                lxapp::warn!(
                    "[InternalBrowser] file chooser directory request failed: {}",
                    err
                );
                FileChooserResponse::Error(err.to_string())
            }
        };
    }

    match owner
        .runtime
        .choose_file(ChooseFileRequest {
            multiple: request.allow_multiple,
            filters: file_filters_from_accept_types(&request.accept_types),
            title: Some("Choose file".to_string()),
            default_path: None,
        })
        .await
    {
        Ok(result) if !result.canceled && !result.paths.is_empty() => FileChooserResponse::Files(
            result
                .paths
                .into_iter()
                .map(|value| FileChooserFile {
                    path: (!value.contains("://")).then_some(value.clone()),
                    uri: value.contains("://").then_some(value),
                })
                .collect(),
        ),
        Ok(_) => FileChooserResponse::Cancel,
        Err(err) => {
            publish_browser_file_chooser_failed_event(&request, &err.to_string());
            lxapp::warn!("[InternalBrowser] file chooser request failed: {}", err);
            FileChooserResponse::Error(err.to_string())
        }
    }
}
