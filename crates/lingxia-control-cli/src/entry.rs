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
use std::path::Path;

use clap::Parser;
use lingxia_devtool_protocol::invocation;

use crate::transport::ControlSocket;
use crate::{app, browser, desktop, skills};

#[derive(Parser)]
#[command(
    disable_help_subcommand = true,
    about = "Drive this product from the command line"
)]
struct Cli {
    /// Authorize input sent to this product's own windows. The machine and
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
    Computer(desktop::DesktopOptions),
    /// Write an agent skill describing these commands
    Skills(skills::SkillsOptions),
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
    let allow_control = cli.allow_control;
    Some(match cli.command {
        Command::Browser(options) => {
            let context = browser::BrowserContext {
                transport: &transport,
                target: std::env::consts::OS.to_string(),
            };
            report(browser::execute(&context, options))
        }
        Command::Computer(options) => desktop::execute(&desktop::Backend::App(&transport), options),
        Command::Skills(options) => skills::execute::<Cli>(&manifest(&transport), options),
        Command::Control { action } => control(state_dir, action),
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

/// The persisted answer lives beside the app's data, which is the directory
/// above the host-owned state directory the socket is in.
fn control(state_dir: &Path, action: ControlAction) -> i32 {
    let Some(app_data_dir) = state_dir.parent() else {
        eprintln!("Error: cannot locate this product's data directory");
        return 10;
    };
    let wanted = match action {
        ControlAction::Status => {
            let on = lingxia_settings::control_enabled(app_data_dir);
            println!("automation interface: {}", if on { "on" } else { "off" });
            return 0;
        }
        ControlAction::Enable => true,
        ControlAction::Disable => false,
    };
    match lingxia_settings::set_control_enabled(app_data_dir, wanted) {
        Ok(()) => {
            println!(
                "automation interface: {}",
                if wanted { "on" } else { "off" }
            );
            // The running app reads this when it starts. Saying so beats a
            // silent success that appears to have done nothing.
            println!("takes effect the next time this product starts");
            0
        }
        Err(error) => {
            eprintln!("Error: {error}");
            10
        }
    }
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
/// The marker covers anything launched through our own shim; this covers
/// someone running the executable inside the bundle directly, which is what a
/// developer does and what a `--help` in a bug report looks like.
const COMMANDS: &[&str] = &[
    "browser",
    "computer",
    "control",
    "skills",
    "doctor",
    "screenshot",
    "windows",
    "mouse",
    "key",
    "help",
    "--help",
    "-h",
];

fn first_argument_is_a_command() -> bool {
    std::env::args()
        .nth(1)
        .is_some_and(|first| COMMANDS.contains(&first.as_str()))
}

/// What the product says about itself in a generated skill.
///
/// The command name comes from the executable actually running, so a skill
/// written by a development build names that build rather than a constant
/// baked in somewhere else.
fn manifest(transport: &dyn crate::transport::Transport) -> skills::Manifest {
    let executable = std::env::current_exe().unwrap_or_default();
    let stem = executable
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("app")
        .strip_suffix(std::env::consts::EXE_SUFFIX)
        .unwrap_or("app")
        .to_string();
    let mut manifest = skills::manifest_for(command_name(&stem), stem);
    // Ask the product rather than assume: a skill that lists a namespace the
    // socket refuses is worse than one that admits it does not know.
    manifest.declared = transport
        .request(lingxia_devtool_protocol::handlers::app::DOCTOR, None)
        .ok()
        .and_then(|result| result?.get("declared")?.as_array().cloned())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect()
        });
    manifest
}

/// Only the same alphabet the launcher uses, so the skill names the command a
/// user can actually type.
fn command_name(product_name: &str) -> String {
    let mut name = String::new();
    for character in product_name.chars() {
        if character.is_ascii_alphanumeric() {
            name.push(character.to_ascii_lowercase());
        } else if !name.is_empty() && !name.ends_with('-') {
            name.push('-');
        }
    }
    let name = name.trim_end_matches('-').to_string();
    if name.is_empty() { "app".into() } else { name }
}

/// Whether this process was started through the product's own launcher.
///
/// The marker is authoritative. A tty check remains as the fallback for a
/// direct invocation, which on Unix still tells app from command line — on
/// Windows it cannot, because a host spawned by a console tool inherits that
/// console.
fn invoked_as_command() -> bool {
    if std::env::var_os(invocation::MARKER).is_some() {
        return true;
    }
    !cfg!(windows) && (std::io::stdout().is_terminal() || std::io::stdin().is_terminal())
}

#[cfg(test)]
mod tests {
    use super::*;

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
