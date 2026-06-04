// Download/upload persistence runs as detached background tasks spawned on the
// executor; the join handle is intentionally dropped (fire-and-forget).
#![allow(clippy::let_underscore_future)]

mod download;
mod upload;

pub use download::{
    DownloadEvent, DownloadEventKind, DownloadRecord, DownloadStatus, DownloadsSnapshot,
};
pub use upload::{
    UploadBehavior, UploadEvent, UploadFailure, UploadFailureKind, UploadMethod, UploadRequest,
    UploadResult, resolve_upload_file_name, upload_file_with_behavior,
};

use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::sync::broadcast;

pub type Result<T> = std::result::Result<T, DownloadsError>;

/// Integration hooks used by other LingXia runtime crates such as browser.
pub mod runtime {
    pub use crate::download::manager::{
        DownloadBehavior, DownloadTask, browser_download_root, run_browser_download_task,
    };
    pub use crate::download::{
        get_record, get_request_context, has_active_download, record_bridge_event,
        register_active_download, register_browser_retry_handler,
        register_browser_tab_path_resolver, unregister_active_download,
    };
}

/// Resumable user-cache download primitives used by lxapp logic/runtime.
pub mod user_cache {
    pub use crate::download::manager::{
        DownloadBehavior, DownloadEvent, DownloadFailure, DownloadFailureKind, DownloadOwner,
        DownloadOwnerKind, DownloadPersistence, UserCacheDownloadRequest, UserCacheDownloadResult,
        download_request_task_id, download_to_path_with_behavior, download_to_user_cache,
        download_to_user_cache_with_behavior,
    };
}

#[derive(Debug, Error)]
pub enum DownloadsError {
    #[error("invalid parameter: {0}")]
    InvalidParameter(String),
    #[error("resource not found: {0}")]
    ResourceNotFound(String),
    #[error("unsupported operation: {0}")]
    UnsupportedOperation(String),
    #[error("runtime error: {0}")]
    Runtime(String),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("settings error: {0}")]
    Settings(#[from] lingxia_settings::SettingsError),
}

pub fn dir(app_data_dir: &Path) -> PathBuf {
    download::manager::download_root(app_data_dir)
}

pub fn set_dir(app_data_dir: &Path, path: impl Into<PathBuf>) -> Result<()> {
    let path = path.into();
    if path.as_os_str().is_empty() {
        return Err(DownloadsError::InvalidParameter(
            "download directory cannot be empty".to_string(),
        ));
    }
    lingxia_settings::set_download_dir(app_data_dir, Some(path))?;
    Ok(())
}

pub fn reset_dir(app_data_dir: &Path) -> Result<()> {
    lingxia_settings::set_download_dir(app_data_dir, None::<&Path>)?;
    Ok(())
}

pub fn configured_dir(app_data_dir: &Path) -> Result<Option<PathBuf>> {
    Ok(lingxia_settings::get_download_dir(app_data_dir)?)
}

pub fn snapshot(app_data_dir: &Path) -> Result<DownloadsSnapshot> {
    download::snapshot(app_data_dir)
}

pub fn subscribe(app_data_dir: &Path) -> Result<broadcast::Receiver<DownloadEvent>> {
    download::subscribe(app_data_dir)
}

pub fn record(app_data_dir: &Path, task_id: &str) -> Result<Option<DownloadRecord>> {
    download::get_record(app_data_dir, task_id)
}

pub fn clear_completed(app_data_dir: &Path) -> Result<u64> {
    download::clear_completed(app_data_dir)
}

pub fn remove(app_data_dir: &Path, task_id: &str) -> Result<()> {
    let removed = download::remove(app_data_dir, task_id)?;
    if removed.is_none() {
        return Err(DownloadsError::ResourceNotFound(format!(
            "download not found: {task_id}"
        )));
    }
    Ok(())
}

pub fn cancel(app_data_dir: &Path, task_id: &str) -> Result<()> {
    if download::cancel_active_download(task_id) {
        log::info!("[Downloads] cancel requested for task_id={task_id}");
        return Ok(());
    }

    let record = download::get_record(app_data_dir, task_id)?.ok_or_else(|| {
        DownloadsError::ResourceNotFound(format!("download not found: {task_id}"))
    })?;
    if record.status != DownloadStatus::Downloading {
        return Err(DownloadsError::UnsupportedOperation(
            "download is not active".to_string(),
        ));
    }
    Err(DownloadsError::UnsupportedOperation(
        "download can no longer be canceled".to_string(),
    ))
}

pub fn pause(app_data_dir: &Path, task_id: &str) -> Result<()> {
    if download::pause_active_download(task_id) {
        if let Some(record) = download::get_record(app_data_dir, task_id)? {
            download::record_managed_download_paused(
                app_data_dir,
                task_id,
                record.downloaded_bytes,
                record.total_bytes,
            )?;
        }
        log::info!("[Downloads] pause requested for task_id={task_id}");
        return Ok(());
    }

    let record = download::get_record(app_data_dir, task_id)?.ok_or_else(|| {
        DownloadsError::ResourceNotFound(format!("download not found: {task_id}"))
    })?;
    if record.status == DownloadStatus::Paused {
        return Ok(());
    }
    if record.status != DownloadStatus::Downloading {
        return Err(DownloadsError::UnsupportedOperation(
            "download is not active".to_string(),
        ));
    }
    Err(DownloadsError::UnsupportedOperation(
        "download can no longer be paused".to_string(),
    ))
}

