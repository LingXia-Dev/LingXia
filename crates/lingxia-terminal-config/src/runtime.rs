//! Configuration in effect for the running app.
//!
//! Loading and applying live here rather than in each platform SDK:
//! it is pure data work with no platform content, and both hosts need exactly
//! the same behaviour. The engine gets the theme — colors resolve when a frame
//! is drawn, so a theme change is a repaint — and the host is told to re-read
//! the font, which only it can resolve against what is installed.

use crate::{InstalledFont, SurfaceChrome, TerminalConfig, ThemeStore};
use lingxia_terminal::TerminalTheme;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

/// The configuration in effect, so hosts can read it after startup without
/// touching the filesystem again.
static CURRENT: OnceLock<Mutex<TerminalConfig>> = OnceLock::new();

/// Bumped whenever the configuration in effect changes, so hosts notice with
/// one atomic read on a poll they already run.
static GENERATION: AtomicU64 = AtomicU64::new(0);

fn mutations() -> &'static Mutex<()> {
    static MUTATIONS: OnceLock<Mutex<()>> = OnceLock::new();
    MUTATIONS.get_or_init(|| Mutex::new(()))
}

#[derive(Debug)]
pub enum MutationError {
    Config(crate::ConfigError),
    RevisionConflict { expected: u64, actual: u64 },
}

impl std::fmt::Display for MutationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config(error) => error.fmt(f),
            Self::RevisionConflict { expected, actual } => write!(
                f,
                "terminal settings revision conflict: expected {expected}, current revision is {actual}"
            ),
        }
    }
}

impl std::error::Error for MutationError {}

