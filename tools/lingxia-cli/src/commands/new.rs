mod template;
mod validation;

use crate::config::{
    AndroidConfig, HOST_SECRETS_FILE, HarmonyConfig, HostAppConfig, IosConfig, LingXiaConfig,
    LingXiaSecrets,
};
use crate::path_completion::FilePathCompleter;
use crate::versions::{LingXiaVersions, fetch_latest_versions};
use anyhow::{Result, anyhow};
use colored::Colorize;
use dialoguer::{Confirm, Input, MultiSelect, Select, theme::ColorfulTheme};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use template::process_template_dir;
use validation::{validate_package_id, validate_product_name, validate_project_name};

const DEFAULT_PACKAGE_PREFIX: &str = "com.example";
const DEFAULT_ICON_BACKGROUND_COLOR: &str = "#FFFFFF";

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

    // Try npm distribution structure: npm/vendor/lingxia -> ../../templates
    let npm_templates = exe_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("templates"));
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
    product_name: String,
    project_type: ProjectType,
    platforms: Vec<Platform>,
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

fn normalize_platforms(input: Vec<String>) -> Result<Vec<Platform>> {
    if input.iter().any(|p| p.eq_ignore_ascii_case("all")) {
        return Ok(vec![Platform::Android, Platform::Ios, Platform::Harmony]);
    }

    let mut platforms = Vec::new();
    for raw in input {
        let Some(platform) = Platform::from_str(&raw) else {
            return Err(anyhow!("Unknown platform: {}", raw));
        };
        if !platforms.contains(&platform) {
            platforms.push(platform);
        }
    }
    Ok(platforms)
}

