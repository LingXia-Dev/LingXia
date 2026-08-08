//! Automating the machine, run inside the host process.
//!
//! The work is local either way — `lingxia-computer-use` talks to the OS, not
//! to a server. What routing it through the host buys is *whose* permission it
//! is. macOS attributes Accessibility and Screen Recording to the responsible
//! process, so the same binary invoked from two terminals reports two answers,
//! and the entry the user sees in System Settings names the terminal rather
//! than the product they installed. Answered here, the grant belongs to the
//! app bundle: one entry, the product's own name, revocable in the one place
//! a user would look.

use lingxia_computer_use as cu;
use lingxia_control_protocol::ControlResponse;
use lingxia_control_protocol::methods::desktop as method;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

/// This namespace answers with its own error codes rather than the generic
/// `request_failed`, because `lingxia-computer-use` codes are a contract:
/// callers branch on them and a client turns them into its exit status.
pub fn handle(id: String, name: &str, params: Option<Value>) -> Option<ControlResponse> {
    if !name.starts_with("desktop.") {
        return None;
    }
    let outcome = dispatch(name, params.clone());
    // Only after it worked, and only for the commands that change the machine.
    // A person watching wants to see what happened, not what was attempted.
    if let Ok(result) = &outcome
        && let Some(acted) = actuation(name, params.as_ref(), result.as_ref())
    {
        cu::pip::note_activity(acted);
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
fn actuation(name: &str, params: Option<&Value>, result: Option<&Value>) -> Option<cu::Acted> {
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
        return Some(cu::Acted::Somewhere);
    };
    let number = |key: &str| params.get(key).and_then(Value::as_i64).map(|n| n as i32);

    // Prefer the window the command *resolved*, not the one it was asked for.
    // `window focus --match process:Edge` names a query, and answering "no
    // particular window" would leave the viewer mirroring a whole display when
    // the command knew exactly which window it acted on — and a window is the
    // better thing to watch anyway, since its capture survives being covered.
    let resolved = result
        .filter(|_| name.starts_with("desktop.window."))
        .and_then(|value| value.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string);

    // Only the window commands and the accessibility ones name a window. A
    // pointer or key command's `target` is a **process id** — it narrows where
    // the event is delivered and says nothing about coordinates, which stay
    // global on every backend.
    let window = resolved.or_else(|| {
        params
            .get("window_id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| {
                serde_json::from_value::<cu::WindowTarget>(params.get("target")?.clone())
                    .ok()
                    .and_then(|target| match target {
                        cu::WindowTarget::Id(id) => Some(id),
                        cu::WindowTarget::Match(_) => None,
                    })
            })
    });

    let point = number("x")
        .zip(number("y"))
        // A drag ends where it ends; that is the point worth marking.
        .or_else(|| number("to_x").zip(number("to_y")));

    Some(match (point, window) {
        (Some((x, y)), _) => cu::Acted::At { x, y },
        (None, Some(id)) => cu::Acted::Window(id),
        (None, None) => cu::Acted::Somewhere,
    })
}

/// A code the client can act on, plus what went wrong.
struct Failure {
    code: &'static str,
    message: String,
}

/// Malformed parameters are the client's mistake, and `usage` is the code
/// `lingxia-computer-use` uses for exactly that.
fn usage(message: impl Into<String>) -> Failure {
    Failure {
        code: cu::ErrorCode::Usage.as_str(),
        message: message.into(),
    }
}

type Answer = Result<Option<Value>, Failure>;

