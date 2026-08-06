//! The `term` subcommand.
//!
//! Configuration's primary entry point is a command, not a settings window:
//! terminal users can drive one, it scripts, it lives in dotfiles, and its
//! interactive mode previews inside the terminal itself with no window
//! management. The product's own executable carries it (`myapp term …`), so
//! the command always knows which app it configures.
//!
//! The grammar is uniform — a noun, an optional sub-noun, then a value — and
//! every value is positional. Mixing flags into it (`--family x`, `--list`)
//! made three idioms out of one small surface.
//!
//! Arguments are parsed by hand rather than with a parser crate: the surface
//! is a handful of words, and a runtime crate should not carry an argument
//! parser for it.

use crate::{TerminalConfig, ThemeStore};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

/// Installed families, supplied by the platform: enumerating them is platform
/// work the shared layer cannot do.
static INSTALLED_FONTS: std::sync::Mutex<Vec<crate::InstalledFont>> =
    std::sync::Mutex::new(Vec::new());

/// Publish what is installed, so `font list` and `status` report reality.
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

/// What the command produced: text for a person, and the process exit code.
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
///
/// `system_is_dark` decides which slot an unqualified theme change writes:
/// configuration keeps a scheme per appearance, so "use this theme" means the
/// one in effect right now, not both.
pub fn run(app_data_dir: &Path, command: &str, args: &[String], system_is_dark: bool) -> Output {
    let words: Vec<&str> = args
        .iter()
        .map(String::as_str)
        .filter(|arg| !arg.starts_with("--"))
        .collect();
    match words.first().copied() {
        None | Some("help") => Output::ok(help(command)),
        Some("status") => status(app_data_dir, system_is_dark),
        Some("path") => Output::ok(TerminalConfig::path(app_data_dir).display().to_string()),
        Some("theme") => theme(app_data_dir, &words[1..], args, system_is_dark),
        Some("font") => font(app_data_dir, &words[1..]),
        Some("reset") => reset(app_data_dir, &words[1..]),
        Some(other) => Output::error(format!("unknown command '{other}'\n\n{}", help(command))),
    }
}

/// Report an invocation this binary does not recognize.
///
/// Separate from `run` because it answers without a configuration directory:
/// it is reached before the app exists, when someone typed something wrong.
pub fn unknown(command: &str, arguments: &[String]) -> Output {
    let first = arguments.first().map(String::as_str).unwrap_or("");
    Output::error(format!("unknown command '{first}'\n\n{}", help(command)))
}

fn help(command: &str) -> String {
    format!(
        "\
{command} term — terminal configuration

  status                 what is in effect
  path                   where the configuration lives

  theme                  choose one, previewing as you go
  theme <name>           use it for the current appearance
  theme <name> --light   ... or for a named one (--light, --dark)
  theme mode <mode>      system, light or dark
  theme list             schemes available
  theme show <name>      print a scheme — also what its file looks like
  theme import <file>    add a scheme, optionally `as <name>`

  font                   the font in effect
  font <family>          use it, or the next installed candidate
  font size <points>
  font ligatures on|off
  font list              installed monospace families

  reset [font|theme]     back to defaults"
    )
}

fn status(app_data_dir: &Path, system_is_dark: bool) -> Output {
    let (config, error) = TerminalConfig::load(app_data_dir, &serde_json::Value::Null);
    let resolved = crate::resolve_font(&config.font, &installed_fonts());
    let mut text = String::new();
    if let Some(error) = error {
        text.push_str(&format!("{error}\n\n"));
    }
    text.push_str(&format!(
        "font    {} {}pt{}\n",
        if resolved.family.is_empty() {
            "(none resolved)"
        } else {
            &resolved.family
        },
        config.font.size,
        if resolved.fell_back {
            "  (none of the configured families is installed)"
        } else {
            ""
        }
    ));
    if !resolved.missing.is_empty() {
        text.push_str(&format!(
            "        not installed: {}\n",
            resolved.missing.join(", ")
        ));
    }
    text.push_str(&format!(
        "theme   {} ({:?}: light={} dark={})\n",
        config.theme.selected(system_is_dark),
        config.theme.mode,
        config.theme.light,
        config.theme.dark
    ));
    text.push_str(&format!(
        "config  {}",
        TerminalConfig::path(app_data_dir).display()
    ));
    Output::ok(text)
}

