use super::icons;
use super::locate_templates_dir;
use super::template::process_template_dir;
use super::types::{LxAppInfo, ProjectConfig};
use crate::versions::LingXiaVersions;
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub(super) fn create_lxapp_from_template(
    target_dir: &Path,
    project_name: &str,
    product_name: &str,
    framework: &str,
    versions: &LingXiaVersions,
    lingxia_core_version: &str,
    lingxia_types_version: &str,
    lingxia_components_version: &str,
) -> Result<()> {
    if target_dir.exists() {
        return Err(anyhow!(
            "Directory '{}' already exists",
            target_dir.display()
        ));
    }

    fs::create_dir_all(target_dir)?;

    let templates_base = locate_templates_dir()?;
    let template_dir = templates_base
        .join("lxapp-create")
        .join(framework.to_lowercase());
    if !template_dir.exists() {
        return Err(anyhow!(
            "LxApp template not found at: {}",
            template_dir.display()
        ));
    }

    let slug = slugify(project_name);
    let mut vars = HashMap::new();
    vars.insert("APP_PACKAGE_NAME".to_string(), slug.clone());
    vars.insert("APP_ID".to_string(), slug);
    vars.insert("APP_DISPLAY_NAME".to_string(), product_name.to_string());

    vars.insert("RONG_VERSION".to_string(), versions.rong.clone());
    vars.insert(
        "LINGXIA_CORE_VERSION".to_string(),
        lingxia_core_version.to_string(),
    );
    vars.insert(
        "LINGXIA_TYPES_VERSION".to_string(),
        lingxia_types_version.to_string(),
    );
    vars.insert(
        "LINGXIA_COMPONENTS_VERSION".to_string(),
        lingxia_components_version.to_string(),
    );

    process_template_dir(&template_dir, target_dir, &vars)?;
    icons::ensure_lxapp_public_icon(target_dir)?;

    Ok(())
}

pub(super) fn create_lxapp_project(
    config: &ProjectConfig,
    lxapp_dir_name: &str,
    framework: &str,
    versions: &LingXiaVersions,
    lingxia_core_version: &str,
    lingxia_types_version: &str,
    lingxia_components_version: &str,
) -> Result<LxAppInfo> {
    let lxapp_dir_name = lxapp_dir_name.trim();
    let lxapp_dir = config.target_dir.join(lxapp_dir_name);
    println!("  Creating LxApp project...");
    create_lxapp_from_template(
        &lxapp_dir,
        lxapp_dir_name,
        &config.product_name,
        framework,
        versions,
        lingxia_core_version,
        lingxia_types_version,
        lingxia_components_version,
    )?;
    Ok(LxAppInfo {
        app_id: lxapp_dir_name.to_string(),
    })
}

pub(super) fn slugify(value: &str) -> String {
    let mut out = String::new();
    let mut last_was_dash = false;

    for ch in value.trim().chars() {
        let ch = ch.to_ascii_lowercase();
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_was_dash = false;
        } else if !last_was_dash {
            out.push('-');
            last_was_dash = true;
        }
    }

    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "lingxia-app".to_string()
    } else {
        out
    }
}
