use clap::{Args, Subcommand};
use lingxia_computer_use as cu;
use serde::Serialize;

#[derive(Args, Clone)]
pub struct DesktopOptions {
    /// Authorize mutating desktop commands (or set LXDEV_DESKTOP_ALLOW_CONTROL=1)
    #[arg(long, global = true)]
    allow_control: bool,
    /// Authorize destructive commands like `window close` (or set
    /// LXDEV_DESKTOP_ALLOW_DESTRUCTIVE=1)
    #[arg(long, global = true)]
    allow_destructive: bool,
    #[command(subcommand)]
    command: DesktopCommand,
}

/// Shared window selector: exactly one of `--window` / `--match`.
#[derive(Args, Clone)]
pub struct WindowSel {
    /// Window id from `desktop windows`
    #[arg(long)]
    window: Option<String>,
    /// Match query (text | title: | class: | process: | pid:)
    #[arg(long = "match")]
    match_query: Option<String>,
    /// Print JSON output
    #[arg(long)]
    json: bool,
}

impl WindowSel {
    fn target(&self) -> cu::Result<cu::WindowTarget> {
        match (&self.window, &self.match_query) {
            (Some(id), None) => Ok(cu::WindowTarget::Id(id.clone())),
            (None, Some(q)) => Ok(cu::WindowTarget::Match(cu::WindowQuery::parse(q))),
            (None, None) => Err(cu::Error::Usage("pass --window <id> or --match <query>".into())),
            (Some(_), Some(_)) => {
                Err(cu::Error::Usage("pass only one of --window / --match".into()))
            }
        }
    }
}

