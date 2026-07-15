use crate::client;
use crate::project::SessionInfo;
use crate::screenshot;
use anyhow::{Result, bail};
use clap::{Args, Subcommand, ValueEnum};
use lingxia_devtool_protocol::handlers;
use serde_json::{Map, Value, json};

#[derive(Args, Clone)]
pub struct AppOptions {
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
        /// Specific window id (from `lxdev app windows`); defaults to the
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
    /// Specific window id (from `lxdev app windows`); defaults to the
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
    /// Specific window id (from `lxdev app windows`); defaults to the
    /// platform's focused/main window.
    #[arg(long)]
    window: Option<String>,
    /// Print JSON output
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone)]
pub struct MousePointOptions {
    /// X coordinate in logical window content points
    #[arg(long)]
    x: f64,
    /// Y coordinate in logical window content points
    #[arg(long)]
    y: f64,
    #[command(flatten)]
    target: MouseTargetOptions,
}

#[derive(Args, Clone)]
pub struct MouseButtonPointOptions {
    /// X coordinate in logical window content points
    #[arg(long)]
    x: f64,
    /// Y coordinate in logical window content points
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
    /// X coordinate in logical window content points
    #[arg(long)]
    x: f64,
    /// Y coordinate in logical window content points
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
    /// Starting X coordinate in logical window content points
    #[arg(long)]
    from_x: f64,
    /// Starting Y coordinate in logical window content points
    #[arg(long)]
    from_y: f64,
    /// Ending X coordinate in logical window content points
    #[arg(long)]
    to_x: f64,
    /// Ending Y coordinate in logical window content points
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
    /// X coordinate in logical window content points
    #[arg(long)]
    x: f64,
    /// Y coordinate in logical window content points
    #[arg(long)]
    y: f64,
    /// Horizontal scroll delta in logical points
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    dx: f64,
    /// Vertical scroll delta in logical points
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

pub fn execute(info: &SessionInfo, options: AppOptions) -> Result<()> {
    match options.command {
        AppCommand::Doctor { json } => execute_doctor(info, json),
        AppCommand::Screenshot {
            window,
            output,
            json,
        } => execute_screenshot(info, window, output, json),
        AppCommand::Windows { json } => execute_windows(info, json),
        AppCommand::Mouse { command } => {
            require_desktop_input(info, "mouse")?;
            execute_mouse(info, command)
        }
        AppCommand::Key { command } => {
            require_desktop_input(info, "key")?;
            execute_key(info, command)
        }
    }
}

fn execute_doctor(info: &SessionInfo, json_output: bool) -> Result<()> {
    let mut data = client::execute_command(&info.ws_url, handlers::app::DOCTOR, None)?
        .unwrap_or_else(|| json!({}));
    if let Value::Object(map) = &mut data {
        map.insert("session_id".to_string(), json!(info.session_id));
    }
    if json_output {
        println!("{}", serde_json::to_string_pretty(&data)?);
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
    println!("session      {}", info.session_id);
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

fn require_desktop_input(info: &SessionInfo, what: &str) -> Result<()> {
    let hint = match info.target.as_str() {
        "macos" | "windows" | "lxapp" => return Ok(()),
        "android" => "`adb shell input`",
        "harmony" => "`hdc shell uitest uiInput`",
        _ => "`lxdev lxapp page click/type` (web content)",
    };
    bail!("app {what} is desktop-only; on {} use {hint}", info.target)
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
    let target = screenshot::safe_component(&info.target);
    screenshot::write_png(output, format!("app-{target}-{ts}.png"), &bytes)
}

fn execute_mouse(info: &SessionInfo, command: MouseCommand) -> Result<()> {
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
        data = client::execute_command(&info.ws_url, handlers::app::MOUSE, Some(payload))?
            .unwrap_or(Value::Null);
    }

    if target.json {
        println!("{}", serde_json::to_string_pretty(&data)?);
        return Ok(());
    }

    let action = data
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("mouse");
    let window_id = data
        .get("window_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    println!("Sent app mouse {action} to window {window_id}");
    Ok(())
}

fn execute_key(info: &SessionInfo, command: KeyCommand) -> Result<()> {
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
    let data = client::execute_command(&info.ws_url, handlers::app::KEYBOARD, Some(payload))?
        .unwrap_or(Value::Null);

    if target.json {
        println!("{}", serde_json::to_string_pretty(&data)?);
        return Ok(());
    }

    let action = data.get("action").and_then(Value::as_str).unwrap_or("key");
    let window_id = data
        .get("window_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    println!("Sent app key {action} to window {window_id}");
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
}
