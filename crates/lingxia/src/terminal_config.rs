//! Terminal configuration applied to the running engine and hosts.
//!
//! The configuration layer owns schema and storage; this is the seam that
//! turns a loaded configuration into effect: themes go straight into the
//! engine (a repaint, since cell colors resolve at frame time), while font
//! settings are handed to the platform renderer, which is the only side that
//! knows what is installed and how to measure it.

use lingxia_terminal_config::{TerminalConfig, ThemeStore};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// The configuration in effect, so hosts can read it after startup without
/// touching the filesystem again.
static CURRENT: OnceLock<Mutex<TerminalConfig>> = OnceLock::new();

fn current() -> &'static Mutex<TerminalConfig> {
    CURRENT.get_or_init(|| Mutex::new(TerminalConfig::default()))
}

/// Load `terminal.json` over the product defaults and apply what the engine
/// owns. Returns the configuration for the host to consume.
///
/// A broken file is logged and skipped rather than propagated: the terminal
/// must still open.
pub fn load(app_data_dir: PathBuf, product_defaults: &str, system_is_dark: bool) -> TerminalConfig {
    let defaults = serde_json::from_str::<serde_json::Value>(product_defaults)
        .unwrap_or(serde_json::Value::Null);
    let path = TerminalConfig::path(&app_data_dir);
    let (config, error) = TerminalConfig::load(&app_data_dir, &defaults);
    if let Some(error) = error {
        log::warn!("{error}; continuing on defaults");
    }
    log::info!(
        "terminal config: {} ({}), font {:?} {}pt, theme mode {:?}",
        path.display(),
        if path.exists() { "found" } else { "absent" },
        config.font.family,
        config.font.size,
        config.theme.mode
    );
    apply_theme(&app_data_dir, &config, system_is_dark);
    if let Ok(mut slot) = current().lock() {
        *slot = config.clone();
    }
    config
}

/// Push the configured theme into the engine. Cell colors are resolved when a
/// frame is built, so this is a repaint of every live session — no reflow and
/// no respawn.
pub fn apply_theme(app_data_dir: &std::path::Path, config: &TerminalConfig, system_is_dark: bool) {
    let name = config.theme.selected(system_is_dark);
    let store = ThemeStore::new(app_data_dir);
    log::info!("terminal theme: selecting '{name}' (dark appearance: {system_is_dark})");
    let Some(theme) = store.get(name) else {
        log::warn!("terminal theme '{name}' not found; keeping the current palette");
        return;
    };
    if let Err(error) = lingxia_terminal::terminal_set_theme_all(&theme) {
        log::warn!("terminal theme '{name}' rejected: {error}");
    }
}

/// The configuration in effect, as JSON for hosts crossing an FFI boundary.
pub fn current_json() -> String {
    current()
        .lock()
        .ok()
        .and_then(|config| serde_json::to_string(&*config).ok())
        .unwrap_or_else(|| "{}".to_string())
}
