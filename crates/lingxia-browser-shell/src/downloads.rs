use crate::host::{HostCancel, await_or_cancel};
use lingxia_error::Error;
use lingxia_platform::traits::app_runtime::{AppRuntime, OpenUrlRequest, OpenUrlTarget};
use lingxia_service::downloads::{
    DownloadEvent, DownloadRecord, DownloadStatus, DownloadsError, DownloadsSnapshot,
};
use lingxia_service::file::{OpenFileRequest, RevealInFileManagerRequest};
use lxapp::LxApp;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClearCompletedResult {
    removed: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DownloadTaskIdInput {
    task_id: String,
}

/// Build a `file://` URL for a local path without external dependencies.
///
/// - Windows drive paths: `C:\Users\x` → `file:///C:/Users/x`
/// - UNC paths: `\\server\share\x` → `file://server/share/x`
/// - POSIX paths: `/home/x` → `file:///home/x`
fn file_url_for(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    if let Some(unc) = normalized.strip_prefix("//") {
        // UNC: the server becomes the URL authority.
        format!("file://{unc}")
    } else if normalized.starts_with('/') {
        format!("file://{normalized}")
    } else {
        // Drive-letter (or other rootless) path: needs an empty authority.
        format!("file:///{normalized}")
    }
}

fn download_reveal_path(record: &DownloadRecord) -> PathBuf {
    let target_path = PathBuf::from(&record.target_path);
    let part_path = target_path.with_extension("part");

    // Finished states prefer the final file; in-progress/failed states prefer
    // the .part file. Fall back to whichever of the two exists.
    let candidates = match record.status {
        DownloadStatus::Completed | DownloadStatus::Removed => [target_path.clone(), part_path],
        DownloadStatus::Downloading | DownloadStatus::Failed | DownloadStatus::Paused => {
            [part_path, target_path.clone()]
        }
    };
    for candidate in candidates {
        if candidate.exists() {
            return candidate;
        }
    }

    target_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or(target_path)
}

fn download_fallback_dir(path: &Path) -> Result<PathBuf, lingxia_error::Error> {
    if path.is_dir() {
        return Ok(path.to_path_buf());
    }
    path.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| lingxia_error::Error::invalid_arg("download has no parent directory"))
}

async fn open_file_for_user(app: &LxApp, request: OpenFileRequest) -> Result<(), Error> {
    match lingxia_service::file::review_file(&*app.runtime, request.clone()).await {
        Ok(()) => Ok(()),
        Err(_review_error) => lingxia_service::file::open_external(&*app.runtime, request).await,
    }
}

fn map_downloads_error(err: DownloadsError) -> lingxia_error::Error {
    match err {
        DownloadsError::InvalidParameter(message) => lingxia_error::Error::invalid_arg(message),
        DownloadsError::ResourceNotFound(message) => lingxia_error::Error::not_found(message),
        DownloadsError::UnsupportedOperation(message) => {
            lingxia_error::Error::not_supported(message)
        }
        DownloadsError::Runtime(message) => lingxia_error::Error::internal(message),
        DownloadsError::Io(err) => lingxia_error::Error::io(err.to_string()),
        DownloadsError::Json(err) => {
            lingxia_error::Error::internal(format!("JSON Processing Error: {err}"))
        }
    }
}

#[lingxia::framework_native("downloads.list", audience = "control-surface-only")]
fn list_downloads(app: Arc<LxApp>) -> lingxia_error::Result<DownloadsSnapshot> {
    lingxia_service::downloads::snapshot(&app.app_data_dir()).map_err(map_downloads_error)
}

#[lingxia::framework_native("downloads.clearCompleted", audience = "control-surface-only")]
fn clear_completed_downloads(app: Arc<LxApp>) -> lingxia_error::Result<ClearCompletedResult> {
    let removed = lingxia_service::downloads::clear_completed(&app.app_data_dir())
        .map_err(map_downloads_error)?;
    Ok(ClearCompletedResult { removed })
}

#[lingxia::framework_native("downloads.remove", audience = "control-surface-only")]
fn remove_download_route(app: Arc<LxApp>, input: DownloadTaskIdInput) -> lingxia_error::Result<()> {
    if input.task_id.trim().is_empty() {
        return Err(lingxia_error::Error::invalid_arg(
            "downloads.remove requires taskId",
        ));
    }
    lingxia_service::downloads::remove(&app.app_data_dir(), &input.task_id)
        .map_err(map_downloads_error)?;
    Ok(())
}

#[lingxia::framework_native("downloads.cancel", audience = "control-surface-only")]
fn cancel_download_route(app: Arc<LxApp>, input: DownloadTaskIdInput) -> lingxia_error::Result<()> {
    if input.task_id.trim().is_empty() {
        return Err(lingxia_error::Error::invalid_arg(
            "downloads.cancel requires taskId",
        ));
    }
    lingxia_service::downloads::cancel(&app.app_data_dir(), &input.task_id)
        .map_err(map_downloads_error)
}

#[lingxia::framework_native("downloads.pause", audience = "control-surface-only")]
fn pause_download_route(app: Arc<LxApp>, input: DownloadTaskIdInput) -> lingxia_error::Result<()> {
    if input.task_id.trim().is_empty() {
        return Err(lingxia_error::Error::invalid_arg(
            "downloads.pause requires taskId",
        ));
    }
    lingxia_service::downloads::pause(&app.app_data_dir(), &input.task_id)
        .map_err(map_downloads_error)
}

