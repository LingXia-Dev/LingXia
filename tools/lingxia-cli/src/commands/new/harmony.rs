use super::locate_templates_dir;
use super::template::process_template_dir;
use super::types::ProjectConfig;
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::fs;

pub fn create_harmony_project(config: &ProjectConfig) -> Result<()> {
    let harmony_dir = config.target_dir.join("harmony");
    fs::create_dir_all(&harmony_dir)?;

    // Locate templates directory
    let templates_base = locate_templates_dir()?;
    let template_dir = templates_base.join("harmony-native");

    if !template_dir.exists() {
        return Err(anyhow!(
            "HarmonyOS template not found at: {}",
            template_dir.display()
        ));
    }

    // Build variable substitution map
    let mut vars = HashMap::new();
    vars.insert("PROJECT_NAME".to_string(), config.name.clone());
    vars.insert("PRODUCT_NAME".to_string(), config.product_name.clone());
    vars.insert("PACKAGE_ID".to_string(), config.package_id.clone());

    // Process all template files into harmony/ subdirectory
    process_template_dir(&template_dir, &harmony_dir, &vars)?;

    println!("  Created HarmonyOS project structure");
    Ok(())
}
