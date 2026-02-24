use crate::config::HarmonyConfig;
use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const MODULE_JSON5_REL_PATH: &str = "entry/src/main/module.json5";
const WRITE_IMAGEVIDEO_PERMISSION: &str = "ohos.permission.WRITE_IMAGEVIDEO";
const MANAGED_ACL_PERMISSIONS: &[&str] = &[WRITE_IMAGEVIDEO_PERMISSION];

pub fn resolve_harmony_dir(
    project_root: &Path,
    _harmony_config: Option<&HarmonyConfig>,
) -> Result<PathBuf> {
    let harmony_dir = project_root.join("harmony");
    if harmony_dir.join("build-profile.json5").exists() {
        return Ok(harmony_dir);
    }

    if project_root.join("build-profile.json5").exists() {
        return Ok(project_root.to_path_buf());
    }

    Err(anyhow!(
        "HarmonyOS project not found.\n\
         Expected build-profile.json5 in: {}/harmony/",
        project_root.display()
    ))
}

pub fn resolve_harmony_rawfile_dir(project_root: &Path) -> Result<PathBuf> {
    let harmony_dir = project_root.join("harmony");
    if harmony_dir.exists() {
        Ok(harmony_dir.join("entry/src/main/resources/rawfile"))
    } else {
        Ok(project_root.join("entry/src/main/resources/rawfile"))
    }
}

pub fn sync_acl_permissions(harmony_dir: &Path, acl_permissions: &[String]) -> Result<bool> {
    let acl_permissions = dedup_acl_permissions(acl_permissions);

    let module_path = harmony_dir.join(MODULE_JSON5_REL_PATH);
    let content = std::fs::read_to_string(&module_path)
        .with_context(|| format!("Failed to read {}", module_path.display()))?;

    let mut root: Value = json5::from_str(&content)
        .with_context(|| format!("Failed to parse {}", module_path.display()))?;
    let module_obj = root
        .get_mut("module")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| anyhow!("Invalid module.json5: missing top-level `module` object"))?;
    let default_ability = infer_default_ability_name(module_obj);

    let request_permissions = module_obj
        .entry("requestPermissions".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    let permissions_array = request_permissions
        .as_array_mut()
        .ok_or_else(|| anyhow!("Invalid module.json5: `requestPermissions` must be an array"))?;

    let desired_set: HashSet<&str> = acl_permissions.iter().map(String::as_str).collect();
    let mut existing_names = HashSet::new();
    let mut changed = false;

    for entry in permissions_array.iter_mut() {
        let Some(name) = permission_name(entry).map(ToOwned::to_owned) else {
            continue;
        };
        existing_names.insert(name.clone());
        if !desired_set.contains(name.as_str()) {
            continue;
        }
        let before = entry.clone();
        normalize_permission_entry(entry, &name, &default_ability);
        if *entry != before {
            changed = true;
        }
    }

    let mut seen_managed_names = HashSet::new();
    permissions_array.retain(|entry| {
        let Some(name) = permission_name(entry) else {
            return true;
        };
        if is_managed_acl_permission(name) && !desired_set.contains(name) {
            changed = true;
            return false;
        }
        if is_managed_acl_permission(name) && !seen_managed_names.insert(name.to_string()) {
            changed = true;
            return false;
        }
        true
    });

    for acl in &acl_permissions {
        if existing_names.contains(acl) {
            continue;
        }
        permissions_array.push(default_permission_entry(acl, &default_ability));
        changed = true;
    }

    if !changed {
        return Ok(false);
    }

    let updated =
        serde_json::to_string_pretty(&root).context("Failed to serialize module.json5")?;

    std::fs::write(&module_path, format!("{updated}\n"))
        .with_context(|| format!("Failed to write {}", module_path.display()))?;
    Ok(true)
}

fn infer_default_ability_name(module_obj: &serde_json::Map<String, Value>) -> String {
    module_obj
        .get("abilities")
        .and_then(Value::as_array)
        .and_then(|abilities| {
            abilities
                .iter()
                .find_map(|ability| ability.get("name").and_then(Value::as_str))
        })
        .map(str::to_string)
        .unwrap_or_else(|| "EntryAbility".to_string())
}

