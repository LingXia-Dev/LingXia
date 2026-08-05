//! The `term` subcommand.
//!
//! Configuration's primary entry point is a CLI, not a settings window:
//! terminal users can drive one, it scripts, it goes in dotfiles, and its
//! interactive mode previews inside the terminal itself without any window
//! management. The product's own executable carries it (`myapp term …`), so
//! the command always knows which app it belongs to.
//!
//! Arguments are parsed by hand rather than with a parser crate: the surface
//! is a handful of flags, and a runtime crate should not carry an argument
//! parser for it.

use crate::{TerminalConfig, ThemeStore};
use std::path::{Path, PathBuf};

/// Installed families, supplied by the platform: enumerating them is platform
/// work the shared layer cannot do.
static INSTALLED_FONTS: std::sync::Mutex<Vec<crate::InstalledFont>> =
    std::sync::Mutex::new(Vec::new());

/// Publish what is installed, so `font --list` and `status` report reality.
pub fn set_installed_fonts(fonts: Vec<crate::InstalledFont>) {
    if let Ok(mut slot) = INSTALLED_FONTS.lock() {
        *slot = fonts;
    }
}

fn installed_fonts() -> Vec<crate::InstalledFont> {
    INSTALLED_FONTS
        .lock()
        .map(|fonts| fonts.clone())
        .unwrap_or_default()
}

/// What the CLI produced: text for a human, and the process exit code.
pub struct Output {
    pub text: String,
    pub code: i32,
}

impl Output {
    fn ok(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            code: 0,
        }
    }

    fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            code: 1,
        }
    }
}

/// Run `term …`. `args` excludes the executable and the `term` word itself.
pub fn run(app_data_dir: &Path, command: &str, args: &[String]) -> Output {
    let json = args.iter().any(|arg| arg == "--json");
    match args.first().map(String::as_str) {
        None | Some("--help") | Some("-h") => Output::ok(help(command)),
        Some("status") => status(app_data_dir, json),
        Some("theme") => theme(app_data_dir, &args[1..], json),
        Some("font") => font(app_data_dir, &args[1..], json),
        Some("config") => config(app_data_dir, &args[1..], json),
        Some(other) => Output::error(format!("unknown command '{other}'\n\n{}", help(command))),
    }
}

fn help(command: &str) -> String {
    format!(
        "\
{command} term — terminal configuration

  theme <name>              use a color scheme
  theme --list              schemes available, built-in and imported
  font --family <name>      set the font, first installed candidate wins
  font --size <points>      set the font size
  font --list               installed monospace families
  config --path             where the configuration lives
  status                    what is in effect right now

Add --json to any command for machine-readable output.
Changes are written to the configuration file; a running app applies them
immediately, otherwise they take effect at next launch."
    )
}

fn status(app_data_dir: &Path, json: bool) -> Output {
    let (config, error) = TerminalConfig::load(app_data_dir, &serde_json::Value::Null);
    let path = TerminalConfig::path(app_data_dir);
    let resolved = crate::resolve_font(&config.font, &installed_fonts());

    if json {
        let payload = serde_json::json!({
            "path": path,
            "exists": path.exists(),
            "config": config,
            "font": resolved,
            "error": error.map(|error| error.to_string()),
        });
        return Output::ok(payload.to_string());
    }

    let mut text = format!("config    {}\n", path.display());
    if let Some(error) = error {
        text.push_str(&format!("          {error}\n"));
    }
    text.push_str(&format!(
        "font      {} {}pt{}\n",
        if resolved.family.is_empty() {
            "(none resolved)"
        } else {
            &resolved.family
        },
        config.font.size,
        if resolved.fell_back {
            " — none of the configured families is installed"
        } else {
            ""
        }
    ));
    if !resolved.missing.is_empty() {
        text.push_str(&format!(
            "          not installed: {}\n",
            resolved.missing.join(", ")
        ));
    }
    text.push_str(&format!(
        "theme     {:?}, light={} dark={}\n",
        config.theme.mode, config.theme.light, config.theme.dark
    ));
    Output::ok(text)
}