fn dispatch(name: &str, params: Option<Value>) -> Answer {
    match name {
        method::DOCTOR => encode(cu::doctor()),
        method::PERMISSIONS => encode(cu::permissions()),
        // Prompting is a foreground act — macOS shows the dialog against the
        // app, which is the point of answering it here.
        method::REQUEST_PERMISSIONS => encode(cu::request_permissions()),
        method::DISPLAYS => report(cu::displays()),
        method::WINDOWS => {
            let args: cu::wire::Windows = decode(params)?;
            report(cu::windows(&args.query))
        }
        method::SCREENSHOT => {
            let args: cu::wire::Screenshot = decode(params)?;
            report(cu::screenshot(args.target))
        }
        method::PIXEL => {
            let args: cu::wire::Point = decode(params)?;
            report(cu::pixel(args.x, args.y))
        }
        method::WAIT_WINDOW => {
            let args: cu::wire::WaitWindow = decode(params)?;
            report(cu::wait_window(&args.query, args.visible, args.timeout_ms))
        }
        method::WAIT_PIXEL => {
            let args: cu::wire::WaitPixel = decode(params)?;
            report(cu::wait_pixel(
                args.x,
                args.y,
                &args.hex,
                args.tolerance,
                args.timeout_ms,
            ))
        }

        method::window::STATUS => window(params, cu::window::status),
        method::window::FOCUS => window(params, cu::window::focus),
        method::window::ACTIVATE => window(params, cu::window::activate),
        method::window::RAISE => window(params, cu::window::raise),
        method::window::MINIMIZE => window(params, cu::window::minimize),
        method::window::RESTORE => window(params, cu::window::restore),
        method::window::MAXIMIZE => window(params, cu::window::maximize),
        method::window::CLOSE => window(params, cu::window::close),
        method::window::MOVE => {
            let args: cu::wire::WindowMove = decode(params)?;
            report(cu::window::move_to(&args.target, args.x, args.y))
        }
        method::window::MOVE_DISPLAY => {
            let args: cu::wire::WindowMoveDisplay = decode(params)?;
            report(cu::window::move_to_display(&args.target, &args.display_id))
        }
        method::window::RESIZE => {
            let args: cu::wire::WindowResize = decode(params)?;
            report(cu::window::resize(&args.target, args.width, args.height))
        }
        method::window::SET_ALWAYS_ON_TOP => {
            let args: cu::wire::WindowAlwaysOnTop = decode(params)?;
            report(cu::window::set_always_on_top(&args.target, args.on))
        }

        method::pointer::MOVE => {
            let args: cu::wire::PointerMove = decode(params)?;
            report(cu::input::pointer_move(args.x, args.y, args.target))
        }
        method::pointer::DOWN => {
            let args: cu::wire::PointerButton = decode(params)?;
            report(cu::input::pointer_down(
                args.x,
                args.y,
                args.button,
                args.target,
            ))
        }
        method::pointer::UP => {
            let args: cu::wire::PointerButton = decode(params)?;
            report(cu::input::pointer_up(
                args.x,
                args.y,
                args.button,
                args.target,
            ))
        }
        method::pointer::CLICK => {
            let args: cu::wire::PointerClick = decode(params)?;
            report(cu::input::pointer_click(
                args.x,
                args.y,
                args.button,
                args.count,
                args.target,
            ))
        }
        method::pointer::SCROLL => {
            let args: cu::wire::PointerScroll = decode(params)?;
            report(cu::input::pointer_scroll(
                args.x,
                args.y,
                args.dx,
                args.dy,
                args.target,
            ))
        }
        method::pointer::DRAG => {
            let args: cu::wire::PointerDrag = decode(params)?;
            report(cu::input::pointer_drag(
                args.from_x,
                args.from_y,
                args.to_x,
                args.to_y,
                args.button,
                args.target,
            ))
        }

        method::key::TYPE => {
            let args: cu::wire::KeyText = decode(params)?;
            report(cu::input::key_type(&args.text, args.target))
        }
        method::key::DOWN => {
            let args: cu::wire::KeyName = decode(params)?;
            report(cu::input::key_down(&args.name, args.target))
        }
        method::key::UP => {
            let args: cu::wire::KeyName = decode(params)?;
            report(cu::input::key_up(&args.name, args.target))
        }
        method::key::PRESS => {
            let args: cu::wire::KeyPress = decode(params)?;
            report(cu::input::key_press(
                &args.name,
                &args.modifiers,
                args.target,
            ))
        }

        method::ax::TREE => {
            let args: cu::wire::AxTree = decode(params)?;
            report(cu::ax::tree(&args.window_id, args.depth, args.max_nodes))
        }
        method::ax::HIT_TEST => {
            let args: cu::wire::Point = decode(params)?;
            report(cu::ax::hit_test(args.x, args.y))
        }
        method::ax::QUERY => {
            let args: cu::wire::AxSearch = decode(params)?;
            report(cu::ax::query(
                &args.window_id,
                &args.query,
                args.all,
                args.index,
            ))
        }
        method::ax::INVOKE => ax(params, cu::ax::invoke),
        method::ax::FOCUS => ax(params, cu::ax::focus),
        method::ax::SELECT => ax(params, cu::ax::select),
        method::ax::EXPAND => ax(params, cu::ax::expand),
        method::ax::COLLAPSE => ax(params, cu::ax::collapse),
        method::ax::SCROLL_INTO_VIEW => ax(params, cu::ax::scroll_into_view),
        method::ax::SET_VALUE => {
            let args: cu::wire::AxSetValue = decode(params)?;
            report(cu::ax::set_value(&args.window_id, &args.query, &args.value))
        }
        method::ax::WAIT => {
            let args: cu::wire::AxWait = decode(params)?;
            report(cu::ax::wait(
                &args.window_id,
                &args.query,
                &args.state,
                args.timeout_ms,
            ))
        }

        method::clipboard::GET => report(cu::clipboard::get()),
        method::clipboard::CLEAR => report(cu::clipboard::clear()),
        method::clipboard::PASTE => report(cu::clipboard::paste()),
        method::clipboard::SET => {
            let args: cu::wire::ClipboardSet = decode(params)?;
            report(cu::clipboard::set(&args.text))
        }

        method::app::LAUNCH => {
            let args: cu::wire::AppLaunch = decode(params)?;
            report(cu::app::launch(
                &args.app,
                &args.args,
                args.wait_window.as_deref(),
                args.timeout_ms,
            ))
        }
        method::app::QUIT => {
            let args: cu::wire::AppQuit = decode(params)?;
            report(cu::app::quit(args.target, args.force))
        }

        method::process::LIST => {
            let args: cu::wire::ProcessList = decode(params)?;
            report(cu::process::list(args.filter.as_deref()))
        }
        method::process::KILL => {
            let args: cu::wire::ProcessKill = decode(params)?;
            report(cu::process::kill(args.pid, args.force))
        }

        other => Err(usage(format!("unknown desktop method: {other}"))),
    }
}

