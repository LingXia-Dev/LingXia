//! Automating the machine, run inside the host process.
//!
//! The work is local either way — `lingxia-device-io` talks to the OS, not
//! to a server. What routing it through the host buys is *whose* permission it
//! is. macOS attributes Accessibility and Screen Recording to the responsible
//! process, so the same binary invoked from two terminals reports two answers,
//! and the entry the user sees in System Settings names the terminal rather
//! than the product they installed. Answered here, the grant belongs to the
//! app bundle: one entry, the product's own name, revocable in the one place
//! a user would look.

use lingxia_control_protocol::ControlResponse;
use lingxia_control_protocol::methods::desktop as method;
use lingxia_device_io as device_io;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

/// This namespace answers with its own error codes rather than the generic
/// `request_failed`, because `lingxia-device-io` codes are a contract:
/// callers branch on them and a client turns them into its exit status.
static SESSION: std::sync::Mutex<Option<device_io::supervision::SupervisionGuard>> =
    std::sync::Mutex::new(None);

fn session() -> std::sync::MutexGuard<'static, Option<device_io::supervision::SupervisionGuard>> {
    SESSION.lock().unwrap_or_else(|error| error.into_inner())
}

fn ensure_session() {
    let mut slot = session();
    if slot.is_none() {
        *slot = device_io::supervision::SupervisionGuard::begin(
            device_io::supervision::SessionKind::Control,
        )
        .ok();
    }
}

/// Trusted host lifecycle: the local-control endpoint has gone away.
#[cfg(feature = "local-control")]
pub fn end_session() {
    *session() = None;
}

fn report_guarded<T: Serialize>(
    run: impl FnOnce(&device_io::supervision::GuardedInput<'_>) -> device_io::Result<T>,
) -> Answer {
    ensure_session();
    let slot = session();
    let Some(guard) = slot.as_ref() else {
        return Err(Failure {
            code: device_io::ErrorCode::Unavailable.as_str(),
            message: "supervision session is not active".into(),
        });
    };
    match guard.input() {
        Ok(input) => report(run(&input)),
        Err(error) => Err(Failure {
            code: error.code().as_str(),
            message: error.to_string(),
        }),
    }
}

pub fn handle(id: String, name: &str, params: Option<Value>) -> Option<ControlResponse> {
    if !name.starts_with("desktop.") {
        return None;
    }
    ensure_session();
    // Input and accessibility commands can make their target disappear. Keep
    // a server-resolved snapshot from immediately before dispatch both to bind
    // untrusted viewer metadata to the real destination and to remember which
    // display held a window that closes successfully.
    let activity_target = pre_actuation_target(name, params.as_ref());
    let outcome = dispatch(name, params.clone());
    // Only after it worked, and only for the commands that change the machine.
    // A person watching wants to see what happened, not what was attempted.
    if let Ok(result) = &outcome
        && let Some(acted) = actuation(
            name,
            params.as_ref(),
            result.as_ref(),
            activity_target.as_ref(),
        )
    {
        device_io::supervision::note_activity(acted);
    }
    Some(match outcome {
        Ok(result) => ControlResponse::success(id, result),
        Err(Failure { code, message }) => ControlResponse::error(id, code, message),
    })
}

/// What a method just did to the machine, or `None` when it only looked.
///
/// Read off the raw parameters rather than the typed structs: this needs the
/// same two fields from a dozen different call shapes, and decoding each one
/// again to reach them would put a second copy of every argument list here to
/// drift against the first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputDelivery {
    #[cfg(target_os = "macos")]
    Process(u32),
    Foreground,
}

/// The destination the platform backend actually used for a successful input
/// command. macOS can post directly to a pid; Windows input is delivered only
/// to the foreground window (the CLI activates it before making the request).
#[cfg(target_os = "macos")]
fn input_delivery(params: &Value) -> Option<InputDelivery> {
    Some(
        params
            .get("target")
            .and_then(Value::as_u64)
            .and_then(|pid| u32::try_from(pid).ok())
            .map_or(InputDelivery::Foreground, InputDelivery::Process),
    )
}

