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

use std::ffi::{OsStr, OsString};
use std::io::IsTerminal;
use std::path::Path;

use clap::Parser;
use lingxia_control_protocol::invocation;

#[cfg(feature = "desktop")]
use crate::desktop;
use crate::transport::ControlSocket;
use crate::{app, browser, extra};

#[derive(Parser)]
#[command(
    disable_help_subcommand = true,
    about = "Drive this product from the command line"
)]
struct Cli {
    /// Acknowledge input sent to this product's own windows. The machine and
    /// browser namespaces carry their own copies of this flag; this one covers
    /// the commands that sit at the top level.
    #[arg(long, global = true)]
    allow_control: bool,
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
    /// Drive the in-app browser: tabs, navigation, page content
    Browser(browser::BrowserOptions),
    /// Automate the machine: windows, capture, input, accessibility, clipboard
    #[cfg(feature = "desktop")]
    Computer(desktop::DesktopOptions),
    /// Turn the automation interface on or off, and report it
    Control {
        #[command(subcommand)]
        action: ControlAction,
    },
}

/// Shipping the capability is not the same as switching it on: this endpoint
/// hands any local process the product's whole automation surface, so the build
/// decides whether the ability exists and the user decides whether it listens.
///
/// A settings screen can call `control::set_enabled` for immediate effect. This
/// exists so a product that has not built one is still something a person can
/// switch on, rather than a capability with no way to reach it.
#[derive(clap::Subcommand)]
pub enum ControlAction {
    /// Report whether the interface is switched on
    Status,
    /// Allow local processes to drive this product
    Enable,
    /// Stop answering, and remove the command from PATH
    Disable,
}

/// Run the command line and return its exit code, or `None` to become the app.
pub fn run_if_invoked(state_dir: &Path) -> Option<i32> {
    let (explicit, args) = invocation_args();
    if !invocation_is_command(explicit, cfg!(windows), std::io::stdin().is_terminal())
        && !first_argument_is_a_command(&args)
    {
        // The OS launched the product. Without this check an unrecognized
        // argument would fall through to startup and die against a running
        // instance's databases.
        return None;
    }
    let _console = crate::console::attach_parent();

    let endpoint = crate::transport::endpoint_in(state_dir);
    let transport = ControlSocket::at(endpoint);
    if let Some(command) = args
        .get(1)
        .and_then(|arg| arg.to_str())
        .and_then(extra::get)
    {
        return Some((command.execute)(&transport, &args[2..]));
    }
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        // `try_parse` hands back help and usage rather than printing them, and
        // the host exits on our code — so nothing else will ever show them.
        Err(error) => {
            let _ = error.print();
            if is_top_level_help(&args) {
                print_extra_commands();
            }
            return Some(error.exit_code());
        }
    };
    let allow_control = cli.allow_control;
    Some(match cli.command {
        Command::Browser(options) => {
            let context = browser::BrowserContext {
                transport: &transport,
                target: std::env::consts::OS.to_string(),
            };
            report(browser::execute(&context, options))
        }
        #[cfg(feature = "desktop")]
        Command::Computer(options) => desktop::execute(&desktop::Backend::App(&transport), options),
        Command::Control { action } => control(state_dir, &transport, action),
        Command::Own(command) => {
            let context = app::AppContext {
                transport: &transport,
                target: std::env::consts::OS.to_string(),
                session: None,
            };
            report(app::execute(
                &context,
                app::AppOptions {
                    allow_control,
                    command,
                },
            ))
        }
    })
}