pub fn retry(app_data_dir: &Path, task_id: &str) -> Result<()> {
    let record = download::get_record(app_data_dir, task_id)?.ok_or_else(|| {
        DownloadsError::ResourceNotFound(format!("download not found: {task_id}"))
    })?;
    if record.status != DownloadStatus::Failed {
        return Err(DownloadsError::UnsupportedOperation(
            "download is not retryable".to_string(),
        ));
    }
    if !record.retry {
        return Err(DownloadsError::UnsupportedOperation(
            "download cannot be retried".to_string(),
        ));
    }
    if download::has_active_download(task_id) {
        return Err(DownloadsError::UnsupportedOperation(
            "download is already active".to_string(),
        ));
    }

    let request_context =
        download::get_request_context(app_data_dir, task_id)?.ok_or_else(|| {
            DownloadsError::UnsupportedOperation(
                "download retry context is unavailable".to_string(),
            )
        })?;

    if matches!(record.owner.kind, user_cache::DownloadOwnerKind::LxApp) {
        let task_id_owned = task_id.to_string();
        let app_data_dir_clone = app_data_dir.to_path_buf();
        let owner_appid = record.owner.appid.clone();
        let url = record.url.clone();
        let headers = request_context.headers.clone();
        let user_agent = request_context.user_agent.clone();
        let target_path = PathBuf::from(&record.target_path);
        let behavior = request_context.behavior;

        let _ = rong::RongExecutor::global().spawn(async move {
            let persistence = user_cache::DownloadPersistence::new(
                app_data_dir_clone.clone(),
                task_id_owned.clone(),
                user_cache::DownloadOwner {
                    kind: user_cache::DownloadOwnerKind::LxApp,
                    appid: owner_appid,
                    page_path: None,
                    tab_id: None,
                },
                true,
            );
            let result = user_cache::download_to_path_with_behavior(
                Some(persistence),
                target_path,
                user_cache::UserCacheDownloadRequest { url, headers },
                user_agent,
                behavior,
                |_| {},
            )
            .await;
            if let Err(err) = result {
                log::warn!(
                    "[Downloads] retry download task failed task_id={} url={} reason={}",
                    task_id_owned,
                    err.url,
                    err.error
                );
            }
        });

        return Ok(());
    }

    download::retry_browser_owned_download(task_id)
}

pub fn resume(app_data_dir: &Path, task_id: &str) -> Result<()> {
    let record = download::get_record(app_data_dir, task_id)?.ok_or_else(|| {
        DownloadsError::ResourceNotFound(format!("download not found: {task_id}"))
    })?;
    if record.status != DownloadStatus::Paused {
        return Err(DownloadsError::UnsupportedOperation(
            "download is not paused".to_string(),
        ));
    }
    if download::has_active_download(task_id) {
        return Err(DownloadsError::UnsupportedOperation(
            "download is already active".to_string(),
        ));
    }

    let request_context =
        download::get_request_context(app_data_dir, task_id)?.ok_or_else(|| {
            DownloadsError::UnsupportedOperation(
                "download retry context is unavailable".to_string(),
            )
        })?;

    if matches!(record.owner.kind, user_cache::DownloadOwnerKind::LxApp) {
        let task_id_owned = task_id.to_string();
        let app_data_dir_clone = app_data_dir.to_path_buf();
        let owner_appid = record.owner.appid.clone();
        let url = record.url.clone();
        let headers = request_context.headers.clone();
        let user_agent = request_context.user_agent.clone();
        let target_path = PathBuf::from(&record.target_path);
        let behavior = request_context.behavior;

        let _ = rong::RongExecutor::global().spawn(async move {
            let persistence = user_cache::DownloadPersistence::new(
                app_data_dir_clone.clone(),
                task_id_owned.clone(),
                user_cache::DownloadOwner {
                    kind: user_cache::DownloadOwnerKind::LxApp,
                    appid: owner_appid,
                    page_path: None,
                    tab_id: None,
                },
                true,
            );
            let result = user_cache::download_to_path_with_behavior(
                Some(persistence),
                target_path,
                user_cache::UserCacheDownloadRequest { url, headers },
                user_agent,
                behavior,
                |_| {},
            )
            .await;
            if let Err(err) = result
                && err.error != download::manager::DOWNLOAD_PAUSED_ERROR
            {
                log::warn!(
                    "[Downloads] resume download task failed task_id={} url={} reason={}",
                    task_id_owned,
                    err.url,
                    err.error
                );
            }
        });

        return Ok(());
    }

    download::retry_browser_owned_download(task_id)
}
