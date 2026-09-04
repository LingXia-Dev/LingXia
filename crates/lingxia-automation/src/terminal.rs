//! Trusted automation for native terminal workspace state.

use crate::resolve::json_to_js;
use crate::{auto_err, host_automation_authority, native_terminal_authority, require_host_context};
use lxapp::LxApp;
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

fn authority(ctx: &JSContext) -> JSResult<lxapp::terminal_automation::TerminalAutomationAuthority> {
    require_host_context(ctx)?;
    if host_automation_authority(ctx).is_some() {
        return native_terminal_authority().cloned().ok_or_else(|| {
            auto_err("native terminal automation authority was not installed by host bootstrap")
        });
    }
    let app = LxApp::from_ctx(ctx)?;
    lxapp::terminal_automation::TerminalAutomationAuthority::for_lxapp(&app).map_err(auto_err)
}

async fn wait_for_surface(
    authority: &lxapp::terminal_automation::TerminalAutomationAuthority,
    surface: &str,
) -> JSResult<lxapp::terminal_automation::TerminalSurfaceHandle> {
    let deadline = Instant::now() + HOST_TIMEOUT;
    loop {
        match lxapp::terminal_automation::bind_surface(authority, surface) {
            Ok(handle) => return Ok(handle),
            Err(error)
                if error.contains("is not available for this owner")
                    && Instant::now() < deadline =>
            {
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
    async fn snapshot(&self, ctx: JSContext, options: JSValue) -> JSResult<JSValue> {
        let authority = authority(&ctx)?;
        let options = options.to_rust::<SurfaceOptions>()?;
        let handle = wait_for_surface(&authority, options.surface.trim()).await?;
        let snapshot = handle.snapshot().map_err(auto_err)?;
        json_to_js(&ctx, &snapshot)
    }

    /// Send text to the focused pane through the native PTY input path.
    #[js_method]
    async fn input(&self, ctx: JSContext, options: JSValue) -> JSResult<JSValue> {
        let authority = authority(&ctx)?;
        let options = options.to_rust::<InputOptions>()?;
        let surface = options.surface.trim();
        if surface.is_empty() {
            return Err(auto_err("terminal input requires a surface id"));
        }
        let handle = wait_for_surface(&authority, surface).await?;
        let snapshot = handle
            .run_command("input", json!({ "text": options.text }), HOST_TIMEOUT)
            .await
            .map_err(auto_err)?;
        json_to_js(&ctx, &snapshot)
    }

    /// Expand one native terminal surface to the full content area, or put it
    /// back at its docked size.
    #[js_method(rename = "setMaximized")]
    async fn set_maximized(&self, ctx: JSContext, options: JSValue) -> JSResult<JSValue> {
        let authority = authority(&ctx)?;
        let options = options.to_rust::<MaximizeOptions>()?;
        let surface = options.surface.trim();
        if surface.is_empty() {
            return Err(auto_err("terminal setMaximized requires a surface id"));
        }
        let handle = wait_for_surface(&authority, surface).await?;
        let snapshot = handle
            .run_command(
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
    async fn new_tab(&self, ctx: JSContext, options: JSValue) -> JSResult<JSValue> {
        let authority = authority(&ctx)?;
        let options = options.to_rust::<SurfaceOptions>()?;
        let surface = options.surface.trim();
        if surface.is_empty() {
            return Err(auto_err("terminal newTab requires a surface id"));
        }
        let handle = wait_for_surface(&authority, surface).await?;
        let snapshot = handle
            .run_command("newTab", json!({}), HOST_TIMEOUT)
            .await
            .map_err(auto_err)?;
        json_to_js(&ctx, &snapshot)
    }

    /// Split the active pane in one native terminal surface.
    #[js_method]
    async fn split(&self, ctx: JSContext, options: JSValue) -> JSResult<JSValue> {
        let authority = authority(&ctx)?;
        let options = options.to_rust::<SplitOptions>()?;
        let surface = options.surface.trim();
        if surface.is_empty() {
            return Err(auto_err("terminal split requires a surface id"));
        }
        let direction = split_direction(options.direction.trim())?;
        let handle = wait_for_surface(&authority, surface).await?;
        let snapshot = handle
            .run_command("split", json!({ "direction": direction }), HOST_TIMEOUT)
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
