//! `desktop` tier — session-less local-OS desktop automation, the in-process
//! JS binding of `lxdev desktop` over the shared `lingxia-computer-use`
//! backend (Windows / macOS; other platforms report `unsupported`).
//!
//! The backend's serde DTOs are the command contract's JSON, so results match
//! the CLI `--json` envelopes exactly, and its error taxonomy surfaces here as
//! stable `E_DESKTOP_<CODE>` JS error codes. Backend calls are blocking OS
//! calls; every method hops through `spawn_blocking` to keep the single JS
//! logic thread responsive.

use crate::resolve::json_to_js;
use lingxia_computer_use as cu;
use rong::{
    Class, FromJSObject, HostError, JSContext, JSObject, JSResult, JSValue, function::Optional,
    js_class, js_method,
};
use serde_json::json;
use std::time::{Duration, Instant};

fn illegal_ctor() -> rong::RongJSError {
    HostError::new(
        rong::error::E_ILLEGAL_CONSTRUCTOR,
        "Use lx.automation().desktop",
    )
    .into()
}

/// Map the backend error taxonomy to stable JS error codes
/// (`E_DESKTOP_NOT_FOUND`, `E_DESKTOP_TIMEOUT`, ...).
fn desktop_err(err: cu::Error) -> rong::RongJSError {
    let code = match err.code() {
        cu::ErrorCode::Usage => "E_DESKTOP_USAGE",
        cu::ErrorCode::NotFound => "E_DESKTOP_NOT_FOUND",
        cu::ErrorCode::Ambiguous => "E_DESKTOP_AMBIGUOUS",
        cu::ErrorCode::Timeout => "E_DESKTOP_TIMEOUT",
        cu::ErrorCode::Permission => "E_DESKTOP_PERMISSION",
        cu::ErrorCode::Unsupported => "E_DESKTOP_UNSUPPORTED",
        cu::ErrorCode::Unavailable => "E_DESKTOP_UNAVAILABLE",
        cu::ErrorCode::Stale => "E_DESKTOP_STALE",
        cu::ErrorCode::Failed => "E_DESKTOP_FAILED",
    };
    HostError::new(code, err.to_string()).into()
}

fn usage(msg: impl Into<String>) -> rong::RongJSError {
    desktop_err(cu::Error::Usage(msg.into()))
}

/// Run a blocking backend call off the JS logic thread.
async fn blocking<T, F>(f: F) -> JSResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> cu::Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|err| desktop_err(cu::Error::Failed(err.to_string())))?
        .map_err(desktop_err)
}

fn to_js<T: serde::Serialize>(ctx: &JSContext, value: &T) -> JSResult<JSValue> {
    let json = serde_json::to_value(value)
        .map_err(|err| desktop_err(cu::Error::Failed(err.to_string())))?;
    json_to_js(ctx, &json)
}

/// Parse the `at: [x, y]` coordinate form (backend-native desktop pixels).
fn point(at: &[f64], flag: &str) -> JSResult<(i32, i32)> {
    match at {
        [x, y] => Ok((x.round() as i32, y.round() as i32)),
        _ => Err(usage(format!("{flag}: expected [x, y]"))),
    }
}

/// Exactly one of `window` (id) / `match` (query) selects a window target.
fn window_target(
    window: &Option<String>,
    match_query: &Option<String>,
) -> JSResult<cu::WindowTarget> {
    match (window, match_query) {
        (Some(id), None) => Ok(cu::WindowTarget::Id(id.clone())),
        (None, Some(q)) => Ok(cu::WindowTarget::Match(cu::WindowQuery::parse(q))),
        (None, None) => Err(usage("pass window (id) or match (query)")),
        (Some(_), Some(_)) => Err(usage("pass only one of window / match")),
    }
}

fn mouse_button(raw: &Option<String>) -> JSResult<cu::MouseButton> {
    match raw.as_deref().map(str::trim) {
        None | Some("") | Some("left") => Ok(cu::MouseButton::Left),
        Some("right") => Ok(cu::MouseButton::Right),
        Some("middle") => Ok(cu::MouseButton::Middle),
        Some(other) => Err(usage(format!(
            "unknown button '{other}' (expected left | right | middle)"
        ))),
    }
}

fn modifiers(raw: &Option<Vec<String>>) -> JSResult<Vec<cu::Modifier>> {
    raw.iter()
        .flatten()
        .map(|value| match value.trim() {
            "ctrl" => Ok(cu::Modifier::Ctrl),
            "shift" => Ok(cu::Modifier::Shift),
            "alt" => Ok(cu::Modifier::Alt),
            "meta" => Ok(cu::Modifier::Meta),
            other => Err(usage(format!(
                "unknown modifier '{other}' (expected ctrl | shift | alt | meta)"
            ))),
        })
        .collect()
}