impl From<crate::ConfigError> for MutationError {
    fn from(error: crate::ConfigError) -> Self {
        Self::Config(error)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MutationResult {
    pub config: TerminalConfig,
    pub revision: u64,
}

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
    let _guard = mutations()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let defaults = serde_json::from_str::<serde_json::Value>(product_defaults)
        .unwrap_or(serde_json::Value::Null);
    load_locked(app_data_dir, &defaults, system_is_dark)
}

fn load_locked(
    app_data_dir: PathBuf,
    defaults: &serde_json::Value,
    system_is_dark: bool,
) -> TerminalConfig {
    let path = TerminalConfig::path(&app_data_dir);
    let (config, error) = TerminalConfig::load(&app_data_dir, defaults);
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
    apply_theme_locked(&app_data_dir, &config, system_is_dark);
    publish(config.clone());
    config
}

/// Persist and publish a partial configuration update.
pub fn apply_config(
    app_data_dir: &std::path::Path,
    product_defaults: &serde_json::Value,
    overlay: &serde_json::Value,
    system_is_dark: bool,
) -> Result<TerminalConfig, crate::ConfigError> {
    let _guard = mutations()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    apply_config_locked(app_data_dir, product_defaults, overlay, system_is_dark)
}

pub fn apply_config_if_revision(
    app_data_dir: &std::path::Path,
    product_defaults: &serde_json::Value,
    overlay: &serde_json::Value,
    expected_revision: u64,
    system_is_dark: bool,
) -> Result<MutationResult, MutationError> {
    let _guard = mutations()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    require_revision(expected_revision)?;
    let config = apply_config_locked(app_data_dir, product_defaults, overlay, system_is_dark)?;
    Ok(MutationResult {
        config,
        revision: generation(),
    })
}

fn apply_config_locked(
    app_data_dir: &std::path::Path,
    product_defaults: &serde_json::Value,
    overlay: &serde_json::Value,
    system_is_dark: bool,
) -> Result<TerminalConfig, crate::ConfigError> {
    let (current, error) = TerminalConfig::load(app_data_dir, product_defaults);
    if let Some(error) = error {
        log::warn!("{error}; applying the update over resolved defaults");
    }
    let next = current
        .with_overlay(overlay)
        .map_err(|reason| crate::ConfigError::Invalid {
            path: TerminalConfig::path(app_data_dir),
            reason,
        })?;
    validate_overlay_theme_names(app_data_dir, overlay)?;
    save_and_publish(app_data_dir, product_defaults, next, system_is_dark)
}

fn validate_overlay_theme_names(
    app_data_dir: &std::path::Path,
    overlay: &serde_json::Value,
) -> Result<(), crate::ConfigError> {
    let Some(theme) = overlay.get("theme").and_then(serde_json::Value::as_object) else {
        return Ok(());
    };
    let store = ThemeStore::new(app_data_dir);
    for field in ["light", "dark"] {
        let Some(name) = theme.get(field).and_then(serde_json::Value::as_str) else {
            continue;
        };
        if store.get(name).is_none() {
            return Err(crate::ConfigError::Invalid {
                path: TerminalConfig::path(app_data_dir),
                reason: format!("theme.{field} names an unknown color scheme '{name}'"),
            });
        }
    }
    Ok(())
}

/// Remove all user overrides, or only the requested section.
pub fn reset_config(
    app_data_dir: &std::path::Path,
    product_defaults: &serde_json::Value,
    scope: Option<&str>,
    system_is_dark: bool,
) -> Result<TerminalConfig, crate::ConfigError> {
    let _guard = mutations()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    reset_config_locked(app_data_dir, product_defaults, scope, system_is_dark)
}

pub fn reset_config_if_revision(
    app_data_dir: &std::path::Path,
    product_defaults: &serde_json::Value,
    scope: Option<&str>,
    expected_revision: u64,
    system_is_dark: bool,
) -> Result<MutationResult, MutationError> {
    let _guard = mutations()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    require_revision(expected_revision)?;
    let config = reset_config_locked(app_data_dir, product_defaults, scope, system_is_dark)?;
    Ok(MutationResult {
        config,
        revision: generation(),
    })
}

fn reset_config_locked(
    app_data_dir: &std::path::Path,
    product_defaults: &serde_json::Value,
    scope: Option<&str>,
    system_is_dark: bool,
) -> Result<TerminalConfig, crate::ConfigError> {
    let defaults = TerminalConfig::from_defaults(product_defaults);
    let (mut next, _) = TerminalConfig::load(app_data_dir, product_defaults);
    match scope {
        None => next = defaults,
        Some("font") => next.font = defaults.font,
        Some("theme") => next.theme = defaults.theme,
        Some(other) => {
            return Err(crate::ConfigError::Invalid {
                path: TerminalConfig::path(app_data_dir),
                reason: format!("reset scope must be font or theme, got '{other}'"),
            });
        }
    }
    save_and_publish(app_data_dir, product_defaults, next, system_is_dark)
}

fn save_and_publish(
    app_data_dir: &std::path::Path,
    product_defaults: &serde_json::Value,
    config: TerminalConfig,
    system_is_dark: bool,
) -> Result<TerminalConfig, crate::ConfigError> {
    config.save(app_data_dir, product_defaults)?;
    apply_theme_locked(app_data_dir, &config, system_is_dark);
    publish(config.clone());
    Ok(config)
}

fn require_revision(expected: u64) -> Result<(), MutationError> {
    let actual = generation();
    if expected == actual {
        Ok(())
    } else {
        Err(MutationError::RevisionConflict { expected, actual })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemePreviewLease(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemePreviewRequest {
    lease: ThemePreviewLease,
    order: u64,
}

#[derive(Debug, Default)]
struct PreviewState {
    latest_request: u64,
    active: Option<ThemePreviewLease>,
    latest_by_lease: HashMap<u64, u64>,
}

impl PreviewState {
    fn begin_show(&mut self, request: ThemePreviewRequest) -> bool {
        let lease_request = self.latest_by_lease.entry(request.lease.0).or_default();
        if request.order <= *lease_request || request.order <= self.latest_request {
            return false;
        }
        *lease_request = request.order;
        self.latest_request = request.order;
        true
    }

    fn begin_clear(&mut self, request: ThemePreviewRequest) -> bool {
        let lease_request = self.latest_by_lease.entry(request.lease.0).or_default();
        if request.order <= *lease_request {
            return false;
        }
        *lease_request = request.order;
        if request.order <= self.latest_request || self.active != Some(request.lease) {
            return false;
        }
        self.latest_request = request.order;
        self.active = None;
        true
    }
}

fn previews() -> &'static Mutex<PreviewState> {
    static PREVIEWS: OnceLock<Mutex<PreviewState>> = OnceLock::new();
    PREVIEWS.get_or_init(|| Mutex::new(PreviewState::default()))
}

static NEXT_PREVIEW_LEASE: AtomicU64 = AtomicU64::new(1);
static NEXT_PREVIEW_REQUEST: AtomicU64 = AtomicU64::new(1);

pub fn create_theme_preview_lease() -> ThemePreviewLease {
    ThemePreviewLease(NEXT_PREVIEW_LEASE.fetch_add(1, Ordering::Relaxed))
}

pub fn create_theme_preview_request(lease: ThemePreviewLease) -> ThemePreviewRequest {
    ThemePreviewRequest {
        lease,
        order: NEXT_PREVIEW_REQUEST.fetch_add(1, Ordering::Relaxed),
    }
}

pub fn preview_theme_for_request(
    request: ThemePreviewRequest,
    theme: &TerminalTheme,
) -> Result<(), String> {
    let mut state = previews().lock().unwrap_or_else(|error| error.into_inner());
    if !state.begin_show(request) {
        return Ok(());
    }
    lingxia_terminal::terminal_set_theme_all(theme).map_err(|error| error.to_string())?;
    if let Ok(mut slot) = chrome().lock() {
        *slot = SurfaceChrome::derive(theme);
    }
    state.active = Some(request.lease);
    Ok(())
}

pub fn end_theme_preview_for_request(
    request: ThemePreviewRequest,
    app_data_dir: &std::path::Path,
    system_is_dark: bool,
) {
    let mut state = previews().lock().unwrap_or_else(|error| error.into_inner());
    if !state.begin_clear(request) {
        return;
    }
    apply_theme_unlocked(app_data_dir, &current_config(), system_is_dark);
}

fn legacy_preview_lease() -> ThemePreviewLease {
    static LEGACY: OnceLock<ThemePreviewLease> = OnceLock::new();
    *LEGACY.get_or_init(create_theme_preview_lease)
}

/// Preview a scheme without persisting the selection.
pub fn preview_theme(theme: &TerminalTheme) -> Result<(), String> {
    let lease = legacy_preview_lease();
    preview_theme_for_request(create_theme_preview_request(lease), theme)
}

/// End a preview by restoring the saved selection.
pub fn end_theme_preview(app_data_dir: &std::path::Path, system_is_dark: bool) {
    let lease = legacy_preview_lease();
    end_theme_preview_for_request(
        create_theme_preview_request(lease),
        app_data_dir,
        system_is_dark,
    );
}

/// A generation means "this changed", not "this was read".
///
/// Hosts re-read on a moved generation, and re-reading goes through `load`,
/// which publishes — so bumping unconditionally makes the two chase each
/// other: the Apple host reloaded the file and re-enumerated every installed
/// font four times a second, forever.
fn publish(config: TerminalConfig) -> u64 {
    let Ok(mut slot) = current().lock() else {
        return generation();
    };
    if *slot == config {
        return generation();
    }
    *slot = config;
    GENERATION.fetch_add(1, Ordering::Relaxed) + 1
}

/// Push the configured theme into the engine. Cell colors are resolved when a
/// frame is built, so this is a repaint of every live session — no reflow and
/// no respawn.
pub fn apply_theme(app_data_dir: &std::path::Path, config: &TerminalConfig, system_is_dark: bool) {
    let _guard = mutations()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    apply_theme_locked(app_data_dir, config, system_is_dark);
}

fn apply_theme_locked(
    app_data_dir: &std::path::Path,
    config: &TerminalConfig,
    system_is_dark: bool,
) {
    let mut state = previews().lock().unwrap_or_else(|error| error.into_inner());
    state.latest_request = NEXT_PREVIEW_REQUEST.fetch_add(1, Ordering::Relaxed);
    state.active = None;
    apply_theme_unlocked(app_data_dir, config, system_is_dark);
}

fn apply_theme_unlocked(
    app_data_dir: &std::path::Path,
    config: &TerminalConfig,
    system_is_dark: bool,
) {
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

pub fn current_snapshot() -> (TerminalConfig, u64) {
    let slot = current().lock().unwrap_or_else(|error| error.into_inner());
    (slot.clone(), generation())
}

/// The configuration in effect, as JSON for hosts crossing an FFI boundary.
pub fn current_json() -> String {
    current()
        .lock()
        .ok()
        .and_then(|config| serde_json::to_string(&*config).ok())
        .unwrap_or_else(|| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_revision_cannot_overwrite_a_newer_update() {
        let dir = tempfile::tempdir().expect("temp dir");
        let defaults = serde_json::json!({});
        let _ = load(dir.path().to_path_buf(), "{}", false);
        let before = generation();

        let committed = apply_config_if_revision(
            dir.path(),
            &defaults,
            &serde_json::json!({"font": {"size": 15.0}}),
            before,
            false,
        )
        .expect("first update");
        let error = apply_config_if_revision(
            dir.path(),
            &defaults,
            &serde_json::json!({"font": {"size": 18.0}}),
            before,
            false,
        )
        .expect_err("stale update");

        assert!(matches!(
            error,
            MutationError::RevisionConflict {
                expected,
                actual
            } if expected == before && actual == committed.revision
        ));
        let (saved, load_error) = TerminalConfig::load(dir.path(), &defaults);
        assert!(load_error.is_none());
        assert_eq!(saved.font.size, 15.0);
    }

    #[test]
    fn preview_requests_ignore_stale_show_and_foreign_clear() {
        let first = ThemePreviewLease(1);
        let second = ThemePreviewLease(2);
        let stale_show = ThemePreviewRequest {
            lease: first,
            order: 2,
        };
        let second_show = ThemePreviewRequest {
            lease: second,
            order: 3,
        };
        let foreign_clear = ThemePreviewRequest {
            lease: first,
            order: 4,
        };
        let mut state = PreviewState {
            latest_request: 1,
            active: Some(first),
            latest_by_lease: HashMap::from([(first.0, 1)]),
        };

        assert!(state.begin_show(second_show));
        state.active = Some(second);
        assert!(!state.begin_show(stale_show));
        assert!(!state.begin_clear(foreign_clear));
        assert_eq!(state.active, Some(second));

        let older_first_show = ThemePreviewRequest {
            lease: first,
            order: 3,
        };
        assert!(!state.begin_show(older_first_show));
    }

    #[test]
    fn updates_reject_unknown_color_scheme_names() {
        let dir = tempfile::tempdir().expect("temp dir");
        let error = apply_config(
            dir.path(),
            &serde_json::json!({}),
            &serde_json::json!({"theme": {"dark": "does-not-exist"}}),
            false,
        )
        .expect_err("unknown scheme");
        assert!(error.to_string().contains("theme.dark"), "{error}");
        assert!(!TerminalConfig::path(dir.path()).exists());
    }
}
