use super::locate_templates_dir;
use super::template::process_template_dir;
use super::types::ProjectConfig;
use super::validation::swift_target_name_from_project_name;
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::fs;

pub(super) fn create_apple_project(
    config: &ProjectConfig,
    output_dir_name: &str,
    template_dir_name: &str,
    platform_label: &str,
) -> Result<()> {
    let output_dir = config.target_dir.join(output_dir_name);
    fs::create_dir_all(&output_dir)?;

    let templates_base = locate_templates_dir()?;
    let template_dir = templates_base.join(template_dir_name);
    if !template_dir.exists() {
        return Err(anyhow!(
            "{platform_label} template not found at: {}",
            template_dir.display()
        ));
    }

    let swift_target_name = swift_target_name_from_project_name(&config.name);
    let mut vars = HashMap::new();
    vars.insert("PROJECT_NAME".to_string(), config.name.clone());
    vars.insert("PRODUCT_NAME".to_string(), config.product_name.clone());
    vars.insert("PACKAGE_ID".to_string(), config.package_id.clone());
    vars.insert("SWIFT_TARGET_NAME".to_string(), swift_target_name);

    process_template_dir(&template_dir, &output_dir, &vars)?;
    println!("  Created {platform_label} project structure");
    Ok(())
}