/// Resolve the optional background-input target: an explicit `pid`, or a
/// `window` id mapped to its owning process. `None` = foreground input.
fn input_target(pid: Option<u32>, window: Option<String>) -> cu::Result<Option<u32>> {
    if let Some(pid) = pid {
        return Ok(Some(pid));
    }
    match window {
        Some(id) => Ok(Some(cu::window::status(&cu::WindowTarget::Id(id))?.pid)),
        None => Ok(None),
    }
}

const WAIT_DEFAULT_MS: u64 = 5_000;

fn capture_to_js(ctx: &JSContext, capture: &cu::Capture) -> JSResult<JSValue> {
    use base64::{Engine as _, engine::general_purpose};
    json_to_js(
        ctx,
        &json!({
            "format": "png",
            "base64": general_purpose::STANDARD.encode(&capture.png),
            "width": capture.width,
            "height": capture.height,
            "occlusionIndependent": capture.occlusion_independent,
            "backend": capture.backend,
        }),
    )
}

// ===================== desktop =====================

#[js_class(clone)]
pub(crate) struct JSDesktopDriver {}

impl JSDesktopDriver {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

#[derive(FromJSObject, Default)]
struct PermissionsOpt {
    request: Option<bool>,
}

#[derive(FromJSObject, Default)]
struct WindowsOpt {
    #[js_name = "match"]
    match_query: Option<String>,
}

#[derive(FromJSObject, Default)]
struct ScreenshotOpt {
    /// Monitor by 1-based index (as listed by `displays()`).
    display: Option<usize>,
    /// Window by id (occlusion-independent capture).
    window: Option<String>,
    /// Region as `[x, y, w, h]` in backend-native desktop coordinates.
    region: Option<Vec<f64>>,
}

#[derive(FromJSObject)]
struct AtOpt {
    at: Vec<f64>,
}

#[derive(FromJSObject)]
struct SnapshotOpt {
    /// Window id from `windows()`.
    window: String,
    /// Skip the accessibility tree.
    #[js_name = "noAx"]
    no_ax: Option<bool>,
    /// Limit ax tree depth.
    depth: Option<u32>,
}

#[js_class(rename = "DesktopDriver")]
impl JSDesktopDriver {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(illegal_ctor())
    }

    /// Backend, capability, and permission report (`lxdev desktop doctor`).
    #[js_method]
    async fn doctor(&self, ctx: JSContext) -> JSResult<JSValue> {
        let doctor = blocking(|| Ok(cu::doctor())).await?;
        to_js(&ctx, &doctor)
    }

    /// The host process's OS-permission grants; `{ request: true }` triggers
    /// the OS prompts for anything not yet granted.
    #[js_method]
    async fn permissions(
        &self,
        ctx: JSContext,
        options: Optional<PermissionsOpt>,
    ) -> JSResult<JSValue> {
        let request = options.0.unwrap_or_default().request.unwrap_or(false);
        let permissions = blocking(move || {
            Ok(if request {
                cu::request_permissions()
            } else {
                cu::permissions()
            })
        })
        .await?;
        to_js(&ctx, &permissions)
    }

    /// Enumerate monitors (backend-native desktop coordinates).
    #[js_method]
    async fn displays(&self, ctx: JSContext) -> JSResult<JSValue> {
        let displays = blocking(cu::displays).await?;
        to_js(&ctx, &displays)
    }

    /// Enumerate top-level OS windows, optionally filtered by a match query
    /// (`text | title: | class: | process: | pid:`).
    #[js_method]
    async fn windows(&self, ctx: JSContext, options: Optional<WindowsOpt>) -> JSResult<JSValue> {
        let options = options.0.unwrap_or_default();
        let query = options
            .match_query
            .as_deref()
            .map(cu::WindowQuery::parse)
            .unwrap_or_default();
        let windows = blocking(move || cu::windows(&query)).await?;
        to_js(&ctx, &windows)
    }

    /// Capture the screen (default), a display, a window, or a region.
    #[js_method]
    async fn screenshot(
        &self,
        ctx: JSContext,
        options: Optional<ScreenshotOpt>,
    ) -> JSResult<JSValue> {
        let options = options.0.unwrap_or_default();
        let target = match (&options.display, &options.window, &options.region) {
            (None, None, None) => cu::CaptureTarget::Screen,
            (Some(index), None, None) => cu::CaptureTarget::Display(*index),
            (None, Some(id), None) => cu::CaptureTarget::Window(id.clone()),
            (None, None, Some(region)) => match region.as_slice() {
                [x, y, w, h] => cu::CaptureTarget::Region {
                    x: x.round() as i32,
                    y: y.round() as i32,
                    w: w.round() as i32,
                    h: h.round() as i32,
                },
                _ => return Err(usage("region: expected [x, y, w, h]")),
            },
            _ => return Err(usage("pass only one of display / window / region")),
        };
        let capture = blocking(move || cu::screenshot(target)).await?;
        capture_to_js(&ctx, &capture)
    }