/// The window commands that differ only in which verb they call.
fn window(
    params: Option<Value>,
    action: fn(&cu::WindowTarget) -> cu::Result<cu::Window>,
) -> Answer {
    let args: cu::wire::WindowAction = decode(params)?;
    report(action(&args.target))
}

/// The accessibility commands that act on one matched node.
fn ax(params: Option<Value>, action: fn(&str, &cu::AxQuery) -> cu::Result<cu::Ack>) -> Answer {
    let args: cu::wire::AxAction = decode(params)?;
    report(action(&args.window_id, &args.query))
}

fn decode<T: DeserializeOwned>(params: Option<Value>) -> Result<T, Failure> {
    serde_json::from_value(params.unwrap_or(Value::Null)).map_err(|error| usage(error.to_string()))
}

fn report<T: Serialize>(result: cu::Result<T>) -> Answer {
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
        assert_eq!(error.code, cu::ErrorCode::Usage.as_str());
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
                actuation(name, Some(&serde_json::json!({"x": 1, "y": 2})), None).is_none(),
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
                actuation(name, Some(&serde_json::json!({})), None).is_some(),
                "{name} changes the machine"
            );
        }

        // Commands whose parameters are the unit type send no params at all.
        // Reading that as "nothing happened" left the viewer asleep through a
        // clipboard paste.
        for name in ["desktop.clipboard.paste", "desktop.clipboard.clear"] {
            assert!(
                actuation(name, None, None).is_some(),
                "{name} changes the machine even with no parameters"
            );
        }
    }

    /// A pointer command carrying a window makes its coordinates relative to
    /// that window. Marking those as global puts the ring somewhere the click
    /// never happened.
    #[test]
    fn a_point_keeps_the_space_it_arrived_in() {
        let global = actuation(
            "desktop.pointer.click",
            Some(&serde_json::json!({"x": 40, "y": 90})),
            None,
        );
        assert!(matches!(global, Some(cu::Acted::At { x: 40, y: 90 })));

        // `target` on input is a process id, not a window, and it does not
        // change the space the coordinates are in. Treating it as a window
        // sent the viewer chasing a window that never existed.
        let delivered = actuation(
            "desktop.pointer.click",
            Some(&serde_json::json!({"x": 40, "y": 90, "target": 291})),
            None,
        );
        assert!(matches!(delivered, Some(cu::Acted::At { x: 40, y: 90 })));

        // A drag ends where it ends; that is the point worth marking.
        let drag = actuation(
            "desktop.pointer.drag",
            Some(&serde_json::json!({"from_x": 1, "from_y": 2, "to_x": 30, "to_y": 40})),
            None,
        );
        assert!(matches!(drag, Some(cu::Acted::At { x: 30, y: 40 })));

        // A window command names a window and no point inside it.
        let window = actuation(
            "desktop.window.focus",
            Some(&serde_json::json!({"target": {"Id": "0x7f"}})),
            None,
        );
        let Some(cu::Acted::Window(id)) = window else {
            panic!("a window command names its window");
        };
        assert_eq!(id, "0x7f");

        // A `--match` names a query, not a window, but the command resolved
        // one and said so. Watching the display instead would mirror whatever
        // the person switched to next, which is how a viewer stops showing the
        // thing it was opened for.
        let matched = actuation(
            "desktop.window.focus",
            Some(&serde_json::json!({"target": {"Match": {"process": "Edge"}}})),
            Some(&serde_json::json!({"id": "0x2600", "title": "Bing"})),
        );
        let Some(cu::Acted::Window(id)) = matched else {
            panic!("the window a command resolved is the window to watch");
        };
        assert_eq!(id, "0x2600");
    }
}
