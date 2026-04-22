pub use lingxia_service::downloads::{
    DownloadEvent, DownloadEventKind, DownloadRecord, DownloadStatus, DownloadsSnapshot,
};

pub type DownloadEventStream = tokio::sync::broadcast::Receiver<DownloadEvent>;

pub type Result<T> = crate::Result<T>;

pub fn dir(app: &crate::LxApp) -> std::path::PathBuf {
    lingxia_service::downloads::dir(&app.app_data_dir())
}

pub fn configured_dir(app: &crate::LxApp) -> Result<Option<std::path::PathBuf>> {
    lingxia_service::downloads::configured_dir(&app.app_data_dir()).map_err(Into::into)
}

pub fn set_dir(app: &crate::LxApp, path: impl Into<std::path::PathBuf>) -> Result<()> {
    lingxia_service::downloads::set_dir(&app.app_data_dir(), path).map_err(Into::into)
}

pub fn reset_dir(app: &crate::LxApp) -> Result<()> {
    lingxia_service::downloads::reset_dir(&app.app_data_dir()).map_err(Into::into)
}

pub fn snapshot(app: &crate::LxApp) -> Result<DownloadsSnapshot> {
    lingxia_service::downloads::snapshot(&app.app_data_dir()).map_err(Into::into)
}

pub fn subscribe(app: &crate::LxApp) -> Result<DownloadEventStream> {
    lingxia_service::downloads::subscribe(&app.app_data_dir()).map_err(Into::into)
}

pub fn watch(app: &crate::LxApp) -> Result<DownloadEventStream> {
    subscribe(app)
}

pub fn record(app: &crate::LxApp, task_id: &str) -> Result<Option<DownloadRecord>> {
    lingxia_service::downloads::record(&app.app_data_dir(), task_id).map_err(Into::into)
}

pub fn clear_completed(app: &crate::LxApp) -> Result<u64> {
    lingxia_service::downloads::clear_completed(&app.app_data_dir()).map_err(Into::into)
}

pub fn remove(app: &crate::LxApp, task_id: &str) -> Result<()> {
    lingxia_service::downloads::remove(&app.app_data_dir(), task_id).map_err(Into::into)
}

pub fn cancel(app: &crate::LxApp, task_id: &str) -> Result<()> {
    lingxia_service::downloads::cancel(&app.app_data_dir(), task_id).map_err(Into::into)
}

pub fn pause(app: &crate::LxApp, task_id: &str) -> Result<()> {
    lingxia_service::downloads::pause(&app.app_data_dir(), task_id).map_err(Into::into)
}

pub fn retry(app: &crate::LxApp, task_id: &str) -> Result<()> {
    lingxia_service::downloads::retry(&app.app_data_dir(), task_id).map_err(Into::into)
}

pub fn resume(app: &crate::LxApp, task_id: &str) -> Result<()> {
    lingxia_service::downloads::resume(&app.app_data_dir(), task_id).map_err(Into::into)
}
