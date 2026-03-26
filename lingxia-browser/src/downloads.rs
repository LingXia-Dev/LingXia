use crate::platform_error::map_platform_error;
use futures::stream;
use lingxia::host::{
    HostCancel, HostResult, HostTypedStreamItem, await_or_cancel, stream_event, stream_return,
};
use lingxia::{DownloadEvent, DownloadRecord, DownloadStatus, DownloadsSnapshot, LxApp};
use lingxia_platform::PlatformError;
use lingxia_platform::traits::app_runtime::{AppRuntime, OpenUrlRequest, OpenUrlTarget};
use lingxia_platform::traits::file::{
    ChooseDirectoryRequest, FileInteraction, OpenDocumentRequest, RevealInFileManagerRequest,
};
use lxapp::LxAppError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadSettingsResult {
    download_dir: String,
    uses_default_dir: bool,
    can_choose_directory: bool,
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

fn download_settings_result(app: &LxApp) -> HostResult<DownloadSettingsResult> {
    let effective = lingxia::download_dir(app)?;
    let configured = lxapp::settings::get_download_dir(&app.app_data_dir())?;
    Ok(DownloadSettingsResult {
        download_dir: effective.to_string_lossy().to_string(),
        uses_default_dir: configured.is_none(),
        can_choose_directory: cfg!(target_os = "macos"),
    })
}

#[lingxia::host("downloads.list")]
fn list_downloads(app: Arc<LxApp>) -> HostResult<DownloadsSnapshot> {
    Ok(lingxia::downloads_snapshot(&app)?)
}

#[lingxia::host("downloads.clearCompleted")]
fn clear_completed_downloads(app: Arc<LxApp>) -> HostResult<ClearCompletedResult> {
    let removed = lingxia::clear_completed_downloads(&app)?;
    Ok(ClearCompletedResult { removed })
}

#[lingxia::host("downloads.remove")]
fn remove_download_route(app: Arc<LxApp>, input: DownloadTaskIdInput) -> HostResult<()> {
    if input.task_id.trim().is_empty() {
        return Err(LxAppError::InvalidParameter(
            "downloads.remove requires taskId".to_string(),
        ));
    }
    lingxia::remove_download(&app, &input.task_id)?;
    Ok(())
}

#[lingxia::host("downloads.cancel")]
fn cancel_download_route(app: Arc<LxApp>, input: DownloadTaskIdInput) -> HostResult<()> {
    if input.task_id.trim().is_empty() {
        return Err(LxAppError::InvalidParameter(
            "downloads.cancel requires taskId".to_string(),
        ));
    }
    lingxia::cancel_download(&app, &input.task_id)
}

#[lingxia::host("downloads.retry")]
fn retry_download_route(_app: Arc<LxApp>, input: DownloadTaskIdInput) -> HostResult<()> {
    if input.task_id.trim().is_empty() {
        return Err(LxAppError::InvalidParameter(
            "downloads.retry requires taskId".to_string(),
        ));
    }
    lxapp::retry_download(&input.task_id)
}

#[lingxia::host("downloads.open")]
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

    let record = lingxia::download_record(&app, &input.task_id)?
        .ok_or_else(|| LxAppError::ResourceNotFound(format!("download not found: {}", input.task_id)))?;
    if record.status != DownloadStatus::Completed {
        return Err(LxAppError::UnsupportedOperation(
            "download is not completed".to_string(),
        ));
    }

    await_or_cancel(&mut cancel, async move {
        app.runtime
            .open_document(OpenDocumentRequest {
                file_path: record.target_path,
                mime_type: record.mime_type,
                show_menu: Some(false),
            })
            .await
            .map_err(|e| map_platform_error("downloads.open", e))
    })
    .await
}

#[lingxia::host("downloads.reveal")]
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

    let record = lingxia::download_record(&app, &input.task_id)?
        .ok_or_else(|| LxAppError::ResourceNotFound(format!("download not found: {}", input.task_id)))?;
    let reveal_path = download_reveal_path(&record);
    let fallback_dir = download_fallback_dir(&reveal_path)?;

    await_or_cancel(&mut cancel, async move {
        match app
            .runtime
            .reveal_in_file_manager(RevealInFileManagerRequest {
                path: reveal_path.to_string_lossy().to_string(),
            })
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

#[lingxia::host("downloads.watch", stream)]
fn watch_downloads(
    app: Arc<LxApp>,
    cancel: HostCancel,
) -> HostResult<impl futures::Stream<Item = HostTypedStreamItem<DownloadEvent, ()>> + Send + 'static>
{
    let mut rx: broadcast::Receiver<DownloadEvent> = lingxia::subscribe_downloads(&app)?;
    let (tx, out_rx) = mpsc::unbounded_channel::<HostTypedStreamItem<DownloadEvent, ()>>();

    let _ = rong::bg::spawn(async move {
        let mut cancel = cancel;
        loop {
            tokio::select! {
                _ = &mut cancel => {
                    let _ = tx.send(Ok(stream_return(())));
                    break;
                }
                recv = rx.recv() => {
                    match recv {
                        Ok(event) => {
                            if tx.send(Ok(stream_event(event))).is_err() {
                                break;
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            let _ = tx.send(Err(LxAppError::Bridge(format!(
                                "download stream lagged by {skipped} events"
                            ))));
                            break;
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            let _ = tx.send(Ok(stream_return(())));
                            break;
                        }
                    }
                }
            }
        }
    });

    Ok(stream::unfold(out_rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    }))
}

#[lingxia::host("downloads.getSettings")]
fn get_download_settings(app: Arc<LxApp>) -> HostResult<DownloadSettingsResult> {
    download_settings_result(&app)
}

#[lingxia::host("downloads.chooseDirectory")]
async fn choose_download_directory(
    app: Arc<LxApp>,
    mut cancel: HostCancel,
) -> HostResult<DownloadSettingsResult> {
    let current_dir = lingxia::download_dir(&app)?.to_string_lossy().to_string();
    let app_for_picker = app.clone();
    let result = await_or_cancel(&mut cancel, async move {
        app_for_picker
            .runtime
            .choose_directory(ChooseDirectoryRequest {
                title: Some("Choose Download Folder".to_string()),
                default_path: Some(current_dir),
            })
            .await
            .map_err(|e| map_platform_error("downloads.chooseDirectory", e))
    })
    .await?;

    if !result.canceled {
        if let Some(path) = result.paths.first() {
            lingxia::set_download_dir(&app, PathBuf::from(path))?;
        }
    }

    download_settings_result(&app)
}

#[lingxia::host("downloads.resetDirectory")]
fn reset_download_directory(app: Arc<LxApp>) -> HostResult<DownloadSettingsResult> {
    lingxia::reset_download_dir(&app)?;
    download_settings_result(&app)
}

pub(crate) fn register() {
    lingxia::register_hosts![
        list_downloads,
        clear_completed_downloads,
        remove_download_route,
        cancel_download_route,
        retry_download_route,
        open_download_route,
        reveal_download_route,
        watch_downloads,
        get_download_settings,
        choose_download_directory,
        reset_download_directory,
    ];
}
