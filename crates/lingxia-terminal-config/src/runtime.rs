//! Configuration in effect for the running app.
//!
//! Loading, watching and applying live here rather than in each platform SDK:
//! it is pure data work with no platform content, and both hosts need exactly
//! the same behaviour. The engine gets the theme — colors resolve when a frame
//! is drawn, so a theme change is a repaint — and the host is told to re-read
//! the font, which only it can resolve against what is installed.

use crate::{ConfigWatcher, TerminalConfig, ThemeStore};
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

/// Publish the platform's installed families, so `term font list` and
/// `term status` report what is really available.
pub fn set_installed_fonts(fonts: Vec<crate::InstalledFont>) {
    crate::cli::set_installed_fonts(fonts);
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
    start_watching(app_data_dir, defaults, config.clone(), system_is_dark);
    config
}

/// Adopt saved changes as they happen.
///
/// Watching the file rather than having the CLI announce its own writes covers
/// every way it can change — an editor, a dotfile manager, the CLI — with one
/// mechanism, and leaves the CLI as nothing more than a validating editor of
/// the file.
fn start_watching(
    app_data_dir: PathBuf,
    product_defaults: serde_json::Value,
    current: TerminalConfig,
    system_is_dark: bool,
) {
    if WATCHED.set(app_data_dir.clone()).is_err() {
        return;
    }
    let directory = app_data_dir.clone();
    let watcher = ConfigWatcher::new(app_data_dir, product_defaults, current);
    let result = crate::watch(watcher, move |config| {
        log::info!(
            "terminal config reloaded: font {:?} {}pt, theme mode {:?}",
            config.font.family,
            config.font.size,
            config.theme.mode
        );
        apply_theme(&directory, &config, system_is_dark);
        publish(config);
    });
    if let Err(error) = result {
        log::warn!("terminal config changes will not be picked up: {error}");
    }
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

/// The directory whose changes are watched, for diagnostics.
pub fn watched_directory() -> Option<PathBuf> {
    WATCHED.get().cloned()
}

static WATCHED: OnceLock<PathBuf> = OnceLock::new();

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
