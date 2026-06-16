//! `lx.shell` — the host-shell owner of top-level, host-declared surfaces.
//!
//! Whereas `lx.navigator` drives the in-app page stack and `lx.surface` opens
//! an lxapp's own asides/floats, `lx.shell` controls the surfaces the host
//! declares in its `ui` config (e.g. the AI-chat panel or the terminal). Only
//! platforms with a host shell that manages declared surfaces (currently macOS)
//! act on these calls; elsewhere they reject with a not-supported error.

use lxapp::{LxApp, LxAppError};
use rong::{HostError, JSContext, JSFunc, JSObject, JSResult, RongJSError};

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let lx = ctx.global().get::<_, JSObject>("lx")?;
    let shell = JSObject::new(ctx);
    // Show / hide / flip a host-declared top-level surface by its `ui` id.
    shell.set("open", JSFunc::new(ctx, shell_open)?)?;
    shell.set("close", JSFunc::new(ctx, shell_close)?)?;
    shell.set("toggle", JSFunc::new(ctx, shell_toggle)?)?;
    lx.set("shell", shell)?;
    Ok(())
}

fn shell_open(ctx: JSContext, id: String) -> JSResult<bool> {
    set_visible(&ctx, &id, true)
}

fn shell_close(ctx: JSContext, id: String) -> JSResult<bool> {
    set_visible(&ctx, &id, false)
}

fn shell_toggle(ctx: JSContext, id: String) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    lxapp.toggle_shell_surface(&id).map_err(shell_error)?;
    Ok(true)
}

fn set_visible(ctx: &JSContext, id: &str, visible: bool) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(ctx)?;
    lxapp
        .set_shell_surface_visible(id, visible)
        .map_err(shell_error)?;
    Ok(true)
}

fn shell_error(err: LxAppError) -> RongJSError {
    HostError::new(rong::error::E_INTERNAL, err.to_string())
        .with_data(rong::err_data!({ code: ("shell_surface_failed") }))
        .into()
}