    /// Read one pixel's color at `at: [x, y]`.
    #[js_method]
    async fn pixel(&self, ctx: JSContext, options: AtOpt) -> JSResult<JSValue> {
        let (x, y) = point(&options.at, "at")?;
        let pixel = blocking(move || cu::pixel(x, y)).await?;
        to_js(&ctx, &pixel)
    }

    /// One-shot window snapshot: window info + PNG screenshot + ax tree, in a
    /// single object (`lxdev desktop snapshot`). `noAx` skips the tree; `depth`
    /// caps it. Screenshot/ax degrade to `null` if unavailable rather than
    /// failing the whole call.
    #[js_method]
    async fn snapshot(&self, ctx: JSContext, options: SnapshotOpt) -> JSResult<JSValue> {
        use base64::{Engine as _, engine::general_purpose};
        let window = options.window;
        let no_ax = options.no_ax.unwrap_or(false);
        let depth = options.depth;
        let (info, shot, ax) = blocking(move || {
            let info = cu::window::status(&cu::WindowTarget::Id(window.clone()))?;
            let shot = cu::screenshot(cu::CaptureTarget::Window(window.clone())).ok();
            let ax = if no_ax {
                None
            } else {
                cu::ax::tree(&window, depth, None).ok()
            };
            Ok((info, shot, ax))
        })
        .await?;
        let screenshot = shot.map(|c| {
            json!({
                "format": "png",
                "width": c.width,
                "height": c.height,
                "occlusionIndependent": c.occlusion_independent,
                "base64": general_purpose::STANDARD.encode(&c.png),
            })
        });
        to_js(
            &ctx,
            &json!({ "window": info, "screenshot": screenshot, "ax": ax }),
        )
    }

    #[js_method(getter, enumerable)]
    fn window(&self, ctx: JSContext) -> JSResult<JSObject> {
        Ok(Class::lookup::<JSDesktopWindow>(&ctx)?.instance(JSDesktopWindow::new()))
    }

    #[js_method(getter, enumerable)]
    fn pointer(&self, ctx: JSContext) -> JSResult<JSObject> {
        Ok(Class::lookup::<JSDesktopPointer>(&ctx)?.instance(JSDesktopPointer::new()))
    }

    #[js_method(getter, enumerable)]
    fn key(&self, ctx: JSContext) -> JSResult<JSObject> {
        Ok(Class::lookup::<JSDesktopKey>(&ctx)?.instance(JSDesktopKey::new()))
    }

    #[js_method(getter, enumerable)]
    fn clipboard(&self, ctx: JSContext) -> JSResult<JSObject> {
        Ok(Class::lookup::<JSDesktopClipboard>(&ctx)?.instance(JSDesktopClipboard::new()))
    }

    #[js_method(getter, enumerable)]
    fn ax(&self, ctx: JSContext) -> JSResult<JSObject> {
        Ok(Class::lookup::<JSDesktopAx>(&ctx)?.instance(JSDesktopAx::new()))
    }

    #[js_method(getter, enumerable)]
    fn wait(&self, ctx: JSContext) -> JSResult<JSObject> {
        Ok(Class::lookup::<JSDesktopWait>(&ctx)?.instance(JSDesktopWait::new()))
    }

    #[js_method(getter, enumerable)]
    fn app(&self, ctx: JSContext) -> JSResult<JSObject> {
        Ok(Class::lookup::<JSDesktopApp>(&ctx)?.instance(JSDesktopApp::new()))
    }

    #[js_method(getter, enumerable)]
    fn process(&self, ctx: JSContext) -> JSResult<JSObject> {
        Ok(Class::lookup::<JSDesktopProcess>(&ctx)?.instance(JSDesktopProcess::new()))
    }
}

// ===================== desktop.window.* =====================

#[js_class(clone)]
pub(crate) struct JSDesktopWindow {}

impl JSDesktopWindow {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

#[derive(FromJSObject, Default)]
struct WindowSel {
    /// Window id from `windows()`.
    window: Option<String>,
    /// Match query (`text | title: | class: | process: | pid:`); must resolve
    /// to exactly one window.
    #[js_name = "match"]
    match_query: Option<String>,
}

#[derive(FromJSObject)]
struct WindowMove {
    window: Option<String>,
    #[js_name = "match"]
    match_query: Option<String>,
    to: Vec<f64>,
}

#[derive(FromJSObject)]
struct WindowResize {
    window: Option<String>,
    #[js_name = "match"]
    match_query: Option<String>,
    width: f64,
    height: f64,
}

#[derive(FromJSObject)]
struct WindowMoveDisplay {
    window: Option<String>,
    #[js_name = "match"]
    match_query: Option<String>,
    display: String,
}

#[derive(FromJSObject)]
struct WindowAlwaysOnTop {
    window: Option<String>,
    #[js_name = "match"]
    match_query: Option<String>,
    on: bool,
}

#[js_class(rename = "DesktopWindow")]
impl JSDesktopWindow {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(illegal_ctor())
    }

