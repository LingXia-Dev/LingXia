//! `runner.*` methods: the simulated environment (device preset, orientation,
//! appearance) owned by the runner host.

use lingxia_control_protocol::methods;
use serde::Deserialize;
use serde_json::Value;

pub(crate) fn handle_runner_command(
    handler: &str,
    args: Option<Value>,
) -> Option<Result<Option<Value>, String>> {
    if !handler.starts_with("runner.") {
        return None;
    }
    Some(handle_runner_command_impl(handler, args))
}

#[derive(Deserialize)]
struct RunnerSetArgs {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    landscape: Option<bool>,
    #[serde(default)]
    appearance: Option<String>,
    #[serde(default, rename = "textScale")]
    text_scale: Option<f64>,
    #[serde(default)]
    contrast: Option<String>,
}

fn handle_runner_command_impl(handler: &str, args: Option<Value>) -> Result<Option<Value>, String> {
    match handler {
        methods::runner::PRESETS => serde_json::to_value(lingxia::dev::device_list()?)
            .map(Some)
            .map_err(|err| err.to_string()),
        methods::runner::GET => serde_json::to_value(lingxia::dev::device_get()?)
            .map(Some)
            .map_err(|err| err.to_string()),
        methods::runner::SET => {
            let parsed: RunnerSetArgs =
                serde_json::from_value(args.unwrap_or_else(|| serde_json::json!({})))
                    .map_err(|e| format!("invalid args for {}: {}", handler, e))?;
            let change = lingxia::dev::DeviceChange {
                id: parsed.id,
                landscape: parsed.landscape,
                appearance: parsed.appearance.as_deref().map(str::parse).transpose()?,
                text_scale: parsed.text_scale,
                contrast: parsed.contrast.as_deref().map(str::parse).transpose()?,
            };
            if change.is_empty() {
                return Err(
                    "runner.set requires at least one of id, landscape, appearance, textScale, contrast"
                        .into(),
                );
            }
            serde_json::to_value(crate::util::run_async(lingxia::dev::device_set(change))?)
            .map(Some)
            .map_err(|err| err.to_string())
        }
        other => Err(format!("unknown runner handler: {}", other)),
    }
}
