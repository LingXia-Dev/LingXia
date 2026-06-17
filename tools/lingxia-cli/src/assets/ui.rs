use crate::config::LingXiaConfig;
use anyhow::{Result, anyhow};
use serde_json::{Map, Value, json};

const TERMINAL_SURFACE_ID: &str = "terminal";
const TERMINAL_ACTIVATOR_ID: &str = "terminalSidebar";
pub(super) const TERMINAL_ICON_SOURCE: &str = "__lingxia_builtin__/terminal.svg";

pub(super) fn effective_ui_config(config: &LingXiaConfig) -> Result<Option<Value>> {
    let Some(ui) = config.ui.as_ref() else {
        return Ok(None);
    };
    let mut ui = ui.clone();
    let terminal_enabled = config
        .capabilities
        .as_ref()
        .map(|capabilities| capabilities.terminal)
        .unwrap_or(false);
    if terminal_enabled {
        let edge = config
            .capabilities
            .as_ref()
            .and_then(|capabilities| capabilities.terminal_edge.as_deref())
            .unwrap_or("bottom");
        if edge != "bottom" && edge != "top" {
            return Err(anyhow!(
                "capabilities.terminalEdge must be 'top' or 'bottom', got '{edge}'"
            ));
        }
        add_terminal_ui(&mut ui, edge)?;
    } else if contains_terminal_surface(&ui) {
        return Err(anyhow!(
            "ui contains terminal content but capabilities.terminal is not enabled"
        ));
    }
    Ok(Some(ui))
}

fn add_terminal_ui(ui: &mut Value, edge: &str) -> Result<()> {
    let obj = ui
        .as_object_mut()
        .ok_or_else(|| anyhow!("ui must be a JSON object"))?;
    let root_surface = root_surface_id(obj)?;

    let surfaces = obj
        .get_mut("surfaces")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| anyhow!("ui.surfaces must be an array"))?;
    if !contains_id(surfaces, TERMINAL_SURFACE_ID) {
        surfaces.push(json!({
            "id": TERMINAL_SURFACE_ID,
            "role": "aside",
            "attachTo": root_surface,
            "edge": edge,
            "size": { "height": 320 },
            "content": {
                "kind": "terminal"
            }
        }));
    }

    let activators = ensure_array_field(obj, "activators")?;
    if !contains_id(activators, TERMINAL_ACTIVATOR_ID) {
        activators.push(json!({
            "id": TERMINAL_ACTIVATOR_ID,
            "kind": "sidebarItem",
            "hostSurface": root_surface,
            "label": "Terminal",
            "icon": TERMINAL_ICON_SOURCE,
            "action": {
                "kind": "toggleSurface",
                "surface": TERMINAL_SURFACE_ID
            }
        }));
    }

    Ok(())
}

fn root_surface_id(ui: &Map<String, Value>) -> Result<String> {
    let surfaces = ui
        .get("surfaces")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("ui.surfaces must be an array"))?;

    let initial_surface = ui
        .get("launch")
        .and_then(Value::as_object)
        .and_then(|launch| launch.get("initialSurface"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty());
    if let Some(initial_surface) = initial_surface
        && surface_is_root(surfaces, initial_surface)
    {
        return Ok(initial_surface.to_string());
    }

    for surface in surfaces {
        if surface_is_root_value(surface)
            && let Some(id) = surface
                .as_object()
                .and_then(|surface| surface.get("id"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|id| !id.is_empty())
        {
            return Ok(id.to_string());
        }
    }

    ui.get("launch")
        .and_then(Value::as_object)
        .and_then(|launch| launch.get("initialSurface"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| anyhow!("ui.launch.initialSurface must be a non-empty string"))
}

fn ensure_array_field<'a>(
    obj: &'a mut Map<String, Value>,
    key: &str,
) -> Result<&'a mut Vec<Value>> {
    if !obj.contains_key(key) {
        obj.insert(key.to_string(), Value::Array(Vec::new()));
    }
    obj.get_mut(key)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| anyhow!("ui.{key} must be an array"))
}

fn surface_is_root(surfaces: &[Value], id: &str) -> bool {
    surfaces.iter().any(|surface| {
        surface
            .as_object()
            .and_then(|surface| surface.get("id"))
            .and_then(Value::as_str)
            .map(str::trim)
            == Some(id)
            && surface_is_root_value(surface)
    })
}

fn surface_is_root_value(surface: &Value) -> bool {
    surface
        .as_object()
        .and_then(|surface| surface.get("role"))
        .and_then(Value::as_str)
        .is_some_and(|role| matches!(role, "main" | "float"))
}

fn contains_id(items: &[Value], id: &str) -> bool {
    items.iter().any(|item| {
        item.get("id")
            .and_then(Value::as_str)
            .map(|value| value == id)
            .unwrap_or(false)
    })
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