    /// Report a window's state (read-only).
    #[js_method]
    async fn status(&self, ctx: JSContext, options: WindowSel) -> JSResult<JSValue> {
        let target = window_target(&options.window, &options.match_query)?;
        let window = blocking(move || cu::window::status(&target)).await?;
        to_js(&ctx, &window)
    }
    /// Focus (activate + raise) a window.
    #[js_method]
    async fn focus(&self, ctx: JSContext, options: WindowSel) -> JSResult<JSValue> {
        let target = window_target(&options.window, &options.match_query)?;
        let window = blocking(move || cu::window::focus(&target)).await?;
        to_js(&ctx, &window)
    }
    /// Activate a window's app.
    #[js_method]
    async fn activate(&self, ctx: JSContext, options: WindowSel) -> JSResult<JSValue> {
        let target = window_target(&options.window, &options.match_query)?;
        let window = blocking(move || cu::window::activate(&target)).await?;
        to_js(&ctx, &window)
    }
    /// Raise a window without focusing it.
    #[js_method(rename = "raise")]
    async fn raise_window(&self, ctx: JSContext, options: WindowSel) -> JSResult<JSValue> {
        let target = window_target(&options.window, &options.match_query)?;
        let window = blocking(move || cu::window::raise(&target)).await?;
        to_js(&ctx, &window)
    }
    /// Minimize a window.
    #[js_method]
    async fn minimize(&self, ctx: JSContext, options: WindowSel) -> JSResult<JSValue> {
        let target = window_target(&options.window, &options.match_query)?;
        let window = blocking(move || cu::window::minimize(&target)).await?;
        to_js(&ctx, &window)
    }
    /// Maximize a window.
    #[js_method]
    async fn maximize(&self, ctx: JSContext, options: WindowSel) -> JSResult<JSValue> {
        let target = window_target(&options.window, &options.match_query)?;
        let window = blocking(move || cu::window::maximize(&target)).await?;
        to_js(&ctx, &window)
    }
    /// Restore a minimized/maximized window.
    #[js_method]
    async fn restore(&self, ctx: JSContext, options: WindowSel) -> JSResult<JSValue> {
        let target = window_target(&options.window, &options.match_query)?;
        let window = blocking(move || cu::window::restore(&target)).await?;
        to_js(&ctx, &window)
    }
    /// Close a window (destructive).
    #[js_method]
    async fn close(&self, ctx: JSContext, options: WindowSel) -> JSResult<JSValue> {
        let target = window_target(&options.window, &options.match_query)?;
        let window = blocking(move || cu::window::close(&target)).await?;
        to_js(&ctx, &window)
    }

    /// Move a window to `to: [x, y]` (backend-native desktop coordinates).
    #[js_method(rename = "moveTo")]
    async fn move_to(&self, ctx: JSContext, options: WindowMove) -> JSResult<JSValue> {
        let target = window_target(&options.window, &options.match_query)?;
        let (x, y) = point(&options.to, "to")?;
        let window = blocking(move || cu::window::move_to(&target, x, y)).await?;
        to_js(&ctx, &window)
    }

    /// Move a window onto a display by id (from `displays()`).
    #[js_method(rename = "moveToDisplay")]
    async fn move_to_display(
        &self,
        ctx: JSContext,
        options: WindowMoveDisplay,
    ) -> JSResult<JSValue> {
        let target = window_target(&options.window, &options.match_query)?;
        let window =
            blocking(move || cu::window::move_to_display(&target, &options.display)).await?;
        to_js(&ctx, &window)
    }

    /// Resize a window to `width` × `height`.
    #[js_method]
    async fn resize(&self, ctx: JSContext, options: WindowResize) -> JSResult<JSValue> {
        let target = window_target(&options.window, &options.match_query)?;
        let (w, h) = (options.width.round() as i32, options.height.round() as i32);
        let window = blocking(move || cu::window::resize(&target, w, h)).await?;
        to_js(&ctx, &window)
    }

    /// Pin (or unpin) a window above normal windows.
    #[js_method(rename = "setAlwaysOnTop")]
    async fn set_always_on_top(
        &self,
        ctx: JSContext,
        options: WindowAlwaysOnTop,
    ) -> JSResult<JSValue> {
        let target = window_target(&options.window, &options.match_query)?;
        let on = options.on;
        let window = blocking(move || cu::window::set_always_on_top(&target, on)).await?;
        to_js(&ctx, &window)
    }
}

// ===================== desktop.pointer.* =====================

#[js_class(clone)]
pub(crate) struct JSDesktopPointer {}

