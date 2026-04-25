//! Persisted host settings helpers scoped to an [`crate::LxApp`].

pub use lingxia_service::settings::Settings;

/// Result type used by the settings facade.
pub type Result<T> = crate::Result<T>;

/// Returns the on-disk path to the app's persisted settings file.
pub fn path(app: &crate::LxApp) -> std::path::PathBuf {
    lingxia_service::settings::path(&app.app_data_dir())
}

/// Loads persisted settings for the given app.
pub fn load(app: &crate::LxApp) -> Result<Settings> {
    lingxia_service::settings::load(&app.app_data_dir()).map_err(Into::into)
}

/// Saves the full settings payload for the given app.
pub fn save(app: &crate::LxApp, settings: &Settings) -> Result<()> {
    lingxia_service::settings::save(&app.app_data_dir(), settings).map_err(Into::into)
}

/// Returns the configured downloads directory override, if present.
pub fn download_dir(app: &crate::LxApp) -> Result<Option<std::path::PathBuf>> {
    lingxia_service::settings::download_dir(&app.app_data_dir()).map_err(Into::into)
}

/// Returns the effective downloads directory after applying settings overrides.
pub fn effective_download_dir(app: &crate::LxApp) -> std::path::PathBuf {
    crate::downloads::dir(app)
}

/// Sets the downloads directory override.
pub fn set_download_dir(app: &crate::LxApp, path: impl Into<std::path::PathBuf>) -> Result<()> {
    crate::downloads::set_dir(app, path)
}

/// Clears any downloads directory override.
pub fn reset_download_dir(app: &crate::LxApp) -> Result<()> {
    crate::downloads::reset_dir(app)
}

/// Sets or clears the downloads directory override using an optional path.
pub fn set_download_dir_option(
    app: &crate::LxApp,
    path: Option<impl AsRef<std::path::Path>>,
) -> Result<()> {
    lingxia_service::settings::set_download_dir(&app.app_data_dir(), path).map_err(Into::into)
}
