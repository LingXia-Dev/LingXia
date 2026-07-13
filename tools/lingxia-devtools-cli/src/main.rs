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
mod remotes;
mod screenshot;
mod sessions;

use project::SessionSelector;

#[derive(Parser)]
#[command(name = "lxdev")]
#[command(about = "LingXia devtools client", long_about = None)]
#[command(version)]
struct Cli {
    /// Select the dev session by id prefix or target name (android, ios,
    /// macos, harmony, windows, lxapp) or an attached remote name. Optional
    /// when only one session is live. Falls back to the LXDEV_SESSION env var.
    #[arg(long, global = true)]
    session: Option<String>,

    /// Target a dev websocket URL directly (one-off remote control), e.g.
    /// "ws://192.168.1.20:39142/?token=…" printed by `lingxia dev --lan`.
    /// For a persistent pairing use `lxdev attach` instead. Falls back to
    /// the LXDEV_WS env var.
    #[arg(long, global = true)]
    ws: Option<String>,

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
    /// Attach a remote dev session by its ws URL (from `lingxia dev --lan`)
    Attach {
        /// The attach URL printed by `lingxia dev --lan`, including ?token=
        ws_url: String,
        /// Name for the remote session (defaults to the URL host)
        #[arg(long)]
        name: Option<String>,
    },
    /// Detach a previously attached remote dev session
    Detach {
        /// The attached session name
        name: String,
    },
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
    let direct_ws = cli.ws.or_else(|| std::env::var("LXDEV_WS").ok());
    let resolve = |selector: &SessionSelector| -> Result<project::SessionInfo> {
        match &direct_ws {
            Some(ws_url) => Ok(remotes::direct_session_info(ws_url)),
            None => project::resolve_session(selector),
        }
    };

    match cli.command {
        Commands::Browser(options) => {
            let info = resolve(&selector)?;
            browser::execute(&info, options)
        }
        Commands::Lxapp(options) => {
            if lxapp::handle_pre_session(&std::env::current_dir()?, &options)? {
                return Ok(());
            }
            let info = resolve(&selector)?;
            let project_root = std::path::PathBuf::from(&info.project_root);
            lxapp::execute(&project_root, &info, options)
        }
        Commands::Logs(options) => {
            let info = resolve(&selector)?;
            if info.log_file.is_empty() {
                // Remote session: the log file lives on the host machine —
                // stream it through the dev server instead.
                logs::execute_remote(&info.ws_url, options)
            } else {
                logs::execute(Path::new(&info.log_file), options)
            }
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
        Commands::Attach { ws_url, name } => remotes::attach(&ws_url, name),
        Commands::Detach { name } => remotes::detach(&name),
    }
}