impl JSDesktopPointer {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

#[derive(FromJSObject)]
struct DesktopPointerAt {
    /// Target coordinate as `[x, y]` in backend-native desktop pixels.
    at: Vec<f64>,
    /// Background-input target: a window id resolved to its owning process.
    window: Option<String>,
    pid: Option<u32>,
}

#[derive(FromJSObject)]
struct DesktopPointerButton {
    at: Vec<f64>,
    button: Option<String>,
    window: Option<String>,
    pid: Option<u32>,
}

#[derive(FromJSObject)]
struct DesktopPointerClick {
    at: Vec<f64>,
    button: Option<String>,
    count: Option<u32>,
    window: Option<String>,
    pid: Option<u32>,
}

#[derive(FromJSObject)]
struct DesktopPointerDrag {
    from: Vec<f64>,
    to: Vec<f64>,
    button: Option<String>,
    window: Option<String>,
    pid: Option<u32>,
}

#[derive(FromJSObject)]
struct DesktopPointerScroll {
    at: Vec<f64>,
    dx: Option<f64>,
    dy: Option<f64>,
    window: Option<String>,
    pid: Option<u32>,
}

#[js_class(rename = "DesktopPointer")]
impl JSDesktopPointer {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(illegal_ctor())
    }

    #[js_method(rename = "move")]
    async fn pointer_move(&self, ctx: JSContext, o: DesktopPointerAt) -> JSResult<JSValue> {
        let (x, y) = point(&o.at, "at")?;
        let ack = blocking(move || {
            let target = input_target(o.pid, o.window)?;
            cu::input::pointer_move(x, y, target)
        })
        .await?;
        to_js(&ctx, &ack)
    }

    #[js_method]
    async fn down(&self, ctx: JSContext, o: DesktopPointerButton) -> JSResult<JSValue> {
        let (x, y) = point(&o.at, "at")?;
        let button = mouse_button(&o.button)?;
        let ack = blocking(move || {
            let target = input_target(o.pid, o.window)?;
            cu::input::pointer_down(x, y, button, target)
        })
        .await?;
        to_js(&ctx, &ack)
    }

    #[js_method]
    async fn up(&self, ctx: JSContext, o: DesktopPointerButton) -> JSResult<JSValue> {
        let (x, y) = point(&o.at, "at")?;
        let button = mouse_button(&o.button)?;
        let ack = blocking(move || {
            let target = input_target(o.pid, o.window)?;
            cu::input::pointer_up(x, y, button, target)
        })
        .await?;
        to_js(&ctx, &ack)
    }

    #[js_method]
    async fn click(&self, ctx: JSContext, o: DesktopPointerClick) -> JSResult<JSValue> {
        let (x, y) = point(&o.at, "at")?;
        let button = mouse_button(&o.button)?;
        let count = o.count.unwrap_or(1);
        let ack = blocking(move || {
            let target = input_target(o.pid, o.window)?;
            cu::input::pointer_click(x, y, button, count, target)
        })
        .await?;
        to_js(&ctx, &ack)
    }

    #[js_method]
    async fn drag(&self, ctx: JSContext, o: DesktopPointerDrag) -> JSResult<JSValue> {
        let (fx, fy) = point(&o.from, "from")?;
        let (tx, ty) = point(&o.to, "to")?;
        let button = mouse_button(&o.button)?;
        let ack = blocking(move || {
            let target = input_target(o.pid, o.window)?;
            cu::input::pointer_drag(fx, fy, tx, ty, button, target)
        })
        .await?;
        to_js(&ctx, &ack)
    }

    /// Scroll by `dx`/`dy` notches at `at: [x, y]`.
    #[js_method]
    async fn scroll(&self, ctx: JSContext, o: DesktopPointerScroll) -> JSResult<JSValue> {
        let (x, y) = point(&o.at, "at")?;
        let (dx, dy) = (
            o.dx.unwrap_or(0.0).round() as i32,
            o.dy.unwrap_or(0.0).round() as i32,
        );
        let ack = blocking(move || {
            let target = input_target(o.pid, o.window)?;
            cu::input::pointer_scroll(x, y, dx, dy, target)
        })
        .await?;
        to_js(&ctx, &ack)
    }
}

// ===================== desktop.key.* =====================

#[js_class(clone)]
pub(crate) struct JSDesktopKey {}

impl JSDesktopKey {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

#[derive(FromJSObject)]
struct DesktopKeyType {
    text: String,
    window: Option<String>,
    pid: Option<u32>,
}

#[derive(FromJSObject)]
struct DesktopKeyPress {
    key: String,
    modifiers: Option<Vec<String>>,
    window: Option<String>,
    pid: Option<u32>,
}

#[derive(FromJSObject)]
struct DesktopKeyName {
    key: String,
    window: Option<String>,
    pid: Option<u32>,
}

#[js_class(rename = "DesktopKey")]
impl JSDesktopKey {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(illegal_ctor())
    }

    /// Type literal text into the focused control.
    #[js_method(rename = "type")]
    async fn key_type(&self, ctx: JSContext, o: DesktopKeyType) -> JSResult<JSValue> {
        let ack = blocking(move || {
            let target = input_target(o.pid, o.window)?;
            cu::input::key_type(&o.text, target)
        })
        .await?;
        to_js(&ctx, &ack)
    }

