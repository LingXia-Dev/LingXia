use crate::client;
use crate::lxapp_build;
use crate::project::SessionInfo;
use crate::screenshot;
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
    /// Report the selected session's automation capabilities
    Doctor {
        /// Print JSON output
        #[arg(long)]
        json: bool,
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
    /// Navigate the lxapp runtime by page name
    Nav(NavOptions),
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
    /// Rebuild the lxapp front-end bundle and reload the running lxapp
    Reload(lxapp_build::ReloadOptions),
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
    /// Restart an lxapp (without rebuilding; use `reload` for build + restart)
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
    /// Inspect or switch the simulated device (runner only)
    Device(DeviceOptions),
}

#[derive(Args, Clone)]
pub struct DeviceOptions {
    #[command(subcommand)]
    command: DeviceCommand,
}

#[derive(Subcommand, Clone)]
pub enum DeviceCommand {
    /// List the device presets the runner offers
    List {
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Print the currently selected device
    Get {
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Switch the simulated device by preset id
    Set {
        /// Device preset id (see `lxdev lxapp device list`)
        #[arg(long)]
        id: String,
        /// Force landscape orientation
        #[arg(long, conflicts_with = "portrait")]
        landscape: bool,
        /// Force portrait orientation
        #[arg(long)]
        portrait: bool,
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

#[derive(Args, Clone)]
pub struct NavOptions {
    #[command(subcommand)]
    command: NavCommand,
}

#[derive(Subcommand, Clone)]
pub enum NavCommand {
    /// Push a configured page onto the page stack
    To(PageNavOptions),
    /// Replace the current page with a configured page
    Redirect(PageNavOptions),
    /// Switch to a configured tab page
    #[command(name = "switch-tab")]
    SwitchTab(PageNavOptions),
    /// Clear the stack and relaunch at a configured page
    Relaunch(PageNavOptions),
    /// Navigate back in the lxapp page stack
    Back(NavBackOptions),
}

#[derive(Args, Clone)]
pub struct PageNavOptions {
    /// Page name from lxapp.json
    page: String,
    /// LxApp context; defaults to current
    #[arg(long, default_value = "current")]
    app: String,
    /// Query string pair; repeat for multiple keys
    #[arg(long = "query", value_name = "KEY=VALUE")]
    query: Vec<String>,
    /// Print JSON output
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone)]
pub struct NavBackOptions {
    /// LxApp context; defaults to current
    #[arg(long, default_value = "current")]
    app: String,
    /// Number of pages to go back
    #[arg(long, default_value_t = 1)]
    delta: u32,
    /// Print JSON output
    #[arg(long)]
    json: bool,
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
        /// Page name or runtime instance id; defaults to current page
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
        /// Page name or runtime instance id; defaults to current page
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
        /// Page name or runtime instance id; defaults to current page
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
        /// Page name or runtime instance id; defaults to current page
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
        /// Page name or runtime instance id; defaults to current page
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
        /// Page name or runtime instance id; defaults to current page
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
        /// Page name or runtime instance id; defaults to current page
        #[arg(long)]
        page: Option<String>,
        /// LxApp context; defaults to current
        #[arg(long, default_value = "current")]
        app: String,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Scroll the page DOM by a delta (finds the nearest scrollable container)
    Scroll {
        /// Horizontal delta in CSS pixels (positive = right)
        #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
        dx: f64,
        /// Vertical delta in CSS pixels (positive = down)
        #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
        dy: f64,
        /// Page name or runtime instance id; defaults to current page
        #[arg(long)]
        page: Option<String>,
        /// LxApp context; defaults to current
        #[arg(long, default_value = "current")]
        app: String,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Scroll the first matching DOM element into view
    ScrollTo {
        /// CSS selector to scroll into view
        #[arg(long)]
        css: String,
        /// Page name or runtime instance id; defaults to current page
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
    /// Capture a PNG screenshot of an lxapp page's WebView
    Screenshot {
        /// LxApp context; defaults to current
        #[arg(long, default_value = "current")]
        app: String,
        /// Page name or runtime instance id; defaults to current
        #[arg(long, default_value = "current")]
        page: String,
        /// Output path; use `-` for stdout. Default: .lingxia/screenshots/<app>-<page>-<ts>.png
        #[arg(long, short = 'o')]
        output: Option<String>,
        /// Print the JSON envelope instead of writing a PNG file
        #[arg(long)]
        json: bool,
    },
}

pub fn execute(project_root: &Path, info: &SessionInfo, options: LxAppOptions) -> Result<()> {
    let ws_url = info.ws_url.as_str();

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
        LxAppCommand::Doctor { json } => execute_doctor(info, json)?,
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
        LxAppCommand::Nav(options) => execute_nav(ws_url, options)?,
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
        LxAppCommand::Reload(options) => lxapp_build::execute(ws_url, &options)?,
        LxAppCommand::Restart { app, json } => action(ws_url, handlers::lxapp::RESTART, app, json)?,
        LxAppCommand::Uninstall { app, json } => {
            action(ws_url, handlers::lxapp::UNINSTALL, app, json)?
        }
        LxAppCommand::Device(options) => execute_device(ws_url, options)?,
    }

    Ok(())
}

pub fn handle_pre_session(project_root: &Path, options: &LxAppOptions) -> Result<bool> {
    if options.args.is_empty() || is_top_level_help(&options.args) {
        print_dynamic_help(commands_for_project(project_root));
        return Ok(true);
    }

    match parse_lxapp_cli(options.args.clone()) {
        Ok(_) => Ok(false),
        Err(err) => {
            if let Some(clap_err) = err.downcast_ref::<clap::Error>()
                && matches!(
                    clap_err.kind(),
                    clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
                )
            {
                clap_err.print()?;
                return Ok(true);
            }
            Err(err)
        }
    }
}

fn execute_device(ws_url: &str, options: DeviceOptions) -> Result<()> {
    match options.command {
        DeviceCommand::List { json } => {
            let data = client::execute_command(ws_url, handlers::lxapp_device::LIST, None)?
                .unwrap_or_else(|| json!([]));
            if json {
                print_json(&data, false)?;
            } else {
                print_device_list(&data);
            }
        }
        DeviceCommand::Get { json } => {
            let data = client::execute_command(ws_url, handlers::lxapp_device::GET, None)?
                .unwrap_or(Value::Null);
            if json {
                print_json(&data, false)?;
            } else {
                print_device_state(&data);
            }
        }
        DeviceCommand::Set {
            id,
            landscape,
            portrait,
            json,
        } => {
            // Leave orientation to the runner default (tablet=landscape,
            // phone/desktop=portrait) unless a flag pins it.
            let orientation = if landscape {
                Some(true)
            } else if portrait {
                Some(false)
            } else {
                None
            };
            let data = client::execute_command(
                ws_url,
                handlers::lxapp_device::SET,
                Some(json!({ "id": id, "landscape": orientation })),
            )?
            .unwrap_or(Value::Null);
            if json {
                print_json(&data, false)?;
            } else {
                print_device_state(&data);
            }
        }
    }
    Ok(())
}

fn print_device_list(data: &Value) {
    let Some(array) = data.as_array() else {
        let _ = print_json(data, false);
        return;
    };
    if array.is_empty() {
        println!("No devices reported by the session.");
        return;
    }
    println!(
        "{:<3}  {:<20}  {:<8}  {:<11}  ID",
        "CUR", "NAME", "GROUP", "SIZE"
    );
    for dev in array {
        let id = dev.get("id").and_then(Value::as_str).unwrap_or("-");
        let name = dev.get("name").and_then(Value::as_str).unwrap_or("");
        let group = dev.get("group").and_then(Value::as_str).unwrap_or("");
        let width = dev.get("width").and_then(Value::as_u64).unwrap_or(0);
        let height = dev.get("height").and_then(Value::as_u64).unwrap_or(0);
        let current = dev.get("current").and_then(Value::as_bool).unwrap_or(false);
        println!(
            "{:<3}  {:<20}  {:<8}  {:<11}  {}",
            if current { " * " } else { "" },
            name,
            group,
            format!("{width}x{height}"),
            id,
        );
    }
}

fn print_device_state(data: &Value) {
    if data.is_null() {
        println!("No device reported by the session.");
        return;
    }
    let name = data.get("name").and_then(Value::as_str).unwrap_or("");
    let id = data.get("id").and_then(Value::as_str).unwrap_or("-");
    let width = data.get("width").and_then(Value::as_u64).unwrap_or(0);
    let height = data.get("height").and_then(Value::as_u64).unwrap_or(0);
    let landscape = data
        .get("landscape")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let orientation = if landscape { "landscape" } else { "portrait" };
    println!("{name} ({id})  {width}x{height}  {orientation}");
}

fn execute_doctor(info: &SessionInfo, json: bool) -> Result<()> {
    let mut data = client::execute_command(&info.ws_url, handlers::lxapp::DOCTOR, None)?
        .unwrap_or_else(|| json!({}));
    // The runtime doesn't know the CLI's session id; graft it on so the doctor
    // envelope is self-describing.
    if let Value::Object(map) = &mut data {
        map.insert("session_id".to_string(), json!(info.session_id));
    }

    if json {
        print_json(&data, false)?;
        return Ok(());
    }

    let get = |path: &[&str]| -> Option<&Value> {
        let mut cur = &data;
        for key in path {
            cur = cur.get(*key)?;
        }
        Some(cur)
    };
    let cap = |name: &str| -> String {
        match get(&["capabilities", name, "supported"]).and_then(Value::as_bool) {
            Some(true) => {
                let tier = get(&["capabilities", name, "tier"])
                    .and_then(Value::as_str)
                    .map(|t| format!(" ({t})"))
                    .unwrap_or_default();
                format!("yes{tier}")
            }
            _ => "no".to_string(),
        }
    };
    println!("session      {}", info.session_id);
    println!(
        "platform     {}",
        get(&["platform"]).and_then(Value::as_str).unwrap_or("-")
    );
    println!(
        "backend      {}",
        get(&["backend"]).and_then(Value::as_str).unwrap_or("-")
    );
    println!("page shot    {}", cap("page_screenshot"));
    println!("page input   {}", cap("page_input"));
    println!("runner       {}", cap("runner"));
    Ok(())
}

fn execute_nav(ws_url: &str, options: NavOptions) -> Result<()> {
    match options.command {
        NavCommand::To(options) => execute_page_nav(ws_url, handlers::lxapp_nav::TO, options)?,
        NavCommand::Redirect(options) => {
            execute_page_nav(ws_url, handlers::lxapp_nav::REDIRECT, options)?
        }
        NavCommand::SwitchTab(options) => {
            execute_page_nav(ws_url, handlers::lxapp_nav::SWITCH_TAB, options)?
        }
        NavCommand::Relaunch(options) => {
            execute_page_nav(ws_url, handlers::lxapp_nav::RELAUNCH, options)?
        }
        NavCommand::Back(options) => {
            let data = client::execute_command(
                ws_url,
                handlers::lxapp_nav::BACK,
                Some(json!({ "appid": options.app, "delta": options.delta })),
            )?;
            print_optional_json(data, options.json)?;
        }
    }
    Ok(())
}

fn execute_page_nav(ws_url: &str, handler: &str, options: PageNavOptions) -> Result<()> {
    let query = parse_query_pairs(&options.query)?;
    let data = client::execute_command(
        ws_url,
        handler,
        Some(json!({
            "appid": options.app,
            "page": options.page,
            "query": query,
        })),
    )?;
    print_optional_json(data, options.json)
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
        PageCommand::Scroll {
            dx,
            dy,
            page,
            app,
            json,
        } => {
            let data = client::execute_command(
                ws_url,
                handlers::lxapp_page::SCROLL,
                Some(json!({
                    "appid": app,
                    "page": page,
                    "dx": dx,
                    "dy": dy,
                })),
            )?;
            print_optional_json(data, json)?;
        }
        PageCommand::ScrollTo {
            css,
            page,
            app,
            json,
        } => {
            let data = client::execute_command(
                ws_url,
                handlers::lxapp_page::SCROLL_TO,
                Some(json!({
                    "appid": app,
                    "page": page,
                    "selector": css,
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
        PageCommand::Screenshot {
            app,
            page,
            output,
            json,
        } => {
            execute_page_screenshot(ws_url, app, page, output, json)?;
        }
    }

    Ok(())
}

fn execute_page_screenshot(
    ws_url: &str,
    app: String,
    page: String,
    output: Option<String>,
    json: bool,
) -> Result<()> {
    let data = client::execute_command(
        ws_url,
        handlers::lxapp_page::SCREENSHOT,
        Some(json!({ "appid": app, "page": page })),
    )?
    .unwrap_or(Value::Null);

    if json {
        println!("{}", serde_json::to_string(&data)?);
        return Ok(());
    }

    let bytes = screenshot::decode_png_payload(&data, handlers::lxapp_page::SCREENSHOT)?;
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let app = data.get("appid").and_then(Value::as_str).unwrap_or(&app);
    let page = data
        .get("page")
        .and_then(Value::as_str)
        .filter(|page| !page.is_empty())
        .or_else(|| data.get("path").and_then(Value::as_str))
        .unwrap_or(&page);
    let app = screenshot::safe_component(app);
    let page = screenshot::safe_component(page);
    screenshot::write_png(output, format!("{app}-{page}-{ts}.png"), &bytes)
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

fn parse_query_pairs(pairs: &[String]) -> Result<Option<Value>> {
    if pairs.is_empty() {
        return Ok(None);
    }

    let mut query = serde_json::Map::new();
    for pair in pairs {
        let Some((key, value)) = pair.split_once('=') else {
            return Err(anyhow::anyhow!("--query must be KEY=VALUE"));
        };
        let key = key.trim();
        if key.is_empty() {
            return Err(anyhow::anyhow!("--query key must not be empty"));
        }
        query.insert(key.to_string(), Value::String(value.to_string()));
    }

    Ok(Some(Value::Object(query)))
}

fn commands_for_project(project_root: &Path) -> &'static [&'static str] {
    if project_root.join("lxapp.json").exists() && !project_root.join("lingxia.yaml").exists() {
        &[
            "doctor", "info", "pages", "page", "nav", "device", "eval", "reload",
        ]
    } else {
        &[
            "list",
            "current",
            "doctor",
            "info",
            "pages",
            "page",
            "nav",
            "device",
            "eval",
            "reload",
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
        println!("  {:<12}{}", command, command_description(command));
    }
    println!("  help        Print this message or the help of the given command(s)");
    println!();
    println!("Options:");
    println!("  -h, --help  Print help");
}

fn command_description(command: &str) -> &'static str {
    match command {
        "list" => "List open lxapps",
        "current" => "Print the current lxapp",
        "doctor" => "Report the session's automation capabilities",
        "info" => "Print lxapp runtime summary",
        "pages" => "Print configured lxapp pages",
        "page" => "Inspect and automate lxapp pages",
        "nav" => "Navigate the lxapp runtime by page name",
        "device" => "Inspect or switch the simulated device",
        "eval" => "Evaluate JavaScript in the lxapp logic runtime",
        "reload" => "Rebuild the lxapp front-end bundle and reload the running lxapp",
        "open" => "Open an lxapp",
        "close" => "Close an lxapp",
        "restart" => "Restart an lxapp (without rebuilding)",
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn parses_nav_switch_tab_by_page_name() {
        let cli = parse_lxapp_cli(args(&[
            "nav",
            "switch-tab",
            "profile",
            "--app",
            "demo",
            "--query",
            "tab=account",
            "--json",
        ]))
        .unwrap();

        let LxAppCommand::Nav(options) = cli.command else {
            panic!("expected nav command");
        };
        let NavCommand::SwitchTab(options) = options.command else {
            panic!("expected switch-tab command");
        };

        assert_eq!(options.page, "profile");
        assert_eq!(options.app, "demo");
        assert_eq!(options.query, vec!["tab=account"]);
        assert!(options.json);
    }

    #[test]
    fn rejects_host_surface_commands() {
        assert!(parse_lxapp_cli(args(&["windows"])).is_err());
        assert!(parse_lxapp_cli(args(&["screenshot"])).is_err());
        assert!(parse_lxapp_cli(args(&["page", "pointer"])).is_err());
        assert!(parse_lxapp_cli(args(&["page", "key"])).is_err());
    }

    #[test]
    fn parses_lxapp_reload_options() {
        let cli = parse_lxapp_cli(args(&[
            "reload",
            "--release",
            "--framework",
            "vue",
            "--json",
        ]))
        .unwrap();

        let LxAppCommand::Reload(options) = cli.command else {
            panic!("expected reload command");
        };

        assert!(options.release);
        assert_eq!(options.framework.as_deref(), Some("vue"));
        assert!(options.json);
        // The one-stop default: build then reload the current lxapp.
        assert!(!options.build_only);
        assert_eq!(options.app, "current");
    }

    #[test]
    fn parses_lxapp_reload_build_only() {
        let cli = parse_lxapp_cli(args(&["reload", "--build-only", "--app", "demo"])).unwrap();

        let LxAppCommand::Reload(options) = cli.command else {
            panic!("expected reload command");
        };

        assert!(options.build_only);
        assert_eq!(options.app, "demo");
    }

    #[test]
    fn parses_restart() {
        let cli = parse_lxapp_cli(args(&["restart", "demo", "--json"])).unwrap();

        let LxAppCommand::Restart { app, json } = cli.command else {
            panic!("expected restart command");
        };

        assert_eq!(app, "demo");
        assert!(json);
    }

    #[test]
    fn query_pairs_become_json_object() {
        let query = parse_query_pairs(&args(&["tab=account", "empty=", "encoded=a/b c"])).unwrap();

        assert_eq!(
            query,
            Some(json!({
                "tab": "account",
                "empty": "",
                "encoded": "a/b c",
            }))
        );
    }

    #[test]
    fn query_pairs_reject_missing_separator() {
        let err = parse_query_pairs(&args(&["tab"])).unwrap_err();
        assert!(err.to_string().contains("KEY=VALUE"));
    }
}
