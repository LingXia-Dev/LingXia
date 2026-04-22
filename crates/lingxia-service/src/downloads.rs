pub use lingxia_transfer::{
    DownloadEvent, DownloadEventKind, DownloadRecord, DownloadStatus, DownloadsError,
    DownloadsSnapshot,
};

pub type Result<T> = lingxia_transfer::Result<T>;

pub fn dir(app_data_dir: &std::path::Path) -> std::path::PathBuf {
    lingxia_transfer::dir(app_data_dir)
}

pub fn configured_dir(app_data_dir: &std::path::Path) -> Result<Option<std::path::PathBuf>> {
    lingxia_transfer::configured_dir(app_data_dir)
}

pub fn set_dir(app_data_dir: &std::path::Path, path: impl Into<std::path::PathBuf>) -> Result<()> {
    lingxia_transfer::set_dir(app_data_dir, path)
}

pub fn reset_dir(app_data_dir: &std::path::Path) -> Result<()> {
    lingxia_transfer::reset_dir(app_data_dir)
}

pub fn snapshot(app_data_dir: &std::path::Path) -> Result<DownloadsSnapshot> {
    lingxia_transfer::snapshot(app_data_dir)
}

pub fn subscribe(
    app_data_dir: &std::path::Path,
) -> Result<tokio::sync::broadcast::Receiver<DownloadEvent>> {
    lingxia_transfer::subscribe(app_data_dir)
}

pub fn record(app_data_dir: &std::path::Path, task_id: &str) -> Result<Option<DownloadRecord>> {
    lingxia_transfer::record(app_data_dir, task_id)
}

pub fn clear_completed(app_data_dir: &std::path::Path) -> Result<u64> {
    lingxia_transfer::clear_completed(app_data_dir)
}

pub fn remove(app_data_dir: &std::path::Path, task_id: &str) -> Result<()> {
    lingxia_transfer::remove(app_data_dir, task_id)
}

pub fn cancel(app_data_dir: &std::path::Path, task_id: &str) -> Result<()> {
    lingxia_transfer::cancel(app_data_dir, task_id)
}

pub fn pause(app_data_dir: &std::path::Path, task_id: &str) -> Result<()> {
    lingxia_transfer::pause(app_data_dir, task_id)
}

pub fn retry(app_data_dir: &std::path::Path, task_id: &str) -> Result<()> {
    lingxia_transfer::retry(app_data_dir, task_id)
}

pub fn resume(app_data_dir: &std::path::Path, task_id: &str) -> Result<()> {
    lingxia_transfer::resume(app_data_dir, task_id)
}
