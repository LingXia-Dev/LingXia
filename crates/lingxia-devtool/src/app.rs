use crate::util::{png_dimensions, png_response, run_async};
use lingxia_devtool_protocol::handlers;
use serde::Deserialize;
use serde_json::{Value, json};

pub(crate) fn handle_app_command(
    handler: &str,
    args: Option<Value>,
) -> Option<Result<Option<Value>, String>> {
    if !handler.starts_with("app.") {
        return None;
    }
    Some(handle_app_command_impl(handler, args))
}

#[derive(Default, Deserialize)]
struct AppScreenshotArgs {
    #[serde(default)]
    window_id: Option<String>,
}

fn handle_app_command_impl(handler: &str, args: Option<Value>) -> Result<Option<Value>, String> {
    match handler {
        handlers::app::DOCTOR => Ok(Some(build_app_doctor())),
        handlers::app::SCREENSHOT => {
            let parsed: AppScreenshotArgs = match args {
                Some(value) => serde_json::from_value(value)
                    .map_err(|e| format!("invalid args for {}: {}", handler, e))?,
                None => AppScreenshotArgs::default(),
            };
            let window_id = parsed.window_id;
            let (window, bytes) = run_async(lingxia::dev::take_app_screenshot_with_info(
                window_id.as_deref(),
            ))?;
            let (pixel_width, pixel_height) = png_dimensions(&bytes).unwrap_or_default();
            let scale_x =
                (window.width > 0).then(|| f64::from(pixel_width) / f64::from(window.width));
            let scale_y =
                (window.height > 0).then(|| f64::from(pixel_height) / f64::from(window.height));
            Ok(Some(png_response(
                "app",
                app_window_coordinate_space(),
                &bytes,
                [
                    ("window_id", json!(window.id)),
                    ("window", json!(window)),
                    ("content_width", json!(window.width)),
                    ("content_height", json!(window.height)),
                    ("scale_x", json!(scale_x)),
                    ("scale_y", json!(scale_y)),
                ],
            )))
        }
        handlers::app::WINDOWS => {
            let windows = run_async(lingxia::dev::list_app_windows())?;
            serde_json::to_value(windows)
                .map(Some)
                .map_err(|err| err.to_string())
        }
        handlers::app::MOUSE => {
            let request: lingxia::dev::AppMouseRequest = match args {
                Some(value) => serde_json::from_value(value)
                    .map_err(|e| format!("invalid args for {}: {}", handler, e))?,
                None => return Err(format!("missing args for {}", handler)),
            };
            let result = run_async(lingxia::dev::perform_app_mouse(request))?;
            serde_json::to_value(result)
                .map(Some)
                .map_err(|err| err.to_string())
        }
        handlers::app::KEYBOARD => {
            let request: lingxia::dev::AppKeyboardRequest = match args {
                Some(value) => serde_json::from_value(value)
                    .map_err(|e| format!("invalid args for {}: {}", handler, e))?,
                None => return Err(format!("missing args for {}", handler)),
            };
            let result = run_async(lingxia::dev::perform_app_keyboard(request))?;
            serde_json::to_value(result)
                .map(Some)
                .map_err(|err| err.to_string())
        }
        other => Err(format!("unknown app handler: {}", other)),
    }
}

fn build_app_doctor() -> Value {
    let platform = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "android") {
        "android"
    } else if cfg!(target_os = "ios") {
        "ios"
    } else if cfg!(all(target_os = "linux", target_env = "ohos")) {
        "harmony"
    } else {
        "unknown"
    };
    let desktop = cfg!(any(target_os = "windows", target_os = "macos"));
    let modifiers = if cfg!(target_os = "windows") {
        json!({
            "supported": true,
            "reliability": "best_effort",
            "reason": "message injection cannot update GetKeyState for chorded shortcuts",
        })
    } else {
        json!({ "supported": desktop, "reliability": if desktop { "native" } else { "unsupported" } })
    };
    json!({
        "target": "app",
        "platform": platform,
        "capabilities": {
            "windows": { "supported": true },
            "screenshot": { "supported": true },
            "mouse": { "supported": desktop },
            "keyboard": { "supported": desktop },
            "keyboard_modifiers": modifiers,
        },
        "coordinate_spaces": {
            "window": app_window_coordinate_space(),
        }
    })
}

#[cfg(target_os = "windows")]
fn app_window_coordinate_space() -> &'static str {
    "window_content_pixels"
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn app_window_coordinate_space() -> &'static str {
    "window_content_points"
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios")))]
fn app_window_coordinate_space() -> &'static str {
    "screen_pixels"
}
