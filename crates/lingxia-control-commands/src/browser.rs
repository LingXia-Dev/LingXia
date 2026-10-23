use crate::output;
use crate::transport::Transport;
use anyhow::{Context, Result, anyhow};
use clap::{Args, Subcommand};
use lingxia_control_protocol::methods;
use serde_json::{Value, json};

#[derive(Args, Clone)]
pub struct BrowserOptions {
    /// Acknowledge commands that lose state, like closing a tab or clearing cookies
    #[arg(long, global = true)]
    pub allow_destructive: bool,
    #[command(subcommand)]
    pub command: BrowserCommand,
}

/// What a browser command does to the page, so it can be gated the same way a
/// desktop command is.
///
/// A tab is somebody's logged-in session. Typing into it, running script in it,
/// and deleting its cookies are all things an agent should have been told it
/// may do — the fact that the surface is web rather than native changes
/// nothing about that.
fn effect(command: &BrowserCommand) -> Effect {
    match command {
        // Looking.
        BrowserCommand::Tabs { .. }
        | BrowserCommand::Current { .. }
        | BrowserCommand::Query { .. }
        | BrowserCommand::Snapshot { .. }
        | BrowserCommand::Text { .. }
        | BrowserCommand::Wait { .. }
        | BrowserCommand::WaitUrl { .. }
        | BrowserCommand::WaitNavigation { .. }
        | BrowserCommand::Screenshot { .. } => Effect::Reads,

        // Loses something that cannot be got back by repeating the command.
        BrowserCommand::Close { .. } => Effect::Destroys,
        BrowserCommand::Console { clear, .. } => {
            if *clear {
                Effect::Changes
            } else {
                Effect::Reads
            }
        }
        BrowserCommand::Cookies(options) => match options.command {
            CookiesCommand::List { .. } => Effect::Reads,
            CookiesCommand::Delete { .. } | CookiesCommand::Clear { .. } => Effect::Destroys,
            CookiesCommand::Set { .. } => Effect::Changes,
        },

        // Everything else changes the page or the browser session.
        BrowserCommand::Open { .. }
        | BrowserCommand::Activate { .. }
        | BrowserCommand::Reload { .. }
        | BrowserCommand::Back { .. }
        | BrowserCommand::Forward { .. }
        | BrowserCommand::UserAgent(_)
        | BrowserCommand::Eval { .. }
        | BrowserCommand::Click { .. }
        | BrowserCommand::Hover { .. }
        | BrowserCommand::Drag { .. }
        | BrowserCommand::SelectOption { .. }
        | BrowserCommand::Check { .. }
        | BrowserCommand::Uncheck { .. }
        | BrowserCommand::SetInputFiles { .. }
        | BrowserCommand::Type { .. }
        | BrowserCommand::Fill { .. }
        | BrowserCommand::Press { .. }
        | BrowserCommand::Scroll { .. }
        | BrowserCommand::ScrollTo { .. }
        | BrowserCommand::Network(_) => Effect::Changes,
    }
}