/// Execute the new project command
pub fn execute(
    name: Option<String>,
    project_type: Option<String>,
    platforms: Vec<String>,
    package_id: Option<String>,
    icon: Option<String>,
    yes: bool,
) -> Result<()> {
    println!("{}", "Create a new LingXia project".bold());
    println!();

    // Fetch LingXia versions from GitHub
    let versions = fetch_latest_versions()?;
    println!(
        "  rong={}, sdk={}, rustCrate={}",
        versions.rong.cyan(),
        versions.sdk.cyan(),
        versions.rust_crate.cyan()
    );
    println!();

    let project_type = gather_project_type(project_type)?;
    let name = gather_project_name(name)?;
    let product_name = gather_product_name(&name, yes)?;

    if matches!(project_type, ProjectType::LxApp) {
        let framework = gather_lxapp_framework(yes)?;
        let target_dir = std::env::current_dir()?.join(&name);
        create_lxapp_from_template(&target_dir, &name, &product_name, &framework, &versions)?;

        println!();
        println!("{}", "Project created successfully!".green().bold());
        println!();
        println!("{}", "Next steps:".bold());
        println!("  cd {}", name);
        println!("  lingxia build");
        println!();
        return Ok(());
    }

    let config =
        gather_native_project_info(name, product_name, project_type, platforms, package_id, yes)?;
    let theme = ColorfulTheme::default();

    println!();
    println!("{}", "Project Configuration:".bold());
    println!("  Name:        {}", config.name.cyan());
    if config.product_name != config.name {
        println!("  Product:     {}", config.product_name.cyan());
    }
    println!("  Type:        {}", config.project_type.as_str().cyan());
    let platform_list = config
        .platforms
        .iter()
        .map(|p| p.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    println!("  Platforms:   {}", platform_list.cyan());
    println!("  Package ID:  {}", config.package_id.cyan());
    println!(
        "  Directory:   {}",
        config.target_dir.display().to_string().cyan()
    );
    println!();

    if !yes {
        let confirmed = Confirm::with_theme(&theme)
            .with_prompt("Create project?")
            .default(true)
            .interact()?;

        if !confirmed {
            println!("{}", "Project creation cancelled.".yellow());
            return Ok(());
        }
    }

    create_project(&config, &versions)?;
    create_rust_library(&config, &versions)?;

    // Ask user if they want to configure app icon (if not provided via CLI)
    let icon_config = match (icon, yes) {
        (Some(path), _) => Some((path, DEFAULT_ICON_BACKGROUND_COLOR.to_string())),
        (None, true) => None,
        (None, false) => {
            println!();
            let configure_icon = Confirm::with_theme(&theme)
                .with_prompt("Do you want to configure an app icon?")
                .default(false)
                .interact()?;

            if !configure_icon {
                None
            } else {
                let path: String = Input::with_theme(&theme)
                    .with_prompt("Path to app icon (PNG, recommended 1024x1024)")
                    .completion_with(&FilePathCompleter::new())
                    .interact_text()?;
                let background_color: String = Input::with_theme(&theme)
                    .with_prompt("Adaptive icon background color (e.g. #FFFFFF)")
                    .default(DEFAULT_ICON_BACKGROUND_COLOR.to_string())
                    .validate_with(|input: &String| -> Result<(), String> {
                        crate::appicon::normalize_android_color(input)
                            .map(|_| ())
                            .map_err(|e| e.to_string())
                    })
                    .interact_text()?;

                Some((
                    path,
                    crate::appicon::normalize_android_color(&background_color)?,
                ))
            }
        }
    };

    // Generate app icons if icon path is provided
    if let Some((icon_path, background_color)) = icon_config {
        generate_app_icons(&config, &icon_path, &background_color)?;
    }

    let lxapp_dir_name = gather_lxapp_dir_name(yes)?;
    let lxapp_framework = gather_lxapp_framework(yes)?;
    let lxapp_info = create_lxapp_project(&config, &lxapp_dir_name, &lxapp_framework, &versions)?;
    generate_config_file(&config, &lxapp_info)?;
    generate_secrets_file(&config)?;
    ensure_root_gitignore(&config)?;

    println!();
    println!("{}", "Project created successfully!".green().bold());
    println!();
    println!("{}", "Next steps:".bold());
    println!("  cd {}", config.name);
    println!("  lingxia build");
    println!();

    Ok(())
}

fn gather_project_name(name: Option<String>) -> Result<String> {
    match name {
        Some(n) => {
            validate_project_name(&n)?;
            Ok(n)
        }
        None => {
            let input: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Project name")
                .validate_with(|input: &String| -> Result<(), String> {
                    validate_project_name(input).map_err(|e| e.to_string())
                })
                .interact_text()?;
            Ok(input)
        }
    }
}

fn gather_product_name(project_name: &str, yes: bool) -> Result<String> {
    if yes {
        return Ok(project_name.to_string());
    }

    let input: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Product name")
        .with_initial_text(project_name.to_string())
        .validate_with(|input: &String| -> Result<(), String> {
            validate_product_name(input).map_err(|e| e.to_string())
        })
        .interact_text()?;
    Ok(input.trim().to_string())
}

fn gather_project_type(project_type: Option<String>) -> Result<ProjectType> {
    Ok(match project_type.and_then(|t| ProjectType::from_str(&t)) {
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
    })
}

fn gather_native_project_info(
    name: String,
    product_name: String,
    project_type: ProjectType,
    platforms: Vec<String>,
    package_id: Option<String>,
    yes: bool,
) -> Result<ProjectConfig> {
    let platforms = if !platforms.is_empty() {
        normalize_platforms(platforms)?
    } else if yes {
        vec![Platform::Android, Platform::Ios, Platform::Harmony]
    } else {
        println!("Use ↑/↓ to move, Space to select, Enter to confirm.");

        let items = vec!["Android", "iOS", "Harmony", "All (Android + iOS + Harmony)"];
        let defaults = vec![false, false, false, false];
        let selections = MultiSelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Target platforms")
            .items(&items)
            .defaults(&defaults)
            .interact()?;

        if selections.is_empty() {
            return Err(anyhow!(
                "At least one platform must be selected (press Space to toggle)"
            ));
        }

        let has_all = selections.contains(&3);
        let has_specific = selections.iter().any(|idx| *idx != 3);

        if has_all && !has_specific {
            vec![Platform::Android, Platform::Ios, Platform::Harmony]
        } else {
            let mut selected = Vec::new();
            for idx in selections {
                if idx == 3 {
                    continue;
                }
                let platform = match idx {
                    0 => Platform::Android,
                    1 => Platform::Ios,
                    2 => Platform::Harmony,
                    _ => unreachable!(),
                };
                if !selected.contains(&platform) {
                    selected.push(platform);
                }
            }
            selected
        }
    };

    let default_package_id = format!("{}.{}", DEFAULT_PACKAGE_PREFIX, name.to_lowercase());
    let package_id = match package_id {
        Some(p) => {
            validate_package_id(&p)?;
            p
        }
        None => {
            let input: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Package ID")
                .with_initial_text(default_package_id.clone())
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
        product_name,
        project_type,
        platforms,
        package_id,
        target_dir,
    })
}

fn gather_lxapp_dir_name(yes: bool) -> Result<String> {
    let default_name = "homelxapp".to_string();
    if yes {
        return Ok(default_name);
    }

    let name: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("LxApp (lightweight application) name")
        .with_initial_text(default_name)
        .validate_with(|input: &String| -> Result<(), String> {
            let trimmed = input.trim();
            if trimmed.is_empty() {
                return Err("LxApp directory name cannot be empty".to_string());
            }
            if trimmed.contains('/') || trimmed.contains('\\') {
                return Err("LxApp directory name cannot contain path separators".to_string());
            }
            if slugify(trimmed) != trimmed {
                return Err(
                    "Use lowercase letters, numbers, and dashes only (e.g. 'home-lxapp')"
                        .to_string(),
                );
            }
            Ok(())
        })
        .interact_text()?;

    Ok(name.trim().to_string())
}

fn gather_lxapp_framework(yes: bool) -> Result<String> {
    if yes {
        return Ok("react".to_string());
    }

    let choices = vec!["React", "Vue"];
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Choose framework")
        .items(&choices)
        .default(0)
        .interact()?;
    Ok(choices[selection].to_lowercase())
}

fn create_project(config: &ProjectConfig, versions: &LingXiaVersions) -> Result<()> {
    if config.target_dir.exists() {
        return Err(anyhow!(
            "Directory '{}' already exists",
            config.target_dir.display()
        ));
    }

    println!();
    println!("{}", "Creating project structure...".bold());

    fs::create_dir_all(&config.target_dir)?;

    let mut created_any = false;
    for platform in &config.platforms {
        match platform {
            Platform::Android => {
                create_android_project(config, versions)?;
                created_any = true;
            }
            Platform::Ios => {
                create_ios_placeholder(config)?;
                created_any = true;
            }
            Platform::Harmony => {
                create_harmony_placeholder(config)?;
                created_any = true;
            }
        }
    }

    if !created_any {
        return Err(anyhow!("No platforms selected"));
    }

    Ok(())
}

fn create_android_project(config: &ProjectConfig, versions: &LingXiaVersions) -> Result<()> {
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

    vars.insert("SDK_VERSION".to_string(), versions.sdk.clone());

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

fn create_ios_placeholder(config: &ProjectConfig) -> Result<()> {
    let ios_dir = config.target_dir.join("ios");
    fs::create_dir_all(&ios_dir)?;
    let readme = ios_dir.join("README.md");
    if !readme.exists() {
        fs::write(
            &readme,
            "iOS template is not yet available. This directory is reserved for future use.\n",
        )?;
    }
    println!("  Created iOS placeholder directory");
    Ok(())
}

fn create_harmony_placeholder(config: &ProjectConfig) -> Result<()> {
    let harmony_dir = config.target_dir.join("harmony");
    fs::create_dir_all(&harmony_dir)?;
    let readme = harmony_dir.join("README.md");
    if !readme.exists() {
        fs::write(
            &readme,
            "HarmonyOS template is not yet available. This directory is reserved for future use.\n",
        )?;
    }
    println!("  Created HarmonyOS placeholder directory");
    Ok(())
}

fn create_rust_library(config: &ProjectConfig, versions: &LingXiaVersions) -> Result<()> {
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

    vars.insert("CRATE_VERSION".to_string(), versions.rust_crate.clone());

    // Process all template files into {project}-lib/ directory
    process_template_dir(&template_dir, &lib_dir, &vars)?;

    println!("  Created Rust library: {}", lib_name);

    Ok(())
}

#[derive(Debug, Clone)]
struct LxAppInfo {
    app_id: String,
}

fn generate_config_file(config: &ProjectConfig, lxapp: &LxAppInfo) -> Result<()> {
    let platforms = config
        .platforms
        .iter()
        .map(|p| p.as_str().to_string())
        .collect::<Vec<_>>();

    let android = if config.platforms.contains(&Platform::Android) {
        Some(AndroidConfig {
            package_id: config.package_id.clone(),
            min_sdk: Some(29),
            target_sdk: Some(35),
            compile_sdk: Some(35),
            ndk_version: None,
            api_level: None,
        })
    } else {
        None
    };

    let ios = if config.platforms.contains(&Platform::Ios) {
        Some(IosConfig {
            bundle_id: config.package_id.clone(),
            deployment_target: None,
            swift_version: None,
        })
    } else {
        None
    };

    let harmony = if config.platforms.contains(&Platform::Harmony) {
        Some(HarmonyConfig {
            bundle_name: config.package_id.clone(),
            compile_sdk_version: None,
            compatible_sdk_version: None,
        })
    } else {
        None
    };

    let lingxia_config = LingXiaConfig {
        app: Some(HostAppConfig {
            product_name: config.product_name.clone(),
            product_version: "1.0.0".to_string(),
            api_server: None,
            platforms: platforms.clone(),
            home_lxapp_id: lxapp.app_id.clone(),
            home_lxapp_version: "1.0.0".to_string(),
        }),
        android,
        ios,
        harmony,
        resources: None,
    };

    // Save config file
    lingxia_config.save(&config.target_dir)?;

    Ok(())
}

fn generate_secrets_file(config: &ProjectConfig) -> Result<()> {
    let secrets_path = config.target_dir.join(HOST_SECRETS_FILE);
    if secrets_path.exists() {
        return Ok(());
    }

    let secrets = LingXiaSecrets::default();
    fs::write(secrets_path, serde_json::to_string_pretty(&secrets)?)?;
    Ok(())
}

fn ensure_root_gitignore(config: &ProjectConfig) -> Result<()> {
    let gitignore_path = config.target_dir.join(".gitignore");
    let android_assets = format!(
        "android/{}/",
        crate::platform::detector::ANDROID_ASSETS_REL_PATH
    );
    let standalone_assets = format!("{}/", crate::platform::detector::ANDROID_ASSETS_REL_PATH);
    let required = [
        HOST_SECRETS_FILE,
        android_assets.as_str(),
        standalone_assets.as_str(),
    ];

    let existing = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path)?
    } else {
        String::new()
    };

    let mut append = String::new();
    for line in required {
        if !existing.lines().any(|l| l.trim() == line) {
            append.push_str(line);
            append.push('\n');
        }
    }
    if append.is_empty() {
        return Ok(());
    }

    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&gitignore_path)?;
    if !existing.is_empty() && !existing.ends_with('\n') {
        f.write_all(b"\n")?;
    }
    f.write_all(append.as_bytes())?;
    Ok(())
}