/// What is *true* right now, not only what was written down.
///
/// The persisted setting and the running app can disagree — a product that has
/// not restarted is still listening after the setting says otherwise — and a
/// consent control that reports "off" while automation is live is worse than
/// one that reports nothing. So `status` asks the endpoint, and `disable`
/// tells it to stop rather than leaving a note for next time.
fn control(
    state_dir: &Path,
    transport: &dyn crate::transport::Transport,
    action: ControlAction,
) -> i32 {
    let Some(app_data_dir) = state_dir.parent() else {
        eprintln!("Error: cannot locate this product's data directory");
        return 10;
    };
    let listening = || {
        transport
            .request(lingxia_control_protocol::methods::control::STATUS, None)
            .ok()
            .and_then(|value| value?.get("listening")?.as_bool())
            .unwrap_or(false)
    };

    match action {
        ControlAction::Status => {
            let live = listening();
            let persisted = lingxia_settings::control_enabled(app_data_dir);
            println!(
                "automation interface: {}",
                match (live, persisted) {
                    (true, _) => "on, and answering now",
                    (false, true) => "on at next start (nothing is listening yet)",
                    (false, false) => "off",
                }
            );
            if live || persisted {
                println!("command: {}", launcher_path(state_dir).display());
            }
            0
        }
        ControlAction::Enable => match lingxia_settings::set_control_enabled(app_data_dir, true) {
            Ok(()) => {
                println!("automation interface: on at next start");
                let bin = launcher_path(state_dir);
                println!("command: {}", bin.display());
                // The app cannot change the PATH of a terminal already open, so
                // the one thing it can usefully do is say the line to add.
                if let Some(dir) = bin.parent() {
                    let (profile, instruction) = path_profile_instruction(
                        dir,
                        if cfg!(windows) {
                            ProfileShell::PowerShell
                        } else {
                            ProfileShell::Posix
                        },
                    );
                    println!("to type it bare, add to your {profile} profile:");
                    println!("  {instruction}");
                }
                0
            }
            Err(error) => {
                eprintln!("Error: {error}");
                10
            }
        },
        ControlAction::Disable => {
            // Ask the running product first: it is the only thing that can
            // actually stop answering, and it persists the choice too.
            let stopped = transport
                .request(lingxia_control_protocol::methods::control::DISABLE, None)
                .is_ok();
            if !stopped
                && let Err(error) = lingxia_settings::set_control_enabled(app_data_dir, false)
            {
                eprintln!("Error: {error}");
                return 10;
            }
            println!("automation interface: off");
            if !stopped {
                println!("nothing was listening; it will not start next time either");
            }
            0
        }
    }
}

#[derive(Clone, Copy)]
enum ProfileShell {
    Posix,
    PowerShell,
}

fn path_profile_instruction(dir: &Path, shell: ProfileShell) -> (&'static str, String) {
    let dir = dir.to_string_lossy();
    match shell {
        ProfileShell::Posix => {
            let escaped = dir.replace('\'', "'\"'\"'");
            ("shell", format!("export PATH='{escaped}':\"$PATH\""))
        }
        ProfileShell::PowerShell => {
            let escaped = dir.replace('\'', "''");
            (
                "PowerShell",
                format!("$env:Path = '{escaped};' + $env:Path"),
            )
        }
    }
}

/// Where the product writes the launcher when the interface is on.
fn launcher_path(state_dir: &Path) -> std::path::PathBuf {
    let stem = std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.file_name().and_then(|name| name.to_str()).map(|name| {
                name.trim_end_matches(std::env::consts::EXE_SUFFIX)
                    .to_string()
            })
        })
        .unwrap_or_else(|| "app".to_string());
    state_dir
        .join("bin")
        .join(launcher_file_name(&stem, cfg!(windows)))
}

fn launcher_file_name(stem: &str, windows: bool) -> String {
    let name = invocation::command_name(stem);
    if windows { format!("{name}.exe") } else { name }
}

/// One rule for what a failure costs the caller. The documented scheme is
/// 2 usage, 3 not found, 4 ambiguous, 5 timeout, 6 permission, 7 unsupported,
/// 8 unavailable, 9 stale, 10 failed — and an agent branches on it, so a
/// command that answered every problem with `1` was making that impossible.
fn report(outcome: anyhow::Result<()>) -> i32 {
    match outcome {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("Error: {error}");
            crate::guard::exit_code(&error)
        }
    }
}

/// Whether the first argument names one of our subcommands.
///
/// The explicit launcher argument covers anything launched through our own
/// shim; this covers
/// someone running the executable inside the bundle directly, which is what a
/// developer does and what a `--help` in a bug report looks like.
const COMMANDS: &[&str] = &[
    "browser",
    #[cfg(feature = "desktop")]
    "computer",
    "control",
    "doctor",
    "screenshot",
    "windows",
    "mouse",
    "key",
    "help",
    "--help",
    "-h",
];

