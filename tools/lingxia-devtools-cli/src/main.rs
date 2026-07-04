use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use std::path::Path;

mod app;
mod browser;
mod client;
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
    /// Select the dev session by id prefix or platform name (android, ios,
    /// macos, harmony, windows, lxapp). Optional when only one session is live.
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
    /// List or prune dev sessions for this project
    Sessions(SessionsCmd),
    /// Operate on the host app as a whole (window-level screenshot, etc.)
    App(app::AppOptions),
}

#[derive(Args, Clone)]
struct SessionsCmd {
    #[command(subcommand)]
    command: Option<SessionsAction>,

    /// Print pretty JSON output (list only — ignored when a subcommand is given)
    #[arg(long)]
    json: bool,
}

#[derive(Subcommand, Clone)]
enum SessionsAction {
    /// Remove session files whose WS server no longer responds
    Prune,
    /// Stop a dev session by asking its owning `lingxia dev` process to exit
    Stop {
        /// Session id prefix or platform name. Overrides the global --session selector.
        session: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let project_root = std::env::current_dir()?;
    let selector = SessionSelector { query: cli.session };

    match cli.command {
        Commands::Browser(options) => {
            let info = project::resolve_session(&project_root, &selector)?;
            browser::execute(&info, options)
        }
        Commands::Lxapp(options) => {
            let info = project::resolve_session(&project_root, &selector)?;
            lxapp::execute(&project_root, &info, options)
        }
        Commands::Logs(options) => {
            let info = project::resolve_session(&project_root, &selector)?;
            logs::execute(Path::new(&info.log_file), options)
        }
        Commands::Sessions(cmd) => match cmd.command {
            Some(SessionsAction::Prune) => sessions::execute_prune(&project_root),
            Some(SessionsAction::Stop { session }) => {
                let selector = SessionSelector {
                    query: session.or(selector.query),
                };
                sessions::execute_stop(&project_root, &selector)
            }
            None => sessions::execute_list(&project_root, cmd.json),
        },
        Commands::App(options) => {
            let info = project::resolve_session(&project_root, &selector)?;
            app::execute(&info, options)
        }
    }
}
