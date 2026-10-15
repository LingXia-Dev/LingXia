//! `t.app.screenshot()` and `t.device.screenshot()` for a host run.
//!
//! The PNG never enters JS (the runtime's `btoa` would re-encode it). An app
//! capture is taken here, in process, which needs no permission on any
//! platform; a device capture is taken here on a desktop host and by the dev
//! server, outside the app, on a phone. Either way the dev server stages the
//! file under the run for `lxdev` to copy into the report, and JS receives
//! only its path and size.

use lingxia_control_protocol::dev_session::{Capture, CaptureArgs};
use lingxia_control_protocol::methods::session::capture;
use lingxia_platform::traits::device::Device;
use lingxia_platform::traits::screenshot::AppScreenshot;
use rong::{HostError, IntoJSObject, JSResult};

const UNSUPPORTED: &str = "E_CAPTURE_UNSUPPORTED";
#[cfg(any(target_os = "macos", target_os = "windows"))]
const PERMISSION: &str = "E_CAPTURE_PERMISSION";
const FAILED: &str = "E_CAPTURE_FAILED";

/// What a capture resolves with in JS.
#[derive(Debug, IntoJSObject)]
pub(super) struct Shot {
    path: String,
    width: f64,
    height: f64,
    scale: f64,
}

impl From<Capture> for Shot {
    fn from(capture: Capture) -> Self {
        Self {
            path: capture.path,
            width: f64::from(capture.width),
            height: f64::from(capture.height),
            scale: capture.scale,
        }
    }
}

fn error(code: &'static str, message: impl Into<String>) -> rong::RongJSError {
    HostError::new(code, message.into()).into()
}

fn platform() -> JSResult<std::sync::Arc<lingxia_platform::Platform>> {
    lxapp::get_platform().ok_or_else(|| error(FAILED, "the host platform is not initialized"))
}

/// Without a dev session nothing can stage the file for the report.
fn require_session() -> JSResult<()> {
    if crate::network::companion::upstream().is_none() {
        return Err(error(
            UNSUPPORTED,
            "this host has no dev session connection (`lingxia dev`) to stage captures",
        ));
    }
    Ok(())
}

fn upstream_error(err: crate::network::companion::UpstreamError) -> rong::RongJSError {
    let code = if err.code == capture::UNSUPPORTED {
        UNSUPPORTED
    } else {
        FAILED
    };
    error(code, err.message)
}

fn platform_error(err: lingxia_platform::error::PlatformError) -> rong::RongJSError {
    match err {
        lingxia_platform::error::PlatformError::NotSupported(message) => {
            error(UNSUPPORTED, message)
        }
        other => error(FAILED, other.to_string()),
    }
}

fn args(run_id: String, name: String, scale: f64) -> serde_json::Value {
    serde_json::to_value(CaptureArgs {
        run_id,
        name,
        scale,
    })
    .unwrap_or_default()
}

fn staged(value: serde_json::Value) -> JSResult<Shot> {
    serde_json::from_value::<Capture>(value)
        .map(Shot::from)
        .map_err(|err| {
            error(
                FAILED,
                format!("the dev server answered an unreadable capture: {err}"),
            )
        })
}

async fn put(run_id: String, name: String, scale: f64, png: Vec<u8>) -> JSResult<Shot> {
    let value = crate::network::companion::request_with_payload(
        capture::PUT,
        args(run_id, name, scale),
        Some(png),
    )
    .await
    .map_err(upstream_error)?;
    staged(value)
}

/// Everything the host draws in the window showing `surface`, or the current
/// page's window.
pub(super) async fn app_screenshot(
    run_id: String,
    name: String,
    surface: Option<String>,
) -> JSResult<Shot> {
    require_session()?;
    if let Some(surface) = surface.as_deref() {
        require_surface(surface)?;
    }
    let platform = platform()?;
    let window = platform
        .capture_window_id(surface.as_deref())
        .await
        .map_err(platform_error)?;
    let png = platform
        .take_app_screenshot(window.as_deref())
        .await
        .map_err(platform_error)?;
    put(run_id, name, platform.screen_info().scale, png).await
}

/// A surface id `t.app.surfaceLayout()` lists: a main, an aside or a float.
fn require_surface(surface: &str) -> JSResult<()> {
    let app = crate::resolve::resolve_lxapp_by_id("current")?;
    let layout = app
        .surface_derived_layout()
        .ok_or_else(|| error(UNSUPPORTED, "this host has no surface layout"))?;
    let known = layout.mains.iter().any(|id| id == surface)
        || layout.asides.iter().any(|aside| aside.id == surface)
        || layout
            .aside_slots
            .iter()
            .any(|slot| slot.children.iter().any(|id| id == surface))
        || layout.floats.iter().any(|float| float.id == surface);
    if known {
        Ok(())
    } else {
        Err(HostError::new(
            "E_AUTOMATION",
            format!("no surface {surface:?} in the current layout (see t.app.surfaceLayout())"),
        )
        .into())
    }
}

/// The whole display, system UI and other apps included.
pub(super) async fn device_screenshot(run_id: String, name: String) -> JSResult<Shot> {
    require_session()?;
    let scale = platform()?.screen_info().scale;
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let png = display_png()?;
        put(run_id, name, scale, png).await
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        use lingxia_control_protocol::methods::session::device;
        let value =
            crate::network::companion::request(device::SCREENSHOT, args(run_id, name, scale))
                .await
                .map_err(upstream_error)?;
        staged(value)
    }
}

/// The screen, captured by this process: on macOS the Screen Recording grant
/// is the one TCC attributes to it.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn display_png() -> JSResult<Vec<u8>> {
    use lingxia_device_io::{CaptureTarget, Error, capture};
    capture::snapshot(CaptureTarget::Screen)
        .map(|capture| capture.png)
        .map_err(|err| match err {
            Error::Permission(message) => error(PERMISSION, message),
            Error::Unsupported(message) => error(UNSUPPORTED, message),
            other => error(FAILED, other.to_string()),
        })
}