    /// Press a named key with optional `ctrl | shift | alt | meta` modifiers.
    #[js_method]
    async fn press(&self, ctx: JSContext, o: DesktopKeyPress) -> JSResult<JSValue> {
        let mods = modifiers(&o.modifiers)?;
        let ack = blocking(move || {
            let target = input_target(o.pid, o.window)?;
            cu::input::key_press(&o.key, &mods, target)
        })
        .await?;
        to_js(&ctx, &ack)
    }

    #[js_method]
    async fn down(&self, ctx: JSContext, o: DesktopKeyName) -> JSResult<JSValue> {
        let ack = blocking(move || {
            let target = input_target(o.pid, o.window)?;
            cu::input::key_down(&o.key, target)
        })
        .await?;
        to_js(&ctx, &ack)
    }

    #[js_method]
    async fn up(&self, ctx: JSContext, o: DesktopKeyName) -> JSResult<JSValue> {
        let ack = blocking(move || {
            let target = input_target(o.pid, o.window)?;
            cu::input::key_up(&o.key, target)
        })
        .await?;
        to_js(&ctx, &ack)
    }
}

// ===================== desktop.clipboard.* =====================

#[js_class(clone)]
pub(crate) struct JSDesktopClipboard {}

impl JSDesktopClipboard {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

#[derive(FromJSObject)]
struct ClipboardSet {
    text: String,
}

#[js_class(rename = "DesktopClipboard")]
impl JSDesktopClipboard {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(illegal_ctor())
    }

    #[js_method]
    async fn get(&self, ctx: JSContext) -> JSResult<JSValue> {
        let clipboard = blocking(cu::clipboard::get).await?;
        to_js(&ctx, &clipboard)
    }

    #[js_method]
    async fn set(&self, ctx: JSContext, options: ClipboardSet) -> JSResult<JSValue> {
        let ack = blocking(move || cu::clipboard::set(&options.text)).await?;
        to_js(&ctx, &ack)
    }

    #[js_method]
    async fn clear(&self, ctx: JSContext) -> JSResult<JSValue> {
        let ack = blocking(cu::clipboard::clear).await?;
        to_js(&ctx, &ack)
    }

    /// Paste into the focused control (Ctrl/Cmd+V).
    #[js_method]
    async fn paste(&self, ctx: JSContext) -> JSResult<JSValue> {
        let ack = blocking(cu::clipboard::paste).await?;
        to_js(&ctx, &ack)
    }
}

// ===================== desktop.ax.* =====================

#[js_class(clone)]
pub(crate) struct JSDesktopAx {}

impl JSDesktopAx {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

#[derive(FromJSObject)]
struct AxTreeOpt {
    window: String,
    depth: Option<u32>,
    #[js_name = "maxNodes"]
    max_nodes: Option<usize>,
}

#[derive(FromJSObject)]
struct AxQueryOpt {
    window: String,
    /// Node match query (`text | name: | role: | value: | id:`).
    #[js_name = "match"]
    match_query: String,
    all: Option<bool>,
    index: Option<usize>,
}

#[derive(FromJSObject)]
struct AxSelOpt {
    window: String,
    #[js_name = "match"]
    match_query: String,
}

#[derive(FromJSObject)]
struct AxSetValueOpt {
    window: String,
    #[js_name = "match"]
    match_query: String,
    value: String,
}

#[js_class(rename = "DesktopAx")]
impl JSDesktopAx {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(illegal_ctor())
    }

    /// Dump a window's accessibility tree (read-only).
    #[js_method]
    async fn tree(&self, ctx: JSContext, options: AxTreeOpt) -> JSResult<JSValue> {
        let tree =
            blocking(move || cu::ax::tree(&options.window, options.depth, options.max_nodes))
                .await?;
        to_js(&ctx, &tree)
    }

    /// Find matching nodes (read-only).
    #[js_method]
    async fn query(&self, ctx: JSContext, options: AxQueryOpt) -> JSResult<JSValue> {
        let q = cu::AxQuery::parse(&options.match_query);
        let all = options.all.unwrap_or(false);
        let index = options.index;
        let nodes = blocking(move || cu::ax::query(&options.window, &q, all, index)).await?;
        to_js(&ctx, &nodes)
    }

