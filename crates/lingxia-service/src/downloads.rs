pub use lingxia_transfer::{
    DownloadEvent, DownloadEventKind, DownloadRecord, DownloadStatus, DownloadsError,
    DownloadsSnapshot,
};

pub type Result<T> = lingxia_transfer::Result<T>;

use std::path::{Path, PathBuf};

/// Where browser and `lx` downloads land: the configured folder, else the user's
/// system Downloads folder, else one under app data.
pub fn dir(app_data_dir: &Path) -> PathBuf {
    lingxia_settings::get_download_dir(app_data_dir)
        .ok()
        .flatten()
        .or_else(system_downloads_dir)
        .unwrap_or_else(|| app_data_dir.join("downloads"))
}

pub fn configured_dir(app_data_dir: &Path) -> Result<Option<PathBuf>> {
    lingxia_settings::get_download_dir(app_data_dir).map_err(settings_error)
}

pub fn set_dir(app_data_dir: &Path, path: impl Into<PathBuf>) -> Result<()> {
    let path = path.into();
    if path.as_os_str().is_empty() {
        return Err(DownloadsError::InvalidParameter(
            "download directory cannot be empty".to_string(),
        ));
    }
    lingxia_settings::set_download_dir(app_data_dir, Some(path)).map_err(settings_error)
}

pub fn reset_dir(app_data_dir: &Path) -> Result<()> {
    lingxia_settings::set_download_dir(app_data_dir, None::<&Path>).map_err(settings_error)
}

fn settings_error(error: lingxia_settings::SettingsError) -> DownloadsError {
    match error {
        lingxia_settings::SettingsError::Io(error) => DownloadsError::Io(error),
        lingxia_settings::SettingsError::Json(error) => DownloadsError::Json(error),
    }
}

/// macOS `~/Downloads`; Windows the Downloads known folder (`FOLDERID_Downloads`),
/// not `%USERPROFILE%\Downloads` unless that lookup fails.
fn system_downloads_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .filter(|home| !home.is_empty())
            .map(|home| PathBuf::from(home).join("Downloads"))
    }
    #[cfg(windows)]
    {
        windows_known_downloads_dir().or_else(|| {
            std::env::var_os("USERPROFILE")
                .filter(|profile| !profile.is_empty())
                .map(|profile| PathBuf::from(profile).join("Downloads"))
        })
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        None
    }
}

#[cfg(windows)]
fn windows_known_downloads_dir() -> Option<PathBuf> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{FOLDERID_Downloads, KF_FLAG_DEFAULT, SHGetKnownFolderPath};
    use windows::core::PWSTR;

    unsafe {
        let pwstr: PWSTR = SHGetKnownFolderPath(&FOLDERID_Downloads, KF_FLAG_DEFAULT, None).ok()?;
        let path = pwstr
            .to_string()
            .ok()
            .map(PathBuf::from)
            .filter(|path| !path.as_os_str().is_empty());
        CoTaskMemFree(Some(pwstr.0.cast()));
        path
    }
}

pub fn snapshot(app_data_dir: &Path) -> Result<DownloadsSnapshot> {
    let mut snapshot = lingxia_transfer::snapshot(app_data_dir)?;
    snapshot.download_dir = Some(dir(app_data_dir).to_string_lossy().into_owned());
    Ok(snapshot)
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
