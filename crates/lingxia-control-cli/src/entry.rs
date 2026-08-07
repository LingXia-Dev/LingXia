//! The product's executable, acting as its own command line.
//!
//! There is no second binary. A product ships one file that is the app when
//! the OS launches it and the command line when someone types it, which is
//! what keeps the two from ever being different versions of each other.
//!
//! Hosts call [`run_if_invoked`] as the first thing in `main` — before any UI
//! framework and before the runtime initializes, because initialization opens
//! the app's databases and a command must not collide with an instance already
//! running. It returns `None` when the process should carry on and be the app.

use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use clap::Parser;

use crate::transport::{ControlSocket, ENDPOINT_ENV};
use crate::{app, desktop};

/// Set by the launcher so the product knows it was typed rather than launched.
///
/// Guessing from the standard streams cannot work: a GUI-subsystem binary has
/// no console until it borrows one, and a host started by a console tool then
/// looks exactly like a typed command. The launcher is ours, so it says so.
pub const INVOCATION_MARKER: &str = "LINGXIA_CLI_INVOCATION";

#[derive(Parser)]
#[command(
    disable_help_subcommand = true,
    about = "Drive this product from the command line"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// The product's own surface sits at the top level and the machine's gets a
/// prefix, which is the opposite of how `lxdev` reads for a good reason: a
/// development tool has to say *which* app it means, and a product does not.
/// `myapp app screenshot` asks the user to name the thing they already typed.
#[derive(clap::Subcommand)]
enum Command {
    /// This product's own windows: screenshot, windows, mouse, key
    #[command(flatten)]
    Own(app::AppCommand),
    /// Automate the machine: windows, capture, input, accessibility, clipboard
    Computer(desktop::DesktopOptions),
}

/// Run the command line and return its exit code, or `None` to become the app.
pub fn run_if_invoked(state_dir: &Path) -> Option<i32> {
    if !invoked_as_command() && !first_argument_is_a_command() {
        // The OS launched the product. Without this check an unrecognized
        // argument would fall through to startup and die against a running
        // instance's databases.
        return None;
    }

    let endpoint = crate::transport::endpoint_in(state_dir);
    let transport = ControlSocket::at(endpoint);
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        // `try_parse` hands back help and usage rather than printing them, and
        // the host exits on our code — so nothing else will ever show them.
        Err(error) => {
            let _ = error.print();
            return Some(error.exit_code());
        }
    };
    Some(match cli.command {
        Command::Computer(options) => desktop::execute(&desktop::Backend::App(&transport), options),
        Command::Own(command) => {
            let context = app::AppContext {
                transport: &transport,
                target: std::env::consts::OS.to_string(),
                session: None,
            };
            match app::execute(&context, app::AppOptions { command }) {
                Ok(()) => 0,
                Err(error) => {
                    eprintln!("Error: {error}");
                    1
                }
            }
        }
    })
}

/// Whether the first argument names one of our subcommands.
///
/// The marker covers anything launched through our own shim; this covers
/// someone running the executable inside the bundle directly, which is what a
/// developer does and what a `--help` in a bug report looks like.
fn first_argument_is_a_command() -> bool {
    const COMMANDS: &[&str] = &[
        "computer",
        "doctor",
        "screenshot",
        "windows",
        "mouse",
        "key",
        "help",
        "--help",
        "-h",
    ];
    std::env::args()
        .nth(1)
        .is_some_and(|first| COMMANDS.contains(&first.as_str()))
}

/// Whether this process was started through the product's own launcher.
///
/// The marker is authoritative. A tty check remains as the fallback for a
/// direct invocation, which on Unix still tells app from command line — on
/// Windows it cannot, because a host spawned by a console tool inherits that
/// console.
fn invoked_as_command() -> bool {
    if std::env::var_os(INVOCATION_MARKER).is_some() {
        return true;
    }
    !cfg!(windows) && (std::io::stdout().is_terminal() || std::io::stdin().is_terminal())
}

/// Directory holding the launcher, added to `PATH` for sessions we spawn.
pub fn bin_dir(state_dir: &Path) -> PathBuf {
    state_dir.join("bin")
}

/// Write a launcher for the product's executable so the command is typable.
///
/// The real binary lives inside an application bundle, which is neither on
/// `PATH` nor pleasant to type. It is generated at runtime rather than at
/// build time so it always points at the executable actually running — a
/// development build moves, a release does not.
pub fn install_launcher(state_dir: &Path, endpoint: &str) -> std::io::Result<PathBuf> {
    let executable = std::env::current_exe()?;
    let name = command_name(&executable_stem(&executable));
    let directory = bin_dir(state_dir);
    std::fs::create_dir_all(&directory)?;
    let (file_name, script) = launcher_script(&name, &executable, endpoint);
    let path = directory.join(file_name);
    if std::fs::read_to_string(&path).unwrap_or_default() != script {
        std::fs::write(&path, &script)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
        }
    }
    Ok(path)
}