fn theme(app_data_dir: &Path, words: &[&str], args: &[String], system_is_dark: bool) -> Output {
    let store = ThemeStore::new(app_data_dir);
    let (mut config, _) = TerminalConfig::load(app_data_dir, &serde_json::Value::Null);

    match words.first().copied() {
        // No name on a terminal means choosing one, with the terminal itself
        // as the preview. Off a terminal it is a scripted call missing its
        // argument.
        None => {
            return if std::io::stdout().is_terminal() && std::io::stdin().is_terminal() {
                choose(app_data_dir, &config, system_is_dark)
            } else {
                Output::error("name a theme, or use `theme list`")
            };
        }
        Some("list") => {
            let text = store
                .list()
                .iter()
                .map(|entry| {
                    let mark = if entry.name == config.theme.selected(system_is_dark) {
                        "*"
                    } else {
                        " "
                    };
                    format!("{mark} {:<22} {:?}", entry.name, entry.source)
                })
                .collect::<Vec<_>>()
                .join("\n");
            return Output::ok(text);
        }
        Some("show") => {
            let Some(name) = words.get(1) else {
                return Output::error("name the scheme to show");
            };
            let Some(theme) = store.get(name) else {
                return Output::error(format!("no theme named '{name}'"));
            };
            return Output::ok(
                serde_json::to_string_pretty(&theme).unwrap_or_else(|_| "{}".to_string()),
            );
        }
        Some("import") => return import(app_data_dir, &words[1..]),
        Some("mode") => {
            let Some(mode) = words.get(1) else {
                return Output::error("mode takes system, light or dark");
            };
            config.theme.mode = match *mode {
                "system" => crate::ThemeMode::System,
                "light" => crate::ThemeMode::Light,
                "dark" => crate::ThemeMode::Dark,
                other => {
                    return Output::error(format!(
                        "mode takes system, light or dark, got '{other}'"
                    ));
                }
            };
            return apply(app_data_dir, &config, &format!("theme mode = {mode}"));
        }
        _ => {}
    }

    let name = words[0];
    if store.get(name).is_none() {
        return Output::error(format!(
            "no theme named '{name}'; `theme list` shows what is available"
        ));
    }

    // Configuration keeps a scheme per appearance. Writing both would discard
    // the choice made for the other one, so an unqualified change targets the
    // appearance in effect.
    let light = args.iter().any(|arg| arg == "--light");
    let dark = args.iter().any(|arg| arg == "--dark");
    if light && dark {
        config.theme.light = name.to_string();
        config.theme.dark = name.to_string();
        return apply(app_data_dir, &config, &format!("theme = {name} (both)"));
    }
    let target_dark = if light || dark {
        dark
    } else {
        appearance_is_dark(&config, system_is_dark)
    };
    if target_dark {
        config.theme.dark = name.to_string();
    } else {
        config.theme.light = name.to_string();
    }
    let appearance = if target_dark { "dark" } else { "light" };
    apply(
        app_data_dir,
        &config,
        &format!("{appearance} theme = {name}"),
    )
}

/// Which of the two configured schemes is in effect.
fn appearance_is_dark(config: &TerminalConfig, system_is_dark: bool) -> bool {
    match config.theme.mode {
        crate::ThemeMode::Light => false,
        crate::ThemeMode::Dark => true,
        crate::ThemeMode::System => system_is_dark,
    }
}

/// Choose a theme interactively, previewing each in this session.
fn choose(app_data_dir: &Path, config: &TerminalConfig, system_is_dark: bool) -> Output {
    let store = ThemeStore::new(app_data_dir);
    let names: Vec<String> = store.list().into_iter().map(|entry| entry.name).collect();
    let selected = config.theme.selected(system_is_dark).to_string();
    let Some(current) = store.get(&selected) else {
        return Output::error(format!("current theme '{selected}' is missing"));
    };
    match crate::picker::pick_theme(&store, &names, &current, &selected) {
        Ok(crate::Choice::Selected(name)) => {
            let mut config = config.clone();
            let target_dark = appearance_is_dark(&config, system_is_dark);
            if target_dark {
                config.theme.dark = name.clone();
            } else {
                config.theme.light = name.clone();
            }
            let appearance = if target_dark { "dark" } else { "light" };
            apply(
                app_data_dir,
                &config,
                &format!("{appearance} theme = {name}"),
            )
        }
        Ok(crate::Choice::Cancelled) => Output::ok("unchanged"),
        Err(error) => Output::error(error.to_string()),
    }
}

