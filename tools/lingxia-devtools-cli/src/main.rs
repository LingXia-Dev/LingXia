use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use std::path::Path;

mod app;
mod browser;
mod client;
mod desktop;
mod logs;
mod lxapp;
mod lxapp_build;
mod project;
mod screenshot;
mod sessions;

use project::SessionSelector;

#[derive(Parser)]
#[command(name = "lxdev")]
#[command(about = "LingXia devtools client", long_about = None)]
#[command(version)]
struct Cli {
    /// Select the dev session by id prefix or target name (android, ios,
    /// macos, harmony, windows, lxapp). Optional when only one session is
    /// live. Falls back to the LXDEV_SESSION env var.
    #[arg(long, global = true)]
    session: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Control browser tabs in the current dev session
    Browser(browser::BrowserOptions),
    /// Manage lxapps in the current dev session
    Lxapp(lxapp::LxAppOptions),
    /// Query and filter the current dev session log file
    Logs(logs::LogsOptions),
    /// List or stop live dev sessions
    #[command(alias = "sessions")]
    Session(SessionCmd),
    /// Automate the local desktop OS (no dev session required)
    Desktop(desktop::DesktopOptions),
    /// Removed: session commands moved under `lxdev lxapp`
    #[command(hide = true)]
    App(app::AppOptions),
}

#[derive(Args, Clone)]
struct SessionCmd {
    #[command(subcommand)]
    command: Option<SessionAction>,

    /// Print pretty JSON output (list only — ignored when a subcommand is given)
    #[arg(long)]
    json: bool,
}

#[derive(Subcommand, Clone)]
enum SessionAction {
    /// List live dev sessions
    List {
        /// Print pretty JSON output
        #[arg(long)]
        json: bool,
    },
    /// Stop a dev session by asking its owning `lingxia dev` process to exit
    Stop {
        /// Session id prefix or target name. Overrides --session.
        session: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let selector = SessionSelector {
        query: cli.session.or_else(|| std::env::var("LXDEV_SESSION").ok()),
    };

    match cli.command {
        Commands::Browser(options) => {
            let info = project::resolve_session(&selector)?;
            browser::execute(&info, options)
        }
        Commands::Lxapp(options) => {
            if lxapp::handle_pre_session(&std::env::current_dir()?, &options)? {
                return Ok(());
            }
            let info = project::resolve_session(&selector)?;
            let project_root = std::path::PathBuf::from(&info.project_root);
            lxapp::execute(&project_root, &info, options)
        }
        Commands::Logs(options) => {
            let info = project::resolve_session(&selector)?;
            logs::execute(Path::new(&info.log_file), options)
        }
        Commands::Session(cmd) => match cmd.command {
            Some(SessionAction::List { json }) => sessions::execute_list(json),
            Some(SessionAction::Stop { session }) => {
                let selector = SessionSelector {
                    query: session.or(selector.query),
                };
                sessions::execute_stop(&selector)
            }
            None => sessions::execute_list(cmd.json),
        },
        // Local OS automation: no dev session; the handler owns process exit.
        Commands::Desktop(options) => desktop::execute(options),
        // Removed namespace: emit a migration hint without needing a session.
        Commands::App(options) => app::migrate(options),
    }
}
