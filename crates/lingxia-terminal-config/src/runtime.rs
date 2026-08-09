//! Configuration in effect for the running app.
//!
//! Loading and applying live here rather than in each platform SDK:
//! it is pure data work with no platform content, and both hosts need exactly
//! the same behaviour. The engine gets the theme — colors resolve when a frame
//! is drawn, so a theme change is a repaint — and the host is told to re-read
//! the font, which only it can resolve against what is installed.

use crate::{InstalledFont, SurfaceChrome, TerminalConfig, ThemeStore};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

/// The configuration in effect, so hosts can read it after startup without
/// touching the filesystem again.
static CURRENT: OnceLock<Mutex<TerminalConfig>> = OnceLock::new();

/// Bumped whenever the configuration in effect changes, so hosts notice with
/// one atomic read on a poll they already run.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// The generation of the configuration in effect.
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

fn fonts() -> &'static Mutex<Vec<InstalledFont>> {
    static INSTALLED: OnceLock<Mutex<Vec<InstalledFont>>> = OnceLock::new();
    INSTALLED.get_or_init(|| Mutex::new(Vec::new()))
}

/// Publish the platform's installed terminal families.
pub fn set_installed_fonts(installed: Vec<InstalledFont>) {
    if let Ok(mut slot) = fonts().lock() {
        *slot = installed;
    }
}

/// The last installed-font snapshot supplied by the platform SDK.
pub fn installed_fonts() -> Vec<InstalledFont> {
    fonts()
        .lock()
        .map(|installed| installed.clone())
        .unwrap_or_default()
}

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
    publish(config.clone());
    config
}

/// A generation means "this changed", not "this was read".
///
/// Hosts re-read on a moved generation, and re-reading goes through `load`,
/// which publishes — so bumping unconditionally makes the two chase each
/// other: the Apple host reloaded the file and re-enumerated every installed
/// font four times a second, forever.
fn publish(config: TerminalConfig) {
    let Ok(mut slot) = current().lock() else {
        return;
    };
    if *slot == config {
        return;
    }
    *slot = config;
    GENERATION.fetch_add(1, Ordering::Relaxed);
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
        return;
    }
    if let Ok(mut slot) = chrome().lock() {
        *slot = SurfaceChrome::derive(&theme);
    }
}

fn chrome() -> &'static Mutex<SurfaceChrome> {
    static CHROME: OnceLock<Mutex<SurfaceChrome>> = OnceLock::new();
    CHROME.get_or_init(|| Mutex::new(SurfaceChrome::default()))
}

/// Chrome colors for the surface hosting a terminal, from the theme in effect.
///
/// Resolved once here rather than in each platform SDK: the rule is the same
/// on both, and a second copy of it drifts the moment one is edited.
pub fn current_chrome() -> SurfaceChrome {
    chrome()
        .lock()
        .map(|chrome| *chrome)
        .unwrap_or_else(|_| SurfaceChrome::default())
}

/// `current_chrome` as `#rrggbb` strings, for hosts reached over FFI.
pub fn current_chrome_json() -> String {
    let chrome = current_chrome();
    let hex = |color: u32| format!("#{color:06x}");
    serde_json::json!({
        "surface": hex(chrome.surface),
        "header": hex(chrome.header),
        "separator": hex(chrome.separator),
        "text": hex(chrome.text),
        "textMuted": hex(chrome.text_muted),
    })
    .to_string()
}

/// The configuration in effect.
pub fn current_config() -> TerminalConfig {
    current()
        .lock()
        .map(|config| config.clone())
        .unwrap_or_default()
}

/// The configuration in effect, as JSON for hosts crossing an FFI boundary.
pub fn current_json() -> String {
    current()
        .lock()
        .ok()
        .and_then(|config| serde_json::to_string(&*config).ok())
        .unwrap_or_else(|| "{}".to_string())
}
