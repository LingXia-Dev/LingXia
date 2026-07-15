use anyhow::Result;
use clap::{ArgGroup, Args, Parser, Subcommand};
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
    /// Automate the host app surface in the current dev session
    App(app::AppOptions),
    /// Attach a remote dev session by its ws URL (from `lingxia dev --lan`)
    Attach {
        /// The attach URL printed by `lingxia dev --lan`, including ?token=
        ws_url: String,
        /// Name for the remote session (defaults to the URL host)
        #[arg(long)]
        name: Option<String>,
    },
    /// Detach one attached remote, or every currently unreachable remote
    Detach(DetachOptions),
}

#[derive(Args)]
#[command(group(
    ArgGroup::new("detach_target")
        .required(true)
        .multiple(false)
        .args(["name", "unreachable"])
))]
struct DetachOptions {
    /// The attached remote session name
    #[arg(value_name = "NAME")]
    name: Option<String>,

    /// Detach all attached remote sessions that are currently unreachable
    #[arg(long)]
    unreachable: bool,
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

fn main() {
    let args = std::env::args_os().collect::<Vec<_>>();
    let json_errors = args.iter().any(|arg| arg == "--json" || arg == "--pretty");
    let pretty_errors = args.iter().any(|arg| arg == "--pretty");

    if let Err(err) = run() {
        if let Some(clap_err) = err.downcast_ref::<clap::Error>() {
            if matches!(
                clap_err.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) {
                let _ = clap_err.print();
                std::process::exit(0);
            }
            if !json_errors {
                let _ = clap_err.print();
                std::process::exit(2);
            }
        }

        let exit_code = if err.downcast_ref::<clap::Error>().is_some() {
            2
        } else {
            1
        };
        if json_errors {
            let code = if exit_code == 2 {
                "invalid_arguments"
            } else {
                "command_failed"
            };
            let causes = err
                .chain()
                .skip(1)
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            let envelope = serde_json::json!({
                "error": {
                    "code": code,
                    "message": err.to_string(),
                    "causes": causes,
                    "exit_code": exit_code,
                }
            });
            let encoded = if pretty_errors {
                serde_json::to_string_pretty(&envelope)
            } else {
                serde_json::to_string(&envelope)
            };
            eprintln!("{}", encoded.unwrap_or_else(|_| envelope.to_string()));
        } else {
            eprintln!("Error: {err:#}");
        }
        std::process::exit(exit_code);
    }
}

fn run() -> Result<()> {
    let cli = Cli::try_parse()?;
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
        Commands::App(options) => {
            let info = resolve(&selector)?;
            app::execute(&info, options)
        }
        Commands::Attach { ws_url, name } => remotes::attach(&ws_url, name),
        Commands::Detach(options) => {
            if options.unreachable {
                remotes::detach_unreachable()
            } else {
                remotes::detach(
                    options
                        .name
                        .as_deref()
                        .expect("clap requires a detach target"),
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn detach_usage_offers_name_or_unreachable() {
        let command = Cli::command();
        let mut detach = command
            .find_subcommand("detach")
            .expect("detach subcommand")
            .clone();
        let usage = detach.render_usage().to_string();

        assert!(usage.contains("--unreachable"));
        assert!(usage.contains("NAME"));
        assert!(Cli::try_parse_from(["lxdev", "detach", "win"]).is_ok());
        assert!(Cli::try_parse_from(["lxdev", "detach", "--unreachable"]).is_ok());
        assert!(Cli::try_parse_from(["lxdev", "detach"]).is_err());
        assert!(Cli::try_parse_from(["lxdev", "detach", "win", "--unreachable"]).is_err());
    }
}