pub(crate) enum Effect {
    Reads,
    Changes,
    Destroys,
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
    /// Inspect or override the browser session's complete user-agent string
    #[command(name = "ua")]
    UserAgent(UserAgentOptions),
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
    /// Outline the page: roles, names, states and a ref for every element
    /// that can be acted on
    Snapshot {
        #[arg(long, default_value = "current")]
        tab: String,
        /// Snapshot only this subtree
        #[command(flatten)]
        root: OptionalTarget,
        /// text (default), json, or both
        #[arg(long, value_parser = ["text", "json", "both"])]
        format: Option<String>,
        #[arg(long)]
        max_nodes: Option<u32>,
        #[arg(long)]
        max_chars: Option<u32>,
        /// Print only what changed since the previous snapshot
        #[arg(long)]
        since_last: bool,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Click an element (waits until it is visible, enabled, stable, and not covered)
    Click {
        #[arg(long, default_value = "current")]
        tab: String,
        #[command(flatten)]
        target: Target,
        /// left, right, or middle
        #[arg(long, value_parser = ["left", "right", "middle"], default_value = "left")]
        button: String,
        /// 2 for a double click
        #[arg(long, default_value_t = 1)]
        click_count: u32,
        #[command(flatten)]
        action: ActionFlags,
        /// Wait for navigation caused by this click
        #[arg(long)]
        wait_navigation: bool,
        /// Wait for document.readyState after navigation
        #[arg(long)]
        complete: bool,
    },
    /// Move the pointer over an element
    Hover {
        #[arg(long, default_value = "current")]
        tab: String,
        #[command(flatten)]
        target: Target,
        #[command(flatten)]
        action: ActionFlags,
    },
    /// Drag an element onto another
    Drag {
        #[arg(long, default_value = "current")]
        tab: String,
        #[command(flatten)]
        target: Target,
        /// Drop target ref
        #[arg(long, conflicts_with = "to_css")]
        to_ref: Option<String>,
        /// Drop target CSS selector
        #[arg(long)]
        to_css: Option<String>,
        #[command(flatten)]
        action: ActionFlags,
    },
    /// Type text at the end of an element's content, key by key (IME
    /// composition for non-ASCII text)
    Type {
        #[arg(long, default_value = "current")]
        tab: String,
        #[command(flatten)]
        target: Target,
        #[arg(long, allow_hyphen_values = true)]
        text: String,
        /// Clear the current content first
        #[arg(long)]
        clear: bool,
        #[command(flatten)]
        action: ActionFlags,
    },
    /// Replace an element's value (inputs, textareas, date/number/range, contenteditable)
    Fill {
        #[arg(long, default_value = "current")]
        tab: String,
        #[command(flatten)]
        target: Target,
        #[arg(long, allow_hyphen_values = true)]
        text: String,
        #[command(flatten)]
        action: ActionFlags,
    },
    /// Choose options of a <select>
    SelectOption {
        #[arg(long, default_value = "current")]
        tab: String,
        #[command(flatten)]
        target: Target,
        /// Option value or visible label; repeat for a multiple select
        #[arg(long = "option", required_unless_present = "option_index")]
        options: Vec<String>,
        /// Option position (0-based); repeat for a multiple select
        #[arg(long)]
        option_index: Vec<u32>,
        #[command(flatten)]
        action: ActionFlags,
    },
    /// Check a checkbox, radio, or switch (no-op when already checked)
    Check {
        #[arg(long, default_value = "current")]
        tab: String,
        #[command(flatten)]
        target: Target,
        #[command(flatten)]
        action: ActionFlags,
    },
    /// Uncheck a checkbox or switch (no-op when already unchecked)
    Uncheck {
        #[arg(long, default_value = "current")]
        tab: String,
        #[command(flatten)]
        target: Target,
        #[command(flatten)]
        action: ActionFlags,
    },
    /// Put files on an <input type=file> without the file chooser
    SetInputFiles {
        #[arg(long, default_value = "current")]
        tab: String,
        #[command(flatten)]
        target: Target,
        /// Files to attach (none clears the input)
        files: Vec<std::path::PathBuf>,
        #[command(flatten)]
        action: ActionFlags,
    },
    /// Press a key on an element, or on whatever has focus
    Press {
        #[arg(long, default_value = "current")]
        tab: String,
        #[command(flatten)]
        target: OptionalTarget,
        #[arg(long)]
        key: String,
        #[command(flatten)]
        action: ActionFlags,
        /// Wait for navigation caused by this key press
        #[arg(long)]
        wait_navigation: bool,
        /// Wait for document.readyState after navigation
        #[arg(long)]
        complete: bool,
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
    /// Scroll an element into view
    ScrollTo {
        #[arg(long, default_value = "current")]
        tab: String,
        #[command(flatten)]
        target: Target,
        #[command(flatten)]
        action: ActionFlags,
    },
    /// The page's main readable content as Markdown
    Text {
        #[arg(long, default_value = "current")]
        tab: String,
        /// Only this element's content
        #[command(flatten)]
        target: OptionalTarget,
        #[arg(long)]
        max_chars: Option<u32>,
        /// Include navigation, header, footer and sidebars
        #[arg(long)]
        all: bool,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// What the page logged: console messages and uncaught errors
    Console {
        #[arg(long, default_value = "current")]
        tab: String,
        /// Only entries after this seq (from a previous listing's next_seq)
        #[arg(long)]
        since: Option<u64>,
        /// error, warn, info, debug; repeatable
        #[arg(long = "level")]
        levels: Vec<String>,
        /// Clear the captured entries instead of listing them
        #[arg(long, conflicts_with_all = ["since", "levels"])]
        clear: bool,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Manage browser cookies
    Cookies(CookiesOptions),
    /// Inspect network traffic (request/response payloads). Windows sessions
    /// only — requires the WebView2 DevTools protocol.
    Network(NetworkOptions),
    /// Capture a PNG screenshot of the viewport, the full page, or one element
    Screenshot {
        #[arg(long, default_value = "current")]
        tab: String,
        /// Capture this element
        #[command(flatten)]
        target: OptionalTarget,
        /// Capture the whole page, not just the viewport
        #[arg(long)]
        full_page: bool,
        /// Draw each snapshot ref's box and label (viewport only) and print
        /// the ref → box map
        #[arg(long, conflicts_with = "full_page")]
        annotate: bool,
        /// Output path. Use `-` to write the PNG bytes to stdout.
        /// Defaults to `.lingxia/screenshots/<tab>-<ts>.png` under the project root.
        #[arg(long, short = 'o')]
        output: Option<String>,
        /// Print the JSON envelope (tab_id, size_bytes, data_base64) instead of writing a file
        #[arg(long)]
        json: bool,
    },
}

/// The element a command acts on.
#[derive(Args, Clone)]
#[group(required = true, multiple = false)]
pub struct Target {
    /// Element ref from `browser snapshot` (e.g. e12)
    #[arg(long = "ref")]
    pub reference: Option<String>,
    /// CSS selector
    #[arg(long = "css")]
    pub selector: Option<String>,
}

#[derive(Args, Clone)]
pub struct OptionalTarget {
    /// Element ref from `browser snapshot` (e.g. e12)
    #[arg(long = "ref", conflicts_with = "selector")]
    pub reference: Option<String>,
    /// CSS selector
    #[arg(long = "css")]
    pub selector: Option<String>,
}

#[derive(Args, Clone)]
pub struct ActionFlags {
    /// Index among the selector's matches
    #[arg(long)]
    pub index: Option<usize>,
    /// How long to wait for the element to become actionable, in milliseconds
    #[arg(long, default_value_t = 5000)]
    pub timeout_ms: u64,
    /// After acting, print what changed (`diff`) or the whole page (`full`)
    #[arg(long, value_parser = ["diff", "full"])]
    pub snapshot: Option<String>,
    /// Print JSON output
    #[arg(long)]
    pub json: bool,
}

fn target_json(
    reference: &Option<String>,
    selector: &Option<String>,
    index: Option<usize>,
) -> Value {
    let mut object = serde_json::Map::new();
    if let Some(reference) = reference {
        object.insert("ref".to_string(), json!(reference));
    }
    if let Some(selector) = selector {
        object.insert("selector".to_string(), json!(selector));
        if let Some(index) = index {
            object.insert("index".to_string(), json!(index));
        }
    }
    Value::Object(object)
}

/// Args for an action: tab, target, and the shared flags.
fn action_args(tab: &str, target: Value, action: &ActionFlags, extra: Value) -> Value {
    let mut object = match target {
        Value::Object(object) => object,
        _ => serde_json::Map::new(),
    };
    if action.index.is_some() && object.contains_key("selector") {
        object.insert("index".to_string(), json!(action.index));
    }
    object.insert("tab_id".to_string(), json!(tab));
    object.insert("timeout_ms".to_string(), json!(action.timeout_ms));
    if let Some(snapshot) = &action.snapshot {
        object.insert("snapshot".to_string(), json!(snapshot));
    }
    if let Value::Object(extra) = extra {
        object.extend(extra);
    }
    Value::Object(object)
}

/// Without --json an action prints nothing, or the page change it was asked for.
fn print_action(data: Option<Value>, json: bool) -> Result<()> {
    if json {
        return print_json(data.as_ref().unwrap_or(&json!({})), false);
    }
    let Some(snapshot) = data.as_ref().and_then(|d| d.get("snapshot")) else {
        return Ok(());
    };
    print_snapshot_text(snapshot);
    Ok(())
}

fn print_snapshot_text(snapshot: &Value) {
    if snapshot.get("navigated").and_then(Value::as_bool) == Some(true) {
        println!(
            "# navigated to {}",
            snapshot.get("url").and_then(Value::as_str).unwrap_or("")
        );
    }
    if let Some(diff) = snapshot.get("diff").filter(|d| !d.is_null()) {
        if diff.get("unchanged").and_then(Value::as_bool) == Some(true) {
            println!("# no changes");
        } else if let Some(text) = diff.get("text").and_then(Value::as_str) {
            println!("{text}");
        }
    }
    if let Some(text) = snapshot.get("text").and_then(Value::as_str) {
        println!("{text}");
    }
}

#[derive(Args, Clone)]
pub struct UserAgentOptions {
    /// Print compact JSON output
    #[arg(long, global = true, conflicts_with = "pretty")]
    pub json: bool,
    /// Print pretty JSON output
    #[arg(long, global = true, conflicts_with = "json")]
    pub pretty: bool,
    #[command(subcommand)]
    pub command: UserAgentCommand,
}

#[derive(Subcommand, Clone)]
pub enum UserAgentCommand {
    /// Show the configured override and effective navigator.userAgent
    Show,
    /// Set the complete user agent for current and future browser tabs
    Set {
        /// Complete user-agent string; this does not emulate browser features
        #[arg(allow_hyphen_values = true)]
        user_agent: String,
        /// Reload all open browser tabs after applying the override
        #[arg(long)]
        reload: bool,
    },
    /// Restore the platform WebView's default user agent
    Reset {
        /// Reload all open browser tabs after restoring the default
        #[arg(long)]
        reload: bool,
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

/// What these commands need beyond the transport. `lxdev` fills it from a dev
/// session; a product fills it from itself.
pub struct BrowserContext<'a> {
    pub transport: &'a dyn Transport,
    /// Platform the app runs on, as `app.doctor` reports it.
    pub target: String,
}

pub fn execute(context: &BrowserContext, options: BrowserOptions) -> Result<()> {
    let transport = context.transport;
    if matches!(effect(&options.command), Effect::Destroys) {
        crate::guard::confirm_destructive(options.allow_destructive)?;
    }

    match options.command {
        BrowserCommand::Open { url, tab, json } => {
            let url = normalize_open_url_for_target(&url, &context.target);
            let data = transport.request(
                methods::browser::OPEN,
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
            let data = transport.request(methods::browser::TABS, None)?;
            let data = data.unwrap_or_else(|| json!([]));
            if json {
                print_json(&data, false)?;
            } else {
                print_tabs(&data)?;
            }
        }
        BrowserCommand::Current { json } => {
            let data = transport
                .request(methods::browser::CURRENT, None)?
                .unwrap_or(Value::Null);
            if json {
                print_json(&data, false)?;
            } else {
                println!("{}", current_tab_summary(&data)?);
            }
        }
        BrowserCommand::Activate { tab, json } => {
            let data =
                transport.request(methods::browser::ACTIVATE, Some(json!({ "tab_id": tab })))?;
            print_optional_json(data, json)?;
        }
        BrowserCommand::Close { tab, json } => {
            let data =
                transport.request(methods::browser::CLOSE, Some(json!({ "tab_id": tab })))?;
            print_optional_json(data, json)?;
        }
        BrowserCommand::Reload { tab, json } => {
            let data =
                transport.request(methods::browser::RELOAD, Some(json!({ "tab_id": tab })))?;
            print_optional_json(data, json)?;
        }
        BrowserCommand::Back { tab, json } => {
            let data = transport.request(methods::browser::BACK, Some(json!({ "tab_id": tab })))?;
            print_optional_json(data, json)?;
        }
        BrowserCommand::Forward { tab, json } => {
            let data =
                transport.request(methods::browser::FORWARD, Some(json!({ "tab_id": tab })))?;
            print_optional_json(data, json)?;
        }
        BrowserCommand::UserAgent(options) => execute_user_agent(transport, options)?,
        BrowserCommand::Eval {
            tab,
            js,
            wait_navigation,
            complete,
            timeout_ms,
            json,
        } => {
            let data = transport.request(
                methods::browser::EVAL,
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
            let data = transport
                .request(
                    methods::browser::QUERY,
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
            let data = transport
                .request(
                    methods::browser::WAIT,
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
            let data = transport
                .request(
                    methods::browser::WAIT_URL,
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
            let data = transport
                .request(
                    methods::browser::WAIT_NAVIGATION,
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
        BrowserCommand::Snapshot {
            tab,
            root,
            format,
            max_nodes,
            max_chars,
            since_last,
            json,
        } => {
            let mut args = match target_json(&root.reference, &root.selector, None) {
                Value::Object(object) => object,
                _ => serde_json::Map::new(),
            };
            args.insert("tab_id".to_string(), json!(tab));
            if let Some(format) = &format {
                args.insert("format".to_string(), json!(format));
            }
            if let Some(max_nodes) = max_nodes {
                args.insert("max_nodes".to_string(), json!(max_nodes));
            }
            if let Some(max_chars) = max_chars {
                args.insert("max_chars".to_string(), json!(max_chars));
            }
            if since_last {
                args.insert("since".to_string(), json!("last"));
            }
            let data = transport
                .request(methods::browser::SNAPSHOT, Some(Value::Object(args)))?
                .unwrap_or(Value::Null);
            if json || format.as_deref() == Some("json") {
                print_json(&data, false)?;
            } else {
                print_snapshot_text(&data);
                if data.get("truncated").and_then(Value::as_bool) == Some(true) {
                    eprintln!(
                        "snapshot truncated; pass --ref/--css to snapshot a subtree or raise --max-nodes/--max-chars"
                    );
                }
            }
        }
        BrowserCommand::Click {
            tab,
            target,
            button,
            click_count,
            action,
            wait_navigation,
            complete,
        } => {
            let args = action_args(
                &tab,
                target_json(&target.reference, &target.selector, None),
                &action,
                json!({
                    "button": button,
                    "click_count": click_count,
                    "wait_navigation": wait_navigation,
                    "complete": complete,
                }),
            );
            let data = transport.request(methods::browser::CLICK, Some(args))?;
            print_action(data, action.json)?;
        }
        BrowserCommand::Hover {
            tab,
            target,
            action,
        } => {
            let args = action_args(
                &tab,
                target_json(&target.reference, &target.selector, None),
                &action,
                json!({}),
            );
            let data = transport.request(methods::browser::HOVER, Some(args))?;
            print_action(data, action.json)?;
        }
        BrowserCommand::Drag {
            tab,
            target,
            to_ref,
            to_css,
            action,
        } => {
            if to_ref.is_none() == to_css.is_none() {
                return Err(anyhow!("pass exactly one of --to-ref or --to-css"));
            }
            let args = action_args(
                &tab,
                target_json(&target.reference, &target.selector, None),
                &action,
                json!({ "to": target_json(&to_ref, &to_css, None) }),
            );
            let data = transport.request(methods::browser::DRAG, Some(args))?;
            print_action(data, action.json)?;
        }
        BrowserCommand::Type {
            tab,
            target,
            text,
            clear,
            action,
        } => {
            let args = action_args(
                &tab,
                target_json(&target.reference, &target.selector, None),
                &action,
                json!({ "text": text, "clear": clear }),
            );
            let data = transport.request(methods::browser::TYPE, Some(args))?;
            print_action(data, action.json)?;
        }
        BrowserCommand::Fill {
            tab,
            target,
            text,
            action,
        } => {
            let args = action_args(
                &tab,
                target_json(&target.reference, &target.selector, None),
                &action,
                json!({ "text": text }),
            );
            let data = transport.request(methods::browser::FILL, Some(args))?;
            print_action(data, action.json)?;
        }
        BrowserCommand::SelectOption {
            tab,
            target,
            options,
            option_index,
            action,
        } => {
            let mut choices: Vec<Value> = options.into_iter().map(Value::String).collect();
            choices.extend(
                option_index
                    .into_iter()
                    .map(|index| json!({ "index": index })),
            );
            let args = action_args(
                &tab,
                target_json(&target.reference, &target.selector, None),
                &action,
                json!({ "options": choices }),
            );
            let data = transport.request(methods::browser::SELECT_OPTION, Some(args))?;
            print_action(data, action.json)?;
        }
        BrowserCommand::Check {
            tab,
            target,
            action,
        } => {
            let args = action_args(
                &tab,
                target_json(&target.reference, &target.selector, None),
                &action,
                json!({}),
            );
            let data = transport.request(methods::browser::CHECK, Some(args))?;
            print_action(data, action.json)?;
        }
        BrowserCommand::Uncheck {
            tab,
            target,
            action,
        } => {
            let method = methods::browser::UNCHECK;
            let args = action_args(
                &tab,
                target_json(&target.reference, &target.selector, None),
                &action,
                json!({}),
            );
            let data = transport.request(method, Some(args))?;
            print_action(data, action.json)?;
        }
        BrowserCommand::SetInputFiles {
            tab,
            target,
            files,
            action,
        } => {
            let cwd = std::env::current_dir()?;
            let files: Vec<String> = files
                .into_iter()
                .map(|path| {
                    let path = if path.is_absolute() {
                        path
                    } else {
                        cwd.join(path)
                    };
                    path.to_string_lossy().into_owned()
                })
                .collect();
            let args = action_args(
                &tab,
                target_json(&target.reference, &target.selector, None),
                &action,
                json!({ "files": files }),
            );
            let data = transport.request(methods::browser::SET_INPUT_FILES, Some(args))?;
            print_action(data, action.json)?;
        }
        BrowserCommand::Press {
            tab,
            target,
            key,
            action,
            wait_navigation,
            complete,
        } => {
            let args = action_args(
                &tab,
                target_json(&target.reference, &target.selector, None),
                &action,
                json!({
                    "key": key,
                    "wait_navigation": wait_navigation,
                    "complete": complete,
                }),
            );
            let data = transport.request(methods::browser::PRESS, Some(args))?;
            print_action(data, action.json)?;
        }
        BrowserCommand::Scroll { tab, dx, dy, json } => {
            let data = transport.request(
                methods::browser::SCROLL,
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
            target,
            action,
        } => {
            let args = action_args(
                &tab,
                target_json(&target.reference, &target.selector, None),
                &action,
                json!({}),
            );
            let data = transport.request(methods::browser::SCROLL_TO, Some(args))?;
            print_action(data, action.json)?;
        }
        BrowserCommand::Text {
            tab,
            target,
            max_chars,
            all,
            json,
        } => {
            let mut args = match target_json(&target.reference, &target.selector, None) {
                Value::Object(object) => object,
                _ => serde_json::Map::new(),
            };
            args.insert("tab_id".to_string(), json!(tab));
            args.insert("all".to_string(), json!(all));
            if let Some(max_chars) = max_chars {
                args.insert("max_chars".to_string(), json!(max_chars));
            }
            let data = transport
                .request(methods::browser::TEXT, Some(Value::Object(args)))?
                .unwrap_or(Value::Null);
            if json {
                print_json(&data, false)?;
            } else {
                println!(
                    "{}",
                    terminal_safe(data.get("markdown").and_then(Value::as_str).unwrap_or(""))
                );
            }
        }
        BrowserCommand::Console {
            tab,
            since,
            levels,
            clear,
            json,
        } => {
            if clear {
                let data = transport.request(
                    methods::browser::CONSOLE_CLEAR,
                    Some(json!({ "tab_id": tab })),
                )?;
                print_optional_json(data, json)?;
            } else {
                let data = transport
                    .request(
                        methods::browser::CONSOLE_LIST,
                        Some(json!({ "tab_id": tab, "since": since, "levels": levels })),
                    )?
                    .unwrap_or(Value::Null);
                if json {
                    print_json(&data, false)?;
                } else {
                    print_console(&data);
                }
            }
        }
        BrowserCommand::Cookies(options) => execute_cookies(transport, options)?,
        BrowserCommand::Network(options) => execute_network(context, options)?,
        BrowserCommand::Screenshot {
            tab,
            target,
            full_page,
            annotate,
            output,
            json,
        } => {
            let mut args = match target_json(&target.reference, &target.selector, None) {
                Value::Object(object) => object,
                _ => serde_json::Map::new(),
            };
            args.insert("tab_id".to_string(), json!(tab));
            args.insert("full_page".to_string(), json!(full_page));
            args.insert("annotate".to_string(), json!(annotate));
            execute_screenshot(transport, tab, Value::Object(args), output, json)?
        }
    }

    Ok(())
}

fn execute_user_agent(transport: &dyn Transport, options: UserAgentOptions) -> Result<()> {
    let (handler, args, show_human_output) = match options.command {
        UserAgentCommand::Show => (methods::browser::UA_SHOW, json!({}), true),
        UserAgentCommand::Set { user_agent, reload } => (
            methods::browser::UA_SET,
            json!({
                "user_agent": user_agent,
                "reload": reload,
            }),
            false,
        ),
        UserAgentCommand::Reset { reload } => (
            methods::browser::UA_RESET,
            json!({ "reload": reload }),
            false,
        ),
    };
    let data = transport
        .request(handler, Some(args))?
        .unwrap_or(Value::Null);
    if options.json || options.pretty {
        print_json(&data, options.pretty)?;
    } else if show_human_output {
        print_user_agent_state(&data)?;
    }
    Ok(())
}

fn print_user_agent_state(data: &Value) -> Result<()> {
    let configured = match data.get("configured") {
        Some(Value::String(value)) => value.as_str(),
        Some(Value::Null) => "platform default",
        _ => {
            return Err(anyhow!(
                "browser user-agent response had invalid configured state"
            ));
        }
    };
    let effective = match data.get("effective") {
        Some(Value::String(value)) => value.as_str(),
        Some(Value::Null) => "unavailable (no open browser tab)",
        _ => {
            return Err(anyhow!(
                "browser user-agent response had invalid effective state"
            ));
        }
    };
    println!("configured: {configured}");
    println!("effective: {effective}");
    Ok(())
}

fn execute_network(context: &BrowserContext, options: NetworkOptions) -> Result<()> {
    // Runner is platform-neutral session metadata; its compiled runtime
    // handler is the authority on whether WebView2 capture is available.
    let platform = context.target.as_str();
    if !network_capture_may_be_supported(platform) {
        return Err(anyhow!(
            "browser network capture needs a Windows WebView2 session (this session is '{platform}')",
        ));
    }
    let transport = context.transport;
    let tab = options.tab;
    match options.command {
        NetworkCommand::Enable => {
            transport.request(
                methods::browser::NETWORK_ENABLE,
                Some(json!({ "tab_id": tab })),
            )?;
            println!("network capture enabled for {tab}");
        }
        NetworkCommand::Disable => {
            transport.request(
                methods::browser::NETWORK_DISABLE,
                Some(json!({ "tab_id": tab })),
            )?;
            println!("network capture disabled for {tab}");
        }
        NetworkCommand::Clear => {
            transport.request(
                methods::browser::NETWORK_CLEAR,
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
            let snapshot = network_snapshot(transport, &tab)?;
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
            let snapshot = network_snapshot(transport, &tab)?;
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

fn network_capture_may_be_supported(target: &str) -> bool {
    matches!(
        target.to_ascii_lowercase().as_str(),
        "windows" | "lxapp" | "runner"
    )
}

fn network_snapshot(transport: &dyn Transport, tab: &str) -> Result<Value> {
    Ok(transport
        .request(
            methods::browser::NETWORK_LIST,
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

fn execute_screenshot(
    transport: &dyn Transport,
    tab: String,
    args: Value,
    output: Option<String>,
    json: bool,
) -> Result<()> {
    let data = transport
        .request(methods::browser::SCREENSHOT, Some(args))?
        .unwrap_or(Value::Null);

    if json {
        return print_json(&data, false);
    }

    let bytes = output::decode_png_payload(&data, methods::browser::SCREENSHOT)?;
    let resolved_tab = data
        .get("tab_id")
        .and_then(Value::as_str)
        .unwrap_or(tab.as_str())
        .to_string();
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let tab = output::safe_component(&resolved_tab);
    let to_stdout = matches!(output.as_deref(), Some("-"));
    output::write_png(output, format!("{tab}-{ts}.png"), &bytes)?;
    // The marks go to stderr when the PNG itself is on stdout.
    if let Some(marks) = data.get("marks").and_then(Value::as_array) {
        for mark in marks {
            let line = format!(
                "{} {}{} @ {:.0},{:.0} {:.0}x{:.0}",
                mark.get("ref").and_then(Value::as_str).unwrap_or(""),
                mark.get("role").and_then(Value::as_str).unwrap_or(""),
                mark.get("name")
                    .and_then(Value::as_str)
                    .map(|name| format!(" {}", serde_json::to_string(name).unwrap_or_default()))
                    .unwrap_or_default(),
                mark["box"]["x"].as_f64().unwrap_or(0.0),
                mark["box"]["y"].as_f64().unwrap_or(0.0),
                mark["box"]["width"].as_f64().unwrap_or(0.0),
                mark["box"]["height"].as_f64().unwrap_or(0.0),
            );
            if to_stdout {
                eprintln!("{line}");
            } else {
                println!("{line}");
            }
        }
    }
    if data.get("clipped").and_then(Value::as_bool) == Some(true) {
        eprintln!("element is larger than the viewport; the capture is cut at its edges");
    }
    Ok(())
}

/// Page text without terminal control sequences: a page must not be able to
/// drive the terminal it is printed to.
fn terminal_safe(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .collect()
}

fn print_console(data: &Value) {
    let Some(entries) = data.get("entries").and_then(Value::as_array) else {
        return;
    };
    if entries.is_empty() {
        println!("No console entries.");
    }
    for entry in entries {
        println!(
            "[{}] {}: {}",
            entry.get("seq").and_then(Value::as_u64).unwrap_or(0),
            entry.get("level").and_then(Value::as_str).unwrap_or(""),
            terminal_safe(entry.get("text").and_then(Value::as_str).unwrap_or("")),
        );
    }
    if let Some(dropped) = data
        .get("dropped")
        .and_then(Value::as_u64)
        .filter(|d| *d > 0)
    {
        println!("({dropped} older entries were dropped)");
    }
}

fn execute_cookies(transport: &dyn Transport, options: CookiesOptions) -> Result<()> {
    let tab = options.tab;
    match options.command {
        CookiesCommand::List { visible, pretty } => {
            let data = transport
                .request(
                    methods::browser::COOKIES_LIST,
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
            let data = transport.request(
                methods::browser::COOKIES_SET,
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
            let data = transport.request(
                methods::browser::COOKIES_DELETE,
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
            let data = transport.request(
                methods::browser::COOKIES_CLEAR,
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

fn current_tab_summary(data: &Value) -> Result<String> {
    let tab_id = data
        .get("tab_id")
        .and_then(Value::as_str)
        .context("no current browser tab")?;
    let url = data
        .get("current_url")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .or_else(|| data.get("path").and_then(Value::as_str))
        .unwrap_or("-");
    Ok(format!("{tab_id}  {url}"))
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

/// Runner does not bundle the product browser's `lingxia://newtab` webui.
/// Keep an explicitly empty automation target aligned with its native + button.
fn normalize_open_url_for_target(input: &str, target: &str) -> String {
    if input.trim().is_empty() && target.eq_ignore_ascii_case("runner") {
        return "about:blank".to_string();
    }
    normalize_open_url(input)
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
        home_dir()?.into()
    } else if let Some(rest) = input.strip_prefix("~/") {
        let mut home = PathBuf::from(home_dir()?);
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

fn home_dir() -> Option<std::ffi::OsString> {
    if let Some(home) = std::env::var_os("HOME") {
        return Some(home);
    }
    // On Windows HOME is often unset; fall back to USERPROFILE, then
    // HOMEDRIVE+HOMEPATH.
    #[cfg(windows)]
    {
        if let Some(profile) = std::env::var_os("USERPROFILE") {
            return Some(profile);
        }
        if let (Some(drive), Some(path)) =
            (std::env::var_os("HOMEDRIVE"), std::env::var_os("HOMEPATH"))
        {
            let mut home = drive;
            home.push(path);
            return Some(home);
        }
    }
    None
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
    use super::{
        BrowserCommand, BrowserOptions, Effect, current_tab_summary, effect, file_url_from_path,
        local_file_url, network_capture_may_be_supported, normalize_open_url,
        normalize_open_url_for_target,
    };
    use serde_json::json;

    fn effect_of(argv: &[&str]) -> Effect {
        use clap::Parser;
        #[derive(Parser)]
        struct Cli {
            #[command(flatten)]
            options: BrowserOptions,
        }
        let cli = Cli::try_parse_from(std::iter::once("browser").chain(argv.iter().copied()))
            .unwrap_or_else(|error| panic!("{}: {error}", argv.join(" ")));
        effect(&cli.options.command)
    }

    /// A tab is somebody's logged-in session. That the surface is web rather
    /// than native does not make typing into it, running script in it, or
    /// deleting its cookies something an agent may do unasked.
    #[test]
    fn changing_a_page_needs_permission_and_losing_state_needs_more() {
        for argv in [
            vec!["tabs"],
            vec!["current"],
            vec!["screenshot"],
            vec!["cookies", "list"],
        ] {
            assert!(
                matches!(effect_of(&argv), Effect::Reads),
                "`{}` only reads",
                argv.join(" ")
            );
        }

        for argv in [
            vec!["open", "http://example.com"],
            vec!["reload"],
            vec!["back"],
        ] {
            assert!(
                matches!(effect_of(&argv), Effect::Changes),
                "`{}` changes the page",
                argv.join(" ")
            );
        }

        for argv in [vec!["close"], vec!["cookies", "clear"]] {
            assert!(
                matches!(effect_of(&argv), Effect::Destroys),
                "`{}` loses state",
                argv.join(" ")
            );
        }

        // Nothing may be silently added to the read-only side later: the match
        // in `effect` has no wildcard, so a new command fails to compile until
        // someone says what it does.
        assert!(matches!(
            effect(&BrowserCommand::Tabs { json: false }),
            Effect::Reads
        ));
    }
    use std::path::Path;

    #[test]
    fn current_tab_summary_includes_url() {
        let summary = current_tab_summary(&json!({
            "tab_id": "tab-3",
            "current_url": "https://example.com/path",
            "path": "/tabs/tab-3"
        }))
        .unwrap();

        assert_eq!(summary, "tab-3  https://example.com/path");
    }

    #[test]
    fn current_tab_summary_falls_back_to_path() {
        let summary = current_tab_summary(&json!({
            "tab_id": "tab-3",
            "current_url": "",
            "path": "/tabs/tab-3"
        }))
        .unwrap();

        assert_eq!(summary, "tab-3  /tabs/tab-3");
    }

    #[test]
    fn network_capture_allows_platform_neutral_runner_sessions() {
        assert!(network_capture_may_be_supported("windows"));
        assert!(network_capture_may_be_supported("lxapp"));
        assert!(network_capture_may_be_supported("runner"));
        assert!(!network_capture_may_be_supported("macos"));
    }

    #[test]
    fn page_text_cannot_drive_the_terminal() {
        assert_eq!(
            crate::browser::terminal_safe("a\u{1b}[2Jb\nc\td\r"),
            "a[2Jb\nc\td"
        );
    }

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

    #[test]
    fn runner_empty_url_opens_a_blank_tab() {
        assert_eq!(normalize_open_url_for_target("", "runner"), "about:blank");
        assert_eq!(normalize_open_url_for_target("", "macos"), "");
        assert_eq!(
            normalize_open_url_for_target("example.com", "runner"),
            "https://example.com"
        );
    }
}
