use lingxia_devtool_protocol::handlers;
use serde::Deserialize;
use serde_json::Value;

pub(crate) fn handle_lxapp_device_command(
    handler: &str,
    args: Option<Value>,
) -> Option<Result<Option<Value>, String>> {
    if !handler.starts_with("lxapp.device.") {
        return None;
    }
    Some(handle_lxapp_device_command_impl(handler, args))
}

#[derive(Deserialize)]
struct DeviceSetArgs {
    id: String,
    #[serde(default)]
    landscape: Option<bool>,
}

fn handle_lxapp_device_command_impl(
    handler: &str,
    args: Option<Value>,
) -> Result<Option<Value>, String> {
    match handler {
        handlers::lxapp_device::LIST => serde_json::to_value(lingxia::dev::device_list()?)
            .map(Some)
            .map_err(|err| err.to_string()),
        handlers::lxapp_device::GET => serde_json::to_value(lingxia::dev::device_get()?)
            .map(Some)
            .map_err(|err| err.to_string()),
        handlers::lxapp_device::SET => {
            let parsed: DeviceSetArgs = match args {
                Some(value) => serde_json::from_value(value)
                    .map_err(|e| format!("invalid args for {}: {}", handler, e))?,
                None => return Err(format!("missing args for {}", handler)),
            };
            serde_json::to_value(lingxia::dev::device_set(&parsed.id, parsed.landscape)?)
                .map(Some)
                .map_err(|err| err.to_string())
        }
        other => Err(format!("unknown lxapp.device handler: {}", other)),
    }
}
