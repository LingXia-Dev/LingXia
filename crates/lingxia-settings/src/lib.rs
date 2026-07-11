use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webui_language: Option<String>,
}

static SETTINGS_CACHE: OnceLock<DashMap<String, Settings>> = OnceLock::new();

fn cache() -> &'static DashMap<String, Settings> {
    SETTINGS_CACHE.get_or_init(DashMap::new)
}

/// Serializes load-modify-save cycles so concurrent setters cannot drop each
/// other's field.
fn store_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
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
        Ok(bytes) => match serde_json::from_slice::<Settings>(&bytes) {
            Ok(settings) => settings,
            Err(err) => {
                // A corrupt file would otherwise fail every load forever; set
                // it aside and recover with defaults.
                log::error!("corrupt {}: {err}; using defaults", path.display());
                let _ = std::fs::rename(&path, path.with_extension("json.corrupt"));
                Settings::default()
            }
        },
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
    // Temp-write + rename so a crash mid-write cannot truncate the file.
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes)?;
    replace_saved_file(&tmp, &path)?;
    cache().insert(settings_key(app_data_dir), settings.clone());
    Ok(())
}

#[cfg(not(windows))]
fn replace_saved_file(tmp: &Path, path: &Path) -> Result<(), SettingsError> {
    Ok(std::fs::rename(tmp, path)?)
}

#[cfg(windows)]
fn replace_saved_file(tmp: &Path, path: &Path) -> Result<(), SettingsError> {
    let backup = path.with_extension("json.bak");
    if backup.exists() {
        std::fs::remove_file(&backup)?;
    }
    let had_previous = path.exists();
    if had_previous {
        std::fs::rename(path, &backup)?;
    }
    if let Err(err) = std::fs::rename(tmp, path) {
        if had_previous {
            let _ = std::fs::rename(&backup, path);
        }
        return Err(SettingsError::Io(err));
    }
    if had_previous {
        let _ = std::fs::remove_file(backup);
    }
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
    let _guard = store_lock().lock().unwrap_or_else(|e| e.into_inner());
    let mut settings = load(app_data_dir)?;
    settings.download_dir = path.map(|value| value.as_ref().to_string_lossy().to_string());
    save(app_data_dir, &settings)
}

pub fn get_webui_language(app_data_dir: &Path) -> Result<Option<String>, SettingsError> {
    Ok(load(app_data_dir)?
        .webui_language
        .filter(|value| !value.trim().is_empty()))
}

pub fn set_webui_language(
    app_data_dir: &Path,
    language: Option<&str>,
) -> Result<(), SettingsError> {
    let _guard = store_lock().lock().unwrap_or_else(|e| e.into_inner());
    let mut settings = load(app_data_dir)?;
    settings.webui_language = language.map(str::to_string);
    save(app_data_dir, &settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webui_language_round_trips_with_other_settings() {
        let dir = tempfile::tempdir().unwrap();
        set_download_dir(dir.path(), Some(dir.path().join("downloads"))).unwrap();
        set_webui_language(dir.path(), Some("zh-CN")).unwrap();

        assert_eq!(
            get_webui_language(dir.path()).unwrap().as_deref(),
            Some("zh-CN")
        );
        assert_eq!(
            get_download_dir(dir.path()).unwrap(),
            Some(dir.path().join("downloads"))
        );
    }

    #[test]
    fn corrupt_settings_file_recovers_to_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = settings_path(dir.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{ not json").unwrap();

        assert!(load(dir.path()).unwrap().download_dir.is_none());
        assert!(!path.exists());
        assert!(path.with_extension("json.corrupt").exists());

        // The store is writable again after recovery.
        set_webui_language(dir.path(), Some("en-US")).unwrap();
        assert_eq!(
            get_webui_language(dir.path()).unwrap().as_deref(),
            Some("en-US")
        );
    }
}
