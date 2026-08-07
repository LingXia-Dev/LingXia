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
use lingxia_devtool_protocol::DevSessionMessage;
use lingxia_devtool_protocol::handlers::desktop as method;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

/// This namespace answers with its own error codes rather than the generic
/// `request_failed`, because `lingxia-computer-use` codes are a contract:
/// callers branch on them and a client turns them into its exit status.
pub fn handle(id: String, name: &str, params: Option<Value>) -> Option<DevSessionMessage> {
    if !name.starts_with("desktop.") {
        return None;
    }
    Some(match dispatch(name, params) {
        Ok(result) => DevSessionMessage::success(id, result),
        Err(Failure { code, message }) => DevSessionMessage::error(id, code, message),
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
        let DevSessionMessage::Response { error, .. } = mistyped else {
            panic!("expected a response");
        };
        let error = error.expect("an unknown method is an error");
        assert_eq!(error.code, cu::ErrorCode::Usage.as_str());
        assert!(error.message.contains("desktop.windos"));

        assert!(
            handle("1".into(), "browser.open", None).is_none(),
            "another namespace's method must pass through"
        );
    }
}