fn theme(app_data_dir: &Path, args: &[String], json: bool) -> Output {
    let store = ThemeStore::new(app_data_dir);
    if args.iter().any(|arg| arg == "--list") || args.is_empty() {
        let entries = store.list();
        if json {
            return Output::ok(
                serde_json::to_string(&entries).unwrap_or_else(|_| "[]".to_string()),
            );
        }
        let text = entries
            .iter()
            .map(|entry| format!("{:<24} {:?}", entry.name, entry.source))
            .collect::<Vec<_>>()
            .join("\n");
        return Output::ok(text);
    }

    let name = &args[0];
    if store.get(name).is_none() {
        return Output::error(format!(
            "no theme named '{name}'; `term theme --list` shows what is available"
        ));
    }

    let (mut config, _) = TerminalConfig::load(app_data_dir, &serde_json::Value::Null);
    // Pin both appearances: choosing a scheme by name means wanting that
    // scheme, not one of two depending on the time of day.
    config.theme.light = name.clone();
    config.theme.dark = name.clone();
    apply(app_data_dir, &config, json, &format!("theme = {name}"))
}

fn font(app_data_dir: &Path, args: &[String], json: bool) -> Output {
    if args.iter().any(|arg| arg == "--list") {
        let installed = installed_fonts();
        if json {
            return Output::ok(
                serde_json::to_string(&installed).unwrap_or_else(|_| "[]".to_string()),
            );
        }
        if installed.is_empty() {
            return Output::error("font listing is unavailable in this context");
        }
        let text = installed
            .iter()
            .map(|font| {
                format!(
                    "{:<32} {:<12} {}",
                    font.family,
                    if font.ligatures { "ligatures" } else { "" },
                    if font.nerd_icons { "nerd icons" } else { "" }
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        return Output::ok(text);
    }

    let (mut config, _) = TerminalConfig::load(app_data_dir, &serde_json::Value::Null);
    let mut changed = Vec::new();
    if let Some(family) = value_of(args, "--family") {
        config.font.family = vec![family.clone()];
        changed.push(format!("family = {family}"));
    }
    if let Some(size) = value_of(args, "--size") {
        match size.parse::<f32>() {
            Ok(size) if (4.0..=96.0).contains(&size) => {
                config.font.size = size;
                changed.push(format!("size = {size}"));
            }
            _ => return Output::error(format!("font size '{size}' is not between 4 and 96")),
        }
    }
    if let Some(value) = value_of(args, "--ligatures") {
        match value.as_str() {
            "on" | "true" => {
                config.font.ligatures = true;
                changed.push("ligatures = on".to_string());
            }
            "off" | "false" => {
                config.font.ligatures = false;
                changed.push("ligatures = off".to_string());
            }
            other => return Output::error(format!("--ligatures takes on or off, got '{other}'")),
        }
    }
    if changed.is_empty() {
        return Output::error("nothing to change; see `term font --help`");
    }
    apply(app_data_dir, &config, json, &changed.join(", "))
}

fn config(app_data_dir: &Path, args: &[String], json: bool) -> Output {
    let path = TerminalConfig::path(app_data_dir);
    if json {
        return Output::ok(
            serde_json::json!({ "path": path, "exists": path.exists() }).to_string(),
        );
    }
    let _ = args;
    Output::ok(path.display().to_string())
}

/// Persist a change and report it, including whether it is live.
///
/// The file is written first and is the source of truth; telling the running
/// app is a notification. So the command succeeds whether or not an app is
/// running — only `live` differs.
fn apply(app_data_dir: &Path, config: &TerminalConfig, json: bool, summary: &str) -> Output {
    if let Err(error) = config.save(app_data_dir) {
        return Output::error(error.to_string());
    }
    let live = notify(app_data_dir);
    if json {
        return Output::ok(
            serde_json::json!({ "applied": "file", "live": live, "change": summary }).to_string(),
        );
    }
    Output::ok(if live {
        summary.to_string()
    } else {
        format!("{summary} — takes effect at next launch (no running app to notify)")
    })
}

/// Tell a running instance to reload. Not yet implemented; the file is
/// already written, so callers correctly report "next launch".
fn notify(_app_data_dir: &Path) -> bool {
    false
}

fn value_of(args: &[String], flag: &str) -> Option<String> {
    let index = args.iter().position(|arg| arg == flag)?;
    args.get(index + 1)
        .filter(|value| !value.starts_with("--"))
        .cloned()
}

/// Where a host should look for the configuration when running as a CLI.
pub fn config_path(app_data_dir: &Path) -> PathBuf {
    TerminalConfig::path(app_data_dir)
}

/// The command name a user types, derived from the product name.
///
/// Lowercased and stripped to what a shell takes without quoting: the name
/// exists to be typed, and a product called "My Term" must not require
/// escaping.
pub fn command_name(product_name: &str) -> String {
    let name: String = product_name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let name = name.trim_matches('-').replace("--", "-");
    if name.is_empty() {
        "app".to_string()
    } else {
        name
    }
}

/// Directory holding the launcher, added to `PATH` for sessions we spawn.
pub fn bin_dir(app_data_dir: &Path) -> PathBuf {
    lingxia_app_context::app_state_file(app_data_dir, "bin")
}

/// Write a launcher for the product's executable so the command is typable.
///
/// The real binary lives inside an application bundle, which is neither on
/// `PATH` nor pleasant to type. The launcher is generated at runtime rather
/// than at build time so it always points at the executable actually running
/// — a development build moves, a release does not.
pub fn install_launcher(app_data_dir: &Path, product_name: &str) -> std::io::Result<PathBuf> {
    let executable = std::env::current_exe()?;
    let directory = bin_dir(app_data_dir);
    std::fs::create_dir_all(&directory)?;
    let path = directory.join(command_name(product_name));
    let script = format!(
        "#!/bin/sh\n# Generated by LingXia; points at the running executable.\nexec {} \"$@\"\n",
        shell_quote(&executable.to_string_lossy())
    );
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    if existing != script {
        std::fs::write(&path, &script)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
        }
    }
    Ok(path)
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn setting_a_theme_writes_it_and_pins_both_appearances() {
        let dir = tempfile::tempdir().expect("temp dir");
        let output = run(dir.path(), "myapp", &args(&["theme", "lingxia-light"]));
        assert_eq!(output.code, 0, "{}", output.text);

        let (config, error) = TerminalConfig::load(dir.path(), &serde_json::Value::Null);
        assert!(error.is_none());
        assert_eq!(config.theme.light, "lingxia-light");
        assert_eq!(
            config.theme.dark, "lingxia-light",
            "naming a scheme means that scheme, not one of two"
        );
    }

    #[test]
    fn an_unknown_theme_fails_without_writing() {
        let dir = tempfile::tempdir().expect("temp dir");
        let output = run(dir.path(), "myapp", &args(&["theme", "no-such-scheme"]));
        assert_eq!(output.code, 1);
        assert!(output.text.contains("--list"), "{}", output.text);
        assert!(
            !TerminalConfig::path(dir.path()).exists(),
            "a rejected command writes nothing"
        );
    }

    #[test]
    fn font_changes_are_validated_before_they_are_written() {
        let dir = tempfile::tempdir().expect("temp dir");
        let rejected = run(dir.path(), "myapp", &args(&["font", "--size", "900"]));
        assert_eq!(rejected.code, 1);
        assert!(!TerminalConfig::path(dir.path()).exists());

        let accepted = run(
            dir.path(),
            "myapp",
            &args(&["font", "--family", "Iosevka", "--size", "15"]),
        );
        assert_eq!(accepted.code, 0, "{}", accepted.text);
        let (config, _) = TerminalConfig::load(dir.path(), &serde_json::Value::Null);
        assert_eq!(config.font.family, vec!["Iosevka".to_string()]);
        assert_eq!(config.font.size, 15.0);
    }

    #[test]
    fn a_change_reports_that_it_is_not_live_without_a_running_app() {
        let dir = tempfile::tempdir().expect("temp dir");
        let output = run(
            dir.path(),
            "myapp",
            &args(&["font", "--size", "14", "--json"]),
        );
        let payload: serde_json::Value = serde_json::from_str(&output.text).expect("json output");
        assert_eq!(payload["applied"], "file");
        assert_eq!(
            payload["live"], false,
            "the file is written either way; only liveness differs"
        );
    }

    #[test]
    fn status_reports_the_path_and_that_no_font_resolved() {
        let dir = tempfile::tempdir().expect("temp dir");
        let output = run(dir.path(), "myapp", &args(&["status", "--json"]));
        let payload: serde_json::Value = serde_json::from_str(&output.text).expect("json output");
        assert_eq!(payload["exists"], false);
        assert_eq!(
            payload["font"]["fellBack"], true,
            "no platform lister is registered in a test process"
        );
    }

    #[test]
    fn help_names_the_products_own_command() {
        let dir = tempfile::tempdir().expect("temp dir");
        let output = run(dir.path(), "myterm", &[]);
        assert!(
            output.text.starts_with("myterm term"),
            "help must be copy-pasteable: {}",
            output.text
        );
    }
}
