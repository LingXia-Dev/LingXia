//! Shared control dispatch and transports for LingXia apps.
//!
//! Host libraries decide how this service is installed into their own
//! `HostAddon`; this crate only exposes the service entry points.
//!
//! Two transports reach the same methods. The `dev-bridge` feature dials a
//! `lingxia dev` session over a websocket; the `local-control` feature listens
//! on a local IPC endpoint so a shipped product can offer a command line to
//! its own integrations. Both funnel through [`dispatch`], and a host enables only
//! what it ships.

mod app;
#[cfg(feature = "dev-bridge")]
mod bridge;
mod browser;
#[cfg(feature = "computer-use")]
mod desktop;
mod extra;
#[cfg(feature = "local-control")]
pub mod local_control;
mod lxapp;
mod lxapp_nav;
mod lxapp_page;
mod runner;
#[cfg(feature = "test-runtime")]
mod session_test;
mod util;

#[cfg(feature = "dev-bridge")]
pub use bridge::start_dev_session_bridge_from_env;

pub use extra::register_control_namespace;
pub use lingxia_control_protocol::{
    ControlError, ControlRequest, ControlResponse, dev_session, methods,
};

/// Answer one request. Transport-free on purpose: the same methods serve the
/// development websocket and the product's local control socket, and a second
/// copy of this chain would drift the moment a namespace is added to one.
pub fn dispatch(request: ControlRequest) -> ControlResponse {
    let ControlRequest { id, method, params } = request;
    #[cfg(feature = "test-runtime")]
    if let Some(result) = session_test::handle_session_test_command(&method, params.clone()) {
        return command_result(id, result);
    }

    #[cfg(feature = "computer-use")]
    if let Some(response) = desktop::handle(id.clone(), &method, params.clone()) {
        return response;
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
    } else if let Some(result) = extra::handle(&method, params.clone()) {
        command_result(id, result)
    } else {
        match method.as_str() {
            methods::ECHO => ControlResponse::success(id, params),
            other => {
                ControlResponse::error(id, "unknown_method", format!("unknown method: {other}"))
            }
        }
    }
}

/// Parse one request line and answer it. The control socket's entry into the
/// handler chain.
#[cfg(feature = "local-control")]
pub(crate) fn dispatch_line(line: &str) -> ControlResponse {
    local_control::reply_with(line, dispatch)
}

fn command_result(
    command_id: String,
    result: Result<Option<serde_json::Value>, String>,
) -> ControlResponse {
    match result {
        Ok(data) => ControlResponse::success(command_id, data),
        Err(error) => {
            let (code, message) = tagged_handler_error(&error);
            ControlResponse::error(command_id, code, message)
        }
    }
}

/// Handlers may prefix `Err` with `(slug): ` using the same slugs the CLI
/// already branches on (`usage`, `not_found`, `permission`, …). Untagged
/// strings stay `request_failed` so built-in prose errors do not change.
fn tagged_handler_error(error: &str) -> (&'static str, String) {
    const TAGS: &[&str] = &[
        "usage",
        "not_found",
        "ambiguous",
        "timeout",
        "permission",
        "unsupported",
        "unavailable",
        "stale",
        "failed",
    ];
    for tag in TAGS {
        let prefix = format!("({tag}): ");
        if let Some(rest) = error.strip_prefix(&prefix) {
            return (*tag, rest.to_string());
        }
    }
    ("request_failed", error.to_string())
}
