use crate::host::{HostCancel, HostResult, StreamContext, await_or_cancel};
use crate::platform_error::map_platform_error;
use lingxia_platform::PlatformError;
use lingxia_platform::traits::app_runtime::{AppRuntime, OpenUrlRequest, OpenUrlTarget};
use lingxia_service::downloads::{
    DownloadEvent, DownloadRecord, DownloadStatus, DownloadsError, DownloadsSnapshot,
};
use lingxia_service::file::{OpenFileRequest, RevealInFileManagerRequest};
use lxapp::LxApp;
use lxapp::LxAppError;
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

fn file_url_for(path: &Path) -> String {
    format!("file://{}", path.to_string_lossy())
}

fn download_reveal_path(record: &DownloadRecord) -> PathBuf {
    let target_path = PathBuf::from(&record.target_path);
    let part_path = target_path.with_extension("part");

    match record.status {
        DownloadStatus::Downloading => {
            if part_path.exists() {
                return part_path;
            }
            if target_path.exists() {
                return target_path;
            }
        }
        DownloadStatus::Completed => {
            if target_path.exists() {
                return target_path;
            }
            if part_path.exists() {
                return part_path;
            }
        }
        DownloadStatus::Failed => {
            if part_path.exists() {
                return part_path;
            }
            if target_path.exists() {
                return target_path;
            }
        }
        DownloadStatus::Paused => {
            if part_path.exists() {
                return part_path;
            }
            if target_path.exists() {
                return target_path;
            }
        }
        DownloadStatus::Removed => {
            if target_path.exists() {
                return target_path;
            }
            if part_path.exists() {
                return part_path;
            }
        }
    }

    target_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or(target_path)
}

fn download_fallback_dir(path: &Path) -> Result<PathBuf, LxAppError> {
    if path.is_dir() {
        return Ok(path.to_path_buf());
    }
    path.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| LxAppError::InvalidParameter("download has no parent directory".to_string()))
}

async fn open_file_for_user(app: &LxApp, request: OpenFileRequest) -> Result<(), PlatformError> {
    match lingxia_service::file::review_file(&*app.runtime, request.clone()).await {
        Ok(()) => Ok(()),
        Err(_review_error) => lingxia_service::file::open_external(&*app.runtime, request).await,
    }
}

fn map_downloads_error(err: DownloadsError) -> LxAppError {
    match err {
        DownloadsError::InvalidParameter(message) => LxAppError::InvalidParameter(message),
        DownloadsError::ResourceNotFound(message) => LxAppError::ResourceNotFound(message),
        DownloadsError::UnsupportedOperation(message) => LxAppError::UnsupportedOperation(message),
        DownloadsError::Runtime(message) => LxAppError::Runtime(message),
        DownloadsError::Io(err) => LxAppError::IoError(err.to_string()),
        DownloadsError::Json(err) => LxAppError::Bridge(format!("JSON Processing Error: {err}")),
        DownloadsError::Settings(err) => LxAppError::Runtime(err.to_string()),
    }
}

#[lingxia::native("downloads.list")]
fn list_downloads(app: Arc<LxApp>) -> HostResult<DownloadsSnapshot> {
    Ok(lingxia_service::downloads::snapshot(&app.app_data_dir()).map_err(map_downloads_error)?)
}

#[lingxia::native("downloads.clearCompleted")]
fn clear_completed_downloads(app: Arc<LxApp>) -> HostResult<ClearCompletedResult> {
    let removed = lingxia_service::downloads::clear_completed(&app.app_data_dir())
        .map_err(map_downloads_error)?;
    Ok(ClearCompletedResult { removed })
}

#[lingxia::native("downloads.remove")]
fn remove_download_route(app: Arc<LxApp>, input: DownloadTaskIdInput) -> HostResult<()> {
    if input.task_id.trim().is_empty() {
        return Err(LxAppError::InvalidParameter(
            "downloads.remove requires taskId".to_string(),
        ));
    }
    lingxia_service::downloads::remove(&app.app_data_dir(), &input.task_id)
        .map_err(map_downloads_error)?;
    Ok(())
}

#[lingxia::native("downloads.cancel")]
fn cancel_download_route(app: Arc<LxApp>, input: DownloadTaskIdInput) -> HostResult<()> {
    if input.task_id.trim().is_empty() {
        return Err(LxAppError::InvalidParameter(
            "downloads.cancel requires taskId".to_string(),
        ));
    }
    lingxia_service::downloads::cancel(&app.app_data_dir(), &input.task_id)
        .map_err(map_downloads_error)
}

