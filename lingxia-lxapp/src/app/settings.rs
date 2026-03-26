use crate::LxAppError;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_dir: Option<String>,
}

static SETTINGS_CACHE: OnceLock<DashMap<String, AppSettings>> = OnceLock::new();

fn cache() -> &'static DashMap<String, AppSettings> {
    SETTINGS_CACHE.get_or_init(DashMap::new)
}

fn settings_key(app_data_dir: &Path) -> String {
    app_data_dir.to_string_lossy().to_string()
}

pub fn settings_path(app_data_dir: &Path) -> PathBuf {
    super::state::file(app_data_dir, "settings.json")
}

pub fn load_settings(app_data_dir: &Path) -> Result<AppSettings, LxAppError> {
    let key = settings_key(app_data_dir);
    if let Some(entry) = cache().get(&key) {
        return Ok(entry.value().clone());
    }

    let path = settings_path(app_data_dir);
    let settings = match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice::<AppSettings>(&bytes)?,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => AppSettings::default(),
        Err(err) => {
            return Err(LxAppError::IoError(format!(
                "read settings failed: {}",
                err
            )));
        }
    };

    cache().insert(key, settings.clone());
    Ok(settings)
}

pub fn save_settings(app_data_dir: &Path, settings: &AppSettings) -> Result<(), LxAppError> {
    let path = settings_path(app_data_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(settings)?;
    std::fs::write(&path, bytes)?;
    cache().insert(settings_key(app_data_dir), settings.clone());
    Ok(())
}

pub fn get_download_dir(app_data_dir: &Path) -> Result<Option<PathBuf>, LxAppError> {
    Ok(load_settings(app_data_dir)?
        .download_dir
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from))
}

pub fn set_download_dir(
    app_data_dir: &Path,
    path: Option<impl AsRef<Path>>,
) -> Result<(), LxAppError> {
    let mut settings = load_settings(app_data_dir)?;
    settings.download_dir = path.map(|value| value.as_ref().to_string_lossy().to_string());
    save_settings(app_data_dir, &settings)
}
