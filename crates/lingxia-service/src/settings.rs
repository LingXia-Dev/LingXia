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

pub fn webui_language(app_data_dir: &std::path::Path) -> Result<Option<String>> {
    lingxia_settings::get_webui_language(app_data_dir)
}

pub fn set_webui_language(app_data_dir: &std::path::Path, language: Option<&str>) -> Result<()> {
    lingxia_settings::set_webui_language(app_data_dir, language)
}
