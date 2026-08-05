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
        Some("reset") => reset(app_data_dir, &args[1..], json),
        Some(other) => Output::error(format!("unknown command '{other}'\n\n{}", help(command))),
    }
}

/// Report an invocation this binary does not recognize.
///
/// Separate from `run` because it must answer without a configuration
/// directory: it is reached before the app exists, when someone typed
/// something wrong.
pub fn unknown(command: &str, arguments: &[String]) -> Output {
    let first = arguments.first().map(String::as_str).unwrap_or("");
    Output::error(format!("unknown command '{first}'\n\n{}", help(command)))
}

fn help(command: &str) -> String {
    format!(
        "\
{command} term — terminal configuration

  theme <name>          use a color scheme
  theme --list          available schemes
  font --family <name>  set the font
  font --size <points>  set the font size
  font --list           installed monospace families
  config --path         configuration file location
  reset [font|theme]    back to defaults
  status                what is in effect

  --json                machine-readable output"
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
    for flag in ["--family", "--size", "--ligatures"] {
        if args.iter().any(|arg| arg == flag) && value_of(args, flag).is_none() {
            return Output::error(format!("{flag} needs a value"));
        }
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

/// Back to defaults — the way out when a change made the terminal unusable,
/// which a font size can do all by itself.
fn reset(app_data_dir: &Path, args: &[String], json: bool) -> Output {
    let (mut config, _) = TerminalConfig::load(app_data_dir, &serde_json::Value::Null);
    let summary = match args.first().map(String::as_str) {
        None | Some("--json") => {
            config = TerminalConfig::default();
            "everything"
        }
        Some("font") => {
            config.font = crate::FontConfig::default();
            "font"
        }
        Some("theme") => {
            config.theme = crate::ThemeConfig::default();
            "theme"
        }
        Some(other) => {
            return Output::error(format!("reset takes font or theme, got '{other}'"));
        }
    };
    apply(app_data_dir, &config, json, &format!("reset {summary}"))
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

/// Persist a change and report it.
///
/// Writing the file is the whole operation: a running app watches it and
/// adopts the change, so there is nothing to notify and no claim to make
/// about whether one is running.
fn apply(app_data_dir: &Path, config: &TerminalConfig, json: bool, summary: &str) -> Output {
    if let Err(error) = config.save(app_data_dir) {
        return Output::error(error.to_string());
    }
    if json {
        return Output::ok(serde_json::json!({ "applied": "file", "change": summary }).to_string());
    }
    Output::ok(summary.to_string())
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
    let mut name = String::new();
    for character in product_name.chars() {
        if character.is_ascii_alphanumeric() {
            name.push(character.to_ascii_lowercase());
        } else if !name.is_empty() && !name.ends_with('-') {
            name.push('-');
        }
    }
    let name = name.trim_end_matches('-');
    if name.is_empty() {
        "app".to_string()
    } else {
        name.to_string()
    }
}

/// Directory holding the launcher, added to `PATH` for sessions we spawn.
pub fn bin_dir(app_data_dir: &Path) -> PathBuf {
    lingxia_app_context::app_state_file(app_data_dir, "bin")
}

/// Write a launcher for the product's executable so the command is typable.
///
/// The real binary lives inside an application bundle, which is neither on
/// `PATH` nor pleasant to type. It is generated at runtime rather than at
/// build time so it always points at the executable actually running — a
/// development build moves, a release does not.
pub fn install_launcher(app_data_dir: &Path) -> std::io::Result<PathBuf> {
    let executable = std::env::current_exe()?;
    let name = command_name(
        executable
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("app"),
    );
    let directory = bin_dir(app_data_dir);
    std::fs::create_dir_all(&directory)?;
    let path = directory.join(&name);
    // `exec -a` so the program sees the name that was typed: help text quotes
    // argv[0], and every line of it has to be runnable as printed.
    let script = format!(
        "#!/bin/sh\n# Generated by LingXia; points at the running executable.\nexec -a {} {} \"$@\"\n",
        shell_quote(&name),
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
    fn a_change_reports_what_it_wrote() {
        let dir = tempfile::tempdir().expect("temp dir");
        let output = run(
            dir.path(),
            "myapp",
            &args(&["font", "--size", "14", "--json"]),
        );
        let payload: serde_json::Value = serde_json::from_str(&output.text).expect("json output");
        assert_eq!(payload["applied"], "file");
        assert!(
            payload["change"]
                .as_str()
                .unwrap_or_default()
                .contains("14"),
            "the change is named: {}",
            output.text
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
    fn the_command_name_is_lowercase_and_typable() {
        assert_eq!(command_name("LingXiaDemo"), "lingxiademo");
        assert_eq!(command_name("MyTerm"), "myterm");
        assert_eq!(command_name("Odd Name!"), "odd-name");
        assert_eq!(command_name(""), "app", "never produce an empty command");
    }

    #[test]
    fn the_launcher_is_executable_and_keeps_the_typed_name() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = install_launcher(dir.path()).expect("install");
        let script = std::fs::read_to_string(&path).expect("read");
        assert!(script.starts_with("#!/bin/sh"), "{script}");
        assert!(
            script.contains("exec -a"),
            "argv[0] must be the typed name, since help quotes it: {script}"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).expect("stat").permissions().mode();
            assert_eq!(mode & 0o111, 0o111, "the launcher must be executable");
        }
    }

    #[test]
    fn reset_restores_defaults() {
        let dir = tempfile::tempdir().expect("temp dir");
        run(dir.path(), "myapp", &args(&["font", "--size", "72"]));
        run(dir.path(), "myapp", &args(&["theme", "lingxia-light"]));

        let output = run(dir.path(), "myapp", &args(&["reset", "font"]));
        assert_eq!(output.code, 0, "{}", output.text);
        let (config, _) = TerminalConfig::load(dir.path(), &serde_json::Value::Null);
        assert_eq!(
            config.font.size,
            crate::FontConfig::default().size,
            "an unreadable font size needs a way out"
        );
        assert_eq!(
            config.theme.light, "lingxia-light",
            "only the font was reset"
        );

        run(dir.path(), "myapp", &args(&["reset"]));
        let (config, _) = TerminalConfig::load(dir.path(), &serde_json::Value::Null);
        assert_eq!(config, TerminalConfig::default());
    }

    #[test]
    fn a_flag_without_its_value_is_an_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        let output = run(dir.path(), "myapp", &args(&["font", "--size"]));
        assert_eq!(output.code, 1);
        assert!(output.text.contains("needs a value"), "{}", output.text);
        assert!(!TerminalConfig::path(dir.path()).exists());
    }

    #[test]
    fn an_unrecognized_invocation_says_so() {
        let output = unknown("myapp", &args(&["font", "--size"]));
        assert_eq!(output.code, 1);
        assert!(
            output.text.starts_with("unknown command 'font'"),
            "{}",
            output.text
        );
        assert!(output.text.contains("myapp term"), "{}", output.text);
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

    #[test]
    fn command_names_are_typable_without_quoting() {
        assert_eq!(command_name("LingXia Showcase"), "lingxia-showcase");
        assert_eq!(command_name("My  Term! 2"), "my-term-2");
        assert_eq!(
            command_name("  漢字  "),
            "app",
            "nothing typable falls back"
        );
    }

    #[test]
    fn the_launcher_points_at_the_current_executable_and_is_executable() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = install_launcher(dir.path()).expect("launcher");
        let script = std::fs::read_to_string(&path).expect("script");
        let executable = std::env::current_exe().expect("current exe");
        assert!(
            script.contains(&executable.to_string_lossy().into_owned()),
            "launcher must exec the running binary: {script}"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path)
                .expect("metadata")
                .permissions()
                .mode();
            assert_eq!(mode & 0o111, 0o111, "launcher must be executable");
        }
    }
}