/// Add a scheme from a file.
///
/// Three shapes cover what people actually have: this crate's own JSON, which
/// is the Windows Terminal scheme shape every collection publishes, and the
/// `name: value` text of Xresources and kitty. Anything else is reported
/// rather than guessed at.
fn import(app_data_dir: &Path, words: &[&str]) -> Output {
    let Some(source) = words.first() else {
        return Output::error("name the file to import");
    };
    let text = match std::fs::read_to_string(source) {
        Ok(text) => text,
        Err(error) => return Output::error(format!("cannot read {source}: {error}")),
    };
    let stem = Path::new(source)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("imported");
    // `import <file> as <name>` reads the way it is spoken.
    let name = match words.get(1).copied() {
        Some("as") => words.get(2).copied().unwrap_or(stem),
        _ => stem,
    };

    let theme = match crate::parse_scheme(&text) {
        Ok(theme) => theme,
        Err(reason) => {
            return Output::error(format!(
                "{source} is not a scheme this understands: {reason}\n\
                 Accepted: the JSON `theme show` prints, or the `name: value`\n\
                 text of Xresources and kitty."
            ));
        }
    };
    match ThemeStore::new(app_data_dir).import(name, &theme) {
        Ok(_) => Output::ok(format!("imported {name}")),
        Err(error) => Output::error(error.to_string()),
    }
}

