//! Download records and control helpers scoped to an [`crate::LxApp`].

pub use lingxia_service::downloads::{
    DownloadEvent, DownloadEventKind, DownloadRecord, DownloadStatus, DownloadsSnapshot,
};

/// Broadcast receiver type used for download event subscriptions.
pub type DownloadEventStream = tokio::sync::broadcast::Receiver<DownloadEvent>;

/// Result type used by the downloads facade.
pub type Result<T> = crate::Result<T>;

/// Returns the effective downloads directory for the given app.
pub fn dir(app: &crate::LxApp) -> std::path::PathBuf {
    lingxia_service::downloads::dir(&app.app_data_dir())
}

/// Returns the user-configured downloads directory, if one has been set.
pub fn configured_dir(app: &crate::LxApp) -> Result<Option<std::path::PathBuf>> {
    lingxia_service::downloads::configured_dir(&app.app_data_dir()).map_err(Into::into)
}

/// Sets the downloads directory override for the given app.
pub fn set_dir(app: &crate::LxApp, path: impl Into<std::path::PathBuf>) -> Result<()> {
    lingxia_service::downloads::set_dir(&app.app_data_dir(), path).map_err(Into::into)
}

/// Clears the downloads directory override for the given app.
pub fn reset_dir(app: &crate::LxApp) -> Result<()> {
    lingxia_service::downloads::reset_dir(&app.app_data_dir()).map_err(Into::into)
}

/// Returns a snapshot of all known download records for the given app.
pub fn snapshot(app: &crate::LxApp) -> Result<DownloadsSnapshot> {
    lingxia_service::downloads::snapshot(&app.app_data_dir()).map_err(Into::into)
}

/// Subscribes to download events for the given app.
pub fn subscribe(app: &crate::LxApp) -> Result<DownloadEventStream> {
    lingxia_service::downloads::subscribe(&app.app_data_dir()).map_err(Into::into)
}

/// Alias for [`subscribe`].
pub fn watch(app: &crate::LxApp) -> Result<DownloadEventStream> {
    subscribe(app)
}

/// Looks up a single download record by task id.
pub fn record(app: &crate::LxApp, task_id: &str) -> Result<Option<DownloadRecord>> {
    lingxia_service::downloads::record(&app.app_data_dir(), task_id).map_err(Into::into)
}

/// Removes all completed download records and returns the count removed.
pub fn clear_completed(app: &crate::LxApp) -> Result<u64> {
    lingxia_service::downloads::clear_completed(&app.app_data_dir()).map_err(Into::into)
}

/// Deletes a download record and any managed file for the given task id.
pub fn remove(app: &crate::LxApp, task_id: &str) -> Result<()> {
    lingxia_service::downloads::remove(&app.app_data_dir(), task_id).map_err(Into::into)
}

/// Cancels an in-flight download task.
pub fn cancel(app: &crate::LxApp, task_id: &str) -> Result<()> {
    lingxia_service::downloads::cancel(&app.app_data_dir(), task_id).map_err(Into::into)
}

/// Pauses a resumable download task.
pub fn pause(app: &crate::LxApp, task_id: &str) -> Result<()> {
    lingxia_service::downloads::pause(&app.app_data_dir(), task_id).map_err(Into::into)
}

/// Retries a failed or cancelled download task.
pub fn retry(app: &crate::LxApp, task_id: &str) -> Result<()> {
    lingxia_service::downloads::retry(&app.app_data_dir(), task_id).map_err(Into::into)
}

/// Resumes a paused download task.
pub fn resume(app: &crate::LxApp, task_id: &str) -> Result<()> {
    lingxia_service::downloads::resume(&app.app_data_dir(), task_id).map_err(Into::into)
}
