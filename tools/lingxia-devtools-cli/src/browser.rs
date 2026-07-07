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
    /// Inspect network traffic (request/response payloads). Windows sessions
    /// only — requires the WebView2 DevTools protocol.
    Network(NetworkOptions),
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

#[derive(Args, Clone)]
pub struct NetworkOptions {
    #[arg(long, default_value = "current", global = true)]
    pub tab: String,
    #[command(subcommand)]
    pub command: NetworkCommand,
}

#[derive(Subcommand, Clone)]
pub enum NetworkCommand {
    /// Start recording network traffic into the tab's capture buffer
    Enable,
    /// Stop recording (captured entries are kept until `clear`)
    Disable,
    /// List captured requests (summary rows, or full JSON with --json)
    List {
        /// Only requests whose URL contains this substring
        #[arg(long)]
        url: Option<String>,
        /// Only this HTTP method (case-insensitive)
        #[arg(long)]
        method: Option<String>,
        /// Only this resource type (substring, case-insensitive: xhr, fetch,
        /// script, image, document, ...)
        #[arg(long = "type")]
        resource_type: Option<String>,
        /// Only this exact response status
        #[arg(long)]
        status: Option<u16>,
        /// Only failed requests (network failure or status >= 400)
        #[arg(long)]
        failed: bool,
        /// Keep only the most recent N entries
        #[arg(long)]
        limit: Option<usize>,
        /// Print the full JSON entries (headers + bodies) instead of a table
        #[arg(long)]
        json: bool,
    },
    /// Print one captured request in full (headers, payload, response body)
    Get {
        /// The request id from `list`
        #[arg(long)]
        id: String,
        /// Print compact JSON on one line
        #[arg(long)]
        json: bool,
    },
    /// Drop all captured entries
    Clear,
}

pub fn execute(info: &SessionInfo, options: BrowserOptions) -> Result<()> {
    let ws_url = info.ws_url.as_str();

    match options.command {
        BrowserCommand::Open { url, tab, json } => {
            let url = normalize_open_url(&url);
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
            json: _,
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
            // This command always emits JSON regardless of --json.
            print_json(&data, false)?;
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
        BrowserCommand::Network(options) => execute_network(info, options)?,
        BrowserCommand::Screenshot { tab, output, json } => {
            execute_screenshot(ws_url, tab, output, json)?
        }
    }

    Ok(())
}

fn execute_network(info: &SessionInfo, options: NetworkOptions) -> Result<()> {
    // Network capture rides the WebView2 DevTools protocol, so it needs a
    // Windows WebView2 runner. The dev session labels a direct Windows app
    // "windows" and a runner-hosted lxapp "lxapp"; both host WebView2 on
    // Windows, so accept either. lxdev and the runner are co-located, so the
    // real backstop is the runner-side handler (compiled only on Windows) —
    // a non-Windows runner answers "unknown handler" for these commands.
    let platform = info.platform.as_str();
    if !platform.eq_ignore_ascii_case("windows") && !platform.eq_ignore_ascii_case("lxapp") {
        return Err(anyhow!(
            "browser network capture needs a Windows WebView2 session (this session is '{platform}')",
        ));
    }
    let ws_url = info.ws_url.as_str();
    let tab = options.tab;
    match options.command {
        NetworkCommand::Enable => {
            client::execute_command(
                ws_url,
                handlers::browser::NETWORK_ENABLE,
                Some(json!({ "tab_id": tab })),
            )?;
            println!("network capture enabled for {tab}");
        }
        NetworkCommand::Disable => {
            client::execute_command(
                ws_url,
                handlers::browser::NETWORK_DISABLE,
                Some(json!({ "tab_id": tab })),
            )?;
            println!("network capture disabled for {tab}");
        }
        NetworkCommand::Clear => {
            client::execute_command(
                ws_url,
                handlers::browser::NETWORK_CLEAR,
                Some(json!({ "tab_id": tab })),
            )?;
            println!("network capture cleared for {tab}");
        }
        NetworkCommand::List {
            url,
            method,
            resource_type,
            status,
            failed,
            limit,
            json,
        } => {
            let snapshot = network_snapshot(ws_url, &tab)?;
            let filter = NetworkFilter {
                url: url.as_deref(),
                method: method.as_deref(),
                resource_type: resource_type.as_deref(),
                status,
                failed,
            };
            print_network_list(&snapshot, &filter, limit, json)?;
        }
        NetworkCommand::Get { id, json } => {
            let snapshot = network_snapshot(ws_url, &tab)?;
            let entry = snapshot
                .get("entries")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .find(|entry| entry.get("request_id").and_then(Value::as_str) == Some(id.as_str()))
                .ok_or_else(|| anyhow!("no captured request with id {id}"))?;
            if json {
                print_json(entry, false)?;
            } else {
                print_json(entry, true)?;
            }
        }
    }
    Ok(())
}

fn network_snapshot(ws_url: &str, tab: &str) -> Result<Value> {
    Ok(client::execute_command(
        ws_url,
        handlers::browser::NETWORK_LIST,
        Some(json!({ "tab_id": tab })),
    )?
    .unwrap_or(Value::Null))
}

struct NetworkFilter<'a> {
    url: Option<&'a str>,
    method: Option<&'a str>,
    resource_type: Option<&'a str>,
    status: Option<u16>,
    failed: bool,
}

