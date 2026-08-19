use anyhow::Result;
use clap::{Args, Parser, Subcommand};

mod client;
mod logs;
mod lxapp;
mod lxapp_build;
mod project;
mod runner;
mod screenshot;
mod sessions;
mod test;
mod test_bundle;

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
    Browser(lingxia_control_commands::browser::BrowserOptions),
    /// Manage lxapps in the current dev session
    Lxapp(lxapp::LxAppOptions),
    /// Control the simulated environment (device, orientation, appearance);
    /// runner sessions only
    Runner(runner::RunnerOptions),
    /// Query and filter the current dev session log file
    Logs(logs::LogsOptions),
    /// List live dev sessions
    #[command(alias = "sessions")]
    Session(SessionCmd),
    /// Automate the local desktop OS (no dev session required)
    Desktop(lingxia_control_commands::desktop::DesktopOptions),
    /// Automate the host app surface in the current dev session
    App(lingxia_control_commands::app::AppOptions),
    /// Run JavaScript/TypeScript test cases in the current dev session
    Test(test::TestOptions),
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
    let resolve = project::resolve_session;

    match cli.command {
        Commands::Browser(options) => {
            let info = resolve(&selector)?;
            let transport = client::DevSession::new(&info.ws_url);
            let context = lingxia_control_commands::browser::BrowserContext {
                transport: &transport,
                target: info.target.clone(),
            };
            lingxia_control_commands::browser::execute(&context, options)
        }
        Commands::Lxapp(options) => {
            if lxapp::handle_pre_session(&std::env::current_dir()?, &options)? {
                return Ok(());
            }
            let info = resolve(&selector)?;
            let project_root = std::path::PathBuf::from(&info.project_root);
            lxapp::execute(&project_root, &info, options)
        }
        Commands::Runner(options) => {
            let info = resolve(&selector)?;
            runner::execute(&info, options)
        }
        Commands::Logs(options) => {
            let info = resolve(&selector)?;
            logs::execute(&info, options)
        }
        Commands::Session(cmd) => match cmd.command {
            Some(SessionAction::List { json }) => sessions::execute_list(json),
            None => sessions::execute_list(cmd.json),
        },
        // Local OS automation, no dev session. A development tool runs these
        // in its own process: a developer grants lxdev Accessibility once, and
        // there is no product to route to.
        Commands::Desktop(options) => {
            std::process::exit(lingxia_control_commands::desktop::execute(
                &lingxia_control_commands::desktop::Backend::Local,
                options,
            ))
        }
        Commands::App(options) => {
            let info = resolve(&selector)?;
            let transport = client::DevSession::new(&info.ws_url);
            let context = lingxia_control_commands::app::AppContext {
                transport: &transport,
                target: info.target.clone(),
                session: Some(info.session_id.clone()),
            };
            lingxia_control_commands::app::execute(&context, options)
        }
        // Session test runner: the handler owns process exit (run state
        // becomes the exit code).
        Commands::Test(options) => {
            let info = resolve(&selector).map_err(|err| {
                if test::looks_unreachable(&err) {
                    anyhow::anyhow!(test::NO_SESSION_HINT)
                } else {
                    err
                }
            })?;
            test::execute(&info, options)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remote_control_options_are_not_exposed() {
        assert!(Cli::try_parse_from(["lxdev", "attach", "ws://host:39000"]).is_err());
        assert!(Cli::try_parse_from(["lxdev", "detach", "host"]).is_err());
        assert!(Cli::try_parse_from(["lxdev", "--ws", "ws://host:39000", "session"]).is_err());
    }

    #[test]
    fn session_lifecycle_commands_belong_to_lingxia() {
        assert!(Cli::try_parse_from(["lxdev", "stop", "windows"]).is_err());
        assert!(Cli::try_parse_from(["lxdev", "session", "stop", "windows"]).is_err());
    }

    #[test]
    fn logs_accepts_dynamic_origin_and_rejects_removed_source_flags() {
        let cli = Cli::try_parse_from(["lxdev", "logs", "service.api", "--follow"]).unwrap();
        let Commands::Logs(options) = cli.command else {
            panic!("expected logs command");
        };
        assert_eq!(options.origin.as_deref(), Some("service.api"));
        assert!(options.follow);
        assert!(Cli::try_parse_from(["lxdev", "logs", "--source", "native"]).is_err());
        assert!(Cli::try_parse_from(["lxdev", "logs", "--wide"]).is_err());
    }

    #[test]
    fn browser_user_agent_commands_have_stable_cli_shapes() {
        let cli = Cli::try_parse_from([
            "lxdev",
            "browser",
            "ua",
            "set",
            "TestAgent/1.0",
            "--reload",
            "--json",
        ])
        .unwrap();
        let Commands::Browser(options) = cli.command else {
            panic!("expected browser command");
        };
        let lingxia_control_commands::browser::BrowserCommand::UserAgent(options) = options.command
        else {
            panic!("expected user-agent command");
        };
        assert!(options.json);
        assert!(!options.pretty);
        assert!(matches!(
            options.command,
            lingxia_control_commands::browser::UserAgentCommand::Set {
                user_agent,
                reload: true,
            } if user_agent == "TestAgent/1.0"
        ));

        assert!(
            Cli::try_parse_from([
                "lxdev",
                "browser",
                "user-agent",
                "show",
                "--json",
                "--pretty",
            ])
            .is_err()
        );
        assert!(Cli::try_parse_from(["lxdev", "browser", "ua", "reset", "--reload"]).is_ok());
        assert!(Cli::try_parse_from(["lxdev", "browser", "user-agent", "show"]).is_ok());
        assert!(Cli::try_parse_from(["lxdev", "browser", "ua", "show", "--tab", "docs"]).is_err());
        assert!(
            Cli::try_parse_from(["lxdev", "browser", "ua", "configure", "TestAgent/1.0"]).is_err()
        );
    }

    #[test]
    fn test_command_accepts_directory_grep_and_forbid_only() {
        let cli =
            Cli::try_parse_from(["lxdev", "test", "tests/", "--grep", "home", "--forbid-only"])
                .unwrap();
        let Commands::Test(options) = cli.command else {
            panic!("expected test command");
        };
        assert_eq!(options.entry, std::path::PathBuf::from("tests/"));
        assert_eq!(options.grep.as_deref(), Some("home"));
        assert!(options.forbid_only);
    }
}