    /// Atomically match exactly one node and invoke it.
    #[js_method]
    async fn invoke(&self, ctx: JSContext, options: AxSelOpt) -> JSResult<JSValue> {
        let q = cu::AxQuery::parse(&options.match_query);
        let ack = blocking(move || cu::ax::invoke(&options.window, &q)).await?;
        to_js(&ctx, &ack)
    }
    /// Give an element keyboard focus.
    #[js_method]
    async fn focus(&self, ctx: JSContext, options: AxSelOpt) -> JSResult<JSValue> {
        let q = cu::AxQuery::parse(&options.match_query);
        let ack = blocking(move || cu::ax::focus(&options.window, &q)).await?;
        to_js(&ctx, &ack)
    }
    /// Select an item (list/tab/tree item).
    #[js_method]
    async fn select(&self, ctx: JSContext, options: AxSelOpt) -> JSResult<JSValue> {
        let q = cu::AxQuery::parse(&options.match_query);
        let ack = blocking(move || cu::ax::select(&options.window, &q)).await?;
        to_js(&ctx, &ack)
    }
    /// Expand an expandable element.
    #[js_method]
    async fn expand(&self, ctx: JSContext, options: AxSelOpt) -> JSResult<JSValue> {
        let q = cu::AxQuery::parse(&options.match_query);
        let ack = blocking(move || cu::ax::expand(&options.window, &q)).await?;
        to_js(&ctx, &ack)
    }
    /// Collapse an expandable element.
    #[js_method]
    async fn collapse(&self, ctx: JSContext, options: AxSelOpt) -> JSResult<JSValue> {
        let q = cu::AxQuery::parse(&options.match_query);
        let ack = blocking(move || cu::ax::collapse(&options.window, &q)).await?;
        to_js(&ctx, &ack)
    }
    /// Scroll an element into view.
    #[js_method(rename = "scrollIntoView")]
    async fn scroll_into_view(&self, ctx: JSContext, options: AxSelOpt) -> JSResult<JSValue> {
        let q = cu::AxQuery::parse(&options.match_query);
        let ack = blocking(move || cu::ax::scroll_into_view(&options.window, &q)).await?;
        to_js(&ctx, &ack)
    }

    /// Replace an editable element's value.
    #[js_method(rename = "setValue")]
    async fn set_value(&self, ctx: JSContext, options: AxSetValueOpt) -> JSResult<JSValue> {
        let q = cu::AxQuery::parse(&options.match_query);
        let ack = blocking(move || cu::ax::set_value(&options.window, &q, &options.value)).await?;
        to_js(&ctx, &ack)
    }

    /// The accessible element at a screen point (read-only).
    #[js_method(rename = "hitTest")]
    async fn hit_test(&self, ctx: JSContext, options: AtOpt) -> JSResult<JSValue> {
        let (x, y) = point(&options.at, "at")?;
        let node = blocking(move || cu::ax::hit_test(x, y)).await?;
        to_js(&ctx, &node)
    }
}

// ===================== desktop.wait.* =====================

#[js_class(clone)]
pub(crate) struct JSDesktopWait {}

impl JSDesktopWait {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

#[derive(FromJSObject)]
struct WaitWindowOpt {
    #[js_name = "match"]
    match_query: String,
    /// `visible` (default) | `hidden`.
    state: Option<String>,
    #[js_name = "timeoutMs"]
    timeout_ms: Option<u64>,
}

#[derive(FromJSObject)]
struct WaitAxOpt {
    window: String,
    #[js_name = "match"]
    match_query: String,
    /// `exists` (default) | `gone` | `enabled` | `focused`.
    state: Option<String>,
    #[js_name = "timeoutMs"]
    timeout_ms: Option<u64>,
}

#[derive(FromJSObject)]
struct WaitPixelOpt {
    at: Vec<f64>,
    /// Expected color as `#rrggbb`.
    color: String,
    tolerance: Option<u8>,
    #[js_name = "timeoutMs"]
    timeout_ms: Option<u64>,
}

#[js_class(rename = "DesktopWait")]
impl JSDesktopWait {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(illegal_ctor())
    }

    /// Wait until a window matches. `visible` (default) resolves to the window;
    /// `hidden` waits until no window matches and resolves to `{ ok, state }`.
    #[js_method]
    async fn window(&self, ctx: JSContext, options: WaitWindowOpt) -> JSResult<JSValue> {
        let timeout_ms = options.timeout_ms.unwrap_or(WAIT_DEFAULT_MS);
        let match_query = options.match_query;
        match options.state.as_deref() {
            None | Some("visible") => {
                let window = blocking(move || {
                    let query = cu::WindowQuery::parse(&match_query);
                    cu::wait_window(&query, Some(true), timeout_ms)
                })
                .await?;
                to_js(&ctx, &window)
            }
            // The backend only enumerates visible windows, so "hidden" means
            // "no match remains"; poll until the set empties, mirroring
            // `lxdev desktop wait window --state hidden` (which the backend's
            // one-shot `wait_window(visible=false)` rejects outright).
            Some("hidden") => {
                let ok = blocking(move || {
                    let query = cu::WindowQuery::parse(&match_query);
                    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
                    loop {
                        if cu::windows(&query)?.is_empty() {
                            return Ok(true);
                        }
                        if Instant::now() >= deadline {
                            return Err(cu::Error::Timeout(
                                "timed out waiting for window to become hidden".into(),
                            ));
                        }
                        std::thread::sleep(Duration::from_millis(150));
                    }
                })
                .await?;
                to_js(&ctx, &json!({ "ok": ok, "state": "hidden" }))
            }
            Some(other) => Err(usage(format!(
                "unknown state '{other}' (expected visible | hidden)"
            ))),
        }
    }

