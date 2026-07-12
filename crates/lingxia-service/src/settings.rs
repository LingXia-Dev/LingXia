pub use lingxia_settings::{Settings, SettingsError};

pub type Result<T> = std::result::Result<T, SettingsError>;

pub fn path(app_data_dir: &std::path::Path) -> std::path::PathBuf {
    lingxia_settings::settings_path(app_data_dir)
}

pub fn load(app_data_dir: &std::path::Path) -> Result<Settings> {
    lingxia_settings::load(app_data_dir)
}

pub fn save(app_data_dir: &std::path::Path, settings: &Settings) -> Result<()> {
    lingxia_settings::save(app_data_dir, settings)
}

pub fn download_dir(app_data_dir: &std::path::Path) -> Result<Option<std::path::PathBuf>> {
    lingxia_settings::get_download_dir(app_data_dir)
}

pub fn set_download_dir(
    app_data_dir: &std::path::Path,
    path: Option<impl AsRef<std::path::Path>>,
) -> Result<()> {
    lingxia_settings::set_download_dir(app_data_dir, path)
}

/// User override for the product display language; `None` follows the
/// system locale.
pub fn display_language(app_data_dir: &std::path::Path) -> Result<Option<String>> {
    lingxia_settings::get_display_language(app_data_dir)
}

pub fn set_display_language(app_data_dir: &std::path::Path, language: Option<&str>) -> Result<()> {
    lingxia_settings::set_display_language(app_data_dir, language)
}
