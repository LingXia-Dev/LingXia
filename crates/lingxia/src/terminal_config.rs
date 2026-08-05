//! Terminal configuration applied to the running engine and hosts.
//!
//! The configuration layer owns schema and storage; this is the seam that
//! turns a loaded configuration into effect: themes go straight into the
//! engine (a repaint, since cell colors resolve at frame time), while font
//! settings are handed to the platform renderer, which is the only side that
//! knows what is installed and how to measure it.

use lingxia_terminal_config::{ConfigWatcher, TerminalConfig, ThemeStore};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

/// Bumped whenever the configuration in effect changes, so hosts can notice
/// with one atomic read on a poll they already run.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// The generation of the configuration in effect.
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

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
    let result = lingxia_terminal_config::watch(watcher, move |config| {
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

fn publish(config: TerminalConfig) {
    if let Ok(mut slot) = current().lock() {
        *slot = config;
    }
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

/// The configuration in effect, as JSON for hosts crossing an FFI boundary.
pub fn current_json() -> String {
    current()
        .lock()
        .ok()
        .and_then(|config| serde_json::to_string(&*config).ok())
        .unwrap_or_else(|| "{}".to_string())
}

/// Run the `term` CLI if this process was invoked as one.
///
/// Hosts call this from `main` **before** touching any UI framework: the
/// product's executable doubles as its command line, and a configuration
/// command must not open a window. Returns the exit code when it handled the
/// invocation, `None` when the process should carry on and become the app.
pub fn run_cli_if_invoked(app_data_dir: PathBuf) -> Option<i32> {
    use std::io::IsTerminal;

    let mut args = std::env::args();
    let executable = args.next().unwrap_or_default();
    let command = std::path::Path::new(&executable)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("app")
        .to_string();

    let first = args.next();
    // No arguments from a terminal means someone typed the command and wants
    // to know what it does. No arguments without one means the OS launched the
    // bundle, which is the app starting normally.
    let from_terminal = std::io::stdout().is_terminal() || std::io::stdin().is_terminal();
    if first.is_none() && from_terminal {
        println!(
            "{}",
            lingxia_terminal_config::cli::run(&app_data_dir, &command, &[]).text
        );
        return Some(0);
    }
    if first.as_deref() != Some("term") {
        return None;
    }

    let rest: Vec<String> = std::env::args().skip(2).collect();
    let output = lingxia_terminal_config::cli::run(&app_data_dir, &command, &rest);
    if output.code == 0 {
        println!("{}", output.text);
    } else {
        eprintln!("{}", output.text);
    }
    Some(output.code)
}

/// Make the product's command typable: write the launcher and return the
/// environment a spawned session needs to find it.
///
/// The executable lives inside an application bundle, which is neither on
/// `PATH` nor pleasant to type, so sessions we spawn get a launcher directory
/// prepended and the executable's own path, which is what tells a program
/// running inside the terminal that it has one to talk to.
pub fn session_environment(app_data_dir: &std::path::Path) -> Vec<(String, String)> {
    let mut environment = Vec::new();
    match lingxia_terminal_config::cli::install_launcher(app_data_dir) {
        Ok(launcher) => {
            log::info!("terminal command: {}", launcher.display());
            environment.push((
                "LINGXIA_TERMINAL_CLI".to_string(),
                launcher.to_string_lossy().into_owned(),
            ));
            let directory = lingxia_terminal_config::cli::bin_dir(app_data_dir);
            let path = std::env::var("PATH").unwrap_or_default();
            environment.push((
                "PATH".to_string(),
                format!("{}:{path}", directory.to_string_lossy()),
            ));
        }
        Err(error) => log::warn!("terminal command launcher not installed: {error}"),
    }
    environment
}

/// Publish the platform's installed families, so `term font --list` and
/// `term status` report what is really available.
pub fn set_installed_fonts(fonts: Vec<lingxia_terminal_config::InstalledFont>) {
    lingxia_terminal_config::cli::set_installed_fonts(fonts);
}
