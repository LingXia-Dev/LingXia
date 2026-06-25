use super::locate_templates_dir;
use super::template::process_template_dir;
use super::types::ProjectConfig;
use crate::versions::LingXiaVersions;
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::fs;

pub(super) fn create_windows_project(
    config: &ProjectConfig,
    versions: &LingXiaVersions,
) -> Result<()> {
    let windows_dir = config.target_dir.join("windows");
    fs::create_dir_all(&windows_dir)?;

    let templates_base = locate_templates_dir()?;
    let template_dir = templates_base.join("windows");
    if !template_dir.exists() {
        return Err(anyhow!(
            "Windows template not found at: {}",
            template_dir.display()
        ));
    }

    // The native lib crate's package name and directory are both `native`.
    let host_crate_name = super::RUST_LIB_DIR_NAME.to_string();
    let windows_crate_name = format!("{}-windows", config.name);

    let mut vars = HashMap::new();
    vars.insert("PROJECT_NAME".to_string(), config.name.clone());
    vars.insert("HOST_CRATE_NAME".to_string(), host_crate_name);
    vars.insert("WINDOWS_CRATE_NAME".to_string(), windows_crate_name);
    vars.insert("WINDOWS_EXECUTABLE_NAME".to_string(), config.name.clone());
    vars.insert(
        "LINGXIA_VERSION".to_string(),
        versions.lingxia_crate.clone(),
    );

    process_template_dir(&template_dir, &windows_dir, &vars)?;
    println!("  Created Windows host project: windows/");
    Ok(())
}
