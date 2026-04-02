use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_dir: Option<String>,
}

static SETTINGS_CACHE: OnceLock<DashMap<String, Settings>> = OnceLock::new();

fn cache() -> &'static DashMap<String, Settings> {
    SETTINGS_CACHE.get_or_init(DashMap::new)
}

fn settings_key(app_data_dir: &Path) -> String {
    app_data_dir.to_string_lossy().to_string()
}

pub fn settings_path(app_data_dir: &Path) -> PathBuf {
    lingxia_app_context::app_state_file(app_data_dir, "settings.json")
}

pub fn load(app_data_dir: &Path) -> Result<Settings, SettingsError> {
    let key = settings_key(app_data_dir);
    if let Some(entry) = cache().get(&key) {
        return Ok(entry.value().clone());
    }

    let path = settings_path(app_data_dir);
    let settings = match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice::<Settings>(&bytes)?,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Settings::default(),
        Err(err) => return Err(SettingsError::Io(err)),
    };

    cache().insert(key, settings.clone());
    Ok(settings)
}

pub fn save(app_data_dir: &Path, settings: &Settings) -> Result<(), SettingsError> {
    let path = settings_path(app_data_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(settings)?;
    std::fs::write(&path, bytes)?;
    cache().insert(settings_key(app_data_dir), settings.clone());
    Ok(())
}

pub fn get_download_dir(app_data_dir: &Path) -> Result<Option<PathBuf>, SettingsError> {
    Ok(load(app_data_dir)?
        .download_dir
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from))
}

pub fn set_download_dir(
    app_data_dir: &Path,
    path: Option<impl AsRef<Path>>,
) -> Result<(), SettingsError> {
    let mut settings = load(app_data_dir)?;
    settings.download_dir = path.map(|value| value.as_ref().to_string_lossy().to_string());
    save(app_data_dir, &settings)
}
