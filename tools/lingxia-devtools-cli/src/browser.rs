use crate::client;
use crate::project::DevInfo;
use anyhow::{Context, Result, anyhow};
use clap::{Args, Subcommand};
use lingxia_devtool_protocol::handlers;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Args, Clone)]
pub struct BrowserOptions {
    #[command(subcommand)]
    pub command: BrowserCommand,
}

#[derive(Subcommand, Clone)]
pub enum BrowserCommand {
    /// Open a URL in a browser tab
    Open {
        url: String,
        /// Reuse or create a stable tab id
        #[arg(long)]
        tab: Option<String>,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// List browser tabs
    Tabs {
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Close a browser tab
    Close {
        #[arg(long)]
        tab: String,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Evaluate JavaScript in a browser tab
    Eval {
        #[arg(long)]
        tab: String,
        #[arg(long)]
        js: String,
        /// Print compact JSON output
        #[arg(long)]
        json: bool,
    },
    /// Click an element in a browser tab
    Click {
        #[arg(long)]
        tab: String,
        #[arg(long = "css")]
        selector: String,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Type text into an element in a browser tab
    Type {
        #[arg(long)]
        tab: String,
        #[arg(long = "css")]
        selector: String,
        #[arg(long)]
        text: String,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Press a key in a browser tab
    Press {
        #[arg(long)]
        tab: String,
        #[arg(long)]
        key: String,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Scroll a browser tab by a delta
    Scroll {
        #[arg(long)]
        tab: String,
        #[arg(long, default_value_t = 0.0)]
        dx: f64,
        #[arg(long, default_value_t = 0.0)]
        dy: f64,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Scroll an element into view in a browser tab
    ScrollTo {
        #[arg(long)]
        tab: String,
        #[arg(long = "css")]
        selector: String,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Deserialize)]
struct BrowserTabInfo {
    tab_id: String,
    path: String,
    session_id: u64,
    current_url: Option<String>,
    title: Option<String>,
}

pub fn execute(info: &DevInfo, options: BrowserOptions) -> Result<()> {
    let ws_url = info
        .ws_url
        .as_deref()
        .ok_or_else(|| anyhow!("dev websocket URL is missing from .lingxia/dev.json"))?;

    match options.command {
        BrowserCommand::Open { url, tab, json } => {
            let data = client::execute_command(
                ws_url,
                handlers::browser::OPEN,
                Some(json!({
                    "url": url,
                    "tab_id": tab,
                })),
            )?;
            if json {
                print_json(data.as_ref().unwrap_or(&Value::Null), true)?;
            } else {
                let tab_id = data
                    .as_ref()
                    .and_then(|value| value.get("tab_id"))
                    .and_then(Value::as_str)
                    .context("browser.open response did not include tab_id")?;
                println!("{tab_id}");
            }
        }
        BrowserCommand::Tabs { json } => {
            let data = client::execute_command(ws_url, handlers::browser::TABS, None)?;
            let data = data.unwrap_or_else(|| json!([]));
            if json {
                print_json(&data, true)?;
            } else {
                print_tabs(data)?;
            }
        }
        BrowserCommand::Close { tab, json } => {
            let data = client::execute_command(
                ws_url,
                handlers::browser::CLOSE,
                Some(json!({ "tab_id": tab })),
            )?;
            print_optional_json(data, json)?;
        }
        BrowserCommand::Eval { tab, js, json } => {
            let data = client::execute_command(
                ws_url,
                handlers::browser::EVAL,
                Some(json!({
                    "tab_id": tab,
                    "js": js,
                })),
            )?;
            print_json(data.as_ref().unwrap_or(&Value::Null), !json)?;
        }
        BrowserCommand::Click {
            tab,
            selector,
            json,
        } => {
            let data = client::execute_command(
                ws_url,
                handlers::browser::CLICK,
                Some(json!({
                    "tab_id": tab,
                    "selector": selector,
                })),
            )?;
            print_optional_json(data, json)?;
        }
        BrowserCommand::Type {
            tab,
            selector,
            text,
            json,
        } => {
            let data = client::execute_command(
                ws_url,
                handlers::browser::TYPE,
                Some(json!({
                    "tab_id": tab,
                    "selector": selector,
                    "text": text,
                })),
            )?;
            print_optional_json(data, json)?;
        }
        BrowserCommand::Press { tab, key, json } => {
            let data = client::execute_command(
                ws_url,
                handlers::browser::PRESS,
                Some(json!({
                    "tab_id": tab,
                    "key": key,
                })),
            )?;
            print_optional_json(data, json)?;
        }
        BrowserCommand::Scroll { tab, dx, dy, json } => {
            let data = client::execute_command(
                ws_url,
                handlers::browser::SCROLL,
                Some(json!({
                    "tab_id": tab,
                    "dx": dx,
                    "dy": dy,
                })),
            )?;
            print_optional_json(data, json)?;
        }
        BrowserCommand::ScrollTo {
            tab,
            selector,
            json,
        } => {
            let data = client::execute_command(
                ws_url,
                handlers::browser::SCROLL_TO,
                Some(json!({
                    "tab_id": tab,
                    "selector": selector,
                })),
            )?;
            print_optional_json(data, json)?;
        }
    }

    Ok(())
}

fn print_tabs(data: Value) -> Result<()> {
    let tabs: Vec<BrowserTabInfo> =
        serde_json::from_value(data).context("Failed to parse browser.tabs response")?;
    if tabs.is_empty() {
        println!("No browser tabs.");
        return Ok(());
    }

    println!("{:<36}  {:<8}  {:<28}  URL", "TAB ID", "SESSION", "TITLE");
    for tab in tabs {
        let title = tab
            .title
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("-");
        let url = tab
            .current_url
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&tab.path);
        println!(
            "{:<36}  {:<8}  {:<28}  {}",
            tab.tab_id,
            tab.session_id,
            truncate(title, 28),
            url
        );
    }
    Ok(())
}

fn print_optional_json(data: Option<Value>, json: bool) -> Result<()> {
    if json {
        print_json(data.as_ref().unwrap_or(&Value::Null), true)?;
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

fn truncate(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let mut out: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() && max_chars > 1 {
        out = value.chars().take(max_chars.saturating_sub(3)).collect();
        out.push_str("...");
    }
    out
}
