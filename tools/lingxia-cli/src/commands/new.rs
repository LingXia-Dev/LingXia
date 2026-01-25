mod template;
mod validation;

use crate::config::{AndroidConfig, LingXiaConfig, ProjectConfig as ConfigProjectConfig};
use anyhow::{anyhow, Result};
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use template::process_template_dir;
use validation::{validate_package_id, validate_project_name};

const DEFAULT_PACKAGE_PREFIX: &str = "com.example";

/// Locate the templates directory
fn locate_templates_dir() -> Result<PathBuf> {
    // 1. Check environment variable
    if let Ok(dir) = env::var("LINGXIA_TEMPLATES_DIR") {
        let path = PathBuf::from(dir);
        if path.exists() {
            return Ok(path);
        }
    }

    // 2. Get executable path
    let exe_path = env::current_exe()?;
    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| anyhow!("Failed to get executable directory"))?;

    // Try npm distribution structure: bin/lingxia -> ../templates
    let npm_templates = exe_dir.parent().map(|p| p.join("templates"));
    if let Some(ref path) = npm_templates {
        if path.exists() {
            return Ok(path.clone());
        }
    }

    // Try development structure: target/debug/lingxia -> ../../../tools/lingxia-cli/templates
    let dev_templates = exe_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("tools/lingxia-cli/templates"));
    if let Some(ref path) = dev_templates {
        if path.exists() {
            return Ok(path.clone());
        }
    }

    Err(anyhow!(
        "Templates directory not found. Searched:\n\
         - Environment variable: LINGXIA_TEMPLATES_DIR\n\
         - NPM distribution: {:?}\n\
         - Development: {:?}",
        npm_templates,
        dev_templates
    ))
}

