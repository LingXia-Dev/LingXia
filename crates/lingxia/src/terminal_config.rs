//! Terminal configuration as the runtime exposes it.
//!
//! Loading and live application stay in the shared configuration crate, so
//! every platform gets the same behaviour from the same code. This module
//! adds only what needs the runtime: where this app keeps its data.

use std::path::{Path, PathBuf};

use lingxia_terminal_config::TerminalConfig;

pub use lingxia_terminal_config::runtime::{
    apply_theme, current_json, generation, installed_fonts, load, set_installed_fonts,
    visual_generation,
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

/// Re-resolve a system-following terminal theme for the running product.
pub fn refresh_appearance_for_app(system_is_dark: bool) {
    if let Some(data_dir) = app_data_dir() {
        lingxia_terminal_config::runtime::refresh_appearance(&data_dir, system_is_dark);
    }
}