#[lingxia::framework_native("downloads.retry", audience = "control-surface-only")]
fn retry_download_route(app: Arc<LxApp>, input: DownloadTaskIdInput) -> lingxia_error::Result<()> {
    if input.task_id.trim().is_empty() {
        return Err(lingxia_error::Error::invalid_arg(
            "downloads.retry requires taskId",
        ));
    }
    lingxia_service::downloads::retry(&app.app_data_dir(), &input.task_id)
        .map_err(map_downloads_error)
}

#[lingxia::framework_native("downloads.resume", audience = "control-surface-only")]
fn resume_download_route(app: Arc<LxApp>, input: DownloadTaskIdInput) -> lingxia_error::Result<()> {
    if input.task_id.trim().is_empty() {
        return Err(lingxia_error::Error::invalid_arg(
            "downloads.resume requires taskId",
        ));
    }
    lingxia_service::downloads::resume(&app.app_data_dir(), &input.task_id)
        .map_err(map_downloads_error)
}

#[lingxia::framework_native("downloads.open", audience = "control-surface-only")]
async fn open_download_route(
    app: Arc<LxApp>,
    input: DownloadTaskIdInput,
    mut cancel: HostCancel,
) -> lingxia_error::Result<()> {
    if input.task_id.trim().is_empty() {
        return Err(lingxia_error::Error::invalid_arg(
            "downloads.open requires taskId",
        ));
    }

    let record = lingxia_service::downloads::record(&app.app_data_dir(), &input.task_id)
        .map_err(map_downloads_error)?
        .ok_or_else(|| {
            lingxia_error::Error::not_found(format!("download not found: {}", input.task_id))
        })?;
    if record.status != DownloadStatus::Completed {
        return Err(lingxia_error::Error::not_supported(
            "download is not completed",
        ));
    }

    await_or_cancel(&mut cancel, async move {
        open_file_for_user(
            &app,
            OpenFileRequest {
                path: record.target_path,
                mime_type: record.mime_type,
                show_menu: Some(false),
            },
        )
        .await
    })
    .await
}

#[lingxia::framework_native("downloads.reveal", audience = "control-surface-only")]
async fn reveal_download_route(
    app: Arc<LxApp>,
    input: DownloadTaskIdInput,
    mut cancel: HostCancel,
) -> lingxia_error::Result<()> {
    if input.task_id.trim().is_empty() {
        return Err(lingxia_error::Error::invalid_arg(
            "downloads.reveal requires taskId",
        ));
    }

    let record = lingxia_service::downloads::record(&app.app_data_dir(), &input.task_id)
        .map_err(map_downloads_error)?
        .ok_or_else(|| {
            lingxia_error::Error::not_found(format!("download not found: {}", input.task_id))
        })?;
    let reveal_path = download_reveal_path(&record);
    let fallback_dir = download_fallback_dir(&reveal_path)?;

    await_or_cancel(&mut cancel, async move {
        match lingxia_service::file::reveal_in_file_manager(
            &*app.runtime,
            RevealInFileManagerRequest {
                path: reveal_path.to_string_lossy().to_string(),
            },
        )
        .await
        {
            Ok(()) => Ok(()),
            Err(e) if e.is(lingxia_error::code::NOT_SUPPORTED) => app
                .runtime
                .open_url(OpenUrlRequest {
                    owner_appid: app.appid.clone(),
                    owner_session_id: app.session_id(),
                    url: file_url_for(&fallback_dir),
                    target: OpenUrlTarget::External,
                    want_tab_id: false,
                })
                .map(|_| ()),
            Err(e) => Err(e),
        }
    })
    .await
}

/// A `downloads.watch` event: the list first, then each change.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum DownloadsWatchEvent {
    Snapshot { downloads: Vec<DownloadRecord> },
    Change { event: Box<DownloadEvent> },
}

impl From<DownloadEvent> for DownloadsWatchEvent {
    fn from(event: DownloadEvent) -> Self {
        Self::Change {
            event: Box::new(event),
        }
    }
}

#[lingxia::framework_native("downloads.watch", watch, audience = "control-surface-only")]
fn watch_downloads(
    app: Arc<LxApp>,
) -> lingxia_error::Result<impl futures::Stream<Item = DownloadsWatchEvent> + Send + 'static> {
    let dir = app.app_data_dir();
    // Subscribe before reading so no change falls between the snapshot and the first event.
    let changes: broadcast::Receiver<DownloadEvent> =
        lingxia_service::downloads::subscribe(&dir).map_err(map_downloads_error)?;
    let downloads = lingxia_service::downloads::snapshot(&dir)
        .map_err(map_downloads_error)?
        .downloads;
    Ok(crate::state_then_changes(
        DownloadsWatchEvent::Snapshot { downloads },
        changes,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn file_url_for_windows_drive_path() {
        assert_eq!(
            file_url_for(&PathBuf::from(r"C:\Users\admin\Downloads")),
            "file:///C:/Users/admin/Downloads"
        );
    }

    #[test]
    fn file_url_for_unc_path() {
        assert_eq!(
            file_url_for(&PathBuf::from(r"\\server\share\dir")),
            "file://server/share/dir"
        );
    }

    #[test]
    fn file_url_for_posix_path() {
        assert_eq!(
            file_url_for(&PathBuf::from("/home/user/downloads")),
            "file:///home/user/downloads"
        );
    }

    #[test]
    fn framework_routes_are_browser_control_only() {
        for route in [
            list_downloads_host(),
            clear_completed_downloads_host(),
            remove_download_route_host(),
            cancel_download_route_host(),
            pause_download_route_host(),
            retry_download_route_host(),
            resume_download_route_host(),
            open_download_route_host(),
            reveal_download_route_host(),
            watch_downloads_host(),
        ] {
            assert_eq!(
                route.audience(),
                lxapp::host::RouteAudience::ControlSurfaceOnly
            );
        }
    }
}
