use crate::config::LingXiaConfig;
use anyhow::{Result, anyhow};
use serde_json::{Map, Value};

pub(super) const TERMINAL_ICON_SOURCE: &str = "__lingxia_builtin__/terminal.svg";

pub(super) fn effective_ui_config(
    config: &LingXiaConfig,
    platform: Option<&str>,
) -> Result<Option<Value>> {
    let Some(ui) = config.generated_ui.as_ref() else {
        return Ok(None);
    };
    let mut ui = ui.clone();
    let terminal_enabled = config
        .capabilities
        .as_ref()
        .map(|capabilities| capabilities.terminal)
        .unwrap_or(false);
    if contains_terminal_surface(&ui) && !terminal_enabled {
        return Err(anyhow!(
            "ui contains terminal content but capabilities.terminal is not enabled"
        ));
    }
    if let Some(platform) = platform {
        filter_ui_for_platform(&mut ui, platform)?;
    }
    Ok(Some(ui))
}

fn filter_ui_for_platform(ui: &mut Value, platform: &str) -> Result<()> {
    let obj = ui
        .as_object_mut()
        .ok_or_else(|| anyhow!("ui must be a JSON object"))?;
    let launch = obj
        .get("launch")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("ui.launch must be an object"))?;
    let initial_surface = launch
        .get("initialSurface")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("ui.launch.initialSurface must be a non-empty string"))?
        .to_string();

    let surfaces = obj
        .get_mut("surfaces")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| anyhow!("ui.surfaces must be an array"))?;
    let mut kept_surface_ids = std::collections::HashSet::new();
    let mut filtered_surfaces = Vec::with_capacity(surfaces.len());
    for (index, surface) in surfaces.iter().enumerate() {
        let mut surface = surface.clone();
        let surface_obj = surface
            .as_object_mut()
            .ok_or_else(|| anyhow!("ui.surfaces[{index}] must be an object"))?;
        let id = surface_obj
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("ui.surfaces[{index}].id must be a non-empty string"))?
            .to_string();
        if surface_available_on_platform(surface_obj, platform, &format!("ui.surfaces[{index}]"))? {
            surface_obj.remove("platforms");
            kept_surface_ids.insert(id);
            filtered_surfaces.push(surface);
        }
    }
    if filtered_surfaces.is_empty() {
        return Err(anyhow!(
            "ui.surfaces must contain at least one surface available on {platform}"
        ));
    }
    if !kept_surface_ids.contains(initial_surface.as_str()) {
        return Err(anyhow!(
            "ui.launch.initialSurface '{initial_surface}' is not available on {platform}"
        ));
    }
    *surfaces = filtered_surfaces;

    let activators = obj
        .get_mut("activators")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| anyhow!("ui.activators must be an array"))?;
    let mut filtered_activators = Vec::with_capacity(activators.len());
    for (index, activator) in activators.iter().enumerate() {
        let activator_obj = activator
            .as_object()
            .ok_or_else(|| anyhow!("ui.activators[{index}] must be an object"))?;
        let Some(action_surface) = activator_obj
            .get("action")
            .and_then(Value::as_object)
            .and_then(|action| action.get("surface"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if !kept_surface_ids.contains(action_surface) {
            continue;
        }
        if let Some(host_surface) = activator_obj
            .get("hostSurface")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            && !kept_surface_ids.contains(host_surface)
        {
            continue;
        }
        filtered_activators.push(activator.clone());
    }
    *activators = filtered_activators;

    Ok(())
}
fn surface_available_on_platform(
    surface: &Map<String, Value>,
    platform: &str,
    context: &str,
) -> Result<bool> {
    let Some(platforms) = surface.get("platforms") else {
        return Ok(true);
    };
    let platforms = platforms
        .as_array()
        .ok_or_else(|| anyhow!("{context}.platforms must be an array"))?;
    if platforms.is_empty() {
        return Ok(true);
    }
    for (index, value) in platforms.iter().enumerate() {
        let value = value
            .as_str()
            .ok_or_else(|| anyhow!("{context}.platforms[{index}] must be a string"))?;
        if value.eq_ignore_ascii_case(platform) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn contains_terminal_surface(ui: &Value) -> bool {
    ui.get("surfaces")
        .and_then(Value::as_array)
        .map(|surfaces| {
            surfaces.iter().any(|surface| {
                surface
                    .get("content")
                    .and_then(|content| content.get("kind"))
                    .and_then(Value::as_str)
                    == Some("terminal")
            })
        })
        .unwrap_or(false)
}