    /// Wait until an ax node reaches a state.
    #[js_method]
    async fn ax(&self, ctx: JSContext, options: WaitAxOpt) -> JSResult<JSValue> {
        let state = options.state.unwrap_or_else(|| "exists".to_string());
        let timeout_ms = options.timeout_ms.unwrap_or(WAIT_DEFAULT_MS);
        let ack = blocking(move || {
            let q = cu::AxQuery::parse(&options.match_query);
            cu::ax::wait(&options.window, &q, &state, timeout_ms)
        })
        .await?;
        to_js(&ctx, &ack)
    }

    /// Wait until a pixel matches a color (resolves to the pixel).
    #[js_method]
    async fn pixel(&self, ctx: JSContext, options: WaitPixelOpt) -> JSResult<JSValue> {
        let (x, y) = point(&options.at, "at")?;
        let tolerance = options.tolerance.unwrap_or(0);
        let timeout_ms = options.timeout_ms.unwrap_or(WAIT_DEFAULT_MS);
        let pixel =
            blocking(move || cu::wait_pixel(x, y, &options.color, tolerance, timeout_ms)).await?;
        to_js(&ctx, &pixel)
    }
}

// ===================== desktop.app.* / desktop.process.* =====================

#[js_class(clone)]
pub(crate) struct JSDesktopApp {}

impl JSDesktopApp {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

#[derive(FromJSObject)]
struct AppLaunchOpt {
    /// Path or PATH-resolved command.
    app: String,
    args: Option<Vec<String>>,
    /// Wait for a window matching this query before resolving.
    #[js_name = "waitWindow"]
    wait_window: Option<String>,
    #[js_name = "timeoutMs"]
    timeout_ms: Option<u64>,
}

#[derive(FromJSObject, Default)]
struct AppQuitOpt {
    #[js_name = "match"]
    match_query: Option<String>,
    pid: Option<u32>,
    window: Option<String>,
    /// Terminate instead of a graceful close.
    force: Option<bool>,
}

#[js_class(rename = "DesktopApp")]
impl JSDesktopApp {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(illegal_ctor())
    }

    /// Launch an app, optionally waiting for a window to appear.
    #[js_method]
    async fn launch(&self, ctx: JSContext, options: AppLaunchOpt) -> JSResult<JSValue> {
        let timeout_ms = options.timeout_ms.unwrap_or(WAIT_DEFAULT_MS);
        let result = blocking(move || {
            cu::app::launch(
                &options.app,
                options.args.as_deref().unwrap_or(&[]),
                options.wait_window.as_deref(),
                timeout_ms,
            )
        })
        .await?;
        to_js(&ctx, &result)
    }

    /// Quit an app (graceful close, or `force: true` to terminate). Destructive.
    #[js_method]
    async fn quit(&self, ctx: JSContext, options: AppQuitOpt) -> JSResult<JSValue> {
        let target = match (&options.match_query, &options.pid, &options.window) {
            (Some(q), None, None) => cu::QuitTarget::Match(cu::WindowQuery::parse(q)),
            (None, Some(pid), None) => cu::QuitTarget::Pid(*pid),
            (None, None, Some(id)) => cu::QuitTarget::Window(id.clone()),
            _ => return Err(usage("pass exactly one of match / pid / window")),
        };
        let force = options.force.unwrap_or(false);
        let ack = blocking(move || cu::app::quit(target, force)).await?;
        to_js(&ctx, &ack)
    }
}

#[js_class(clone)]
pub(crate) struct JSDesktopProcess {}

impl JSDesktopProcess {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

#[derive(FromJSObject, Default)]
struct ProcessListOpt {
    /// Case-insensitive name substring filter.
    filter: Option<String>,
}

#[derive(FromJSObject)]
struct ProcessKillOpt {
    pid: u32,
    force: Option<bool>,
}

#[js_class(rename = "DesktopProcess")]
impl JSDesktopProcess {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(illegal_ctor())
    }

    /// List running processes (read-only).
    #[js_method]
    async fn list(&self, ctx: JSContext, options: Optional<ProcessListOpt>) -> JSResult<JSValue> {
        let options = options.0.unwrap_or_default();
        let processes = blocking(move || cu::process::list(options.filter.as_deref())).await?;
        to_js(&ctx, &processes)
    }

    /// Terminate a process by pid. Destructive.
    #[js_method]
    async fn kill(&self, ctx: JSContext, options: ProcessKillOpt) -> JSResult<JSValue> {
        let force = options.force.unwrap_or(false);
        let ack = blocking(move || cu::process::kill(options.pid, force)).await?;
        to_js(&ctx, &ack)
    }
}