fn normalize_permission_entry(entry: &mut Value, permission: &str, default_ability: &str) {
    if !entry.is_object() {
        *entry = default_permission_entry(permission, default_ability);
        return;
    }

    let Some(obj) = entry.as_object_mut() else {
        *entry = default_permission_entry(permission, default_ability);
        return;
    };

    obj.insert("name".to_string(), Value::String(permission.to_string()));

    if permission != WRITE_IMAGEVIDEO_PERMISSION {
        return;
    }

    match obj.get("reason").and_then(Value::as_str) {
        Some(reason) if !reason.trim().is_empty() => {}
        _ => {
            obj.insert(
                "reason".to_string(),
                Value::String("$string:lx_permission_media_reason".to_string()),
            );
        }
    }
    ensure_used_scene(obj, default_ability);
}

fn dedup_acl_permissions(acl_permissions: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut unique = Vec::new();

    for permission in acl_permissions {
        let trimmed = permission.trim();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            unique.push(trimmed.to_string());
        }
    }

    unique
}

fn permission_name(entry: &Value) -> Option<&str> {
    match entry {
        Value::Object(obj) => obj.get("name").and_then(Value::as_str),
        Value::String(name) => Some(name.as_str()),
        _ => None,
    }
}

fn default_permission_entry(permission: &str, default_ability: &str) -> Value {
    match permission {
        WRITE_IMAGEVIDEO_PERMISSION => json!({
            "name": WRITE_IMAGEVIDEO_PERMISSION,
            "reason": "$string:lx_permission_media_reason",
            "usedScene": {
                "abilities": [default_ability],
                "when": "inuse"
            }
        }),
        _ => json!({ "name": permission }),
    }
}

fn ensure_used_scene(permission_obj: &mut serde_json::Map<String, Value>, default_ability: &str) {
    let used_scene = permission_obj
        .entry("usedScene".to_string())
        .or_insert_with(|| {
            json!({
                "abilities": [default_ability],
                "when": "inuse"
            })
        });

    let Value::Object(used_scene_obj) = used_scene else {
        *used_scene = json!({
            "abilities": [default_ability],
            "when": "inuse"
        });
        return;
    };

    let abilities = used_scene_obj
        .entry("abilities".to_string())
        .or_insert_with(|| Value::Array(vec![Value::String(default_ability.to_string())]));
    match abilities {
        Value::Array(items) => {
            if items.is_empty() {
                items.push(Value::String(default_ability.to_string()));
            }
        }
        _ => {
            *abilities = Value::Array(vec![Value::String(default_ability.to_string())]);
        }
    }

    match used_scene_obj.get("when").and_then(Value::as_str) {
        Some(when) if !when.trim().is_empty() => {}
        _ => {
            used_scene_obj.insert("when".to_string(), Value::String("inuse".to_string()));
        }
    }
}

fn is_managed_acl_permission(permission: &str) -> bool {
    MANAGED_ACL_PERMISSIONS.contains(&permission)
}

pub fn read_bundle_name(harmony_dir: &Path) -> Result<String> {
    let app_json5_path = harmony_dir.join("AppScope/app.json5");
    if !app_json5_path.exists() {
        return Err(anyhow!(
            "AppScope/app.json5 not found in {}",
            harmony_dir.display()
        ));
    }

    let content = std::fs::read_to_string(&app_json5_path)
        .with_context(|| format!("Failed to read {}", app_json5_path.display()))?;

    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.contains("bundleName") {
            continue;
        }

        let Some(colon_pos) = trimmed.find(':') else {
            continue;
        };
        let value_part = trimmed[colon_pos + 1..].trim();
        let value = value_part
            .trim_matches(|c: char| c == '"' || c == '\'' || c == ',' || c.is_whitespace());
        if !value.is_empty() {
            return Ok(value.to_string());
        }
    }

    Err(anyhow!(
        "bundleName not found in {}",
        app_json5_path.display()
    ))
}

pub fn generate_icons(
    project_root: &Path,
    source_icon: &Path,
    background_color: &str,
    harmony_config: Option<&HarmonyConfig>,
) -> Result<()> {
    let harmony_dir = resolve_harmony_dir(project_root, harmony_config)?;
    crate::appicon::generate_harmony_icons(source_icon, &harmony_dir, background_color)
}
