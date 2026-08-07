//! Devtool runtime bridge and protocol helpers for LingXia apps.
//!
//! Host libraries decide how this service is installed into their own
//! `HostAddon`; this crate only exposes the service entry points.
//!
//! Two transports reach the same handlers. The `dev-bridge` feature dials a
//! `lingxia dev` session over a websocket; the `control-socket` feature listens
//! on a local IPC endpoint so a shipped product can offer a command line and
//! agent skills. Both funnel through [`dispatch`], and a host enables only
//! what it ships.

mod app;
#[cfg(feature = "dev-bridge")]
mod bridge;
mod browser;
#[cfg(feature = "control-socket")]
pub mod control;
#[cfg(feature = "computer-use")]
mod desktop;
mod lxapp;
mod lxapp_nav;
mod lxapp_page;
mod runner;
#[cfg(feature = "test-runtime")]
mod session_test;
mod util;

#[cfg(feature = "dev-bridge")]
pub use bridge::start_devtool_bridge_from_env;

pub use lingxia_devtool_protocol::{
    DEV_SESSION_PROTOCOL_VERSION, DevSessionEvent, DevSessionLog, DevSessionLogLevel,
    DevSessionMessage, DevSessionRole, capabilities, handlers,
};

/// Answer one request. Transport-free on purpose: the same handlers serve the
/// development websocket and the product's local control socket, and a second
/// copy of this chain would drift the moment a namespace is added to one.
pub fn dispatch(
    id: String,
    method: String,
    params: Option<serde_json::Value>,
) -> DevSessionMessage {
    #[cfg(feature = "test-runtime")]
    if let Some(result) = session_test::handle_session_test_command(&method, params.clone()) {
        return command_result(id, result);
    }

    #[cfg(feature = "computer-use")]
    if let Some(result) = desktop::handle_desktop_command(&method, params.clone()) {
        return command_result(id, result);
    }

    if let Some(result) = app::handle_app_command(&method, params.clone()) {
        command_result(id, result)
    } else if let Some(result) = browser::handle_browser_command(&method, params.clone()) {
        command_result(id, result)
    } else if let Some(result) = lxapp_nav::handle_lxapp_nav_command(&method, params.clone()) {
        command_result(id, result)
    } else if let Some(result) = lxapp_page::handle_lxapp_page_command(&method, params.clone()) {
        command_result(id, result)
    } else if let Some(result) = runner::handle_runner_command(&method, params.clone()) {
        command_result(id, result)
    } else if let Some(result) = lxapp::handle_lxapp_command(&method, params.clone()) {
        command_result(id, result)
    } else {
        match method.as_str() {
            handlers::ECHO => DevSessionMessage::success(id, params),
            other => {
                DevSessionMessage::error(id, "unknown_method", format!("unknown method: {other}"))
            }
        }
    }
}

/// Parse one request line and answer it. The control socket's entry into the
/// handler chain.
#[cfg(feature = "control-socket")]
pub(crate) fn dispatch_line(line: &str) -> DevSessionMessage {
    control::reply_with(line, dispatch)
}

fn command_result(
    command_id: String,
    result: Result<Option<serde_json::Value>, String>,
) -> DevSessionMessage {
    match result {
        Ok(data) => DevSessionMessage::success(command_id, data),
        Err(error) => DevSessionMessage::error(command_id, "request_failed", error),
    }
}
