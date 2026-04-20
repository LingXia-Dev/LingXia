use crate::client;
use crate::project::DevInfo;
use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use lingxia_devtool_protocol::handlers;
use serde_json::{Value, json};
use std::path::Path;

#[derive(Args, Clone)]
#[command(disable_help_flag = true)]
pub struct LxAppOptions {
    #[arg(num_args = 0.., trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[derive(Parser, Clone)]
#[command(name = "lxdev lxapp")]
#[command(about = "Manage lxapps in the current dev session", long_about = None)]
struct LxAppCli {
    #[command(subcommand)]
    command: LxAppCommand,
}

#[derive(Subcommand, Clone)]
pub enum LxAppCommand {
    /// List open lxapps
    List {
        /// Include closed/inactive runtime instances
        #[arg(long)]
        all: bool,
        /// Print pretty JSON
        #[arg(long)]
        pretty: bool,
    },
    /// Print the current lxapp
    Current {
        /// Print pretty JSON
        #[arg(long)]
        pretty: bool,
    },
    /// Print lxapp runtime summary
    Info {
        #[arg(default_value = "current")]
        app: String,
        /// Print pretty JSON
        #[arg(long)]
        pretty: bool,
    },
    /// Print configured lxapp pages
    Pages {
        #[arg(default_value = "current")]
        app: String,
        /// Print pretty JSON
        #[arg(long)]
        pretty: bool,
    },
    /// Inspect and automate lxapp pages
    Page(PageOptions),
    /// Evaluate JavaScript in the lxapp logic runtime
    Eval {
        /// JavaScript expression, or a function body that uses return/await
        script: String,
        /// LxApp context; defaults to current
        #[arg(long, default_value = "current")]
        app: String,
        /// Timeout in milliseconds
        #[arg(long, default_value_t = 5000)]
        timeout_ms: u64,
        /// Print pretty JSON
        #[arg(long)]
        pretty: bool,
    },
    /// Open an lxapp
    Open {
        appid: String,
        /// Initial page/path
        #[arg(long)]
        path: Option<String>,
        /// release, preview, or developer
        #[arg(long, default_value = "release")]
        release_type: String,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Close an lxapp
    Close {
        #[arg(default_value = "current")]
        app: String,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Restart an lxapp
    Restart {
        #[arg(default_value = "current")]
        app: String,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Uninstall an lxapp and its data
    Uninstall {
        #[arg(default_value = "current")]
        app: String,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
}

#[derive(Args, Clone)]
pub struct PageOptions {
    #[command(subcommand)]
    command: PageCommand,
}

#[derive(Subcommand, Clone)]
pub enum PageCommand {
    /// Print the current page
    Current {
        /// LxApp context; defaults to current
        #[arg(long, default_value = "current")]
        app: String,
        /// Print pretty JSON
        #[arg(long)]
        pretty: bool,
    },
    /// List configured pages
    List {
        /// LxApp context; defaults to current
        #[arg(long, default_value = "current")]
        app: String,
        /// Print pretty JSON
        #[arg(long)]
        pretty: bool,
    },
    /// Print page status
    Info {
        /// Page name; defaults to current page
        #[arg(long)]
        page: Option<String>,
        /// LxApp context; defaults to current
        #[arg(long, default_value = "current")]
        app: String,
        /// Print pretty JSON
        #[arg(long)]
        pretty: bool,
    },
    /// Evaluate JavaScript in the page WebView
    Eval {
        /// JavaScript expression to evaluate in the page WebView
        script: String,
        /// Page name; defaults to current page
        #[arg(long)]
        page: Option<String>,
        /// LxApp context; defaults to current
        #[arg(long, default_value = "current")]
        app: String,
        /// Timeout in milliseconds
        #[arg(long, default_value_t = 5000)]
        timeout_ms: u64,
        /// Print pretty JSON
        #[arg(long)]
        pretty: bool,
    },
    /// Query element information in the page WebView
    Query {
        #[arg(long = "css")]
        selector: String,
        /// Return every matching element
        #[arg(long)]
        all: bool,
        /// Return the nth matching element
        #[arg(long)]
        index: Option<usize>,
        /// Return full text/value instead of truncating
        #[arg(long)]
        full: bool,
        /// Maximum text/value characters to include
        #[arg(long, default_value_t = 4096)]
        max_text: usize,
        /// Page name; defaults to current page
        #[arg(long)]
        page: Option<String>,
        /// LxApp context; defaults to current
        #[arg(long, default_value = "current")]
        app: String,
        /// Print pretty JSON
        #[arg(long)]
        pretty: bool,
    },
    /// Click an element in the page WebView
    Click {
        #[arg(long = "css")]
        selector: String,
        /// Click the nth matching element
        #[arg(long)]
        index: Option<usize>,
        /// Page name; defaults to current page
        #[arg(long)]
        page: Option<String>,
        /// LxApp context; defaults to current
        #[arg(long, default_value = "current")]
        app: String,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Type text into an element in the page WebView
    Type {
        #[arg(long = "css")]
        selector: String,
        #[arg(long)]
        text: String,
        /// Type into the nth matching element
        #[arg(long)]
        index: Option<usize>,
        /// Page name; defaults to current page
        #[arg(long)]
        page: Option<String>,
        /// LxApp context; defaults to current
        #[arg(long, default_value = "current")]
        app: String,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Replace an element's current value in the page WebView
    Fill {
        #[arg(long = "css")]
        selector: String,
        #[arg(long)]
        text: String,
        /// Fill the nth matching element
        #[arg(long)]
        index: Option<usize>,
        /// Page name; defaults to current page
        #[arg(long)]
        page: Option<String>,
        /// LxApp context; defaults to current
        #[arg(long, default_value = "current")]
        app: String,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Press a key in the page WebView
    Press {
        #[arg(long)]
        key: String,
        /// Page name; defaults to current page
        #[arg(long)]
        page: Option<String>,
        /// LxApp context; defaults to current
        #[arg(long, default_value = "current")]
        app: String,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Navigate back in the lxapp page stack
    Back {
        /// LxApp context; defaults to current
        #[arg(long, default_value = "current")]
        app: String,
        /// Number of pages to go back
        #[arg(long, default_value_t = 1)]
        delta: u32,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
}

pub fn execute(project_root: &Path, info: &DevInfo, options: LxAppOptions) -> Result<()> {
    let ws_url = info
        .ws_url
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("dev websocket URL is missing from .lingxia/dev.json"))?;

    if options.args.is_empty() || is_top_level_help(&options.args) {
        print_dynamic_help(commands_for_project(project_root));
        return Ok(());
    }

    let parsed = parse_lxapp_cli(options.args)?;

    match parsed.command {
        LxAppCommand::List { all, pretty } => {
            let data = client::execute_command(
                ws_url,
                handlers::lxapp::LIST,
                Some(json!({ "all": all })),
            )?
            .unwrap_or_else(|| json!([]));
            print_json(&data, pretty)?;
        }
        LxAppCommand::Current { pretty } => {
            let data = client::execute_command(ws_url, handlers::lxapp::CURRENT, None)?
                .unwrap_or(Value::Null);
            print_json(&data, pretty)?;
        }
        LxAppCommand::Info { app, pretty } => {
            let data = client::execute_command(
                ws_url,
                handlers::lxapp::INFO,
                Some(json!({ "appid": app })),
            )?
            .unwrap_or(Value::Null);
            print_json(&data, pretty)?;
        }
        LxAppCommand::Pages { app, pretty } => {
            let data = client::execute_command(
                ws_url,
                handlers::lxapp::PAGES,
                Some(json!({ "appid": app })),
            )?
            .unwrap_or(Value::Null);
            print_json(&data, pretty)?;
        }
        LxAppCommand::Page(options) => execute_page(ws_url, options)?,
        LxAppCommand::Eval {
            script,
            app,
            timeout_ms,
            pretty,
        } => {
            let data = client::execute_command(
                ws_url,
                handlers::lxapp::EVAL,
                Some(json!({
                    "appid": app,
                    "script": script,
                    "timeout_ms": timeout_ms,
                })),
            )?
            .unwrap_or(Value::Null);
            print_eval_result(&data, pretty)?;
        }
        LxAppCommand::Open {
            appid,
            path,
            release_type,
            json,
        } => {
            let data = client::execute_command(
                ws_url,
                handlers::lxapp::OPEN,
                Some(json!({
                    "appid": appid,
                    "path": path,
                    "release_type": release_type,
                })),
            )?;
            if json {
                print_json(data.as_ref().unwrap_or(&json!({})), false)?;
            } else {
                let appid = data
                    .as_ref()
                    .and_then(|value| value.get("appid"))
                    .and_then(Value::as_str)
                    .context("lxapp.open response did not include appid")?;
                println!("{appid}");
            }
        }
        LxAppCommand::Close { app, json } => action(ws_url, handlers::lxapp::CLOSE, app, json)?,
        LxAppCommand::Restart { app, json } => action(ws_url, handlers::lxapp::RESTART, app, json)?,
        LxAppCommand::Uninstall { app, json } => {
            action(ws_url, handlers::lxapp::UNINSTALL, app, json)?
        }
    }

    Ok(())
}

fn execute_page(ws_url: &str, options: PageOptions) -> Result<()> {
    match options.command {
        PageCommand::Current { app, pretty } => {
            let data = client::execute_command(
                ws_url,
                handlers::lxapp_page::CURRENT,
                Some(json!({ "appid": app })),
            )?
            .unwrap_or(Value::Null);
            print_json(&data, pretty)?;
        }
        PageCommand::List { app, pretty } => {
            let data = client::execute_command(
                ws_url,
                handlers::lxapp_page::LIST,
                Some(json!({ "appid": app })),
            )?
            .unwrap_or(Value::Null);
            print_json(&data, pretty)?;
        }
        PageCommand::Info { page, app, pretty } => {
            let data = client::execute_command(
                ws_url,
                handlers::lxapp_page::INFO,
                Some(json!({ "appid": app, "page": page })),
            )?
            .unwrap_or(Value::Null);
            print_json(&data, pretty)?;
        }
        PageCommand::Eval {
            script,
            page,
            app,
            timeout_ms,
            pretty,
        } => {
            let data = client::execute_command(
                ws_url,
                handlers::lxapp_page::EVAL,
                Some(json!({
                    "appid": app,
                    "page": page,
                    "js": script,
                    "timeout_ms": timeout_ms,
                })),
            )?
            .unwrap_or(Value::Null);
            print_eval_result(&data, pretty)?;
        }
        PageCommand::Query {
            selector,
            all,
            index,
            full,
            max_text,
            page,
            app,
            pretty,
        } => {
            if all && index.is_some() {
                return Err(anyhow::anyhow!("pass either --all or --index, not both"));
            }
            let data = client::execute_command(
                ws_url,
                handlers::lxapp_page::QUERY,
                Some(json!({
                    "appid": app,
                    "page": page,
                    "selector": selector,
                    "all": all,
                    "index": index,
                    "full": full,
                    "max_text": if full { Value::Null } else { json!(max_text) },
                })),
            )?
            .unwrap_or(Value::Null);
            print_json(&data, pretty)?;
        }
        PageCommand::Click {
            selector,
            index,
            page,
            app,
            json,
        } => {
            let data = client::execute_command(
                ws_url,
                handlers::lxapp_page::CLICK,
                Some(json!({
                    "appid": app,
                    "page": page,
                    "selector": selector,
                    "index": index,
                })),
            )?;
            print_optional_json(data, json)?;
        }
        PageCommand::Type {
            selector,
            text,
            index,
            page,
            app,
            json,
        } => {
            let data = client::execute_command(
                ws_url,
                handlers::lxapp_page::TYPE,
                Some(json!({
                    "appid": app,
                    "page": page,
                    "selector": selector,
                    "text": text,
                    "index": index,
                })),
            )?;
            print_optional_json(data, json)?;
        }
        PageCommand::Fill {
            selector,
            text,
            index,
            page,
            app,
            json,
        } => {
            let data = client::execute_command(
                ws_url,
                handlers::lxapp_page::FILL,
                Some(json!({
                    "appid": app,
                    "page": page,
                    "selector": selector,
                    "text": text,
                    "index": index,
                })),
            )?;
            print_optional_json(data, json)?;
        }
        PageCommand::Press {
            key,
            page,
            app,
            json,
        } => {
            let data = client::execute_command(
                ws_url,
                handlers::lxapp_page::PRESS,
                Some(json!({
                    "appid": app,
                    "page": page,
                    "key": key,
                })),
            )?;
            print_optional_json(data, json)?;
        }
        PageCommand::Back { app, delta, json } => {
            let data = client::execute_command(
                ws_url,
                handlers::lxapp_page::BACK,
                Some(json!({ "appid": app, "delta": delta })),
            )?;
            print_optional_json(data, json)?;
        }
    }

    Ok(())
}

fn is_top_level_help(args: &[String]) -> bool {
    matches!(args, [arg] if arg == "--help" || arg == "-h" || arg == "help")
}

fn parse_lxapp_cli(args: Vec<String>) -> Result<LxAppCli> {
    let mut argv = Vec::with_capacity(args.len() + 1);
    argv.push("lxdev lxapp".to_string());
    argv.extend(args);
    LxAppCli::try_parse_from(argv).map_err(Into::into)
}

fn commands_for_project(project_root: &Path) -> &'static [&'static str] {
    if project_root.join("lxapp.json").exists() && !project_root.join("lingxia.yaml").exists() {
        &["info", "pages", "page", "eval"]
    } else {
        &[
            "list",
            "current",
            "info",
            "pages",
            "page",
            "eval",
            "open",
            "close",
            "restart",
            "uninstall",
        ]
    }
}

fn print_dynamic_help(commands: &[&str]) {
    println!("Manage lxapps in the current dev session");
    println!();
    println!("Usage: lxdev lxapp <COMMAND>");
    println!();
    println!("Commands:");
    for command in commands {
        println!("  {:<10}{}", command, command_description(command));
    }
    println!("  help      Print this message or the help of the given command(s)");
    println!();
    println!("Options:");
    println!("  -h, --help  Print help");
}

fn command_description(command: &str) -> &'static str {
    match command {
        "list" => "List open lxapps",
        "current" => "Print the current lxapp",
        "info" => "Print lxapp runtime summary",
        "pages" => "Print configured lxapp pages",
        "page" => "Inspect and automate lxapp pages",
        "eval" => "Evaluate JavaScript in the lxapp logic runtime",
        "open" => "Open an lxapp",
        "close" => "Close an lxapp",
        "restart" => "Restart an lxapp",
        "uninstall" => "Uninstall an lxapp and its data",
        _ => "",
    }
}

fn print_optional_json(data: Option<Value>, json: bool) -> Result<()> {
    if json {
        print_json(data.as_ref().unwrap_or(&json!({})), false)?;
    }
    Ok(())
}

fn action(ws_url: &str, handler: &str, app: String, json: bool) -> Result<()> {
    let data = client::execute_command(ws_url, handler, Some(json!({ "appid": app })))?;
    if json {
        print_json(data.as_ref().unwrap_or(&json!({})), false)?;
    }
    Ok(())
}

fn print_json(value: &Value, pretty: bool) -> Result<()> {
    if pretty {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else {
        println!("{}", serde_json::to_string(value)?);
    }
    Ok(())
}

fn print_eval_result(data: &Value, pretty: bool) -> Result<()> {
    let Some(value) = data.get("value") else {
        return print_json(data, pretty);
    };
    if value.is_null() {
        return Ok(());
    }
    print_json(value, pretty)
}
