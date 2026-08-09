//! Terminal configuration as the runtime exposes it.
//!
//! Loading and live application stay in the shared configuration crate, so
//! every platform gets the same behaviour from the same code. This module
//! adds only what needs the runtime: where this app keeps its data.

use std::path::{Path, PathBuf};

use lingxia_platform::traits::ui::UIUpdate;
use lingxia_terminal_config::{ResolvedFont, TerminalConfig, ThemeDetails, ThemeStore};
use serde::Serialize;

pub use lingxia_terminal_config::runtime::{
    apply_theme, current_json, generation, installed_fonts, load, set_installed_fonts,
};

/// Where this app keeps its data, as the configuration layer wants it.
///
/// Derived from the initialized runtime rather than rebuilt from each
/// platform's conventions: a host that guesses writes a file the app never
/// reads, and the two paths only differ when it matters.
pub fn app_data_dir() -> Option<PathBuf> {
    crate::app::state_dir()
        .ok()
        .and_then(|dir| dir.parent().map(Path::to_path_buf))
}

pub use lingxia_terminal_config::SETTINGS_APP_ID;

fn product_defaults() -> serde_json::Value {
    lingxia_app_context::app_config()
        .and_then(|config| config.terminal.as_ref())
        .map(|terminal| terminal.defaults.clone())
        .unwrap_or_else(|| serde_json::json!({}))
}

/// Load the running product's terminal defaults and user overrides.
pub fn load_for_app(system_is_dark: bool) -> Option<TerminalConfig> {
    let data_dir = app_data_dir()?;
    Some(lingxia_terminal_config::runtime::load(
        data_dir,
        &product_defaults().to_string(),
        system_is_dark,
    ))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSnapshot {
    pub config: TerminalConfig,
    pub resolved_font: ResolvedFont,
    pub appearance_is_dark: bool,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThemeImportResult {
    pub name: String,
}

fn context() -> Result<(PathBuf, serde_json::Value, bool), String> {
    let data_dir =
        app_data_dir().ok_or_else(|| "terminal runtime is not initialized".to_string())?;
    let appearance_is_dark = lxapp::get_platform()
        .map(|platform| platform.host_appearance_dark())
        .unwrap_or(false);
    Ok((data_dir, product_defaults(), appearance_is_dark))
}

fn snapshot(data_dir: &Path, config: TerminalConfig, appearance_is_dark: bool) -> ConfigSnapshot {
    ConfigSnapshot {
        resolved_font: lingxia_terminal_config::resolve_font(
            &config.font,
            &lingxia_terminal_config::runtime::installed_fonts(),
        ),
        path: TerminalConfig::path(data_dir)
            .to_string_lossy()
            .into_owned(),
        appearance_is_dark,
        config,
    }
}

pub fn config_get() -> Result<ConfigSnapshot, String> {
    let (data_dir, defaults, appearance_is_dark) = context()?;
    let config = lingxia_terminal_config::runtime::load(
        data_dir.clone(),
        &defaults.to_string(),
        appearance_is_dark,
    );
    Ok(snapshot(&data_dir, config, appearance_is_dark))
}

pub fn config_apply(overlay: serde_json::Value) -> Result<ConfigSnapshot, String> {
    let (data_dir, defaults, appearance_is_dark) = context()?;
    let config = lingxia_terminal_config::runtime::apply_config(
        &data_dir,
        &defaults,
        &overlay,
        appearance_is_dark,
    )
    .map_err(|error| error.to_string())?;
    Ok(snapshot(&data_dir, config, appearance_is_dark))
}

pub fn config_reset(scope: Option<&str>) -> Result<ConfigSnapshot, String> {
    let (data_dir, defaults, appearance_is_dark) = context()?;
    let config = lingxia_terminal_config::runtime::reset_config(
        &data_dir,
        &defaults,
        scope,
        appearance_is_dark,
    )
    .map_err(|error| error.to_string())?;
    Ok(snapshot(&data_dir, config, appearance_is_dark))
}

pub fn themes_list() -> Result<Vec<ThemeDetails>, String> {
    let (data_dir, _, _) = context()?;
    Ok(ThemeStore::new(&data_dir).list_with_schemes())
}

pub fn theme_import(text: &str, name: Option<&str>) -> Result<ThemeImportResult, String> {
    let (data_dir, _, appearance_is_dark) = context()?;
    let scheme = lingxia_terminal_config::parse_scheme(text)?;
    scheme.to_colors().map_err(|error| error.to_string())?;
    let name = name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .or_else(|| scheme.name.clone())
        .unwrap_or_else(|| "imported".to_string());
    lingxia_terminal_config::runtime::import_theme(
        &data_dir,
        &name,
        &scheme,
        true,
        appearance_is_dark,
    )
    .map_err(|error| error.to_string())?;
    Ok(ThemeImportResult { name })
}

pub fn fonts_list() -> Vec<lingxia_terminal_config::InstalledFont> {
    lingxia_terminal_config::runtime::installed_fonts()
}