pub(crate) fn is_builtin_command_name(name: &str) -> bool {
    COMMANDS.contains(&name)
}

fn first_argument_is_a_command(args: &[OsString]) -> bool {
    args.get(1)
        .and_then(|first| first.to_str())
        .is_some_and(|first| COMMANDS.contains(&first) || extra::is_registered(first))
}

fn is_top_level_help(args: &[OsString]) -> bool {
    let args = args
        .iter()
        .skip(1)
        .filter_map(|arg| arg.to_str())
        .collect::<Vec<_>>();
    match args.as_slice() {
        [] => true,
        [help] => matches!(*help, "--help" | "-h" | "help"),
        _ => false,
    }
}

fn print_extra_commands() {
    let extras = extra::all();
    if extras.is_empty() {
        return;
    }
    eprintln!();
    for command in extras {
        eprintln!("  {:<12} {}", command.name, command.about);
    }
}

/// Remove the launcher's private argument before clap or a provider sees argv.
fn invocation_args() -> (bool, Vec<OsString>) {
    strip_cli_argument(std::env::args_os().collect())
}

fn strip_cli_argument(mut args: Vec<OsString>) -> (bool, Vec<OsString>) {
    let argument = OsStr::new(invocation::CLI_ARGUMENT);
    let position = args
        .iter()
        .skip(1)
        .position(|arg| arg == argument)
        .map(|i| i + 1);
    if let Some(position) = position {
        args.remove(position);
        (true, args)
    } else {
        (false, args)
    }
}

/// The launcher argument is authoritative. TTY remains a Unix fallback for a
/// direct invocation, including commands typed in a product-hosted terminal.
fn invocation_is_command(explicit: bool, windows: bool, stdin_is_terminal: bool) -> bool {
    // GUI dev launchers detach stdin but keep stdout/stderr attached for logs.
    // Output alone therefore cannot distinguish a GUI from an interactive CLI.
    explicit || (!windows && stdin_is_terminal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_profile_guidance_matches_the_shell() {
        assert_eq!(
            path_profile_instruction(Path::new("/opt/My App's/bin"), ProfileShell::Posix),
            (
                "shell",
                r#"export PATH='/opt/My App'"'"'s/bin':"$PATH""#.to_string()
            )
        );
        assert_eq!(
            path_profile_instruction(
                Path::new(r"C:\Program Files\Demo's\bin"),
                ProfileShell::PowerShell,
            ),
            (
                "PowerShell",
                r"$env:Path = 'C:\Program Files\Demo''s\bin;' + $env:Path".to_string()
            )
        );
    }

    #[test]
    fn launcher_filename_uses_the_installer_normalization() {
        assert_eq!(launcher_file_name("My_App", true), "my-app.exe");
        assert_eq!(launcher_file_name("My App", false), "my-app");
    }

    #[test]
    fn gui_dev_launch_is_not_mistaken_for_a_command() {
        assert!(!invocation_is_command(false, false, false));
        assert!(invocation_is_command(true, false, false));
        assert!(invocation_is_command(false, false, true));
        assert!(!invocation_is_command(false, true, true));
    }

    #[test]
    fn launcher_argument_is_removed_before_command_parsing() {
        let args = vec![
            OsString::from("product"),
            OsString::from("screenshot"),
            OsString::from(invocation::CLI_ARGUMENT),
            OsString::from("--json"),
        ];
        let (explicit, args) = strip_cli_argument(args);
        assert!(explicit);
        assert_eq!(
            args,
            ["product", "screenshot", "--json"]
                .into_iter()
                .map(OsString::from)
                .collect::<Vec<_>>()
        );
    }

    /// The subcommands this recognizes before clap runs. That check is what
    /// stops a stray argument from falling through into app startup, so a
    /// command missing from it is a command that launches a window instead.
    #[test]
    fn every_subcommand_is_recognized_before_clap_parses() {
        use clap::CommandFactory;

        for command in Cli::command().get_subcommands() {
            let name = command.get_name();
            assert!(
                COMMANDS.contains(&name),
                "`{name}` would be mistaken for an app launch"
            );
        }
    }
}
