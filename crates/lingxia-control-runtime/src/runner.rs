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
    #[serde(default)]
    capsule: Option<bool>,
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
            let parsed: RunnerSetArgs = match args {
                Some(value) => serde_json::from_value(value)
                    .map_err(|e| format!("invalid args for {}: {}", handler, e))?,
                None => RunnerSetArgs {
                    id: None,
                    landscape: None,
                    appearance: None,
                    capsule: None,
                },
            };
            if parsed.id.is_none()
                && parsed.landscape.is_none()
                && parsed.appearance.is_none()
                && parsed.capsule.is_none()
            {
                return Err(
                    "runner.set requires at least one of id, landscape, appearance, capsule".into(),
                );
            }
            let appearance = parsed
                .appearance
                .as_deref()
                .map(str::parse::<lingxia::dev::Appearance>)
                .transpose()?;
            serde_json::to_value(lingxia::dev::device_set(
                parsed.id.as_deref(),
                parsed.landscape,
                appearance,
                parsed.capsule,
            )?)
            .map(Some)
            .map_err(|err| err.to_string())
        }
        other => Err(format!("unknown runner handler: {}", other)),
    }
}