#[derive(Subcommand, Clone)]
pub enum DesktopCommand {
    /// Report backend, capabilities, and permission status
    Doctor {
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// List monitors/displays (global physical pixels)
    Displays {
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// List local OS windows
    Windows {
        /// Match query: bare text, or a title:/class:/process:/pid: prefix
        #[arg(long = "match")]
        match_query: Option<String>,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Capture a display, window, or region (defaults to the whole screen)
    Screenshot {
        /// Capture a monitor by 1-based index (from `desktop displays`)
        #[arg(long)]
        display: Option<usize>,
        /// Capture a window by id (occlusion-independent)
        #[arg(long)]
        window: Option<String>,
        /// Capture a region as X,Y,W,H in global physical pixels
        #[arg(long)]
        region: Option<String>,
        /// Output path; `-` for stdout. Default: .lingxia/screenshots/desktop-<ts>.png
        #[arg(long, short = 'o')]
        output: Option<String>,
        /// Print the JSON envelope (metadata + base64 PNG)
        #[arg(long)]
        json: bool,
    },
    /// Read the color of a pixel at a screen coordinate
    Pixel {
        /// Coordinate as X,Y in global physical pixels
        #[arg(long)]
        at: String,
        /// Print JSON output
        #[arg(long)]
        json: bool,
    },
    /// Manage a window (focus, move, resize, min/max, close, ...)
    Window {
        #[command(subcommand)]
        action: WindowAction,
    },
    /// Synthesize physical mouse input at screen coordinates
    Pointer {
        #[command(subcommand)]
        action: PointerAction,
    },
    /// Synthesize physical keyboard input
    Key {
        #[command(subcommand)]
        action: KeyAction,
    },
    /// Read/write the system clipboard
    Clipboard {
        #[command(subcommand)]
        action: ClipboardAction,
    },
}

#[derive(Subcommand, Clone)]
pub enum ClipboardAction {
    /// Read the clipboard (read-only)
    Get {
        #[arg(long)]
        json: bool,
    },
    /// Set the clipboard text
    Set {
        #[arg(long)]
        text: String,
        #[arg(long)]
        json: bool,
    },
    /// Empty the clipboard
    Clear {
        #[arg(long)]
        json: bool,
    },
    /// Paste into the focused control (Ctrl+V)
    Paste {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
pub enum CliButton {
    Left,
    Right,
    Middle,
}

impl From<CliButton> for cu::MouseButton {
    fn from(b: CliButton) -> Self {
        match b {
            CliButton::Left => cu::MouseButton::Left,
            CliButton::Right => cu::MouseButton::Right,
            CliButton::Middle => cu::MouseButton::Middle,
        }
    }
}

#[derive(Clone, Copy, clap::ValueEnum)]
pub enum CliModifier {
    Ctrl,
    Shift,
    Alt,
    Meta,
}

impl From<CliModifier> for cu::Modifier {
    fn from(m: CliModifier) -> Self {
        match m {
            CliModifier::Ctrl => cu::Modifier::Ctrl,
            CliModifier::Shift => cu::Modifier::Shift,
            CliModifier::Alt => cu::Modifier::Alt,
            CliModifier::Meta => cu::Modifier::Meta,
        }
    }
}

#[derive(Subcommand, Clone)]
pub enum PointerAction {
    /// Move the cursor to X,Y
    Move {
        #[arg(long)]
        at: String,
        #[arg(long)]
        json: bool,
    },
    /// Press a button at X,Y
    Down {
        #[arg(long)]
        at: String,
        #[arg(long, value_enum, default_value = "left")]
        button: CliButton,
        #[arg(long)]
        json: bool,
    },
    /// Release a button at X,Y
    Up {
        #[arg(long)]
        at: String,
        #[arg(long, value_enum, default_value = "left")]
        button: CliButton,
        #[arg(long)]
        json: bool,
    },
    /// Click at X,Y
    Click {
        #[arg(long)]
        at: String,
        #[arg(long, value_enum, default_value = "left")]
        button: CliButton,
        #[arg(long, default_value_t = 1)]
        count: u32,
        #[arg(long)]
        json: bool,
    },
    /// Drag from one point to another
    Drag {
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        #[arg(long, value_enum, default_value = "left")]
        button: CliButton,
        #[arg(long)]
        json: bool,
    },
    /// Scroll at X,Y by dx/dy notches
    Scroll {
        #[arg(long)]
        at: String,
        #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
        dx: i32,
        #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
        dy: i32,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Clone)]
pub enum KeyAction {
    /// Type literal text (may bypass IME; prefer clipboard paste for CJK)
    Type {
        #[arg(long)]
        text: String,
        #[arg(long)]
        json: bool,
    },
    /// Press a key with optional modifiers
    Press {
        #[arg(long)]
        key: String,
        #[arg(long = "modifier", value_enum)]
        modifier: Vec<CliModifier>,
        #[arg(long)]
        json: bool,
    },
    /// Hold a key down
    Down {
        #[arg(long)]
        key: String,
        #[arg(long)]
        json: bool,
    },
    /// Release a key
    Up {
        #[arg(long)]
        key: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Clone)]
pub enum WindowAction {
    /// Report a window's current state (read-only)
    Status(WindowSel),
    /// Bring a window to the foreground
    Focus(WindowSel),
    /// Activate the single window matching a query
    Activate {
        #[arg(long = "match")]
        match_query: String,
        #[arg(long)]
        json: bool,
    },
    /// Raise a window to the top of the z-order
    Raise(WindowSel),
    /// Move a window to X,Y or to a display
    Move {
        #[command(flatten)]
        sel: WindowSel,
        /// New top-left as X,Y in global physical pixels
        #[arg(long)]
        to: Option<String>,
        /// Move to a display id (from `desktop displays`)
        #[arg(long)]
        display: Option<String>,
    },
    /// Resize a window to W,H
    Resize {
        #[command(flatten)]
        sel: WindowSel,
        /// New size as W,H in physical pixels
        #[arg(long)]
        to: String,
    },
    /// Minimize a window
    Minimize(WindowSel),
    /// Maximize a window
    Maximize(WindowSel),
    /// Restore a minimized/maximized window
    Restore(WindowSel),
    /// Set or clear always-on-top
    AlwaysOnTop {
        #[command(flatten)]
        sel: WindowSel,
        #[arg(long, action = clap::ArgAction::Set)]
        enabled: bool,
    },
    /// Ask a window to close (destructive)
    Close(WindowSel),
}

pub fn execute(options: DesktopOptions) -> ! {
    let allow_control = options.allow_control;
    let allow_destructive = options.allow_destructive;
    match options.command {
        DesktopCommand::Doctor { json } => finish(json, Ok(cu::doctor()), print_doctor),
        DesktopCommand::Displays { json } => finish(json, cu::displays(), print_displays),
        DesktopCommand::Windows { match_query, json } => {
            let query = match_query
                .as_deref()
                .map(cu::WindowQuery::parse)
                .unwrap_or_default();
            finish(json, cu::windows(&query), print_windows)
        }
        DesktopCommand::Screenshot {
            display,
            window,
            region,
            output,
            json,
        } => run_screenshot(display, window, region, output, json),
        DesktopCommand::Pixel { at, json } => {
            let (x, y) = match parse_pair(&at) {
                Ok(p) => p,
                Err(e) => finish::<()>(json, Err(e), |_| {}),
            };
            finish(json, cu::pixel(x, y), print_pixel)
        }
        DesktopCommand::Window { action } => run_window(action, allow_control, allow_destructive),
        DesktopCommand::Pointer { action } => {
            run_pointer(action, allow_control, allow_destructive)
        }
        DesktopCommand::Key { action } => run_key(action, allow_control, allow_destructive),
        DesktopCommand::Clipboard { action } => {
            run_clipboard(action, allow_control, allow_destructive)
        }
    }
}

fn run_clipboard(action: ClipboardAction, allow_control: bool, allow_destructive: bool) -> ! {
    use cu::clipboard as c;
    match action {
        ClipboardAction::Get { json } => finish(json, c::get(), print_clipboard),
        ClipboardAction::Set { text, json } => {
            let r = gate(allow_control, false, allow_destructive).and_then(|_| c::set(&text));
            finish(json, r, print_ack)
        }
        ClipboardAction::Clear { json } => {
            let r = gate(allow_control, false, allow_destructive).and_then(|_| c::clear());
            finish(json, r, print_ack)
        }
        ClipboardAction::Paste { json } => {
            let r = gate(allow_control, false, allow_destructive).and_then(|_| c::paste());
            finish(json, r, print_ack)
        }
    }
}

fn print_clipboard(c: &cu::Clipboard) {
    match &c.text {
        Some(t) => println!("{t}"),
        None => println!("(clipboard has no text)"),
    }
}

fn print_ack(a: &cu::Ack) {
    println!("ok: {}", a.action);
}

fn run_pointer(action: PointerAction, allow_control: bool, allow_destructive: bool) -> ! {
    use cu::input as i;
    let g = gate(allow_control, false, allow_destructive);
    let (json, result) = match action {
        PointerAction::Move { at, json } => (
            json,
            g.and_then(|_| parse_pair(&at))
                .and_then(|(x, y)| i::pointer_move(x, y)),
        ),
        PointerAction::Down { at, button, json } => (
            json,
            g.and_then(|_| parse_pair(&at))
                .and_then(|(x, y)| i::pointer_down(x, y, button.into())),
        ),
        PointerAction::Up { at, button, json } => (
            json,
            g.and_then(|_| parse_pair(&at))
                .and_then(|(x, y)| i::pointer_up(x, y, button.into())),
        ),
        PointerAction::Click {
            at,
            button,
            count,
            json,
        } => (
            json,
            g.and_then(|_| parse_pair(&at))
                .and_then(|(x, y)| i::pointer_click(x, y, button.into(), count)),
        ),
        PointerAction::Drag {
            from,
            to,
            button,
            json,
        } => (
            json,
            g.and_then(|_| Ok((parse_pair(&from)?, parse_pair(&to)?)))
                .and_then(|((fx, fy), (tx, ty))| i::pointer_drag(fx, fy, tx, ty, button.into())),
        ),
        PointerAction::Scroll { at, dx, dy, json } => (
            json,
            g.and_then(|_| parse_pair(&at))
                .and_then(|(x, y)| i::pointer_scroll(x, y, dx, dy)),
        ),
    };
    finish(json, result, print_ack)
}

fn run_key(action: KeyAction, allow_control: bool, allow_destructive: bool) -> ! {
    use cu::input as i;
    let g = gate(allow_control, false, allow_destructive);
    let (json, result) = match action {
        KeyAction::Type { text, json } => (json, g.and_then(|_| i::key_type(&text))),
        KeyAction::Press {
            key,
            modifier,
            json,
        } => {
            let mods: Vec<cu::Modifier> = modifier.into_iter().map(Into::into).collect();
            (json, g.and_then(|_| i::key_press(&key, &mods)))
        }
        KeyAction::Down { key, json } => (json, g.and_then(|_| i::key_down(&key))),
        KeyAction::Up { key, json } => (json, g.and_then(|_| i::key_up(&key))),
    };
    finish(json, result, print_ack)
}

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Authorize a mutating (and optionally destructive) desktop command.
fn gate(allow_control: bool, destructive: bool, allow_destructive: bool) -> cu::Result<()> {
    if !(allow_control || env_flag("LXDEV_DESKTOP_ALLOW_CONTROL")) {
        return Err(cu::Error::Permission(
            "mutating desktop command needs --allow-control (or LXDEV_DESKTOP_ALLOW_CONTROL=1)"
                .into(),
        ));
    }
    if destructive && !(allow_destructive || env_flag("LXDEV_DESKTOP_ALLOW_DESTRUCTIVE")) {
        return Err(cu::Error::Permission(
            "destructive desktop command needs --allow-destructive (or LXDEV_DESKTOP_ALLOW_DESTRUCTIVE=1)"
                .into(),
        ));
    }
    Ok(())
}

fn run_window(action: WindowAction, allow_control: bool, allow_destructive: bool) -> ! {
    use cu::window as w;

    // A gated single-target op that returns the updated window record.
    fn gated(
        sel: WindowSel,
        allow_control: bool,
        destructive: bool,
        allow_destructive: bool,
        op: impl Fn(&cu::WindowTarget) -> cu::Result<cu::Window>,
    ) -> ! {
        let json = sel.json;
        let result = gate(allow_control, destructive, allow_destructive)
            .and_then(|_| sel.target())
            .and_then(|t| op(&t));
        finish(json, result, print_window_one)
    }

    match action {
        WindowAction::Status(sel) => {
            let json = sel.json;
            finish(json, sel.target().and_then(|t| w::status(&t)), print_window_one)
        }
        WindowAction::Focus(sel) => gated(sel, allow_control, false, allow_destructive, w::focus),
        WindowAction::Raise(sel) => gated(sel, allow_control, false, allow_destructive, w::raise),
        WindowAction::Minimize(sel) => {
            gated(sel, allow_control, false, allow_destructive, w::minimize)
        }
        WindowAction::Maximize(sel) => {
            gated(sel, allow_control, false, allow_destructive, w::maximize)
        }
        WindowAction::Restore(sel) => {
            gated(sel, allow_control, false, allow_destructive, w::restore)
        }
        WindowAction::AlwaysOnTop { sel, enabled } => gated(
            sel,
            allow_control,
            false,
            allow_destructive,
            move |t| w::set_always_on_top(t, enabled),
        ),
        WindowAction::Close(sel) => gated(sel, allow_control, true, allow_destructive, w::close),
        WindowAction::Activate { match_query, json } => {
            let result = gate(allow_control, false, allow_destructive)
                .map(|_| cu::WindowQuery::parse(&match_query))
                .and_then(w::activate);
            finish(json, result, print_window_one)
        }
        WindowAction::Move { sel, to, display } => {
            let json = sel.json;
            let result = gate(allow_control, false, allow_destructive)
                .and_then(|_| sel.target())
                .and_then(|t| match (&display, &to) {
                    (Some(d), _) => w::move_to_display(&t, d),
                    (None, Some(xy)) => {
                        let (x, y) = parse_pair(xy)?;
                        w::move_to(&t, x, y)
                    }
                    (None, None) => {
                        Err(cu::Error::Usage("pass --to X,Y or --display <id>".into()))
                    }
                });
            finish(json, result, print_window_one)
        }
        WindowAction::Resize { sel, to } => {
            let json = sel.json;
            let result = gate(allow_control, false, allow_destructive)
                .and_then(|_| sel.target())
                .and_then(|t| {
                    let (wd, ht) = parse_pair(&to)?;
                    w::resize(&t, wd, ht)
                });
            finish(json, result, print_window_one)
        }
    }
}

fn print_window_one(w: &cu::Window) {
    println!(
        "{}  pid {}  {}  {},{} {}x{}  {}{}",
        w.id,
        w.pid,
        w.process,
        w.bounds.x,
        w.bounds.y,
        w.bounds.w,
        w.bounds.h,
        if w.minimized {
            "[min] "
        } else if w.maximized {
            "[max] "
        } else {
            ""
        },
        w.title,
    );
}

/// `X,Y` -> (i32, i32).
fn parse_pair(s: &str) -> cu::Result<(i32, i32)> {
    let (a, b) = s
        .split_once(',')
        .ok_or_else(|| cu::Error::Usage(format!("expected X,Y, got '{s}'")))?;
    Ok((
        a.trim()
            .parse()
            .map_err(|_| cu::Error::Usage(format!("invalid X in '{s}'")))?,
        b.trim()
            .parse()
            .map_err(|_| cu::Error::Usage(format!("invalid Y in '{s}'")))?,
    ))
}

fn run_screenshot(
    display: Option<usize>,
    window: Option<String>,
    region: Option<String>,
    output: Option<String>,
    json: bool,
) -> ! {
    let selectors = display.is_some() as u8 + window.is_some() as u8 + region.is_some() as u8;
    if selectors > 1 {
        finish::<()>(
            json,
            Err(cu::Error::Usage(
                "pass at most one of --display / --window / --region".into(),
            )),
            |_| {},
        );
    }
    let target = if let Some(n) = display {
        cu::CaptureTarget::Display(n)
    } else if let Some(id) = window {
        cu::CaptureTarget::Window(id)
    } else if let Some(r) = region {
        match parse_region(&r) {
            Ok(t) => t,
            Err(e) => finish::<()>(json, Err(e), |_| {}),
        }
    } else {
        cu::CaptureTarget::Screen
    };

    let capture = match cu::screenshot(target) {
        Ok(c) => c,
        Err(e) => finish::<()>(json, Err(e), |_| {}),
    };

    if json {
        use base64::Engine as _;
        let envelope = serde_json::json!({
            "target": "desktop",
            "kind": "screenshot",
            "coordinate_space": "desktop_pixels",
            "backend": capture.backend,
            "occlusion_independent": capture.occlusion_independent,
            "format": "png",
            "width": capture.width,
            "height": capture.height,
            "image": {
                "mime": "image/png",
                "encoding": "base64",
                "data": base64::engine::general_purpose::STANDARD.encode(&capture.png),
            }
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&envelope).unwrap_or_default()
        );
        std::process::exit(0);
    }

    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    match crate::screenshot::write_png(output, format!("desktop-{ts}.png"), &capture.png) {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(10);
        }
    }
}

fn parse_region(s: &str) -> cu::Result<cu::CaptureTarget> {
    let parts: Vec<&str> = s.split(',').map(str::trim).collect();
    if parts.len() != 4 {
        return Err(cu::Error::Usage(format!("expected X,Y,W,H, got '{s}'")));
    }
    let n = |v: &str| {
        v.parse::<i32>()
            .map_err(|_| cu::Error::Usage(format!("invalid number in region '{s}'")))
    };
    Ok(cu::CaptureTarget::Region {
        x: n(parts[0])?,
        y: n(parts[1])?,
        w: n(parts[2])?,
        h: n(parts[3])?,
    })
}

fn print_pixel(p: &cu::Pixel) {
    println!("#{}  rgb({},{},{})  at {},{}", p.hex, p.r, p.g, p.b, p.x, p.y);
}

/// Emit the result and exit with the contract's exit code. `desktop` commands
/// run locally (no dev session), so they own their process exit directly.
fn finish<T: Serialize>(json: bool, result: cu::Result<T>, human: impl Fn(&T)) -> ! {
    match result {
        Ok(value) => {
            if json {
                match serde_json::to_string_pretty(&value) {
                    Ok(text) => println!("{text}"),
                    Err(err) => {
                        eprintln!("Error: failed to serialize output: {err}");
                        std::process::exit(10);
                    }
                }
            } else {
                human(&value);
            }
            std::process::exit(0);
        }
        Err(err) => {
            if json {
                let envelope = serde_json::json!({
                    "error": {
                        "code": err.code(),
                        "message": err.to_string(),
                        "exit_code": err.exit_code(),
                    }
                });
                eprintln!(
                    "{}",
                    serde_json::to_string_pretty(&envelope).unwrap_or_default()
                );
            } else {
                eprintln!("Error: {err}");
            }
            std::process::exit(err.exit_code());
        }
    }
}

fn yn(b: bool) -> &'static str {
    if b { "yes" } else { "no" }
}

fn print_doctor(d: &cu::Doctor) {
    println!("backend    {}", d.backend);
    println!("os         {} {}", d.os, d.os_version);
    let c = &d.capabilities;
    println!("capabilities:");
    println!("  displays            {}", yn(c.displays));
    println!("  windows             {}", yn(c.windows));
    println!("  screenshot          {}", yn(c.screenshot));
    println!("  pixel               {}", yn(c.pixel));
    println!("  pointer             {}", yn(c.pointer));
    println!("  key                 {}", yn(c.key));
    println!("  window management   {}", yn(c.window_management));
    println!("  clipboard           {}", yn(c.clipboard));
    println!("  ax tree             {}", yn(c.ax_tree));
    println!("  ocr                 {}", yn(c.ocr));
}

fn print_displays(displays: &Vec<cu::Display>) {
    if displays.is_empty() {
        println!("No displays reported.");
        return;
    }
    println!(
        "{:<10}  {:<7}  {:<20}  {:<6}  DPI",
        "ID", "PRIMARY", "BOUNDS", "SCALE"
    );
    for d in displays {
        println!(
            "{:<10}  {:<7}  {:<20}  {:<6}  {}",
            d.id,
            yn(d.primary),
            format!("{},{} {}x{}", d.bounds.x, d.bounds.y, d.bounds.w, d.bounds.h),
            format!("{:.2}", d.scale),
            d.dpi,
        );
    }
}

fn print_windows(windows: &Vec<cu::Window>) {
    if windows.is_empty() {
        println!("No matching windows.");
        return;
    }
    println!(
        "{:<12}  {:<6}  {:<18}  {:<19}  {:<3}  TITLE",
        "ID", "PID", "PROCESS", "BOUNDS", "FOC"
    );
    for w in windows {
        println!(
            "{:<12}  {:<6}  {:<18}  {:<19}  {:<3}  {}",
            w.id,
            w.pid,
            truncate(&w.process, 18),
            format!("{},{} {}x{}", w.bounds.x, w.bounds.y, w.bounds.w, w.bounds.h),
            yn(w.focused),
            truncate(&w.title, 60),
        );
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max.saturating_sub(1)).collect::<String>())
    }
}
