use crate::output;
use crate::transport::Transport;
use anyhow::{Result, bail};
use clap::{Args, Subcommand, ValueEnum};
use lingxia_devtool_protocol::handlers;
use serde_json::{Map, Value, json};

/// What these commands need beyond the transport.
///
/// `lxdev` fills this from a dev session; a shipped product fills it from
/// itself, which is why it is not the session type — a product has no session
/// id and its target is simply the OS it is running on.
pub struct AppContext<'a> {
    pub transport: &'a dyn Transport,
    /// Platform name the app is running on, as `app.doctor` reports it.
    pub target: String,
    /// Dev session id, when there is one.
    pub session: Option<String>,
}

#[derive(Args, Clone)]
pub struct AppOptions {
    /// Authorize input sent to the host app window
    #[arg(long, global = true)]
    pub allow_control: bool,
    #[command(subcommand)]
    pub command: AppCommand,
}

#[derive(Subcommand, Clone)]
pub enum AppCommand {
    /// Report host-window automation capabilities
    Doctor {
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Capture a PNG screenshot of the host app's window
    Screenshot {
        /// Specific window id (from `app windows`); defaults to the
        /// platform's focused/main window.
        #[arg(long)]
        window: Option<String>,
        /// Output path; use `-` for stdout. Default:
        /// `.lingxia/screenshots/app-<target>-<ts>.png`
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
    /// Send mouse input to the host app window
    Mouse {
        #[command(subcommand)]
        command: MouseCommand,
    },
    /// Send keyboard input to the host app window's focused control
    Key {
        #[command(subcommand)]
        command: KeyCommand,
    },
}

#[derive(Subcommand, Clone)]
pub enum KeyCommand {
    /// Type literal text into the focused control
    Type(KeyTypeOptions),
    /// Press a named key (return, tab, escape, delete, space, arrows)
    Press(KeyPressOptions),
}

#[derive(Args, Clone)]
pub struct KeyTypeOptions {
    /// Text to type
    #[arg(allow_hyphen_values = true)]
    text: String,
    #[command(flatten)]
    target: KeyTargetOptions,
}

#[derive(Args, Clone)]
pub struct KeyPressOptions {
    /// Key name: return, tab, escape, delete, space, left, right, up, down
    key: String,
    /// Modifier keys held during the press (repeatable)
    #[arg(long, value_enum)]
    modifiers: Vec<KeyModifierArg>,
    #[command(flatten)]
    target: KeyTargetOptions,
}

#[derive(Args, Clone)]
pub struct KeyTargetOptions {
    /// Specific window id (from `app windows`); defaults to the
    /// platform's focused/main window.
    #[arg(long)]
    window: Option<String>,
    /// Print JSON output
    #[arg(long)]
    json: bool,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum KeyModifierArg {
    Command,
    Shift,
    Option,
    Control,
}

impl KeyModifierArg {
    fn as_protocol_str(self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::Shift => "shift",
            Self::Option => "option",
            Self::Control => "control",
        }
    }
}

#[derive(Subcommand, Clone)]
pub enum MouseCommand {
    /// Move the mouse pointer to a window content coordinate
    Move(MousePointOptions),
    /// Press a mouse button at a window content coordinate
    Down(MouseButtonPointOptions),
    /// Release a mouse button at a window content coordinate
    Up(MouseButtonPointOptions),
    /// Click at a window content coordinate
    Click(MouseClickOptions),
    /// Drag between two window content coordinates
    Drag(MouseDragOptions),
    /// Scroll at a window content coordinate
    Scroll(MouseScrollOptions),
}

#[derive(Args, Clone)]
pub struct MouseTargetOptions {
    /// Specific window id (from `app windows`); defaults to the
    /// platform's focused/main window.
    #[arg(long)]
    window: Option<String>,
    /// Print JSON output
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone)]
pub struct MousePointOptions {
    /// X coordinate in platform window-content units
    #[arg(long)]
    x: f64,
    /// Y coordinate in platform window-content units
    #[arg(long)]
    y: f64,
    #[command(flatten)]
    target: MouseTargetOptions,
}

#[derive(Args, Clone)]
pub struct MouseButtonPointOptions {
    /// X coordinate in platform window-content units
    #[arg(long)]
    x: f64,
    /// Y coordinate in platform window-content units
    #[arg(long)]
    y: f64,
    /// Mouse button
    #[arg(long, value_enum, default_value = "left")]
    button: MouseButtonArg,
    #[command(flatten)]
    target: MouseTargetOptions,
}

#[derive(Args, Clone)]
pub struct MouseClickOptions {
    /// X coordinate in platform window-content units
    #[arg(long)]
    x: f64,
    /// Y coordinate in platform window-content units
    #[arg(long)]
    y: f64,
    /// Mouse button
    #[arg(long, value_enum, default_value = "left")]
    button: MouseButtonArg,
    /// Number of clicks to report in the event
    #[arg(long, default_value_t = 1)]
    click_count: u8,
    #[command(flatten)]
    target: MouseTargetOptions,
}

#[derive(Args, Clone)]
pub struct MouseDragOptions {
    /// Starting X coordinate in platform window-content units
    #[arg(long)]
    from_x: f64,
    /// Starting Y coordinate in platform window-content units
    #[arg(long)]
    from_y: f64,
    /// Ending X coordinate in platform window-content units
    #[arg(long)]
    to_x: f64,
    /// Ending Y coordinate in platform window-content units
    #[arg(long)]
    to_y: f64,
    /// Mouse button
    #[arg(long, value_enum, default_value = "left")]
    button: MouseButtonArg,
    #[command(flatten)]
    target: MouseTargetOptions,
}

#[derive(Args, Clone)]
pub struct MouseScrollOptions {
    /// X coordinate in platform window-content units
    #[arg(long)]
    x: f64,
    /// Y coordinate in platform window-content units
    #[arg(long)]
    y: f64,
    /// Horizontal scroll delta in platform window-content units
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    dx: f64,
    /// Vertical scroll delta in platform window-content units
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    dy: f64,
    #[command(flatten)]
    target: MouseTargetOptions,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum MouseButtonArg {
    Left,
    Right,
    Middle,
}

impl MouseButtonArg {
    fn as_protocol_str(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Middle => "middle",
        }
    }
}

pub fn execute(context: &AppContext, options: AppOptions) -> Result<()> {
    // Synthetic input is synthetic input; that the window belongs to the
    // product rather than to some other app does not make it free.
    if matches!(
        options.command,
        AppCommand::Mouse { .. } | AppCommand::Key { .. }
    ) {
        crate::guard::gate(options.allow_control, false, false)?;
    }
    match options.command {
        AppCommand::Doctor { json } => execute_doctor(context, json),
        AppCommand::Screenshot {
            window,
            output,
            json,
        } => execute_screenshot(context, window, output, json),
        AppCommand::Windows { json } => execute_windows(context, json),
        AppCommand::Mouse { command } => {
            require_desktop_input(context, "mouse")?;
            execute_mouse(context, command)
        }
        AppCommand::Key { command } => {
            require_desktop_input(context, "key")?;
            execute_key(context, command)
        }
    }
}

fn execute_doctor(context: &AppContext, json_output: bool) -> Result<()> {
    let mut data = context
        .transport
        .request(handlers::app::DOCTOR, None)?
        .unwrap_or_else(|| json!({}));
    if let (Value::Object(map), Some(session)) = (&mut data, context.session.as_ref()) {
        map.insert("session_id".to_string(), json!(session));
    }
    if json_output {
        println!("{}", encode_machine_json(&data)?);
        return Ok(());
    }

    let capabilities = data.get("capabilities").and_then(Value::as_object);
    let supported = |name: &str| {
        capabilities
            .and_then(|caps| caps.get(name))
            .and_then(|cap| cap.get("supported"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    if let Some(session) = context.session.as_ref() {
        println!("session      {session}");
    }
    println!(
        "platform     {}",
        data.get("platform").and_then(Value::as_str).unwrap_or("-")
    );
    println!(
        "coordinates  {}",
        data.pointer("/coordinate_spaces/window")
            .and_then(Value::as_str)
            .unwrap_or("-")
    );
    for name in ["windows", "screenshot", "mouse", "keyboard"] {
        println!("{name:<12} {}", if supported(name) { "yes" } else { "no" });
    }
    let modifiers = data
        .pointer("/capabilities/keyboard_modifiers/reliability")
        .and_then(Value::as_str)
        .unwrap_or("unsupported");
    println!("modifiers    {modifiers}");
    Ok(())
}

fn require_desktop_input(context: &AppContext, what: &str) -> Result<()> {
    let hint = match context.target.as_str() {
        "macos" | "windows" | "lxapp" => return Ok(()),
        "android" => "`adb shell input`",
        "harmony" => "`hdc shell uitest uiInput`",
        // Named without a binary: this table is mounted by more than one.
        _ => "the `lxapp page click/type` commands (web content)",
    };
    bail!(
        "app {what} is desktop-only; on {} use {hint}",
        context.target
    )
}

fn execute_windows(context: &AppContext, json: bool) -> Result<()> {
    let data = context
        .transport
        .request(handlers::app::WINDOWS, None)?
        .unwrap_or(Value::Array(Vec::new()));

    if json {
        println!("{}", encode_machine_json(&data)?);
        return Ok(());
    }

    let Some(array) = data.as_array() else {
        println!("{}", encode_machine_json(&data)?);
        return Ok(());
    };
    if array.is_empty() {
        println!("No windows reported by host app.");
        return Ok(());
    }
    println!(
        "{:<12}  {:<5}  {:<5}  {:<7}  {:<9}  TITLE",
        "ID", "FOCUS", "MAIN", "VISIBLE", "SIZE"
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
    context: &AppContext,
    window: Option<String>,
    output: Option<String>,
    json: bool,
) -> Result<()> {
    let args = window.as_ref().map(|id| json!({ "window_id": id }));
    let data = context
        .transport
        .request(handlers::app::SCREENSHOT, args)?
        .unwrap_or(Value::Null);

    if json {
        println!("{}", serde_json::to_string(&data)?);
        return Ok(());
    }

    let bytes = output::decode_png_payload(&data, handlers::app::SCREENSHOT)?;
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let target = output::safe_component(&context.target);
    output::write_png(output, format!("app-{target}-{ts}.png"), &bytes)
}

fn execute_mouse(context: &AppContext, command: MouseCommand) -> Result<()> {
    let (target, actions): (MouseTargetOptions, Vec<Value>) = match command {
        MouseCommand::Move(options) => (
            options.target,
            vec![json!({ "kind": "move", "x": options.x, "y": options.y })],
        ),
        MouseCommand::Down(options) => (
            options.target,
            vec![json!({
                "kind": "down",
                "x": options.x,
                "y": options.y,
                "button": options.button.as_protocol_str(),
            })],
        ),
        MouseCommand::Up(options) => (
            options.target,
            vec![json!({
                "kind": "up",
                "x": options.x,
                "y": options.y,
                "button": options.button.as_protocol_str(),
            })],
        ),
        MouseCommand::Click(options) => {
            if options.click_count == 0 {
                bail!("--click-count must be greater than zero");
            }
            (
                options.target,
                vec![
                    json!({ "kind": "move", "x": options.x, "y": options.y }),
                    json!({
                        "kind": "click",
                        "x": options.x,
                        "y": options.y,
                        "button": options.button.as_protocol_str(),
                        "click_count": options.click_count,
                    }),
                ],
            )
        }
        MouseCommand::Drag(options) => (
            options.target,
            vec![json!({
                "kind": "drag",
                "from_x": options.from_x,
                "from_y": options.from_y,
                "to_x": options.to_x,
                "to_y": options.to_y,
                "button": options.button.as_protocol_str(),
            })],
        ),
        MouseCommand::Scroll(options) => (
            options.target,
            vec![json!({
                "kind": "scroll",
                "x": options.x,
                "y": options.y,
                "dx": options.dx,
                "dy": options.dy,
            })],
        ),
    };

    let mut data = Value::Null;
    for action in actions {
        let payload = action_payload(target.window.clone(), action);
        data = context
            .transport
            .request(handlers::app::MOUSE, Some(payload))?
            .unwrap_or(Value::Null);
    }

    if target.json {
        println!("{}", encode_machine_json(&data)?);
        return Ok(());
    }
    Ok(())
}

fn execute_key(context: &AppContext, command: KeyCommand) -> Result<()> {
    let (target, action) = match command {
        KeyCommand::Type(options) => (
            options.target,
            json!({ "kind": "type", "text": options.text }),
        ),
        KeyCommand::Press(options) => {
            let modifiers: Vec<&str> = options
                .modifiers
                .iter()
                .map(|modifier| modifier.as_protocol_str())
                .collect();
            (
                options.target,
                json!({
                    "kind": "press",
                    "key": options.key,
                    "modifiers": modifiers,
                }),
            )
        }
    };

    let payload = action_payload(target.window, action);
    let data = context
        .transport
        .request(handlers::app::KEYBOARD, Some(payload))?
        .unwrap_or(Value::Null);

    if target.json {
        println!("{}", encode_machine_json(&data)?);
        return Ok(());
    }
    if data.get("modifier_reliability").and_then(Value::as_str) == Some("best_effort") {
        eprintln!(
            "Warning: Windows app modifier chords are best-effort; verify the resulting state."
        );
    }
    Ok(())
}

fn action_payload(window: Option<String>, action: Value) -> Value {
    let mut payload = Map::new();
    if let Some(window) = window {
        payload.insert("window_id".to_string(), Value::String(window));
    }
    payload.insert("action".to_string(), action);
    Value::Object(payload)
}

fn encode_machine_json(value: &Value) -> Result<String> {
    serde_json::to_string(value).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct TestCli {
        #[command(flatten)]
        app: AppOptions,
    }

    #[test]
    fn parses_window_screenshot() {
        let cli = TestCli::try_parse_from([
            "test",
            "screenshot",
            "--window",
            "42",
            "--output",
            "capture.png",
        ])
        .unwrap();
        assert!(matches!(
            cli.app.command,
            AppCommand::Screenshot {
                window: Some(window),
                output: Some(output),
                json: false,
            } if window == "42" && output == "capture.png"
        ));
    }

    #[test]
    fn parses_app_doctor_json() {
        let cli = TestCli::try_parse_from(["test", "doctor", "--json"]).unwrap();
        assert!(matches!(cli.app.command, AppCommand::Doctor { json: true }));
    }

    #[test]
    fn parses_mouse_click() {
        let cli = TestCli::try_parse_from([
            "test", "mouse", "click", "--x", "10", "--y", "20", "--window", "42",
        ])
        .unwrap();
        assert!(matches!(
            cli.app.command,
            AppCommand::Mouse {
                command: MouseCommand::Click(MouseClickOptions {
                    target: MouseTargetOptions {
                        window: Some(window),
                        ..
                    },
                    ..
                }),
            } if window == "42"
        ));
    }

    #[test]
    fn app_key_type_accepts_leading_hyphen_text() {
        let cli = TestCli::try_parse_from(["test", "key", "type", "-typed"]).unwrap();
        assert!(matches!(
            cli.app.command,
            AppCommand::Key {
                command: KeyCommand::Type(KeyTypeOptions { text, .. })
            } if text == "-typed"
        ));
    }

    #[test]
    fn machine_json_is_compact() {
        assert_eq!(
            encode_machine_json(&json!({ "ok": true, "action": "click" })).unwrap(),
            r#"{"ok":true,"action":"click"}"#
        );
    }
}