impl NetworkFilter<'_> {
    fn matches(&self, entry: &Value) -> bool {
        let field = |key| entry.get(key).and_then(Value::as_str);
        if let Some(needle) = self.url
            && !field("url").is_some_and(|url| url.contains(needle))
        {
            return false;
        }
        if let Some(method) = self.method
            && !field("method").is_some_and(|m| m.eq_ignore_ascii_case(method))
        {
            return false;
        }
        if let Some(rtype) = self.resource_type
            && !field("resource_type")
                .is_some_and(|t| t.to_ascii_lowercase().contains(&rtype.to_ascii_lowercase()))
        {
            return false;
        }
        let status = entry.get("status").and_then(Value::as_u64);
        if let Some(want) = self.status
            && status != Some(want as u64)
        {
            return false;
        }
        if self.failed {
            let is_failure = field("failed").is_some() || status.is_some_and(|s| s >= 400);
            if !is_failure {
                return false;
            }
        }
        true
    }
}

fn print_network_list(
    snapshot: &Value,
    filter: &NetworkFilter,
    limit: Option<usize>,
    json: bool,
) -> Result<()> {
    let all = snapshot
        .get("entries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut filtered: Vec<&Value> = all.iter().filter(|entry| filter.matches(entry)).collect();
    if let Some(limit) = limit {
        let start = filtered.len().saturating_sub(limit);
        filtered = filtered.split_off(start);
    }

    if json {
        let out = json!({
            "entries": filtered,
            "dropped": snapshot.get("dropped").cloned().unwrap_or(json!(0)),
        });
        return print_json(&out, false);
    }

    let dropped = snapshot.get("dropped").and_then(Value::as_u64).unwrap_or(0);
    println!(
        "{:<8} {:<7} {:<6} {:<10} {:>7} {:>9}  URL",
        "ID", "METHOD", "STATUS", "TYPE", "MS", "BODY"
    );
    for entry in &filtered {
        let id = entry
            .get("request_id")
            .and_then(Value::as_str)
            .unwrap_or("");
        let method = entry.get("method").and_then(Value::as_str).unwrap_or("");
        let status = entry
            .get("status")
            .and_then(Value::as_u64)
            .map(|s| s.to_string())
            .or_else(|| {
                entry
                    .get("failed")
                    .and_then(Value::as_str)
                    .map(|_| "FAIL".to_string())
            })
            .unwrap_or_else(|| "-".to_string());
        let rtype = entry
            .get("resource_type")
            .and_then(Value::as_str)
            .unwrap_or("-");
        let ms = network_duration_ms(entry)
            .map(|ms| format!("{ms:.0}"))
            .unwrap_or_else(|| "-".to_string());
        let body = network_body_label(entry.get("response_body"));
        let url = entry.get("url").and_then(Value::as_str).unwrap_or("");
        println!("{id:<8} {method:<7} {status:<6} {rtype:<10} {ms:>7} {body:>9}  {url}");
    }
    println!(
        "\n{} request(s){}",
        filtered.len(),
        if dropped > 0 {
            format!(", {dropped} dropped (buffer full)")
        } else {
            String::new()
        }
    );
    Ok(())
}

/// Duration in ms from the engine's monotonic started/finished timestamps.
fn network_duration_ms(entry: &Value) -> Option<f64> {
    let started = entry.get("started").and_then(Value::as_f64)?;
    let finished = entry.get("finished").and_then(Value::as_f64)?;
    (finished >= started).then_some((finished - started) * 1000.0)
}

/// Short label for a response body's capture state, for the list table.
fn network_body_label(body: Option<&Value>) -> String {
    match body.and_then(|b| b.get("kind")).and_then(Value::as_str) {
        Some("text") => {
            let len = body
                .and_then(|b| b.get("text"))
                .and_then(Value::as_str)
                .map(str::len)
                .unwrap_or(0);
            format!("{len}B")
        }
        Some("base64") => {
            let len = body
                .and_then(|b| b.get("base64"))
                .and_then(Value::as_str)
                .map(|s| s.len() / 4 * 3)
                .unwrap_or(0);
            format!("~{len}B")
        }
        Some("skipped") => "skipped".to_string(),
        _ => "-".to_string(),
    }
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

/// Default a bare host to a scheme so `lxdev browser open example.com` works.
/// Local/loopback hosts (`localhost:3000`, `127.0.0.1`, `192.168.x`, …) default
/// to `http://` since dev servers rarely serve TLS; everything else to
/// `https://`. Inputs that already carry a scheme (`http://`, `about:`, …) pass
/// through untouched.
fn normalize_open_url(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() || has_url_scheme(trimmed) {
        return trimmed.to_string();
    }
    if let Some(file_url) = local_file_url(trimmed) {
        return file_url;
    }
    let scheme = if is_local_host(host_of(trimmed)) {
        "http"
    } else {
        "https"
    };
    format!("{scheme}://{trimmed}")
}

/// Resolve a local filesystem path to an absolute `file://` URL. Explicit path
/// forms (`/abs`, `./rel`, `../rel`, `~/home`) always resolve; a bare token
/// (e.g. `a.html`) resolves only when a file actually exists at that relative
/// path — otherwise it is left to be treated as a web host. Resolution is done
/// against the CLI's cwd/`$HOME` so the runner receives an absolute path.
fn local_file_url(input: &str) -> Option<String> {
    let explicit = input.starts_with('/')
        || input.starts_with("./")
        || input.starts_with("../")
        || input.starts_with("~/")
        || input == "~"
        || input == "."
        || input == ".."
        // Windows drive-absolute (`C:\…`, `C:/…`) and UNC paths.
        || std::path::Path::new(input).is_absolute();
    let abs = resolve_local_path(input)?;
    (explicit || abs.exists()).then(|| file_url_from_path(&abs))
}

fn resolve_local_path(input: &str) -> Option<std::path::PathBuf> {
    use std::path::PathBuf;
    let expanded: PathBuf = if input == "~" {
        std::env::var_os("HOME")?.into()
    } else if let Some(rest) = input.strip_prefix("~/") {
        let mut home = PathBuf::from(std::env::var_os("HOME")?);
        home.push(rest);
        home
    } else {
        PathBuf::from(input)
    };
    let absolute = if expanded.is_absolute() {
        expanded
    } else {
        std::env::current_dir().ok()?.join(expanded)
    };
    Some(lexically_clean(&absolute))
}

/// Lexically drop `.` and resolve `..` components without touching the
/// filesystem (the target may not exist yet).
fn lexically_clean(path: &std::path::Path) -> std::path::PathBuf {
    use std::path::{Component, PathBuf};
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// Build a `file://` URL from an absolute path, percent-encoding path bytes
/// outside the unreserved set so spaces and other characters are safe.
///
/// Windows paths (`C:\dir\file`) are normalized to forward slashes with a
/// leading slash so they yield `file:///C:/dir/file`; the drive colon is kept
/// literal (RFC 8089). POSIX paths already start with `/` and are unchanged.
fn file_url_from_path(path: &std::path::Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let mut out = String::from("file://");
    if !normalized.starts_with('/') {
        out.push('/');
    }
    for &b in normalized.as_bytes() {
        match b {
            b'/' | b'-' | b'.' | b'_' | b'~' | b':' | b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// The host part of a schemeless authority: drop any `/path`, `?query`, `#frag`,
/// userinfo, and `:port`, handling `[::1]`-style IPv6 literals.
fn host_of(input: &str) -> &str {
    let authority = input
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(input)
        .rsplit('@')
        .next()
        .unwrap_or(input);
    if let Some(rest) = authority.strip_prefix('[') {
        // IPv6 literal: host is up to the closing bracket.
        return rest.split(']').next().unwrap_or(rest);
    }
    authority.split(':').next().unwrap_or(authority)
}

/// Loopback / private-network hosts that should default to `http://`.
fn is_local_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".localhost") || host == "::1" {
        return true;
    }
    if let Ok(ip) = host.parse::<std::net::Ipv4Addr>() {
        // 127.0.0.0/8, 10/8, 172.16/12, 192.168/16, 0.0.0.0
        return ip.is_loopback() || ip.is_private() || ip.is_unspecified();
    }
    false
}

fn has_url_scheme(url: &str) -> bool {
    // Authority-based scheme, e.g. `https://`, `ws://`, `custom://`.
    if let Some(pos) = url.find("://")
        && pos > 0
    {
        let scheme = &url[..pos];
        let mut chars = scheme.chars();
        let valid = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
            && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'));
        if valid {
            return true;
        }
    }
    // Schemeless special schemes that must not be rewritten to a host.
    const SCHEMELESS: &[&str] = &["about:", "data:", "blob:", "file:", "view-source:"];
    let lower = url.to_ascii_lowercase();
    SCHEMELESS.iter().any(|prefix| lower.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::{file_url_from_path, local_file_url, normalize_open_url};
    use std::path::Path;

    #[test]
    fn bare_remote_host_gets_https() {
        assert_eq!(normalize_open_url("example.com"), "https://example.com");
        assert_eq!(
            normalize_open_url("example.com/path"),
            "https://example.com/path"
        );
        assert_eq!(normalize_open_url("  example.com "), "https://example.com");
        assert_eq!(normalize_open_url("8.8.8.8"), "https://8.8.8.8");
    }

    #[test]
    fn absolute_path_becomes_file_url() {
        #[cfg(not(windows))]
        assert_eq!(
            normalize_open_url("/Users/me/page.html"),
            "file:///Users/me/page.html"
        );
        #[cfg(windows)]
        assert_eq!(
            normalize_open_url(r"C:\Users\me\page.html"),
            "file:///C:/Users/me/page.html"
        );
        // Explicit relative/home forms resolve even if missing.
        assert!(normalize_open_url("./a.html").starts_with("file://"));
        assert!(normalize_open_url("~/a.html").starts_with("file://"));
        assert!(normalize_open_url("../up.html").starts_with("file://"));
    }

    #[test]
    fn file_url_encodes_and_resolves_dots() {
        // A forward-slash-rooted path encodes the same on every platform.
        assert_eq!(
            file_url_from_path(Path::new("/a b/c#d.html")),
            "file:///a%20b/c%23d.html"
        );
        // `.`/`..` are cleaned lexically in the absolute result.
        #[cfg(not(windows))]
        assert_eq!(
            local_file_url("/a/./b/../c.html").as_deref(),
            Some("file:///a/c.html")
        );
        #[cfg(windows)]
        assert_eq!(
            local_file_url(r"C:\a\.\b\..\c.html").as_deref(),
            Some("file:///C:/a/c.html")
        );
    }

    #[test]
    fn bare_nonexistent_token_stays_a_host() {
        // `a.html` with no such file in cwd is treated as a web host, not a file.
        assert_eq!(local_file_url("definitely-not-a-real-file.xyz"), None);
    }

    #[test]
    fn bare_local_host_gets_http() {
        assert_eq!(
            normalize_open_url("localhost:3000"),
            "http://localhost:3000"
        );
        assert_eq!(normalize_open_url("localhost"), "http://localhost");
        assert_eq!(
            normalize_open_url("127.0.0.1:8080/x"),
            "http://127.0.0.1:8080/x"
        );
        assert_eq!(
            normalize_open_url("192.168.1.10:5173"),
            "http://192.168.1.10:5173"
        );
        assert_eq!(
            normalize_open_url("app.localhost:3000"),
            "http://app.localhost:3000"
        );
        assert_eq!(normalize_open_url("0.0.0.0:9000"), "http://0.0.0.0:9000");
    }

    #[test]
    fn existing_scheme_is_preserved() {
        assert_eq!(
            normalize_open_url("http://example.com"),
            "http://example.com"
        );
        assert_eq!(
            normalize_open_url("https://example.com"),
            "https://example.com"
        );
        assert_eq!(normalize_open_url("about:blank"), "about:blank");
        assert_eq!(normalize_open_url("data:text/html,hi"), "data:text/html,hi");
    }
}
