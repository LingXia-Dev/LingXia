//! Terminal configuration as the runtime exposes it.
//!
//! Loading and live application stay in the shared configuration crate, so
//! every platform gets the same behaviour from the same code. This module
//! adds only what needs the runtime: where this app keeps its data.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use lingxia_platform::traits::ui::UIUpdate;
use lingxia_terminal::TerminalTheme;
use lingxia_terminal_config::{ResolvedFont, TerminalConfig, ThemeDetails, ThemeStore};
use serde::{Deserialize, Serialize};

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

pub const SETTINGS_APP_ID: &str = "app.lingxia.terminal-settings";

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
    // Product terminal defaults are not yet represented in app.json. Keep the
    // parameter explicit so wiring that layer cannot fork the route behavior.
    Ok((data_dir, serde_json::json!({}), appearance_is_dark))
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
    let (data_dir, _, _) = context()?;
    let scheme = lingxia_terminal_config::parse_scheme(text)?;
    scheme.to_colors().map_err(|error| error.to_string())?;
    let name = name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .or_else(|| scheme.name.clone())
        .unwrap_or_else(|| "imported".to_string());
    ThemeStore::new(&data_dir)
        .import(&name, &scheme)
        .map_err(|error| error.to_string())?;
    Ok(ThemeImportResult { name })
}

pub fn theme_preview(scheme: Option<TerminalTheme>, name: Option<&str>) -> Result<(), String> {
    let (data_dir, _, _) = context()?;
    let scheme = match (scheme, name) {
        (Some(scheme), None) => scheme,
        (None, Some(name)) => ThemeStore::new(&data_dir)
            .get(name)
            .ok_or_else(|| format!("no terminal theme named '{name}'"))?,
        _ => return Err("preview takes exactly one of scheme or name".to_string()),
    };
    lingxia_terminal_config::runtime::preview_theme(&scheme)
}

pub fn theme_preview_end() -> Result<(), String> {
    let (data_dir, _, appearance_is_dark) = context()?;
    lingxia_terminal_config::runtime::end_theme_preview(&data_dir, appearance_is_dark);
    Ok(())
}

pub fn fonts_list() -> Vec<lingxia_terminal_config::InstalledFont> {
    lingxia_terminal_config::runtime::installed_fonts()
}

fn route_error(error: String) -> lxapp::LxAppError {
    lxapp::LxAppError::Runtime(error)
}

fn is_settings_app(appid: &str) -> bool {
    appid == SETTINGS_APP_ID
}

fn require_settings_app(app: &lxapp::LxApp) -> crate::host::HostResult<()> {
    if is_settings_app(&app.appid) {
        Ok(())
    } else {
        Err(lxapp::LxAppError::UnsupportedOperation(format!(
            "terminal settings route is restricted to the built-in settings app (caller: {})",
            app.appid
        )))
    }
}

#[derive(Debug, Deserialize)]
struct ResetInput {
    scope: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ImportInput {
    text: String,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PreviewInput {
    scheme: Option<TerminalTheme>,
    name: Option<String>,
}

#[lingxia::native("terminal.config.get")]
fn route_config_get(app: Arc<lxapp::LxApp>) -> crate::host::HostResult<ConfigSnapshot> {
    require_settings_app(&app)?;
    config_get().map_err(route_error)
}

#[lingxia::native("terminal.config.apply")]
fn route_config_apply(
    app: Arc<lxapp::LxApp>,
    overlay: serde_json::Value,
) -> crate::host::HostResult<ConfigSnapshot> {
    require_settings_app(&app)?;
    config_apply(overlay).map_err(route_error)
}

#[lingxia::native("terminal.config.reset")]
fn route_config_reset(
    app: Arc<lxapp::LxApp>,
    input: ResetInput,
) -> crate::host::HostResult<ConfigSnapshot> {
    require_settings_app(&app)?;
    config_reset(input.scope.as_deref()).map_err(route_error)
}

#[lingxia::native("terminal.themes.list")]
fn route_themes_list(app: Arc<lxapp::LxApp>) -> crate::host::HostResult<Vec<ThemeDetails>> {
    require_settings_app(&app)?;
    themes_list().map_err(route_error)
}

#[lingxia::native("terminal.themes.import")]
fn route_theme_import(
    app: Arc<lxapp::LxApp>,
    input: ImportInput,
) -> crate::host::HostResult<ThemeImportResult> {
    require_settings_app(&app)?;
    theme_import(&input.text, input.name.as_deref()).map_err(route_error)
}

#[lingxia::native("terminal.themes.preview")]
fn route_theme_preview(app: Arc<lxapp::LxApp>, input: PreviewInput) -> crate::host::HostResult<()> {
    require_settings_app(&app)?;
    theme_preview(input.scheme, input.name.as_deref()).map_err(route_error)
}

#[lingxia::native("terminal.themes.previewEnd")]
fn route_theme_preview_end(app: Arc<lxapp::LxApp>) -> crate::host::HostResult<()> {
    require_settings_app(&app)?;
    theme_preview_end().map_err(route_error)
}

#[lingxia::native("terminal.fonts.list")]
fn route_fonts_list(
    app: Arc<lxapp::LxApp>,
) -> crate::host::HostResult<Vec<lingxia_terminal_config::InstalledFont>> {
    require_settings_app(&app)?;
    Ok(fonts_list())
}

pub(crate) fn register_settings_routes() {
    crate::host::register_host_entry(route_config_get_host());
    crate::host::register_host_entry(route_config_apply_host());
    crate::host::register_host_entry(route_config_reset_host());
    crate::host::register_host_entry(route_themes_list_host());
    crate::host::register_host_entry(route_theme_import_host());
    crate::host::register_host_entry(route_theme_preview_host());
    crate::host::register_host_entry(route_theme_preview_end_host());
    crate::host::register_host_entry(route_fonts_list_host());
}

#[cfg(test)]
mod tests {
    use super::is_settings_app;

    #[test]
    fn settings_routes_are_reserved_for_the_bundled_app() {
        assert!(is_settings_app(super::SETTINGS_APP_ID));
        assert!(!is_settings_app("com.example.product"));
    }
}
