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

use clap::{CommandFactory, FromArgMatches, Parser};
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
}

/// Run the command line and return its exit code, or `None` to become the app.
pub fn run_if_invoked(app_data_dir: &Path) -> Option<i32> {
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

    let endpoint = crate::transport::endpoint_in(app_data_dir);
    let transport = ControlSocket::at(endpoint);
    if let Some(command) = args
        .get(1)
        .and_then(|arg| arg.to_str())
        .and_then(extra::get)
    {
        return Some((command.execute)(&transport, &args[2..]));
    }
    let matches = match cli_command(&extra::all()).try_get_matches_from(&args) {
        Ok(matches) => matches,
        Err(error) => {
            let _ = error.print();
            return Some(error.exit_code());
        }
    };
    let cli = match Cli::from_arg_matches(&matches) {
        Ok(cli) => cli,
        // Parsing hands back help and usage rather than printing them, and the
        // host exits on our code — so nothing else will ever show them.
        Err(error) => {
            let _ = error.print();
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
/// The explicit private argument covers agent tooling; this also covers
/// someone running the executable inside the bundle directly, which is what a
/// developer does and what a `--help` in a bug report looks like.
const COMMANDS: &[&str] = &[
    "browser",
    #[cfg(feature = "desktop")]
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

pub(crate) fn is_builtin_command_name(name: &str) -> bool {
    COMMANDS.contains(&name)
}

fn first_argument_is_a_command(args: &[OsString]) -> bool {
    args.get(1)
        .and_then(|first| first.to_str())
        .is_some_and(|first| COMMANDS.contains(&first) || extra::is_registered(first))
}

fn cli_command(extras: &[crate::ExtraProductCommand]) -> clap::Command {
    let mut command = Cli::command();
    for extra in extras {
        command = command.subcommand(clap::Command::new(extra.name).about(extra.about));
    }
    command
}

/// Remove the agent integration's private argument before clap or a provider sees argv.
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

/// The explicit argument is authoritative. TTY remains a Unix fallback for a
/// direct invocation, including commands typed in a product-hosted terminal.
fn invocation_is_command(explicit: bool, windows: bool, stdin_is_terminal: bool) -> bool {
    // GUI dev launchers detach stdin but keep stdout/stderr attached for logs.
    // Output alone therefore cannot distinguish a GUI from an interactive CLI.
    explicit || (!windows && stdin_is_terminal)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_op_extra(_transport: &dyn crate::transport::Transport, _args: &[OsString]) -> i32 {
        0
    }

    #[test]
    fn host_commands_render_inside_the_commands_section() {
        let extras = [crate::ExtraProductCommand {
            name: "function",
            about: "Call product functions",
            execute: no_op_extra,
        }];
        let help = cli_command(&extras).render_help().to_string();
        let function = help.find("function").expect("host command");
        let options = help.find("Options:").expect("options section");

        assert!(function < options, "{help}");
    }

    #[test]
    fn gui_dev_launch_is_not_mistaken_for_a_command() {
        assert!(!invocation_is_command(false, false, false));
        assert!(invocation_is_command(true, false, false));
        assert!(invocation_is_command(false, false, true));
        assert!(!invocation_is_command(false, true, true));
    }

    #[test]
    fn cli_argument_is_removed_before_command_parsing() {
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