#[lingxia::native("downloads.pause")]
fn pause_download_route(app: Arc<LxApp>, input: DownloadTaskIdInput) -> HostResult<()> {
    if input.task_id.trim().is_empty() {
        return Err(LxAppError::InvalidParameter(
            "downloads.pause requires taskId".to_string(),
        ));
    }
    lingxia_service::downloads::pause(&app.app_data_dir(), &input.task_id)
        .map_err(map_downloads_error)
}

#[lingxia::native("downloads.retry")]
fn retry_download_route(app: Arc<LxApp>, input: DownloadTaskIdInput) -> HostResult<()> {
    if input.task_id.trim().is_empty() {
        return Err(LxAppError::InvalidParameter(
            "downloads.retry requires taskId".to_string(),
        ));
    }
    lingxia_service::downloads::retry(&app.app_data_dir(), &input.task_id)
        .map_err(map_downloads_error)
}

#[lingxia::native("downloads.resume")]
fn resume_download_route(app: Arc<LxApp>, input: DownloadTaskIdInput) -> HostResult<()> {
    if input.task_id.trim().is_empty() {
        return Err(LxAppError::InvalidParameter(
            "downloads.resume requires taskId".to_string(),
        ));
    }
    lingxia_service::downloads::resume(&app.app_data_dir(), &input.task_id)
        .map_err(map_downloads_error)
}

#[lingxia::native("downloads.open")]
async fn open_download_route(
    app: Arc<LxApp>,
    input: DownloadTaskIdInput,
    mut cancel: HostCancel,
) -> HostResult<()> {
    if input.task_id.trim().is_empty() {
        return Err(LxAppError::InvalidParameter(
            "downloads.open requires taskId".to_string(),
        ));
    }

    let record = lingxia_service::downloads::record(&app.app_data_dir(), &input.task_id)
        .map_err(map_downloads_error)?
        .ok_or_else(|| {
            LxAppError::ResourceNotFound(format!("download not found: {}", input.task_id))
        })?;
    if record.status != DownloadStatus::Completed {
        return Err(LxAppError::UnsupportedOperation(
            "download is not completed".to_string(),
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
        .map_err(|e| map_platform_error("downloads.open", e))
    })
    .await
}

#[lingxia::native("downloads.reveal")]
async fn reveal_download_route(
    app: Arc<LxApp>,
    input: DownloadTaskIdInput,
    mut cancel: HostCancel,
) -> HostResult<()> {
    if input.task_id.trim().is_empty() {
        return Err(LxAppError::InvalidParameter(
            "downloads.reveal requires taskId".to_string(),
        ));
    }

    let record = lingxia_service::downloads::record(&app.app_data_dir(), &input.task_id)
        .map_err(map_downloads_error)?
        .ok_or_else(|| {
            LxAppError::ResourceNotFound(format!("download not found: {}", input.task_id))
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
            Err(PlatformError::NotSupported(_)) => app
                .runtime
                .open_url(OpenUrlRequest {
                    owner_appid: app.appid.clone(),
                    owner_session_id: app.session_id(),
                    url: file_url_for(&fallback_dir),
                    target: OpenUrlTarget::External,
                })
                .map_err(|e| map_platform_error("downloads.reveal", e)),
            Err(e) => Err(map_platform_error("downloads.reveal", e)),
        }
    })
    .await
}

#[lingxia::native("downloads.watch", stream)]
async fn watch_downloads(
    app: Arc<LxApp>,
    mut stream: StreamContext<DownloadEvent>,
) -> HostResult<()> {
    let mut rx: broadcast::Receiver<DownloadEvent> =
        lingxia_service::downloads::subscribe(&app.app_data_dir()).map_err(map_downloads_error)?;

    loop {
        tokio::select! {
            _ = stream.canceled() => return Ok(()),
            recv = rx.recv() => {
                match recv {
                    Ok(event) => stream.send(event)?,
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        return Err(LxAppError::Bridge(format!(
                            "download stream lagged by {skipped} events"
                        )));
                    }
                    Err(broadcast::error::RecvError::Closed) => return stream.end(()),
                }
            }
        }
    }
}

pub(crate) fn register() {
    lxapp::host::register_host_entry(list_downloads_host());
    lxapp::host::register_host_entry(clear_completed_downloads_host());
    lxapp::host::register_host_entry(remove_download_route_host());
    lxapp::host::register_host_entry(cancel_download_route_host());
    lxapp::host::register_host_entry(pause_download_route_host());
    lxapp::host::register_host_entry(retry_download_route_host());
    lxapp::host::register_host_entry(resume_download_route_host());
    lxapp::host::register_host_entry(open_download_route_host());
    lxapp::host::register_host_entry(reveal_download_route_host());
    lxapp::host::register_host_entry(watch_downloads_host());
}