fn font(app_data_dir: &Path, words: &[&str]) -> Output {
    let (mut config, _) = TerminalConfig::load(app_data_dir, &serde_json::Value::Null);
    match words.first().copied() {
        None => {
            let resolved = crate::resolve_font(&config.font, &installed_fonts());
            Output::ok(format!(
                "{} {}pt",
                if resolved.family.is_empty() {
                    "(none resolved)"
                } else {
                    &resolved.family
                },
                config.font.size
            ))
        }
        Some("list") => {
            let installed = installed_fonts();
            if installed.is_empty() {
                return Output::error("font listing is unavailable in this context");
            }
            let text = installed
                .iter()
                .map(|font| {
                    format!(
                        "{:<32} {:<11} {}",
                        font.family,
                        if font.ligatures { "ligatures" } else { "" },
                        if font.nerd_icons { "nerd icons" } else { "" }
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            Output::ok(text)
        }
        Some("size") => {
            let Some(value) = words.get(1) else {
                return Output::error("size takes a number of points");
            };
            match value.parse::<f32>() {
                Ok(size) if (4.0..=96.0).contains(&size) => {
                    config.font.size = size;
                    apply(app_data_dir, &config, &format!("font size = {size}"))
                }
                _ => Output::error(format!("font size '{value}' is not between 4 and 96")),
            }
        }
        Some("ligatures") => match words.get(1).copied() {
            Some("on") => {
                config.font.ligatures = true;
                apply(app_data_dir, &config, "ligatures = on")
            }
            Some("off") => {
                config.font.ligatures = false;
                apply(app_data_dir, &config, "ligatures = off")
            }
            _ => Output::error("ligatures takes on or off"),
        },
        Some(family) => {
            config.font.family = vec![family.to_string()];
            apply(app_data_dir, &config, &format!("font = {family}"))
        }
    }
}

/// Back to defaults — the way out when a change made the terminal unusable,
/// which a font size can do all by itself.
fn reset(app_data_dir: &Path, words: &[&str]) -> Output {
    let (mut config, _) = TerminalConfig::load(app_data_dir, &serde_json::Value::Null);
    let summary = match words.first().copied() {
        None => {
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
        Some(other) => return Output::error(format!("reset takes font or theme, got '{other}'")),
    };
    apply(app_data_dir, &config, &format!("reset {summary}"))
}

/// Persist a change and report it.
///
/// Writing the file is the whole operation: a running app watches it and
/// adopts the change, so there is nothing to notify and no claim to make
/// about whether one is running.
fn apply(app_data_dir: &Path, config: &TerminalConfig, summary: &str) -> Output {
    match config.save(app_data_dir, &serde_json::Value::Null) {
        Ok(()) => Output::ok(summary.to_string()),
        Err(error) => Output::error(error.to_string()),
    }
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

/// Set by the launcher so the product knows it was invoked as its command
/// line rather than started as an app.
///
/// Guessing from the standard streams cannot work: a GUI-subsystem binary has
/// no console until it borrows one, and a host started by a console tool then
/// looks exactly like a typed command. The launcher is ours, so it says so.
pub const INVOCATION_MARKER: &str = "LINGXIA_TERMINAL_INVOCATION";

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
    let name = command_name(&executable_stem(&executable));
    let directory = bin_dir(app_data_dir);
    std::fs::create_dir_all(&directory)?;
    let (file_name, script) = launcher_script(&name, &executable);
    let path = directory.join(file_name);
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

/// The executable's name without whatever suffix this platform puts on one.
///
/// `EXE_SUFFIX` is empty off Windows, so this is one expression rather than a
/// `cfg` — the same reason `join_paths` below replaces a `:` and a `;`.
fn executable_stem(executable: &Path) -> String {
    let name = executable
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("app");
    name.strip_suffix(std::env::consts::EXE_SUFFIX)
        .unwrap_or(name)
        .to_string()
}

/// The launcher's file name and contents. A shell script and a `.cmd` shim are
/// genuinely different artifacts, so this is the one place that forks.
#[cfg(not(windows))]
fn launcher_script(name: &str, executable: &Path) -> (String, String) {
    // `exec -a` so the program sees the name that was typed: help text quotes
    // argv[0], and every line of it has to be runnable as printed.
    let script = format!(
        "#!/bin/sh\n# Generated by LingXia; points at the running executable.\n{INVOCATION_MARKER}=1 exec -a {} {} \"$@\"\n",
        shell_quote(name),
        shell_quote(&executable.to_string_lossy())
    );
    (name.to_string(), script)
}

/// `.cmd` is what `PATHEXT` makes typable as a bare name. It cannot rewrite
/// argv[0] the way `exec -a` does, and does not need to: the command name is
/// derived from the executable, which the shim leaves alone.
#[cfg(windows)]
fn launcher_script(name: &str, executable: &Path) -> (String, String) {
    let script = format!(
        "@echo off\r\nrem Generated by LingXia; points at the running executable.\r\nset {INVOCATION_MARKER}=1\r\n\"{}\" %*\r\n",
        executable.display()
    );
    (format!("{name}.cmd"), script)
}

#[cfg(not(windows))]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Whether this process was started through the product's own launcher.
///
/// The marker is authoritative. A tty check remains as the fallback for a
/// direct invocation of the executable, which on Unix still tells app from
/// command line — on Windows it cannot, because a host spawned by a console
/// tool inherits that console.
fn invoked_as_command() -> bool {
    if std::env::var_os(INVOCATION_MARKER).is_some() {
        return true;
    }
    !cfg!(windows) && (std::io::stdout().is_terminal() || std::io::stdin().is_terminal())
}

/// Environment a spawned session needs to find the product's command line.
///
/// The executable lives inside an application bundle, which is neither on
/// `PATH` nor pleasant to type, so sessions we spawn get the launcher
/// directory prepended and the executable's own path — which is what tells a
/// program running inside the terminal that it has one to talk to.
pub fn session_environment(app_data_dir: &Path) -> Vec<(String, String)> {
    let mut environment = Vec::new();
    match install_launcher(app_data_dir) {
        Ok(launcher) => {
            log::info!("terminal command: {}", launcher.display());
            environment.push((
                "LINGXIA_TERMINAL_CLI".to_string(),
                launcher.to_string_lossy().into_owned(),
            ));
            // `split_paths`/`join_paths` spell the separator the platform uses,
            // so prepending is one implementation rather than a `:` and a `;`.
            let mut entries = vec![bin_dir(app_data_dir)];
            entries.extend(std::env::split_paths(
                &std::env::var_os("PATH").unwrap_or_default(),
            ));
            match std::env::join_paths(entries) {
                Ok(path) => {
                    environment.push(("PATH".to_string(), path.to_string_lossy().into_owned()))
                }
                Err(error) => log::warn!("terminal PATH not extended: {error}"),
            }
        }
        Err(error) => log::warn!("terminal command launcher not installed: {error}"),
    }
    environment
}

/// Run the `term` command line if this process was invoked as one.
///
/// Hosts call this from `main` **before** touching any UI framework: the
/// product's executable doubles as its command line, and a configuration
/// command must not open a window. Returns the exit code when it handled the
/// invocation, `None` when the process should carry on and become the app.
pub fn run_if_invoked(app_data_dir: &Path, system_is_dark: bool) -> Option<i32> {
    let mut args = std::env::args();
    let executable = args.next().unwrap_or_default();
    let command = command_name(&executable_stem(Path::new(&executable)));

    let first = args.next();
    if !invoked_as_command() && first.as_deref() != Some("term") {
        // The OS launched the product; it is starting as an app. Without this
        // an unrecognized argument would fall through to startup and die
        // against a running instance's databases.
        return None;
    }
    if first.as_deref() != Some("term") {
        let arguments: Vec<String> = std::env::args().skip(1).collect();
        let output = unknown(&command, &arguments);
        eprintln!("{}", output.text);
        return Some(output.code);
    }

    let rest: Vec<String> = std::env::args().skip(2).collect();
    let output = run(app_data_dir, &command, &rest, system_is_dark);
    if output.code == 0 {
        println!("{}", output.text);
    } else {
        eprintln!("{}", output.text);
    }
    Some(output.code)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    fn go(dir: &Path, values: &[&str]) -> Output {
        run(dir, "myapp", &args(values), true)
    }

    fn loaded(dir: &Path) -> TerminalConfig {
        TerminalConfig::load(dir, &serde_json::Value::Null).0
    }

    #[test]
    fn a_theme_change_targets_the_appearance_in_effect() {
        let dir = tempfile::tempdir().expect("temp dir");
        // Following the system, under a dark appearance.
        let output = go(dir.path(), &["theme", "lingxia-light"]);
        assert_eq!(output.code, 0, "{}", output.text);
        let config = loaded(dir.path());
        assert_eq!(config.theme.dark, "lingxia-light", "the slot in effect");
        assert_eq!(
            config.theme.light,
            crate::ThemeConfig::default().light,
            "the other appearance keeps its own choice"
        );

        go(dir.path(), &["theme", "lingxia-dark", "--light"]);
        let config = loaded(dir.path());
        assert_eq!(config.theme.light, "lingxia-dark");
        assert_eq!(config.theme.dark, "lingxia-light", "unchanged");
    }

    #[test]
    fn a_pinned_mode_decides_the_slot_instead_of_the_system() {
        let dir = tempfile::tempdir().expect("temp dir");
        go(dir.path(), &["theme", "mode", "light"]);
        go(dir.path(), &["theme", "lingxia-dark"]);
        let config = loaded(dir.path());
        assert_eq!(
            config.theme.light, "lingxia-dark",
            "pinned to light, so that is the slot in effect"
        );
        assert_eq!(config.theme.dark, crate::ThemeConfig::default().dark);
    }

    #[test]
    fn an_unknown_theme_fails_without_writing() {
        let dir = tempfile::tempdir().expect("temp dir");
        let output = go(dir.path(), &["theme", "no-such-scheme"]);
        assert_eq!(output.code, 1);
        assert!(output.text.contains("theme list"), "{}", output.text);
        assert!(
            !TerminalConfig::path(dir.path()).exists(),
            "a rejected command writes nothing"
        );
    }

    #[test]
    fn font_values_are_positional_and_validated() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert_eq!(go(dir.path(), &["font", "size", "900"]).code, 1);
        assert_eq!(go(dir.path(), &["font", "size"]).code, 1);
        assert!(!TerminalConfig::path(dir.path()).exists());

        assert_eq!(go(dir.path(), &["font", "Iosevka"]).code, 0);
        assert_eq!(go(dir.path(), &["font", "size", "15"]).code, 0);
        assert_eq!(go(dir.path(), &["font", "ligatures", "off"]).code, 0);
        let config = loaded(dir.path());
        assert_eq!(config.font.family, vec!["Iosevka".to_string()]);
        assert_eq!(config.font.size, 15.0);
        assert!(!config.font.ligatures);
    }

    #[test]
    fn reset_restores_defaults() {
        let dir = tempfile::tempdir().expect("temp dir");
        go(dir.path(), &["font", "size", "72"]);
        go(dir.path(), &["theme", "lingxia-light"]);

        go(dir.path(), &["reset", "font"]);
        let config = loaded(dir.path());
        assert_eq!(
            config.font.size,
            crate::FontConfig::default().size,
            "an unreadable font size needs a way out"
        );
        assert_eq!(
            config.theme.dark, "lingxia-light",
            "only the font was reset"
        );

        go(dir.path(), &["reset"]);
        assert_eq!(loaded(dir.path()), TerminalConfig::default());
    }

    #[test]
    fn show_prints_a_scheme_in_the_shape_import_accepts() {
        let dir = tempfile::tempdir().expect("temp dir");
        let printed = go(dir.path(), &["theme", "show", "lingxia-dark"]);
        assert_eq!(printed.code, 0, "{}", printed.text);

        let file = dir.path().join("copy.json");
        std::fs::write(&file, &printed.text).expect("write");
        let imported = go(
            dir.path(),
            &["theme", "import", file.to_str().unwrap(), "as", "copy"],
        );
        assert_eq!(imported.code, 0, "{}", imported.text);
        assert!(
            ThemeStore::new(dir.path()).get("copy").is_some(),
            "what `show` prints is what `import` takes"
        );
    }

    #[test]
    fn an_unimportable_file_names_the_shapes_that_work() {
        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join("nope.txt");
        std::fs::write(&file, "this is not a color scheme").expect("write");
        let output = go(dir.path(), &["theme", "import", file.to_str().unwrap()]);
        assert_eq!(output.code, 1);
        assert!(output.text.contains("Xresources"), "{}", output.text);
    }

    #[test]
    fn the_command_name_is_lowercase_and_typable() {
        assert_eq!(command_name("LingXiaDemo"), "lingxiademo");
        assert_eq!(command_name("MyTerm"), "myterm");
        assert_eq!(command_name("Odd Name!"), "odd-name");
        assert_eq!(command_name(""), "app", "never produce an empty command");
    }

    /// What both launchers owe the caller: the typed name, and the executable
    /// that is actually running. How they spell it is the platform's business.
    #[test]
    fn the_launcher_is_runnable_and_points_at_this_executable() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = install_launcher(dir.path()).expect("launcher");
        let script = std::fs::read_to_string(&path).expect("script");
        let executable = std::env::current_exe().expect("current exe");
        assert!(
            script.contains(&*executable.to_string_lossy()),
            "the launcher must point at the running executable: {script}"
        );
        assert_eq!(
            path.file_stem().and_then(|name| name.to_str()),
            Some(command_name(&executable_stem(&executable)).as_str()),
            "the launcher is named for the command that is typed"
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert!(
                script.contains("exec -a"),
                "argv[0] must be the typed name, since help quotes it: {script}"
            );
            let mode = std::fs::metadata(&path)
                .expect("metadata")
                .permissions()
                .mode();
            assert_eq!(mode & 0o111, 0o111, "the launcher must be executable");
        }
        #[cfg(windows)]
        assert_eq!(
            path.extension().and_then(|name| name.to_str()),
            Some("cmd"),
            "`PATHEXT` is what makes a bare name typable"
        );
    }

    #[test]
    fn an_unrecognized_invocation_says_so() {
        let output = unknown("myapp", &args(&["font", "size"]));
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
        let output = go(dir.path(), &[]);
        assert!(
            output.text.starts_with("myapp term"),
            "help must be copy-pasteable: {}",
            output.text
        );
    }
}
