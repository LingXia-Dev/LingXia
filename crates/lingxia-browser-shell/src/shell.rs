//! Ordered, mixed sidebar Pins exposed to trusted control surfaces.
use crate::host::HostResult;
use lingxia_shell::{MAX_SHELL_PINS, ShellPin};
use lxapp::{LxApp, LxAppError};
use std::sync::Arc;

/// A full sidebar, as a code a caller can branch on.
///
/// Every other rejection here is a programming error the caller cannot act on,
/// but this one is ordinary: the person pinned one thing too many. A caller
/// that must say so in its own language needs to recognise it without matching
/// this crate's English prose, so it travels as a code with the limit in
/// `data` rather than as a sentence.
pub const SHELL_PIN_LIMIT_CODE: &str = "SHELL_PIN_LIMIT";

/// Map a Pin mutation failure, giving the full-sidebar case its own code.
pub(crate) fn map_pin_error(error: lingxia_shell::ShellError) -> LxAppError {
    match error {
        lingxia_shell::ShellError::LimitReached { max } => LxAppError::RongJSHost {
            code: SHELL_PIN_LIMIT_CODE.to_string(),
            message: format!("the sidebar holds {max} Pins"),
            data: Some(serde_json::json!({ "max": max })),
        },
        lingxia_shell::ShellError::InvalidState(_) => {
            LxAppError::InvalidParameter(error.to_string())
        }
        other => LxAppError::Runtime(other.to_string()),
    }
}

#[derive(serde::Deserialize)]
struct ReorderPinsInput {
    items: Vec<ShellPin>,
}

/// The sidebar's Pins and how many it holds.
///
/// The budget is shared with pinned lxapps, so a caller cannot derive the room
/// it has left from its own rows, and hard-coding the total would copy a
/// constant that lives here.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ShellPins {
    items: Vec<ShellPin>,
    max: usize,
}

fn shell_pins() -> HostResult<ShellPins> {
    Ok(ShellPins {
        items: lingxia_shell::pins().map_err(|error| LxAppError::Runtime(error.to_string()))?,
        max: MAX_SHELL_PINS,
    })
}

#[lingxia::framework_native("shell.pins", audience = "control-app-or-browser-only")]
fn pins(_app: Arc<LxApp>) -> HostResult<ShellPins> {
    shell_pins()
}

#[lingxia::framework_native("shell.reorderPins", audience = "control-app-or-browser-only")]
fn reorder_pins(_app: Arc<LxApp>, input: ReorderPinsInput) -> HostResult<ShellPins> {
    lingxia_shell::reorder_pins(input.items).map_err(map_pin_error)?;
    shell_pins()
}

pub(crate) fn register() {
    lxapp::host::register_host_entry(recently_closed_host());
    lxapp::host::register_host_entry(reopen_host());
    lxapp::host::register_host_entry(pins_host());
    lxapp::host::register_host_entry(reorder_pins_host());
}

#[lingxia::framework_native("tabs.recentlyClosed", audience = "browser-control-only")]
fn recently_closed(_app: Arc<LxApp>) -> HostResult<Vec<lingxia_browser::ClosedBrowserTab>> {
    Ok(lingxia_browser::recently_closed())
}
#[derive(serde::Deserialize)]
struct ReopenInput {
    #[serde(default)]
    id: Option<String>,
}
#[lingxia::framework_native("tabs.reopen", audience = "browser-control-only")]
fn reopen(_app: Arc<LxApp>, input: ReopenInput) -> HostResult<String> {
    lingxia_browser::reopen_closed(input.id.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_sidebar_is_a_code_and_a_number_not_a_sentence() {
        match map_pin_error(lingxia_shell::ShellError::LimitReached { max: 8 }) {
            LxAppError::RongJSHost { code, data, .. } => {
                assert_eq!(code, SHELL_PIN_LIMIT_CODE);
                assert_eq!(data.and_then(|data| data["max"].as_u64()), Some(8));
            }
            other => panic!("expected a coded host error, got {other:?}"),
        }
        // Everything else stays an ordinary failure the caller cannot act on.
        assert!(matches!(
            map_pin_error(lingxia_shell::ShellError::InvalidState("x".into())),
            LxAppError::InvalidParameter(_)
        ));
    }
}
