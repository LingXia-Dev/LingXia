//! Trusted automation for native terminal workspace state.

use crate::auto_err;
use crate::resolve::json_to_js;
use rong::{FromJSObject, HostError, JSContext, JSResult, JSValue, js_class, js_method};
use serde_json::json;
use std::time::{Duration, Instant};

const HOST_TIMEOUT: Duration = Duration::from_secs(8);

#[js_class(clone)]
pub(crate) struct JSTerminalDriver {}

impl JSTerminalDriver {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

#[derive(FromJSObject)]
struct SurfaceOptions {
    surface: String,
}

#[derive(FromJSObject)]
struct SplitOptions {
    surface: String,
    direction: String,
}

#[derive(FromJSObject)]
struct InputOptions {
    surface: String,
    text: String,
}

#[derive(FromJSObject)]
struct MaximizeOptions {
    surface: String,
    maximized: bool,
}

async fn wait_for_snapshot(surface: &str) -> JSResult<serde_json::Value> {
    let deadline = Instant::now() + HOST_TIMEOUT;
    loop {
        match lxapp::terminal_automation::snapshot(surface) {
            Ok(snapshot) => return Ok(snapshot),
            Err(error) if Instant::now() < deadline => {
                let _ = error;
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            Err(error) => return Err(auto_err(error)),
        }
    }
}

fn split_direction(value: &str) -> JSResult<&str> {
    match value {
        "left" | "right" | "up" | "down" => Ok(value),
        other => Err(auto_err(format!(
            "unknown terminal split direction '{other}' (expected left | right | up | down)"
        ))),
    }
}

#[js_class(rename = "TerminalDriver")]
impl JSTerminalDriver {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(
            rong::error::E_ILLEGAL_CONSTRUCTOR,
            "Use lx.automation().terminal",
        )
        .into())
    }

    /// Read the native pane tree and the visual configuration it has consumed.
    #[js_method]
    async fn snapshot(&self, ctx: JSContext, options: SurfaceOptions) -> JSResult<JSValue> {
        let snapshot = wait_for_snapshot(options.surface.trim()).await?;
        json_to_js(&ctx, &snapshot)
    }

    /// Send text to the focused pane through the native PTY input path.
    #[js_method]
    async fn input(&self, ctx: JSContext, options: InputOptions) -> JSResult<JSValue> {
        let surface = options.surface.trim();
        if surface.is_empty() {
            return Err(auto_err("terminal input requires a surface id"));
        }
        wait_for_snapshot(surface).await?;
        let snapshot = lxapp::terminal_automation::run_command(
            surface,
            "input",
            json!({ "text": options.text }),
            HOST_TIMEOUT,
        )
        .await
        .map_err(auto_err)?;
        json_to_js(&ctx, &snapshot)
    }

    /// Expand one native terminal surface to the full content area, or put it
    /// back at its docked size.
    #[js_method(rename = "setMaximized")]
    async fn set_maximized(&self, ctx: JSContext, options: MaximizeOptions) -> JSResult<JSValue> {
        let surface = options.surface.trim();
        if surface.is_empty() {
            return Err(auto_err("terminal setMaximized requires a surface id"));
        }
        wait_for_snapshot(surface).await?;
        let snapshot = lxapp::terminal_automation::run_command(
            surface,
            "setMaximized",
            json!({ "maximized": options.maximized }),
            HOST_TIMEOUT,
        )
        .await
        .map_err(auto_err)?;
        json_to_js(&ctx, &snapshot)
    }

    /// Open a tab in one native terminal surface, and activate it.
    #[js_method(rename = "newTab")]
    async fn new_tab(&self, ctx: JSContext, options: SurfaceOptions) -> JSResult<JSValue> {
        let surface = options.surface.trim();
        if surface.is_empty() {
            return Err(auto_err("terminal newTab requires a surface id"));
        }
        wait_for_snapshot(surface).await?;
        let snapshot =
            lxapp::terminal_automation::run_command(surface, "newTab", json!({}), HOST_TIMEOUT)
                .await
                .map_err(auto_err)?;
        json_to_js(&ctx, &snapshot)
    }

    /// Split the active pane in one native terminal surface.
    #[js_method]
    async fn split(&self, ctx: JSContext, options: SplitOptions) -> JSResult<JSValue> {
        let surface = options.surface.trim();
        if surface.is_empty() {
            return Err(auto_err("terminal split requires a surface id"));
        }
        let direction = split_direction(options.direction.trim())?;
        wait_for_snapshot(surface).await?;
        let snapshot = lxapp::terminal_automation::run_command(
            surface,
            "split",
            json!({ "direction": direction }),
            HOST_TIMEOUT,
        )
        .await
        .map_err(auto_err)?;
        json_to_js(&ctx, &snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::split_direction;

    #[test]
    fn split_direction_is_semantic_and_closed() {
        assert_eq!(split_direction("right").unwrap(), "right");
        assert!(split_direction("bottom").is_err());
    }
}
