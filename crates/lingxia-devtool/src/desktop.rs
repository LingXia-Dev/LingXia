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

use lingxia_devtool_protocol::handlers;
use serde_json::Value;

pub fn handle_desktop_command(
    method: &str,
    _params: Option<Value>,
) -> Option<Result<Option<Value>, String>> {
    match method {
        handlers::desktop::DOCTOR => Some(encode(lingxia_computer_use::doctor())),
        handlers::desktop::PERMISSIONS => Some(encode(lingxia_computer_use::permissions())),
        // Prompting is a foreground act — macOS shows the dialog against the
        // app, which is the point of answering it here.
        handlers::desktop::REQUEST_PERMISSIONS => {
            Some(encode(lingxia_computer_use::request_permissions()))
        }
        _ => None,
    }
}

fn encode<T: serde::Serialize>(value: T) -> Result<Option<Value>, String> {
    serde_json::to_value(value)
        .map(Some)
        .map_err(|error| error.to_string())
}
