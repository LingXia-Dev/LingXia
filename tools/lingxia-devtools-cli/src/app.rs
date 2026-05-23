use crate::client;
use crate::project::SessionInfo;
use crate::screenshot;
use anyhow::Result;
use clap::{Args, Subcommand};
use lingxia_devtool_protocol::handlers;
use serde_json::{Value, json};

#[derive(Args, Clone)]
pub struct AppOptions {
    #[command(subcommand)]
    pub command: AppCommand,
}

#[derive(Subcommand, Clone)]
pub enum AppCommand {
    /// Capture a PNG screenshot of the host app's window
    Screenshot {
        /// Specific window id (from `lxdev app windows`); defaults to the
        /// platform's focused/main window.
        #[arg(long)]
        window: Option<String>,
        /// Output path; use `-` for stdout. Default:
        /// `.lingxia/screenshots/app-<platform>-<ts>.png`
        #[arg(long, short = 'o')]
        output: Option<String>,
        /// Print the JSON envelope (format, size_bytes, data_base64)
        #[arg(long)]
        json: bool,
    },
    /// List the host app's top-level windows
    Windows {
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
}

pub fn execute(info: &SessionInfo, options: AppOptions) -> Result<()> {
    match options.command {
        AppCommand::Screenshot {
            window,
            output,
            json,
        } => execute_screenshot(info, window, output, json),
        AppCommand::Windows { json } => execute_windows(info, json),
    }
}

fn execute_windows(info: &SessionInfo, json: bool) -> Result<()> {
    let data = client::execute_command(&info.ws_url, handlers::app::WINDOWS, None)?
        .unwrap_or(Value::Array(Vec::new()));

    if json {
        println!("{}", serde_json::to_string_pretty(&data)?);
        return Ok(());
    }

    let Some(array) = data.as_array() else {
        println!("{}", serde_json::to_string_pretty(&data)?);
        return Ok(());
    };
    if array.is_empty() {
        println!("No windows reported by host app.");
        return Ok(());
    }
    println!(
        "{:<12}  {:<5}  {:<5}  {:<7}  {:<9}  {}",
        "ID", "FOCUS", "MAIN", "VISIBLE", "SIZE", "TITLE"
    );
    for win in array {
        let id = win.get("id").and_then(Value::as_str).unwrap_or("-");
        let focused = win.get("focused").and_then(Value::as_bool).unwrap_or(false);
        let main = win.get("main").and_then(Value::as_bool).unwrap_or(false);
        let visible = win.get("visible").and_then(Value::as_bool).unwrap_or(false);
        let width = win.get("width").and_then(Value::as_u64).unwrap_or(0);
        let height = win.get("height").and_then(Value::as_u64).unwrap_or(0);
        let title = win.get("title").and_then(Value::as_str).unwrap_or("");
        println!(
            "{:<12}  {:<5}  {:<5}  {:<7}  {:<9}  {}",
            id,
            if focused { "yes" } else { "no" },
            if main { "yes" } else { "no" },
            if visible { "yes" } else { "no" },
            format!("{}x{}", width, height),
            title,
        );
    }
    Ok(())
}

fn execute_screenshot(
    info: &SessionInfo,
    window: Option<String>,
    output: Option<String>,
    json: bool,
) -> Result<()> {
    let args = window.as_ref().map(|id| json!({ "window_id": id }));
    let data = client::execute_command(&info.ws_url, handlers::app::SCREENSHOT, args)?
        .unwrap_or(Value::Null);

    if json {
        println!("{}", serde_json::to_string(&data)?);
        return Ok(());
    }

    let bytes = screenshot::decode_png_payload(&data, handlers::app::SCREENSHOT)?;
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let platform = screenshot::safe_component(&info.platform);
    screenshot::write_png(output, format!("app-{platform}-{ts}.png"), &bytes)
}
