use anyhow::Result;
use clap::{Args, Parser, Subcommand};

mod client;
mod logs;
mod lxapp;
mod network;
mod project;
mod runner;
mod scenario;
mod screenshot;
mod sessions;
mod test;
mod test_bundle;
mod test_contract;
mod test_network;
mod test_preset;
mod test_report;
mod test_secrets;
mod test_state;

use project::SessionSelector;

#[derive(Parser)]
#[command(name = "lxdev")]
#[command(about = "LingXia devtools client", long_about = None)]
#[command(version = env!("LXDEV_BUILD_VERSION"))]
struct Cli {
    /// Select the dev session: a name (`lingxia dev --name`), a target
    /// (android, ios, macos, harmony, windows, lxapp), `target@<project-dir>`,
    /// the # from `lxdev session list`, or an id prefix. Without it: the
    /// session of this directory's project, else the only live session.
    /// Falls back to the LXDEV_SESSION env var
    #[arg(long, global = true, value_name = "SESSION")]
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
    /// Automate the host surface in the current dev session: windows,
    /// screenshots, mouse and keyboard input, app links
    Host(lingxia_control_commands::app::AppOptions),
    /// Run JavaScript/TypeScript test cases in the current dev session
    Test(Box<test::TestOptions>),
    /// Put the running app into a named product state from
    /// tests/scenarios/ (development hosts only)
    Scenario(scenario::ScenarioOptions),
    /// Inspect and record the running lxapp's Logic network traffic
    /// (development hosts only)
    Network(network::NetworkOptions),
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
    let json_errors = args.iter().enumerate().any(|(index, arg)| {
        arg == "--json"
            || arg == "--pretty"
            || arg == "--jsonl"
            || arg == "--format=json"
            || arg == "--format=jsonl"
            || (arg == "--format"
                && args
                    .get(index + 1)
                    .is_some_and(|value| value == "json" || value == "jsonl"))
    });
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
    let cwd = std::env::current_dir()?;
    let argv = test_preset::expand(std::env::args_os().collect(), &cwd)?;
    let cli = Cli::try_parse_from(&argv)?;
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
        Commands::Lxapp(mut options) => {
            let selector = match options.take_session() {
                Some(query) => SessionSelector { query: Some(query) },
                None => selector,
            };
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
        Commands::Host(options) => {
            let info = resolve(&selector)?;
            let transport = client::DevSession::new(&info.ws_url);
            let context = lingxia_control_commands::app::AppContext {
                transport: &transport,
                target: info.target.clone(),
                session: Some(info.session_id.clone()),
            };
            lingxia_control_commands::app::execute(&context, options)
        }
        Commands::Scenario(options) => {
            // Listing needs no session: it falls back to this directory's
            // project.
            let info = match resolve(&selector) {
                Ok(info) => Some(info),
                Err(err)
                    if options.is_list() && err.to_string().contains("No live dev session") =>
                {
                    None
                }
                Err(err) => return Err(err),
            };
            scenario::execute(info.as_ref(), options)
        }
        Commands::Network(options) => {
            let info = resolve(&selector)?;
            network::execute(&info, options)
        }
        // Session test runner: the handler owns process exit (run state
        // becomes the exit code).
        Commands::Test(options) => {
            if let Some(test::TestCommand::Report(report)) = &options.command {
                return test_report::execute(report);
            }
            if options.list_presets {
                return test_preset::list(&cwd, options.machine());
            }
            if options.print_args {
                return test_preset::print_args(&argv, options.machine());
            }
            let info = resolve(&selector).map_err(|err| {
                if test::looks_unreachable(&err) {
                    anyhow::anyhow!(test::NO_SESSION_HINT)
                } else {
                    err
                }
            })?;
            test::execute(&info, *options)
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
    fn the_host_surface_is_lxdev_host_and_app_is_gone() {
        for argv in [
            vec!["lxdev", "host", "windows", "--json"],
            vec!["lxdev", "host", "screenshot", "-o", "-"],
            vec!["lxdev", "host", "applink", "https://example.com/x"],
            vec!["lxdev", "--session", "demo", "host", "doctor"],
        ] {
            assert!(Cli::try_parse_from(&argv).is_ok(), "{argv:?}");
        }
        // Renamed without an alias: `app` is not a command any more.
        assert!(Cli::try_parse_from(["lxdev", "app", "windows"]).is_err());
    }

    #[test]
    fn test_report_needs_no_entry_or_session() {
        let cli = Cli::try_parse_from(["lxdev", "test", "report", "--failures"]).unwrap();
        let Commands::Test(options) = cli.command else {
            panic!("expected test command");
        };
        assert!(matches!(
            options.command,
            Some(test::TestCommand::Report(_))
        ));
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
        assert_eq!(options.entry, Some(std::path::PathBuf::from("tests/")));
        assert_eq!(options.grep.as_deref(), Some("home"));
        assert!(options.forbid_only);
    }

    #[test]
    fn network_commands_have_stable_cli_shapes() {
        for argv in [
            vec!["lxdev", "network", "status"],
            vec!["lxdev", "network", "status", "--json"],
            vec![
                "lxdev",
                "network",
                "record",
                "start",
                "--match",
                "**/api/**",
            ],
            vec![
                "lxdev", "network", "record", "stop", "--out", "r.json", "--redact", "x",
            ],
            vec![
                "lxdev", "network", "record", "stop", "--out", "r.json", "--name", "offline",
            ],
        ] {
            assert!(Cli::try_parse_from(&argv).is_ok(), "{argv:?}");
        }
        assert!(Cli::try_parse_from(["lxdev", "network", "record", "stop"]).is_err());
        // Scenarios moved to `lxdev scenario`.
        assert!(Cli::try_parse_from(["lxdev", "network", "scenario", "use", "s.json"]).is_err());
        assert!(Cli::try_parse_from(["lxdev", "network", "scenario", "clear"]).is_err());
    }

    #[test]
    fn scenario_commands_have_stable_cli_shapes() {
        for argv in [
            vec!["lxdev", "scenario", "list"],
            vec!["lxdev", "scenario", "list", "--json"],
            vec!["lxdev", "scenario", "use", "qoe/offline"],
            vec![
                "lxdev",
                "scenario",
                "use",
                "tests/scenarios/offline.json",
                "--appid",
                "app",
                "--json",
            ],
            vec!["lxdev", "scenario", "status", "--json"],
            vec!["lxdev", "scenario", "clear"],
        ] {
            assert!(Cli::try_parse_from(&argv).is_ok(), "{argv:?}");
        }
        assert!(Cli::try_parse_from(["lxdev", "scenario", "use"]).is_err());
        let Commands::Scenario(options) = Cli::try_parse_from(["lxdev", "scenario", "list"])
            .unwrap()
            .command
        else {
            panic!("expected scenario command");
        };
        assert!(options.is_list());
    }

    #[test]
    fn test_presets_parse_with_the_command_line() {
        let parse = |argv: &[&str]| {
            let Commands::Test(options) = Cli::try_parse_from(argv).unwrap().command else {
                panic!("expected test command");
            };
            options
        };
        // What `--preset ci` expands to, then the command line: a later
        // scalar wins, repeatables add up.
        let options = parse(&[
            "lxdev", "test", "--grep", "home", "--tag", "unit", "tests/", "--preset", "ci",
            "--grep", "checkout", "--tag", "!slow",
        ]);
        assert_eq!(options.preset.as_deref(), Some("ci"));
        assert_eq!(options.grep.as_deref(), Some("checkout"));
        // Neither needs an entry.
        assert!(parse(&["lxdev", "test", "--list-presets"]).list_presets);
        assert!(parse(&["lxdev", "test", "--preset", "ci", "--print-args"]).print_args);
        assert!(Cli::try_parse_from(["lxdev", "test"]).is_err());
    }

    #[test]
    fn version_names_the_build_commit() {
        use clap::CommandFactory;
        let version = Cli::command().get_version().unwrap().to_string();
        assert!(version.starts_with(env!("CARGO_PKG_VERSION")), "{version}");
        // Outside a git checkout the stamp is just the release version.
        if std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .is_ok_and(|output| output.status.success())
        {
            assert!(version.contains(" ("), "{version}");
        }
    }
}
