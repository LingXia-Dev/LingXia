use crate::util::{png_response, run_async};
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
        handlers::app::SCREENSHOT => {
            let parsed: AppScreenshotArgs = match args {
                Some(value) => serde_json::from_value(value)
                    .map_err(|e| format!("invalid args for {}: {}", handler, e))?,
                None => AppScreenshotArgs::default(),
            };
            let window_id = parsed.window_id;
            let bytes = run_async(lingxia::dev::take_app_screenshot(window_id.as_deref()))?;
            Ok(Some(png_response(
                "app",
                "session",
                &bytes,
                [("window_id", json!(window_id))],
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