/// Remove the launcher. The other half of a switch that can be turned off:
/// one that only ever adds leaves the machine littered.
pub fn remove_launcher(state_dir: &Path) -> std::io::Result<()> {
    let executable = std::env::current_exe()?;
    let name = command_name(&executable_stem(&executable));
    let (file_name, _) = launcher_script(&name, &executable, "");
    match std::fs::remove_file(bin_dir(state_dir).join(file_name)) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// Environment a spawned session needs to find the product's command line.
///
/// An agent running in the product's own terminal needs no installation step
/// at all: the launcher directory is already on its `PATH`.
pub fn session_environment(state_dir: &Path, endpoint: &str) -> Vec<(String, String)> {
    let mut environment = Vec::new();
    match install_launcher(state_dir, endpoint) {
        Ok(launcher) => {
            log::info!("product command: {}", launcher.display());
            environment.push((
                "LINGXIA_PRODUCT_CLI".to_string(),
                launcher.to_string_lossy().into_owned(),
            ));
            // `split_paths`/`join_paths` spell the separator the platform uses,
            // so prepending is one implementation rather than a `:` and a `;`.
            let mut entries = vec![bin_dir(state_dir)];
            entries.extend(std::env::split_paths(
                &std::env::var_os("PATH").unwrap_or_default(),
            ));
            match std::env::join_paths(entries) {
                Ok(path) => {
                    environment.push(("PATH".to_string(), path.to_string_lossy().into_owned()))
                }
                Err(error) => log::warn!("session PATH not extended: {error}"),
            }
        }
        Err(error) => log::warn!("product command launcher not installed: {error}"),
    }
    environment
}

/// Lowercased and stripped to what a shell takes without quoting: the name
/// exists to be typed, and a product called "My App" must not require escaping.
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

/// The executable's name without whatever suffix this platform puts on one.
///
/// `EXE_SUFFIX` is empty off Windows, so this is one expression rather than a
/// `cfg` — the same reason `join_paths` above replaces a `:` and a `;`.
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
fn launcher_script(name: &str, executable: &Path, endpoint: &str) -> (String, String) {
    // `exec -a` so the program sees the name that was typed: help text quotes
    // argv[0], and every line of it has to be runnable as printed. The
    // endpoint rides along for symmetry with Windows, where it is the only way
    // the client can know the pipe's name.
    let script = format!(
        "#!/bin/sh\n# Generated by LingXia; points at the running executable.\n{INVOCATION_MARKER}=1 {ENDPOINT_ENV}={} exec -a {} {} \"$@\"\n",
        shell_quote(endpoint),
        shell_quote(name),
        shell_quote(&executable.to_string_lossy())
    );
    (name.to_string(), script)
}

/// `.cmd` is what `PATHEXT` makes typable as a bare name. It cannot rewrite
/// argv[0] the way `exec -a` does, and does not need to: the command name is
/// derived from the executable, which the shim leaves alone.
#[cfg(windows)]
fn launcher_script(name: &str, executable: &Path, endpoint: &str) -> (String, String) {
    let script = format!(
        "@echo off\r\nrem Generated by LingXia; points at the running executable.\r\nset {INVOCATION_MARKER}=1\r\nset {ENDPOINT_ENV}={endpoint}\r\n\"{}\" %*\r\n",
        executable.display()
    );
    (format!("{name}.cmd"), script)
}

#[cfg(not(windows))]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_are_typable_without_quoting() {
        assert_eq!(command_name("LingXia Demo"), "lingxia-demo");
        assert_eq!(command_name("My  Term!!"), "my-term");
        assert_eq!(command_name("!!!"), "app");
        assert_eq!(command_name("Foo"), "foo");
    }

    #[test]
    fn launcher_points_at_the_running_executable_and_marks_the_invocation() {
        let (file_name, script) = launcher_script(
            "foo",
            Path::new("/Apps/Foo.app/MacOS/Foo"),
            "/tmp/control.sock",
        );
        assert!(script.contains("/Apps/Foo.app/MacOS/Foo"));
        assert!(
            script.contains(INVOCATION_MARKER),
            "without the marker a GUI-subsystem binary cannot tell a typed \
             command from an app launch"
        );
        assert!(file_name.starts_with("foo"));
    }
}
