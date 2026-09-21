//! Ordered, mixed sidebar Pins exposed to trusted control surfaces.
use crate::host::HostResult;
use lingxia_shell::ShellPin;
use lxapp::{LxApp, LxAppError};
use std::sync::Arc;

#[derive(serde::Deserialize)]
struct ReorderPinsInput {
    items: Vec<ShellPin>,
}

#[lingxia::framework_native("shell.pins", audience = "control-app-or-browser-only")]
fn pins(_app: Arc<LxApp>) -> HostResult<Vec<ShellPin>> {
    lingxia_shell::pins().map_err(|error| LxAppError::Runtime(error.to_string()))
}

#[lingxia::framework_native("shell.reorderPins", audience = "control-app-or-browser-only")]
fn reorder_pins(_app: Arc<LxApp>, input: ReorderPinsInput) -> HostResult<Vec<ShellPin>> {
    lingxia_shell::reorder_pins(input.items).map_err(|error| match error {
        lingxia_shell::ShellError::InvalidState(_)
        | lingxia_shell::ShellError::LimitReached { .. } => {
            LxAppError::InvalidParameter(error.to_string())
        }
        _ => LxAppError::Runtime(error.to_string()),
    })?;
    lingxia_shell::pins().map_err(|error| LxAppError::Runtime(error.to_string()))
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