#[derive(Debug)]
struct ProjectConfig {
    name: String,
    project_type: ProjectType,
    platform: Platform,
    package_id: String,
    target_dir: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ProjectType {
    NativeApp,
    LxApp,
}

impl ProjectType {
    fn as_str(&self) -> &str {
        match self {
            ProjectType::NativeApp => "native-app",
            ProjectType::LxApp => "lxapp",
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "native-app" | "native" => Some(ProjectType::NativeApp),
            "lxapp" | "miniapp" => Some(ProjectType::LxApp),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Platform {
    Android,
    Ios,
    Harmony,
}

impl Platform {
    fn as_str(&self) -> &str {
        match self {
            Platform::Android => "android",
            Platform::Ios => "ios",
            Platform::Harmony => "harmony",
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "android" => Some(Platform::Android),
            "ios" => Some(Platform::Ios),
            "harmony" | "harmonyos" => Some(Platform::Harmony),
            _ => None,
        }
    }
}

/// Execute the new project command
pub fn execute(
    name: Option<String>,
    project_type: Option<String>,
    platform: Option<String>,
    package_id: Option<String>,
    yes: bool,
) -> Result<()> {
    println!("{}", "Create a new LingXia project".bold());
    println!();

    let config = gather_project_info(name, project_type, platform, package_id)?;

    println!();
    println!("{}", "Project Configuration:".bold());
    println!("  Name:        {}", config.name.cyan());
    println!("  Type:        {}", config.project_type.as_str().cyan());
    println!("  Platform:    {}", config.platform.as_str().cyan());
    println!("  Package ID:  {}", config.package_id.cyan());
    println!(
        "  Directory:   {}",
        config.target_dir.display().to_string().cyan()
    );
    println!();

    if !yes {
        let confirmed = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Create project?")
            .default(true)
            .interact()?;

        if !confirmed {
            println!("{}", "Project creation cancelled.".yellow());
            return Ok(());
        }
    }

    create_project(&config)?;
    create_rust_library(&config)?;
    generate_config_file(&config)?;

    println!();
    println!("{}", "Project created successfully!".green().bold());
    println!();
    println!("{}", "Next steps:".bold());
    println!("  cd {}", config.name);
    println!("  # Start developing with:");
    println!("  lingxia dev");
    println!();

    Ok(())
}

fn gather_project_info(
    name: Option<String>,
    project_type: Option<String>,
    platform: Option<String>,
    package_id: Option<String>,
) -> Result<ProjectConfig> {
    // 1. Project Name
    let name = match name {
        Some(n) => {
            validate_project_name(&n)?;
            n
        }
        None => {
            let input: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Project name")
                .validate_with(|input: &String| -> Result<(), String> {
                    validate_project_name(input).map_err(|e| e.to_string())
                })
                .interact_text()?;
            input
        }
    };

    // 2. Project Type
    let project_type = match project_type.and_then(|t| ProjectType::from_str(&t)) {
        Some(t) => t,
        None => {
            let types = vec!["Native Host App", "LingXia Lightweight App (LxApp)"];
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Project type")
                .items(&types)
                .default(0)
                .interact()?;

            match selection {
                0 => ProjectType::NativeApp,
                1 => ProjectType::LxApp,
                _ => unreachable!(),
            }
        }
    };

    // 3. Platform
    let platform = match platform.and_then(|p| Platform::from_str(&p)) {
        Some(p) => p,
        None => {
            let platforms = vec!["Android"];
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Target platform")
                .items(&platforms)
                .default(0)
                .interact()?;

            match selection {
                0 => Platform::Android,
                _ => unreachable!(),
            }
        }
    };

    // 4. Package ID
    let default_package_id = format!("{}.{}", DEFAULT_PACKAGE_PREFIX, name.to_lowercase());
    let package_id = match package_id {
        Some(p) => {
            validate_package_id(&p)?;
            p
        }
        None => {
            let input: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Package ID")
                .default(default_package_id.clone())
                .validate_with(|input: &String| -> Result<(), String> {
                    validate_package_id(input).map_err(|e| e.to_string())
                })
                .interact_text()?;
            input
        }
    };

    let target_dir = std::env::current_dir()?.join(&name);

    Ok(ProjectConfig {
        name,
        project_type,
        platform,
        package_id,
        target_dir,
    })
}

fn create_project(config: &ProjectConfig) -> Result<()> {
    if config.target_dir.exists() {
        return Err(anyhow!(
            "Directory '{}' already exists",
            config.target_dir.display()
        ));
    }

    println!();
    println!("{}", "Creating project structure...".bold());

    match config.platform {
        Platform::Android => create_android_project(config)?,
        Platform::Ios => {
            return Err(anyhow!("iOS support is not yet implemented"));
        }
        Platform::Harmony => {
            return Err(anyhow!("HarmonyOS support is not yet implemented"));
        }
    }

    Ok(())
}

fn create_android_project(config: &ProjectConfig) -> Result<()> {
    let project_root = &config.target_dir;

    // Create root directory
    fs::create_dir_all(project_root)?;

    // Create android subdirectory
    let android_dir = project_root.join("android");
    fs::create_dir_all(&android_dir)?;

    // Locate templates directory
    let templates_base = locate_templates_dir()?;
    let template_dir = templates_base.join("android-native");

    if !template_dir.exists() {
        return Err(anyhow!(
            "Android template not found at: {}",
            template_dir.display()
        ));
    }

    // Build variable substitution map
    let mut vars = HashMap::new();
    vars.insert("PROJECT_NAME".to_string(), config.name.clone());
    vars.insert("PACKAGE_ID".to_string(), config.package_id.clone());

    // Add SDK version variables
    vars.insert("MIN_SDK".to_string(), "29".to_string());
    vars.insert("TARGET_SDK".to_string(), "35".to_string());
    vars.insert("COMPILE_SDK".to_string(), "35".to_string());

    // Process all template files into android/ subdirectory
    process_template_dir(&template_dir, &android_dir, &vars)?;

    // Special handling: Create package directory structure for MainActivity.kt
    let package_path = config.package_id.replace('.', "/");
    let kotlin_dir = android_dir.join(format!("app/src/main/java/{}", package_path));
    fs::create_dir_all(&kotlin_dir)?;

    // Move MainActivity.kt to the correct package directory
    let temp_main_activity = android_dir.join("app/src/main/java/MainActivity.kt");
    if temp_main_activity.exists() {
        let target_main_activity = kotlin_dir.join("MainActivity.kt");
        fs::rename(&temp_main_activity, &target_main_activity)?;
    }

    println!("  Created Android project structure");

    Ok(())
}

fn create_rust_library(config: &ProjectConfig) -> Result<()> {
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
    vars.insert("project_name".to_string(), lib_name.clone());
    vars.insert("PACKAGE_ID".to_string(), config.package_id.clone());

    // Convert package ID to underscore format for JNI function names
    // e.g., com.example.mouke -> com_example_mouke
    let package_id_underscore = config.package_id.replace('.', "_");
    vars.insert("PACKAGE_ID_UNDERSCORE".to_string(), package_id_underscore);

    // Process all template files into {project}-lib/ directory
    process_template_dir(&template_dir, &lib_dir, &vars)?;

    println!("  Created Rust library: {}", lib_name);

    Ok(())
}

fn generate_config_file(config: &ProjectConfig) -> Result<()> {
    let lingxia_config = match config.platform {
        Platform::Android => {
            // Use default Android SDK values
            let android_config = AndroidConfig {
                package_id: config.package_id.clone(),
                min_sdk: Some(29),
                target_sdk: Some(35),
                compile_sdk: Some(35),
                ndk_version: None, // Auto-detect
                api_level: None,   // Derive from target_sdk
            };

            LingXiaConfig {
                project: ConfigProjectConfig {
                    name: config.name.clone(),
                    project_type: config.project_type.as_str().to_string(),
                    platforms: vec!["android".to_string()],
                },
                android: Some(android_config),
                ios: None,
                harmony: None,
                lxapp: None,
                resources: None,
            }
        }
        Platform::Ios => {
            return Err(anyhow!("iOS platform is not yet supported"));
        }
        Platform::Harmony => {
            return Err(anyhow!("HarmonyOS platform is not yet supported"));
        }
    };

    // Save config file
    lingxia_config.save(&config.target_dir)?;

    println!("  Created lingxia.config.json");

    Ok(())
}
