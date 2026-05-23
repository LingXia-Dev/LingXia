use crate::client;
use crate::project::SessionInfo;
use crate::screenshot;
use anyhow::{Context, Result, anyhow};
use clap::{Args, Subcommand};
use lingxia_devtool_protocol::handlers;
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
    /// Print the current browser tab
    Current {
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Activate a browser tab
    Activate {
        #[arg(long, default_value = "current")]
        tab: String,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Close a browser tab
    Close {
        #[arg(long, default_value = "current")]
        tab: String,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Reload a browser tab
    Reload {
        #[arg(long, default_value = "current")]
        tab: String,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Navigate a browser tab back
    Back {
        #[arg(long, default_value = "current")]
        tab: String,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Navigate a browser tab forward
    Forward {
        #[arg(long, default_value = "current")]
        tab: String,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Evaluate JavaScript in a browser tab
    Eval {
        #[arg(long, default_value = "current")]
        tab: String,
        #[arg(long)]
        js: String,
        /// Wait for navigation caused by this script
        #[arg(long)]
        wait_navigation: bool,
        /// Wait for document.readyState after navigation
        #[arg(long)]
        complete: bool,
        /// Navigation wait timeout in milliseconds
        #[arg(long, default_value_t = 5000)]
        timeout_ms: u64,
        #[arg(long, hide = true)]
        json: bool,
    },
    /// Query structured information for an element in a browser tab
    Query {
        #[arg(long, default_value = "current")]
        tab: String,
        #[arg(long = "css")]
        selector: String,
        /// Return full text/value instead of truncating
        #[arg(long)]
        full: bool,
        /// Maximum text/value characters to include
        #[arg(long, default_value_t = 4096)]
        max_text: usize,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Wait for a browser condition
    Wait {
        #[arg(long, default_value = "current")]
        tab: String,
        /// Wait until document.readyState is complete
        #[arg(long)]
        loaded: bool,
        /// Wait until a selector exists
        #[arg(long = "exists")]
        exists: Option<String>,
        /// Wait until a selector is visible
        #[arg(long = "visible")]
        visible: Option<String>,
        /// Wait until a selector is absent or hidden
        #[arg(long = "hidden")]
        hidden: Option<String>,
        /// Wait until a selector is visible, enabled, and editable
        #[arg(long = "editable")]
        editable: Option<String>,
        /// Wait until JavaScript evaluates to true
        #[arg(long)]
        js: Option<String>,
        /// Timeout in milliseconds
        #[arg(long, default_value_t = 5000)]
        timeout_ms: u64,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Wait for a browser tab URL
    WaitUrl {
        #[arg(long, default_value = "current")]
        tab: String,
        /// Exact URL to wait for
        #[arg(long)]
        url: Option<String>,
        /// Substring that the current URL must contain
        #[arg(long)]
        contains: Option<String>,
        /// Timeout in milliseconds
        #[arg(long, default_value_t = 5000)]
        timeout_ms: u64,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Wait until a browser tab navigates away from its current URL
    WaitNavigation {
        #[arg(long, default_value = "current")]
        tab: String,
        /// Original URL to compare against
        #[arg(long)]
        from_url: Option<String>,
        /// Also wait until document.readyState is complete after URL changes
        #[arg(long)]
        complete: bool,
        /// Timeout in milliseconds
        #[arg(long, default_value_t = 5000)]
        timeout_ms: u64,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Click an element in a browser tab
    Click {
        #[arg(long, default_value = "current")]
        tab: String,
        #[arg(long = "css")]
        selector: String,
        /// Wait for navigation caused by this click
        #[arg(long)]
        wait_navigation: bool,
        /// Wait for document.readyState after navigation
        #[arg(long)]
        complete: bool,
        /// Navigation wait timeout in milliseconds
        #[arg(long, default_value_t = 5000)]
        timeout_ms: u64,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Type text into an element in a browser tab
    Type {
        #[arg(long, default_value = "current")]
        tab: String,
        #[arg(long = "css")]
        selector: String,
        #[arg(long)]
        text: String,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Replace an element's current text value
    Fill {
        #[arg(long, default_value = "current")]
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
        #[arg(long, default_value = "current")]
        tab: String,
        #[arg(long)]
        key: String,
        /// Wait for navigation caused by this key press
        #[arg(long)]
        wait_navigation: bool,
        /// Wait for document.readyState after navigation
        #[arg(long)]
        complete: bool,
        /// Navigation wait timeout in milliseconds
        #[arg(long, default_value_t = 5000)]
        timeout_ms: u64,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Scroll a browser tab by a delta
    Scroll {
        #[arg(long, default_value = "current")]
        tab: String,
        #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
        dx: f64,
        #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
        dy: f64,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Scroll an element into view in a browser tab
    ScrollTo {
        #[arg(long, default_value = "current")]
        tab: String,
        #[arg(long = "css")]
        selector: String,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Manage browser cookies
    Cookies(CookiesOptions),
    /// Capture a PNG screenshot of the current/specified tab
    Screenshot {
        #[arg(long, default_value = "current")]
        tab: String,
        /// Output path. Use `-` to write the PNG bytes to stdout.
        /// Defaults to `.lingxia/screenshots/<tab>-<ts>.png` under the project root.
        #[arg(long, short = 'o')]
        output: Option<String>,
        /// Print the JSON envelope (tab_id, size_bytes, data_base64) instead of writing a file
        #[arg(long)]
        json: bool,
    },
}

#[derive(Args, Clone)]
pub struct CookiesOptions {
    #[arg(long, default_value = "current", global = true)]
    pub tab: String,
    #[command(subcommand)]
    pub command: CookiesCommand,
}

#[derive(Subcommand, Clone)]
pub enum CookiesCommand {
    /// List all cookies in the tab's WebView cookie store
    List {
        /// Only list cookies visible to the tab's current URL
        #[arg(long)]
        visible: bool,
        /// Print pretty JSON output
        #[arg(long)]
        pretty: bool,
    },
    /// Set a cookie in the tab's WebView cookie store
    Set {
        /// Cookie URL; defaults to the current tab URL
        #[arg(long)]
        url: Option<String>,
        /// Cookie name
        #[arg(long)]
        name: String,
        /// Cookie value
        #[arg(long)]
        value: String,
        /// Domain cookie scope; omit for host-only
        #[arg(long)]
        domain: Option<String>,
        /// Cookie path
        #[arg(long, default_value = "/")]
        path: String,
        /// Mark the cookie Secure
        #[arg(long)]
        secure: bool,
        /// Mark the cookie HttpOnly
        #[arg(long)]
        http_only: bool,
        /// Expiration time in Unix milliseconds
        #[arg(long)]
        expires_unix_ms: Option<i64>,
        /// SameSite value: Lax, Strict, or None
        #[arg(long)]
        same_site: Option<String>,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Delete one cookie by name/domain/path
    Delete {
        /// Cookie name
        #[arg(long)]
        name: String,
        /// Cookie domain exactly as listed
        #[arg(long)]
        domain: String,
        /// Cookie path exactly as listed
        #[arg(long, default_value = "/")]
        path: String,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Clear all cookies in the shared WebView cookie store
    Clear {
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
}

pub fn execute(info: &SessionInfo, options: BrowserOptions) -> Result<()> {
    let ws_url = info.ws_url.as_str();

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
                print_json(data.as_ref().unwrap_or(&Value::Null), false)?;
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
                print_json(&data, false)?;
            } else {
                print_tabs(&data)?;
            }
        }
        BrowserCommand::Current { json } => {
            let data = client::execute_command(ws_url, handlers::browser::CURRENT, None)?
                .unwrap_or(Value::Null);
            if json {
                print_json(&data, false)?;
            } else {
                let tab_id = data
                    .get("tab_id")
                    .and_then(Value::as_str)
                    .context("no current browser tab")?;
                println!("{tab_id}");
            }
        }
        BrowserCommand::Activate { tab, json } => {
            let data = client::execute_command(
                ws_url,
                handlers::browser::ACTIVATE,
                Some(json!({ "tab_id": tab })),
            )?;
            print_optional_json(data, json)?;
        }
        BrowserCommand::Close { tab, json } => {
            let data = client::execute_command(
                ws_url,
                handlers::browser::CLOSE,
                Some(json!({ "tab_id": tab })),
            )?;
            print_optional_json(data, json)?;
        }
        BrowserCommand::Reload { tab, json } => {
            let data = client::execute_command(
                ws_url,
                handlers::browser::RELOAD,
                Some(json!({ "tab_id": tab })),
            )?;
            print_optional_json(data, json)?;
        }
        BrowserCommand::Back { tab, json } => {
            let data = client::execute_command(
                ws_url,
                handlers::browser::BACK,
                Some(json!({ "tab_id": tab })),
            )?;
            print_optional_json(data, json)?;
        }
        BrowserCommand::Forward { tab, json } => {
            let data = client::execute_command(
                ws_url,
                handlers::browser::FORWARD,
                Some(json!({ "tab_id": tab })),
            )?;
            print_optional_json(data, json)?;
        }
        BrowserCommand::Eval {
            tab,
            js,
            wait_navigation,
            complete,
            timeout_ms,
            json,
        } => {
            let data = client::execute_command(
                ws_url,
                handlers::browser::EVAL,
                Some(json!({
                    "tab_id": tab,
                    "js": js,
                    "wait_navigation": wait_navigation,
                    "complete": complete,
                    "timeout_ms": timeout_ms,
                })),
            )?;
            let _ = json;
            print_json(data.as_ref().unwrap_or(&Value::Null), false)?;
        }
        BrowserCommand::Query {
            tab,
            selector,
            full,
            max_text,
            json,
        } => {
            let data = client::execute_command(
                ws_url,
                handlers::browser::QUERY,
                Some(json!({
                    "tab_id": tab,
                    "selector": selector,
                    "full": full,
                    "max_text": max_text,
                })),
            )?
            .unwrap_or(Value::Null);
            if json {
                print_json(&data, false)?;
            } else {
                print_json(&data, false)?;
            }
        }
        BrowserCommand::Wait {
            tab,
            loaded,
            exists,
            visible,
            hidden,
            editable,
            js,
            timeout_ms,
            json,
        } => {
            let condition = wait_condition(loaded, exists, visible, hidden, editable, js)?;
            let data = client::execute_command(
                ws_url,
                handlers::browser::WAIT,
                Some(json!({
                    "tab_id": tab,
                    "condition": condition,
                    "timeout_ms": timeout_ms,
                })),
            )?
            .unwrap_or(Value::Null);
            print_wait_result(data, json)?;
        }
        BrowserCommand::WaitUrl {
            tab,
            url,
            contains,
            timeout_ms,
            json,
        } => {
            if url.is_some() == contains.is_some() {
                return Err(anyhow!("pass exactly one of --url or --contains"));
            }
            let data = client::execute_command(
                ws_url,
                handlers::browser::WAIT_URL,
                Some(json!({
                    "tab_id": tab,
                    "url": url,
                    "contains": contains,
                    "timeout_ms": timeout_ms,
                })),
            )?
            .unwrap_or(Value::Null);
            print_wait_result(data, json)?;
        }
        BrowserCommand::WaitNavigation {
            tab,
            from_url,
            complete,
            timeout_ms,
            json,
        } => {
            let data = client::execute_command(
                ws_url,
                handlers::browser::WAIT_NAVIGATION,
                Some(json!({
                    "tab_id": tab,
                    "from_url": from_url,
                    "complete": complete,
                    "timeout_ms": timeout_ms,
                })),
            )?
            .unwrap_or(Value::Null);
            print_wait_result(data, json)?;
        }
        BrowserCommand::Click {
            tab,
            selector,
            wait_navigation,
            complete,
            timeout_ms,
            json,
        } => {
            let data = client::execute_command(
                ws_url,
                handlers::browser::CLICK,
                Some(json!({
                    "tab_id": tab,
                    "selector": selector,
                    "wait_navigation": wait_navigation,
                    "complete": complete,
                    "timeout_ms": timeout_ms,
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
        BrowserCommand::Fill {
            tab,
            selector,
            text,
            json,
        } => {
            let data = client::execute_command(
                ws_url,
                handlers::browser::FILL,
                Some(json!({
                    "tab_id": tab,
                    "selector": selector,
                    "text": text,
                })),
            )?;
            print_optional_json(data, json)?;
        }
        BrowserCommand::Press {
            tab,
            key,
            wait_navigation,
            complete,
            timeout_ms,
            json,
        } => {
            let data = client::execute_command(
                ws_url,
                handlers::browser::PRESS,
                Some(json!({
                    "tab_id": tab,
                    "key": key,
                    "wait_navigation": wait_navigation,
                    "complete": complete,
                    "timeout_ms": timeout_ms,
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
        BrowserCommand::Cookies(options) => execute_cookies(ws_url, options)?,
        BrowserCommand::Screenshot { tab, output, json } => {
            execute_screenshot(ws_url, tab, output, json)?
        }
    }

    Ok(())
}

fn execute_screenshot(ws_url: &str, tab: String, output: Option<String>, json: bool) -> Result<()> {
    let data = client::execute_command(
        ws_url,
        handlers::browser::SCREENSHOT,
        Some(json!({ "tab_id": tab })),
    )?
    .unwrap_or(Value::Null);

    if json {
        return print_json(&data, false);
    }

    let bytes = screenshot::decode_png_payload(&data, handlers::browser::SCREENSHOT)?;
    let resolved_tab = data
        .get("tab_id")
        .and_then(Value::as_str)
        .unwrap_or(tab.as_str())
        .to_string();
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let tab = screenshot::safe_component(&resolved_tab);
    screenshot::write_png(output, format!("{tab}-{ts}.png"), &bytes)
}

fn execute_cookies(ws_url: &str, options: CookiesOptions) -> Result<()> {
    let tab = options.tab;
    match options.command {
        CookiesCommand::List { visible, pretty } => {
            let data = client::execute_command(
                ws_url,
                handlers::browser::COOKIES_LIST,
                Some(json!({ "tab_id": tab, "visible": visible })),
            )?
            .unwrap_or_else(|| json!([]));
            print_json(&data, pretty)?;
        }
        CookiesCommand::Set {
            url,
            name,
            value,
            domain,
            path,
            secure,
            http_only,
            expires_unix_ms,
            same_site,
            json,
        } => {
            let data = client::execute_command(
                ws_url,
                handlers::browser::COOKIES_SET,
                Some(json!({
                    "tab_id": tab,
                    "cookie": {
                        "name": name,
                        "value": value,
                        "url": url.unwrap_or_default(),
                        "domain": domain,
                        "path": path,
                        "secure": secure,
                        "http_only": http_only,
                        "expires_unix_ms": expires_unix_ms,
                        "same_site": same_site.map(|value| value.to_ascii_lowercase()),
                    }
                })),
            )?;
            print_optional_json(data, json)?;
        }
        CookiesCommand::Delete {
            name,
            domain,
            path,
            json,
        } => {
            let data = client::execute_command(
                ws_url,
                handlers::browser::COOKIES_DELETE,
                Some(json!({
                    "tab_id": tab,
                    "name": name,
                    "domain": domain,
                    "path": path,
                })),
            )?;
            print_optional_json(data, json)?;
        }
        CookiesCommand::Clear { json } => {
            let data = client::execute_command(
                ws_url,
                handlers::browser::COOKIES_CLEAR,
                Some(json!({ "tab_id": tab })),
            )?;
            print_optional_json(data, json)?;
        }
    }
    Ok(())
}

fn wait_condition(
    loaded: bool,
    exists: Option<String>,
    visible: Option<String>,
    hidden: Option<String>,
    editable: Option<String>,
    js: Option<String>,
) -> Result<Value> {
    let mut count = usize::from(loaded);
    count += exists.is_some() as usize;
    count += visible.is_some() as usize;
    count += hidden.is_some() as usize;
    count += editable.is_some() as usize;
    count += js.is_some() as usize;
    if count != 1 {
        return Err(anyhow!(
            "pass exactly one wait condition: --loaded, --exists, --visible, --hidden, --editable, or --js"
        ));
    }

    if loaded {
        return Ok(json!({ "kind": "loaded" }));
    }
    if let Some(selector) = exists {
        return Ok(json!({ "kind": "selector_exists", "selector": selector }));
    }
    if let Some(selector) = visible {
        return Ok(json!({ "kind": "selector_visible", "selector": selector }));
    }
    if let Some(selector) = hidden {
        return Ok(json!({ "kind": "selector_hidden", "selector": selector }));
    }
    if let Some(selector) = editable {
        return Ok(json!({ "kind": "selector_editable", "selector": selector }));
    }
    if let Some(js) = js {
        return Ok(json!({ "kind": "js_true", "js": js }));
    }
    unreachable!("wait condition count was checked")
}

fn print_wait_result(data: Value, json: bool) -> Result<()> {
    if json {
        return print_json(&data, false);
    }
    let elapsed = data
        .get("elapsed_ms")
        .and_then(Value::as_u64)
        .map(|ms| format!(" in {ms}ms"))
        .unwrap_or_default();
    if let Some(url) = data.get("current_url").and_then(Value::as_str) {
        println!("ok{elapsed}: {url}");
    } else {
        println!("ok{elapsed}");
    }
    Ok(())
}

fn print_optional_json(data: Option<Value>, json: bool) -> Result<()> {
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

fn print_tabs(data: &Value) -> Result<()> {
    let Some(tabs) = data.as_array() else {
        return print_json(data, false);
    };
    if tabs.is_empty() {
        println!("No browser tabs.");
        return Ok(());
    }

    println!("{:<36}  {:<8}  {:<28}  URL", "TAB ID", "SESSION", "TITLE");
    for tab in tabs {
        let tab_id = tab.get("tab_id").and_then(Value::as_str).unwrap_or("-");
        let session_id = tab
            .get("session_id")
            .and_then(Value::as_u64)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string());
        let title = tab
            .get("title")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("-");
        let url = tab
            .get("current_url")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .or_else(|| tab.get("path").and_then(Value::as_str))
            .unwrap_or("-");
        println!(
            "{:<36}  {:<8}  {:<28}  {}",
            tab_id,
            session_id,
            truncate(title, 28),
            url
        );
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
