use crate::config::{LingXiaConfig, HOST_CONFIG_FILE, LXAPP_BUILD_CONFIG_FILE};
use crate::lxapp;
use crate::platform::{self, BuildConfig, BuildProfile};
use anyhow::{anyhow, Result};
use colored::Colorize;
use std::env;

/// Execute the build command
///
/// Builds the project using the detected platform's build system.
/// Supports debug and release profiles, custom features, and multi-target builds.
pub fn execute(
    profile: Option<String>,
    prod: bool,
    dev: bool,
    plugin: bool,
    features: Vec<String>,
    build_native: bool,
    targets: Vec<String>,
    platforms: Vec<String>,
) -> Result<()> {
    // Detect project root (current directory)
    let project_root = env::current_dir()?;
    let lxapp_json_exists = project_root.join("lxapp.json").exists();
    let lxapp_config_exists = project_root.join(LXAPP_BUILD_CONFIG_FILE).exists();

    println!("{}", "🚀 LingXia Build".bold().cyan());
    println!();

    let host_config_exists = project_root.join(HOST_CONFIG_FILE).exists();

    if lxapp_json_exists && lxapp_config_exists && !host_config_exists {
        let mut args = vec!["build".to_string()];
        if prod {
            args.push("--prod".to_string());
        }
        if dev {
            args.push("--dev".to_string());
        }
        if plugin {
            args.push("--plugin".to_string());
        }

        println!("  Using LxApp JS builder");
        println!();
        return lxapp::run(&args);
    }

    if lxapp_json_exists && !lxapp_config_exists {
        return Err(anyhow!(
            "{} not found. LxApp projects must include both lxapp.json and {}.",
            LXAPP_BUILD_CONFIG_FILE,
            LXAPP_BUILD_CONFIG_FILE
        ));
    }

    if lxapp_config_exists && !lxapp_json_exists {
        return Err(anyhow!(
            "lxapp.json not found. LxApp projects must include both lxapp.json and {}.",
            LXAPP_BUILD_CONFIG_FILE
        ));
    }

    if !host_config_exists {
        return Err(anyhow!(
            "No config file found in {}.\n\
             Expected one of:\n\
             - {} (native host project)\n\
             - lxapp.json + {} (LxApp project)",
            project_root.display(),
            HOST_CONFIG_FILE,
            LXAPP_BUILD_CONFIG_FILE
        ));
    }

    // Host/native build
    let config = LingXiaConfig::load(&project_root)?;

    println!("  Using {}", HOST_CONFIG_FILE);
    println!("  Project: {}", config.project.name.cyan());
    println!("  Type: {}", config.project.project_type.cyan());

    // Determine platforms from config (no auto-detection/fallback).
    let available_platforms: Vec<platform::detector::PlatformType> = config
        .project
        .platforms
        .iter()
        .map(|p| p.parse())
        .collect::<Result<Vec<_>, _>>()?;

    if available_platforms.is_empty() {
        return Err(anyhow!(
            "No platform configured in lingxia.config.json.\n\
             Set project.platforms to include at least one of: android, ios, harmony"
        ));
    }

    // Determine which platforms to build
    let explicit_platforms = !platforms.is_empty();
    let platforms_to_build: Vec<platform::detector::PlatformType> = if explicit_platforms {
        let mut selected = Vec::new();
        for p in platforms {
            let platform_type: platform::detector::PlatformType = p.parse()?;
            if !available_platforms.contains(&platform_type) {
                return Err(anyhow!(
                    "Platform '{}' not detected in project directory",
                    platform_type.as_str()
                ));
            }
            if !selected.contains(&platform_type) {
                selected.push(platform_type);
            }
        }
        selected
    } else {
        available_platforms
    };

    println!();

    // Parse build profile
    let build_profile = match profile.as_deref() {
        Some("debug") | None => BuildProfile::Debug,
        Some("release") => BuildProfile::Release,
        Some(p) => return Err(anyhow!("Invalid profile: {}. Use 'debug' or 'release'", p)),
    };

    // Default targets if none specified
    let build_targets = if targets.is_empty() {
        vec!["aarch64-linux-android".to_string()]
    } else {
        targets
    };

    // Build each selected platform
    let mut all_artifacts = Vec::new();

    for platform_type in platforms_to_build {
        println!(
            "{}",
            format!("📦 Building {} platform...", platform_type.as_str()).bold()
        );
        println!();

        let platform = match platform::detector::create_platform(&platform_type) {
            Ok(p) => p,
            Err(e) => {
                if explicit_platforms {
                    return Err(e);
                }
                eprintln!(
                    "  {} Skipping {}: {}",
                    "Warning:".yellow(),
                    platform_type.as_str(),
                    e
                );
                continue;
            }
        };

        let build_config = BuildConfig {
            project_root: project_root.clone(),
            profile: build_profile,
            features: features.clone(),
            build_native,
            targets: build_targets.clone(),
            lingxia_config: Some(config.clone()),
        };

        let artifacts = platform.build(&build_config)?;
        all_artifacts.push((platform_type, artifacts));

        println!();
    }

    if all_artifacts.is_empty() {
        return Err(anyhow!("No supported platforms to build."));
    }

    // Print build summary
    println!("{}", "📊 Build Summary:".bold().green());
    for (platform_type, artifacts) in all_artifacts {
        println!(
            "  {} {} → {}",
            "✓".green(),
            platform_type.as_str().cyan(),
            artifacts.path().display().to_string().cyan()
        );
    }
    println!();

    Ok(())
}
