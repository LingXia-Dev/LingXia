use super::android;
use super::harmony;
use super::ios;
use super::locate_templates_dir;
use super::macos;
use super::template::process_template_dir;
use super::types::{Platform, ProjectConfig};
use crate::versions::LingXiaVersions;
use anyhow::{Result, anyhow};
use colored::Colorize;
use std::collections::HashMap;
use std::fs;

pub(super) fn create_project(config: &ProjectConfig, versions: &LingXiaVersions) -> Result<()> {
    if config.target_dir.exists() {
        return Err(anyhow!(
            "Directory '{}' already exists",
            config.target_dir.display()
        ));
    }

    println!();
    println!("{}", "Creating project structure...".bold());

    fs::create_dir_all(&config.target_dir)?;
    create_root_gitignore(config)?;

    let mut created_any = false;
    for platform in &config.platforms {
        match platform {
            Platform::Android => {
                android::create_android_project(config, versions)?;
                created_any = true;
            }
            Platform::Ios => {
                ios::create_ios_project(config)?;
                created_any = true;
            }
            Platform::Macos => {
                macos::create_macos_project(config)?;
                created_any = true;
            }
            Platform::Harmony => {
                harmony::create_harmony_project(config)?;
                created_any = true;
            }
        }
    }

    if !created_any {
        return Err(anyhow!("No platforms selected"));
    }

    Ok(())
}

fn create_root_gitignore(config: &ProjectConfig) -> Result<()> {
    let templates_base = locate_templates_dir()?;
    let template_dir = templates_base.join("native-root");
    if !template_dir.exists() {
        return Err(anyhow!(
            "Native root template not found at: {}",
            template_dir.display()
        ));
    }

    let vars: HashMap<String, String> = HashMap::new();
    process_template_dir(&template_dir, &config.target_dir, &vars)?;
    Ok(())
}

pub(super) fn create_rust_library(
    config: &ProjectConfig,
    versions: &LingXiaVersions,
) -> Result<()> {
    let project_root = &config.target_dir;
    let lib_name = format!("{}-lib", config.name);
    let lib_dir = project_root.join(&lib_name);

    // Create library directory
    fs::create_dir_all(&lib_dir)?;

    // Locate templates directory
    let templates_base = locate_templates_dir()?;
    let template_dir = templates_base.join("rust-lib");

    if !template_dir.exists() {
        return Err(anyhow!(
            "Rust library template not found at: {}",
            template_dir.display()
        ));
    }

    // Build variable substitution map
    let mut vars = HashMap::new();
    vars.insert("PROJECT_NAME".to_string(), lib_name.clone());
    vars.insert("PACKAGE_ID".to_string(), config.package_id.clone());

    // Convert package ID to underscore format for JNI function names
    // e.g., com.example.mouke -> com_example_mouke
    let package_id_underscore = config.package_id.replace('.', "_");
    vars.insert("PACKAGE_ID_UNDERSCORE".to_string(), package_id_underscore);

    vars.insert("CRATE_VERSION".to_string(), versions.rust_crate.clone());

    // Process all template files into {project}-lib/ directory
    process_template_dir(&template_dir, &lib_dir, &vars)?;

    println!("  Created Rust library: {}", lib_name);

    Ok(())
}