fn create_lxapp_from_template(
    target_dir: &Path,
    project_name: &str,
    product_name: &str,
    framework: &str,
    versions: &LingXiaVersions,
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

    process_template_dir(&template_dir, target_dir, &vars)?;
    Ok(())
}

fn create_lxapp_project(
    config: &ProjectConfig,
    lxapp_dir_name: &str,
    framework: &str,
    versions: &LingXiaVersions,
) -> Result<LxAppInfo> {
    let lxapp_dir_name = lxapp_dir_name.trim();
    let lxapp_dir = config.target_dir.join(&lxapp_dir_name);
    println!("  Creating LxApp project...");
    create_lxapp_from_template(
        &lxapp_dir,
        lxapp_dir_name,
        &config.product_name,
        framework,
        versions,
    )?;
    Ok(LxAppInfo {
        app_id: lxapp_dir_name.to_string(),
    })
}

fn slugify(value: &str) -> String {
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

fn generate_app_icons(
    config: &ProjectConfig,
    icon_path: &str,
    background_color: &str,
) -> Result<()> {
    use crate::appicon;
    use std::path::PathBuf;

    let icon_path = PathBuf::from(icon_path);
    if !icon_path.exists() {
        eprintln!("Warning: Icon file not found: {:?}", icon_path);
        eprintln!("Skipping icon generation.");
        return Ok(());
    }

    println!("  Generating app icons...");

    for platform in &config.platforms {
        match platform {
            Platform::Android => {
                let res_dir = config.target_dir.join("android/app/src/main/res");
                if !res_dir.exists() {
                    eprintln!("Warning: Android res directory not found: {:?}", res_dir);
                    eprintln!("Skipping Android icon generation.");
                    continue;
                }
                appicon::generate_android_icons(&icon_path, &res_dir, background_color)?;
            }
            Platform::Ios => {
                eprintln!("Warning: iOS icon generation not yet implemented");
            }
            Platform::Harmony => {
                eprintln!("Warning: HarmonyOS icon generation not yet implemented");
            }
        }
    }

    Ok(())
}