#[cfg(target_os = "windows")]
fn input_delivery(_params: &Value) -> Option<InputDelivery> {
    Some(InputDelivery::Foreground)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn input_delivery(_params: &Value) -> Option<InputDelivery> {
    None
}

#[derive(Debug, Clone)]
struct WindowSnapshot {
    id: String,
    focused: bool,
    bounds: device_io::Rect,
    #[cfg(any(target_os = "macos", test))]
    display_id: String,
}

impl From<device_io::Window> for WindowSnapshot {
    fn from(window: device_io::Window) -> Self {
        Self {
            id: window.id,
            focused: window.focused,
            bounds: window.bounds,
            #[cfg(any(target_os = "macos", test))]
            display_id: window.display_id,
        }
    }
}

#[derive(Debug, Clone)]
enum ActivityTarget {
    Window(WindowSnapshot),
    #[cfg(any(target_os = "macos", test))]
    Display {
        x: i32,
        y: i32,
    },
}

fn pointer_point(name: &str, params: &Value) -> Option<(i32, i32)> {
    let number = |key: &str| {
        params
            .get(key)
            .and_then(Value::as_i64)
            .and_then(|number| i32::try_from(number).ok())
    };
    if name == method::pointer::DRAG {
        number("to_x").zip(number("to_y"))
    } else if name.starts_with("desktop.pointer.") {
        number("x").zip(number("y"))
    } else {
        None
    }
}

/// The window that owns a drag is chosen by mouse-down at its origin even
/// though the marker belongs at the destination where the drag finishes.
fn pointer_target_point(name: &str, params: &Value) -> Option<(i32, i32)> {
    #[cfg(target_os = "windows")]
    if name == method::pointer::SCROLL {
        // Win32 wheel events may go to the focus window or, depending on the
        // user's inactive-window scrolling policy, the hover window. A plain
        // hit-test cannot prove which policy received this event.
        return None;
    }
    if name == method::pointer::DRAG {
        let number = |key: &str| {
            params
                .get(key)
                .and_then(Value::as_i64)
                .and_then(|number| i32::try_from(number).ok())
        };
        number("from_x").zip(number("from_y"))
    } else if matches!(name, method::pointer::MOVE | method::pointer::UP) {
        // A standalone UP (and MOVE while a button is held) can be routed to
        // the window that captured the preceding DOWN, not the window under
        // its current coordinates. Without cross-request capture state an
        // exact window claim would be guesswork; the point/display is honest.
        None
    } else {
        pointer_point(name, params)
    }
}

fn verified_foreground_key_window(window: WindowSnapshot) -> Option<WindowSnapshot> {
    window.focused.then_some(window)
}

#[cfg(any(target_os = "macos", test))]
fn process_key_target(windows: Vec<WindowSnapshot>) -> Option<ActivityTarget> {
    match windows.as_slice() {
        [] => None,
        [window] => Some(ActivityTarget::Window(window.clone())),
        [first, rest @ ..]
            if rest
                .iter()
                .all(|window| window.display_id == first.display_id) =>
        {
            let (x, y) = rect_center(first.bounds)?;
            Some(ActivityTarget::Display { x, y })
        }
        _ => None,
    }
}

#[cfg(any(target_os = "macos", test))]
fn process_input_target(
    name: &str,
    pid: u32,
    mut process_windows: impl FnMut(u32) -> Option<Vec<WindowSnapshot>>,
) -> Option<ActivityTarget> {
    // CGEventPostToPid addresses a process, not the top-level window proposed
    // by the client. Pointer activity therefore follows its global point. A
    // key may follow a sole visible process window, or only the common display
    // when several candidates make the exact recipient unknowable.
    if name.starts_with("desktop.pointer.") {
        None
    } else {
        process_key_target(process_windows(pid)?)
    }
}

fn pre_actuation_target(name: &str, params: Option<&Value>) -> Option<ActivityTarget> {
    pre_actuation_target_with_status(
        name,
        params,
        |id| {
            device_io::window::status(&device_io::WindowTarget::Id(id.to_string()))
                .ok()
                .map(Into::into)
        },
        |pid| {
            device_io::windows(&device_io::WindowQuery::by_pid(pid))
                .ok()
                .map(|windows| windows.into_iter().map(WindowSnapshot::from).collect())
        },
        actual_pointer_window,
    )
}

#[cfg(target_os = "windows")]
fn actual_pointer_window(x: i32, y: i32) -> Option<WindowSnapshot> {
    device_io::input_window_at_point(x, y).map(Into::into)
}

#[cfg(target_os = "macos")]
fn actual_pointer_window(x: i32, y: i32) -> Option<WindowSnapshot> {
    device_io::windows(&device_io::WindowQuery::default())
        .ok()?
        .into_iter()
        .find(|window| {
            let bounds = window.bounds;
            let (x, y) = (i64::from(x), i64::from(y));
            let (left, top) = (i64::from(bounds.x), i64::from(bounds.y));
            x >= left && x < left + i64::from(bounds.w) && y >= top && y < top + i64::from(bounds.h)
        })
        .map(Into::into)
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn actual_pointer_window(_x: i32, _y: i32) -> Option<WindowSnapshot> {
    None
}

fn pre_actuation_target_with_status(
    name: &str,
    params: Option<&Value>,
    mut window_status: impl FnMut(&str) -> Option<WindowSnapshot>,
    mut process_windows: impl FnMut(u32) -> Option<Vec<WindowSnapshot>>,
    mut point_window: impl FnMut(i32, i32) -> Option<WindowSnapshot>,
) -> Option<ActivityTarget> {
    // On Windows this provider is intentionally unused; mentioning it outside
    // the platform arm keeps the shared test seam warning-free.
    let _ = &mut process_windows;
    let params = params?;
    let input = name.starts_with("desktop.pointer.") || name.starts_with("desktop.key.");
    let mutating_ax = name.starts_with("desktop.ax.")
        && !matches!(
            name,
            "desktop.ax.tree" | "desktop.ax.hit_test" | "desktop.ax.query" | "desktop.ax.wait"
        );
    if !input && !mutating_ax {
        return None;
    }
    if input {
        match input_delivery(params)? {
            #[cfg(target_os = "macos")]
            InputDelivery::Process(pid) => process_input_target(name, pid, process_windows),
            InputDelivery::Foreground => {
                if name.starts_with("desktop.pointer.") {
                    let (x, y) = pointer_target_point(name, params)?;
                    point_window(x, y).map(ActivityTarget::Window)
                } else {
                    let id = params.get("window_id")?.as_str()?;
                    verified_foreground_key_window(window_status(id)?).map(ActivityTarget::Window)
                }
            }
        }
    } else {
        // AX dispatch uses this exact id as its destination, so the successful
        // command itself authenticates the association. The status lookup is
        // needed for a trusted fallback display if the action closes it.
        let id = params.get("window_id")?.as_str()?;
        Some(ActivityTarget::Window(window_status(id)?))
    }
}

fn rect_center(rect: device_io::Rect) -> Option<(i32, i32)> {
    let x = i64::from(rect.x) + i64::from(rect.w) / 2;
    let y = i64::from(rect.y) + i64::from(rect.h) / 2;
    Some((i32::try_from(x).ok()?, i32::try_from(y).ok()?))
}

fn result_window(result: Option<&Value>) -> Option<(String, (i32, i32))> {
    let value = result?;
    let id = value.get("id")?.as_str()?.to_string();
    let bounds = value.get("bounds")?;
    let rect = device_io::Rect {
        x: i32::try_from(bounds.get("x")?.as_i64()?).ok()?,
        y: i32::try_from(bounds.get("y")?.as_i64()?).ok()?,
        w: i32::try_from(bounds.get("w")?.as_i64()?).ok()?,
        h: i32::try_from(bounds.get("h")?.as_i64()?).ok()?,
    };
    Some((id, rect_center(rect)?))
}

fn actuation(
    name: &str,
    params: Option<&Value>,
    result: Option<&Value>,
    activity_target: Option<&ActivityTarget>,
) -> Option<device_io::Acted> {
    const ACTUATES: &[&str] = &[
        "desktop.pointer.",
        "desktop.key.",
        "desktop.window.",
        "desktop.ax.",
        "desktop.app.",
    ];
    // Prefix families whose members all change something, minus the readers
    // that happen to share their prefix.
    const READS: &[&str] = &[
        "desktop.window.status",
        "desktop.ax.tree",
        "desktop.ax.hit_test",
        "desktop.ax.query",
        "desktop.ax.wait",
    ];
    let named = matches!(
        name,
        "desktop.clipboard.set"
            | "desktop.clipboard.paste"
            | "desktop.clipboard.clear"
            | "desktop.process.kill"
            // Prompting puts a system dialog in front of the user and can
            // change what the app is allowed to do. Filing that under "only
            // looked at the machine" is how a permission grant happens with
            // nobody watching.
            | "desktop.permissions.request"
    );
    if READS.contains(&name) || (!named && !ACTUATES.iter().any(|prefix| name.starts_with(prefix)))
    {
        return None;
    }

    // Commands whose parameters are the unit type arrive with no `params` at
    // all, and they still change the machine — clipboard clear and paste are
    // both in that shape.
    let Some(params) = params else {
        return Some(device_io::Acted::Somewhere);
    };
    // Prefer the window the command *resolved*, not the one it was asked for.
    // `window focus --match process:Edge` names a query, and answering "no
    // particular window" would leave the viewer mirroring a whole display when
    // the command knew exactly which window it acted on — and a window is the
    // better thing to watch anyway, since its capture survives being covered.
    let resolved = name
        .starts_with("desktop.window.")
        .then(|| result_window(result))
        .flatten();
    let window = resolved.or_else(|| match activity_target? {
        ActivityTarget::Window(snapshot) => {
            Some((snapshot.id.clone(), rect_center(snapshot.bounds)?))
        }
        #[cfg(any(target_os = "macos", test))]
        ActivityTarget::Display { .. } => None,
    });
    #[cfg(any(target_os = "macos", test))]
    let display = activity_target.and_then(|target| match target {
        ActivityTarget::Display { x, y } => Some((*x, *y)),
        ActivityTarget::Window(_) => None,
    });
    #[cfg(not(any(target_os = "macos", test)))]
    let display = None;

    // Only pointer coordinates describe a location acted on. Window move also
    // has x/y parameters, but those are a requested frame origin, not a click
    // marker. A drag is represented by its destination.
    let point = pointer_point(name, params);

    Some(match (point, window) {
        (Some((x, y)), Some((id, _))) => device_io::Acted::AtWindow { x, y, id },
        (Some((x, y)), None) => device_io::Acted::At { x, y },
        (None, Some((id, (x, y)))) => device_io::Acted::WindowWithFallback { id, x, y },
        (None, None) => match display {
            Some((x, y)) => device_io::Acted::Display { x, y },
            None => device_io::Acted::Somewhere,
        },
    })
}

/// A code the client can act on, plus what went wrong.
struct Failure {
    code: &'static str,
    message: String,
}

/// Malformed parameters are the client's mistake, and `usage` is the code
/// `lingxia-device-io` uses for exactly that.
fn usage(message: impl Into<String>) -> Failure {
    Failure {
        code: device_io::ErrorCode::Usage.as_str(),
        message: message.into(),
    }
}

type Answer = Result<Option<Value>, Failure>;

fn dispatch(name: &str, params: Option<Value>) -> Answer {
    match name {
        method::DOCTOR => encode(device_io::doctor()),
        method::PERMISSIONS => encode(device_io::permissions()),
        // Prompting is a foreground act — macOS shows the dialog against the
        // app, which is the point of answering it here.
        method::REQUEST_PERMISSIONS => encode(device_io::request_permissions()),
        method::DISPLAYS => report(device_io::displays()),
        method::WINDOWS => {
            let args: device_io::wire::Windows = decode(params)?;
            report(device_io::windows(&args.query))
        }
        method::SCREENSHOT => {
            let args: device_io::wire::Screenshot = decode(params)?;
            report(device_io::capture::snapshot(args.target))
        }
        method::PIXEL => {
            let args: device_io::wire::Point = decode(params)?;
            report(device_io::capture::pixel(args.x, args.y))
        }
        method::WAIT_WINDOW => {
            let args: device_io::wire::WaitWindow = decode(params)?;
            report(device_io::wait_window(
                &args.query,
                args.visible,
                args.timeout_ms,
            ))
        }
        method::WAIT_PIXEL => {
            let args: device_io::wire::WaitPixel = decode(params)?;
            report(device_io::capture::wait_pixel(
                args.x,
                args.y,
                &args.hex,
                args.tolerance,
                args.timeout_ms,
            ))
        }

        method::window::STATUS => window(params, device_io::window::status),
        method::window::FOCUS => window(params, device_io::window::focus),
        method::window::ACTIVATE => window(params, device_io::window::activate),
        method::window::RAISE => window(params, device_io::window::raise),
        method::window::MINIMIZE => window(params, device_io::window::minimize),
        method::window::RESTORE => window(params, device_io::window::restore),
        method::window::MAXIMIZE => window(params, device_io::window::maximize),
        method::window::CLOSE => window(params, device_io::window::close),
        method::window::MOVE => {
            let args: device_io::wire::WindowMove = decode(params)?;
            report(device_io::window::move_to(&args.target, args.x, args.y))
        }
        method::window::MOVE_DISPLAY => {
            let args: device_io::wire::WindowMoveDisplay = decode(params)?;
            report(device_io::window::move_to_display(
                &args.target,
                &args.display_id,
            ))
        }
        method::window::RESIZE => {
            let args: device_io::wire::WindowResize = decode(params)?;
            report(device_io::window::resize(
                &args.target,
                args.width,
                args.height,
            ))
        }
        method::window::SET_ALWAYS_ON_TOP => {
            let args: device_io::wire::WindowAlwaysOnTop = decode(params)?;
            report(device_io::window::set_always_on_top(&args.target, args.on))
        }

        method::pointer::MOVE => {
            let args: device_io::wire::PointerMove = decode(params)?;
            report_guarded(|input| input.pointer_move(args.x, args.y, args.target))
        }
        method::pointer::DOWN => {
            let args: device_io::wire::PointerButton = decode(params)?;
            report_guarded(|input| input.pointer_down(args.x, args.y, args.button, args.target))
        }
        method::pointer::UP => {
            let args: device_io::wire::PointerButton = decode(params)?;
            report_guarded(|input| input.pointer_up(args.x, args.y, args.button, args.target))
        }
        method::pointer::CLICK => {
            let args: device_io::wire::PointerClick = decode(params)?;
            report_guarded(|input| {
                input.pointer_click(args.x, args.y, args.button, args.count, args.target)
            })
        }
        method::pointer::SCROLL => {
            let args: device_io::wire::PointerScroll = decode(params)?;
            report_guarded(|input| {
                input.pointer_scroll(args.x, args.y, args.dx, args.dy, args.target)
            })
        }
        method::pointer::DRAG => {
            let args: device_io::wire::PointerDrag = decode(params)?;
            report_guarded(|input| {
                input.pointer_drag(
                    args.from_x,
                    args.from_y,
                    args.to_x,
                    args.to_y,
                    args.button,
                    args.target,
                )
            })
        }

        method::key::TYPE => {
            let args: device_io::wire::KeyText = decode(params)?;
            report_guarded(|input| input.key_type(&args.text, args.target))
        }
        method::key::DOWN => {
            let args: device_io::wire::KeyName = decode(params)?;
            report_guarded(|input| input.key_down(&args.name, args.target))
        }
        method::key::UP => {
            let args: device_io::wire::KeyName = decode(params)?;
            report_guarded(|input| input.key_up(&args.name, args.target))
        }
        method::key::PRESS => {
            let args: device_io::wire::KeyPress = decode(params)?;
            report_guarded(|input| input.key_press(&args.name, &args.modifiers, args.target))
        }

        method::ax::TREE => {
            let args: device_io::wire::AxTree = decode(params)?;
            report(device_io::ax::tree(
                &args.window_id,
                args.depth,
                args.max_nodes,
            ))
        }
        method::ax::HIT_TEST => {
            let args: device_io::wire::Point = decode(params)?;
            report(device_io::ax::hit_test(args.x, args.y))
        }
        method::ax::QUERY => {
            let args: device_io::wire::AxSearch = decode(params)?;
            report(device_io::ax::query(
                &args.window_id,
                &args.query,
                args.all,
                args.index,
            ))
        }
        method::ax::INVOKE => ax(params, device_io::ax::invoke),
        method::ax::FOCUS => ax(params, device_io::ax::focus),
        method::ax::SELECT => ax(params, device_io::ax::select),
        method::ax::EXPAND => ax(params, device_io::ax::expand),
        method::ax::COLLAPSE => ax(params, device_io::ax::collapse),
        method::ax::SCROLL_INTO_VIEW => ax(params, device_io::ax::scroll_into_view),
        method::ax::SET_VALUE => {
            let args: device_io::wire::AxSetValue = decode(params)?;
            report(device_io::ax::set_value(
                &args.window_id,
                &args.query,
                &args.value,
            ))
        }
        method::ax::WAIT => {
            let args: device_io::wire::AxWait = decode(params)?;
            report(device_io::ax::wait(
                &args.window_id,
                &args.query,
                &args.state,
                args.timeout_ms,
            ))
        }

        method::clipboard::GET => report(device_io::clipboard::get()),
        method::clipboard::CLEAR => report(device_io::clipboard::clear()),
        method::clipboard::PASTE => report(device_io::clipboard::paste()),
        method::clipboard::SET => {
            let args: device_io::wire::ClipboardSet = decode(params)?;
            report(device_io::clipboard::set(&args.text))
        }

        method::app::LAUNCH => {
            let args: device_io::wire::AppLaunch = decode(params)?;
            report(device_io::app::launch(
                &args.app,
                &args.args,
                args.wait_window.as_deref(),
                args.timeout_ms,
            ))
        }
        method::app::QUIT => {
            let args: device_io::wire::AppQuit = decode(params)?;
            report(device_io::app::quit(args.target, args.force))
        }

        method::process::LIST => {
            let args: device_io::wire::ProcessList = decode(params)?;
            report(device_io::process::list(args.filter.as_deref()))
        }
        method::process::KILL => {
            let args: device_io::wire::ProcessKill = decode(params)?;
            report(device_io::process::kill(args.pid, args.force))
        }

        other => Err(usage(format!("unknown desktop method: {other}"))),
    }
}

/// The window commands that differ only in which verb they call.
fn window(
    params: Option<Value>,
    action: fn(&device_io::WindowTarget) -> device_io::Result<device_io::Window>,
) -> Answer {
    let args: device_io::wire::WindowAction = decode(params)?;
    report(action(&args.target))
}

/// The accessibility commands that act on one matched node.
fn ax(
    params: Option<Value>,
    action: fn(&str, &device_io::AxQuery) -> device_io::Result<device_io::Ack>,
) -> Answer {
    let args: device_io::wire::AxAction = decode(params)?;
    report(action(&args.window_id, &args.query))
}

fn decode<T: DeserializeOwned>(params: Option<Value>) -> Result<T, Failure> {
    serde_json::from_value(params.unwrap_or(Value::Null)).map_err(|error| usage(error.to_string()))
}

fn report<T: Serialize>(result: device_io::Result<T>) -> Answer {
    match result {
        Ok(value) => encode(value),
        Err(error) => Err(Failure {
            code: error.code().as_str(),
            message: error.to_string(),
        }),
    }
}

fn encode<T: Serialize>(value: T) -> Answer {
    serde_json::to_value(value)
        .map(Some)
        .map_err(|error| usage(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This namespace claims every `desktop.` name, so a typo has to fail here
    /// rather than fall through to another namespace's handler and be reported
    /// as that one's unknown method.
    #[test]
    fn claims_its_prefix_and_refuses_the_rest() {
        let mistyped = handle("1".into(), "desktop.windos", None)
            .expect("a desktop-prefixed name belongs to this namespace");
        let error = mistyped.error.expect("an unknown method is an error");
        assert_eq!(error.code, device_io::ErrorCode::Usage.as_str());
        assert!(error.message.contains("desktop.windos"));

        assert!(
            handle("1".into(), "browser.open", None).is_none(),
            "another namespace's method must pass through"
        );
    }

    /// The viewer opens itself off this classification, so a method landing on
    /// the wrong side of it either pops a window up at someone who only asked
    /// what their screen looked like, or leaves them watching nothing while
    /// their machine is driven.
    #[test]
    fn only_the_commands_that_change_something_wake_the_viewer() {
        for name in [
            "desktop.screenshot",
            "desktop.windows",
            "desktop.displays",
            "desktop.pixel",
            "desktop.doctor",
            "desktop.clipboard.get",
            "desktop.process.list",
            "desktop.wait.window",
            "desktop.window.status",
            "desktop.ax.tree",
            "desktop.ax.query",
            // The viewer's own commands, or it would reopen what was closed.
            "desktop.pip.hide",
            "desktop.pip.status",
        ] {
            assert!(
                actuation(name, Some(&serde_json::json!({"x": 1, "y": 2})), None, None,).is_none(),
                "{name} only looks at the machine"
            );
        }

        for name in [
            "desktop.pointer.click",
            "desktop.key.type",
            "desktop.window.focus",
            "desktop.window.close",
            "desktop.ax.invoke",
            "desktop.ax.set_value",
            "desktop.app.launch",
            "desktop.clipboard.set",
            "desktop.process.kill",
            "desktop.permissions.request",
        ] {
            assert!(
                actuation(name, Some(&serde_json::json!({})), None, None).is_some(),
                "{name} changes the machine"
            );
        }

        // Commands whose parameters are the unit type send no params at all.
        // Reading that as "nothing happened" left the viewer asleep through a
        // clipboard paste.
        for name in ["desktop.clipboard.paste", "desktop.clipboard.clear"] {
            assert!(
                actuation(name, None, None, None).is_some(),
                "{name} changes the machine even with no parameters"
            );
        }
    }

    /// Input targeting controls delivery but never changes the global
    /// coordinate space used by pointer actions and viewer markers.
    #[test]
    fn a_point_keeps_the_space_it_arrived_in() {
        let global = actuation(
            "desktop.pointer.click",
            Some(&serde_json::json!({"x": 40, "y": 90})),
            None,
            None,
        );
        assert!(matches!(
            global,
            Some(device_io::Acted::At { x: 40, y: 90 })
        ));

        // `target` on input is a process id, not a window, and it does not
        // change the space the coordinates are in. Treating it as a window
        // sent the viewer chasing a window that never existed.
        let delivered = actuation(
            "desktop.pointer.click",
            Some(&serde_json::json!({"x": 40, "y": 90, "target": 291})),
            None,
            None,
        );
        assert!(matches!(
            delivered,
            Some(device_io::Acted::At { x: 40, y: 90 })
        ));

        // Product input keeps delivery pid and viewer window separate. The
        // point remains a marker while the viewer follows the window.
        let target = ActivityTarget::Window(WindowSnapshot {
            id: "0x7f".into(),
            focused: true,
            bounds: device_io::Rect {
                x: 0,
                y: 0,
                w: 800,
                h: 600,
            },
            display_id: "1".into(),
        });
        let targeted = actuation(
            "desktop.pointer.click",
            Some(&serde_json::json!({
                "x": 40,
                "y": 90,
                "target": 291,
                "window_id": "0x7f"
            })),
            None,
            Some(&target),
        );
        assert!(matches!(
            targeted,
            Some(device_io::Acted::AtWindow { x: 40, y: 90, ref id }) if id == "0x7f"
        ));

        // A drag ends where it ends; that is the point worth marking.
        let drag = actuation(
            "desktop.pointer.drag",
            Some(&serde_json::json!({"from_x": 1, "from_y": 2, "to_x": 30, "to_y": 40})),
            None,
            None,
        );
        assert!(matches!(drag, Some(device_io::Acted::At { x: 30, y: 40 })));

        // A window command names a window and no point inside it.
        let window = actuation(
            "desktop.window.focus",
            Some(&serde_json::json!({"target": {"Id": "0x7f"}})),
            Some(&serde_json::json!({
                "id": "0x7f",
                "bounds": {"x": 100, "y": 200, "w": 800, "h": 600}
            })),
            None,
        );
        let Some(device_io::Acted::WindowWithFallback { id, x, y }) = window else {
            panic!("a window command names its window");
        };
        assert_eq!(id, "0x7f");
        assert_eq!((x, y), (500, 500));

        // A `--match` names a query, not a window, but the command resolved
        // one and said so. Watching the display instead would mirror whatever
        // the person switched to next, which is how a viewer stops showing the
        // thing it was opened for.
        let matched = actuation(
            "desktop.window.focus",
            Some(&serde_json::json!({"target": {"Match": {"process": "Edge"}}})),
            Some(&serde_json::json!({
                "id": "0x2600",
                "title": "Bing",
                "bounds": {"x": 100, "y": 200, "w": 800, "h": 600}
            })),
            None,
        );
        let Some(device_io::Acted::WindowWithFallback { id, x, y }) = matched else {
            panic!("the resolved window and its fallback display must be retained");
        };
        assert_eq!(id, "0x2600");
        assert_eq!((x, y), (500, 500));
    }

    fn snapshot(id: &str, focused: bool, bounds: device_io::Rect) -> WindowSnapshot {
        WindowSnapshot {
            id: id.into(),
            focused,
            bounds,
            display_id: if bounds.x >= 1920 { "2" } else { "1" }.into(),
        }
    }

    #[test]
    fn client_window_metadata_must_match_the_real_input_destination() {
        let bounds = device_io::Rect {
            x: 100,
            y: 200,
            w: 800,
            h: 600,
        };
        assert!(
            verified_foreground_key_window(snapshot("stale", false, bounds)).is_none(),
            "a Windows id must still be the activated foreground window"
        );
    }

    #[test]
    fn foreground_pointer_follows_the_actual_hit_window_not_the_proposal() {
        let params = serde_json::json!({
            "x": 400,
            "y": 400,
            "window_id": "base"
        });
        let base = snapshot(
            "base",
            true,
            device_io::Rect {
                x: 100,
                y: 200,
                w: 800,
                h: 600,
            },
        );
        let popup = snapshot(
            "popup",
            false,
            device_io::Rect {
                x: 300,
                y: 300,
                w: 300,
                h: 200,
            },
        );
        let target = pre_actuation_target_with_status(
            method::pointer::CLICK,
            Some(&params),
            |_| Some(base.clone()),
            |_| None,
            |x, y| {
                assert_eq!((x, y), (400, 400));
                Some(popup.clone())
            },
        );
        let Some(ActivityTarget::Window(target)) = target else {
            panic!("the actual topmost hit window is the viewer target");
        };
        assert_eq!(target.id, "popup");
        assert!(matches!(
            actuation(
                method::pointer::CLICK,
                Some(&params),
                None,
                Some(&ActivityTarget::Window(target)),
            ),
            Some(device_io::Acted::AtWindow {
                x: 400,
                y: 400,
                ref id
            }) if id == "popup"
        ));

        let no_hit = pre_actuation_target_with_status(
            method::pointer::CLICK,
            Some(&params),
            |_| Some(base.clone()),
            |_| None,
            |_, _| None,
        );
        assert!(
            no_hit.is_none(),
            "an unknown hit safely follows the display"
        );
    }

    #[test]
    fn foreground_pointer_without_a_hit_uses_its_global_point() {
        let params = serde_json::json!({"x": 40, "y": 90, "window_id": "base"});
        let acted = actuation(method::pointer::CLICK, Some(&params), None, None);
        assert!(matches!(acted, Some(device_io::Acted::At { x: 40, y: 90 })));
    }

    #[test]
    fn drag_binds_at_mouse_down_but_marks_its_destination() {
        let params = serde_json::json!({
            "from_x": 120,
            "from_y": 130,
            "to_x": 900,
            "to_y": 700
        });
        let source = snapshot(
            "source",
            true,
            device_io::Rect {
                x: 100,
                y: 100,
                w: 500,
                h: 400,
            },
        );
        let target = pre_actuation_target_with_status(
            method::pointer::DRAG,
            Some(&params),
            |_| None,
            |_| None,
            |x, y| {
                assert_eq!((x, y), (120, 130));
                Some(source.clone())
            },
        );
        assert!(matches!(
            actuation(method::pointer::DRAG, Some(&params), None, target.as_ref()),
            Some(device_io::Acted::AtWindow {
                x: 900,
                y: 700,
                ref id
            }) if id == "source"
        ));
    }

    #[test]
    fn ambiguous_pointer_routing_degrades_to_the_acted_point() {
        let assert_degrades = |name| {
            let params = serde_json::json!({
                "x": 900,
                "y": 700,
                "window_id": "destination"
            });
            let target = pre_actuation_target_with_status(
                name,
                Some(&params),
                |_| None,
                |_| None,
                |_, _| panic!("captured pointer routing cannot be inferred by hit-testing"),
            );
            assert!(target.is_none());
            assert!(matches!(
                actuation(name, Some(&params), None, target.as_ref()),
                Some(device_io::Acted::At { x: 900, y: 700 })
            ));
        };
        for name in [method::pointer::MOVE, method::pointer::UP] {
            assert_degrades(name);
        }
        #[cfg(target_os = "windows")]
        assert_degrades(method::pointer::SCROLL);
    }

    #[test]
    fn process_directed_input_never_claims_a_proposed_same_pid_window() {
        let first = snapshot(
            "first",
            false,
            device_io::Rect {
                x: 100,
                y: 100,
                w: 700,
                h: 500,
            },
        );
        let second = snapshot(
            "second",
            false,
            device_io::Rect {
                x: 900,
                y: 100,
                w: 700,
                h: 500,
            },
        );
        assert!(
            process_input_target(method::pointer::CLICK, 42, |pid| {
                assert_eq!(pid, 42);
                Some(vec![first.clone(), second.clone()])
            })
            .is_none(),
            "CGEventPostToPid cannot authenticate which same-pid window receives a pointer event"
        );
        assert!(matches!(
            process_input_target(method::key::PRESS, 42, |pid| {
                assert_eq!(pid, 42);
                Some(vec![first.clone(), second.clone()])
            }),
            Some(ActivityTarget::Display { .. })
        ));

        let second_display = snapshot(
            "second",
            false,
            device_io::Rect {
                x: 2100,
                y: 100,
                w: 700,
                h: 500,
            },
        );
        assert!(
            process_input_target(method::key::PRESS, 42, |pid| {
                assert_eq!(pid, 42);
                Some(vec![first.clone(), second_display.clone()])
            })
            .is_none(),
            "two possible key windows on different displays have no honest viewer target"
        );
    }

    #[test]
    fn non_pointer_xy_is_never_used_as_a_marker() {
        let moved = actuation(
            method::window::MOVE,
            Some(&serde_json::json!({
                "target": {"Id": "0x42"},
                "x": 2000,
                "y": 100
            })),
            Some(&serde_json::json!({
                "id": "0x42",
                "bounds": {"x": 2000, "y": 100, "w": 600, "h": 400}
            })),
            None,
        );
        assert!(matches!(
            moved,
            Some(device_io::Acted::WindowWithFallback {
                ref id,
                x: 2300,
                y: 300
            }) if id == "0x42"
        ));
    }

    #[test]
    fn disappearing_key_and_ax_targets_keep_their_secondary_display_fallback() {
        let target = ActivityTarget::Window(snapshot(
            "0x42",
            true,
            device_io::Rect {
                x: 1920,
                y: 100,
                w: 800,
                h: 600,
            },
        ));
        for (name, params) in [
            (
                method::key::PRESS,
                serde_json::json!({"name": "w", "window_id": "0x42"}),
            ),
            (
                method::ax::INVOKE,
                serde_json::json!({"window_id": "0x42", "query": {"role": "button"}}),
            ),
        ] {
            let acted = actuation(name, Some(&params), None, Some(&target));
            assert!(matches!(
                acted,
                Some(device_io::Acted::WindowWithFallback {
                    ref id,
                    x: 2320,
                    y: 400
                }) if id == "0x42"
            ));
        }
    }
}
