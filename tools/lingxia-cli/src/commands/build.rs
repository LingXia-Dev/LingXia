use crate::config::{HOST_CONFIG_FILE, LXAPP_BUILD_CONFIG_FILE, LingXiaConfig};
use crate::host_assets::prepare_host_assets;
use crate::lxapp;
use crate::platform::{self, BuildConfig, BuildProfile};
use anyhow::{Result, anyhow};
use colored::Colorize;
use std::env;

/// Execute the build command
///
/// Builds the project using the detected platform's build system.
/// Supports debug and release profiles, custom features, and multi-target builds.
pub fn execute(
    release: bool,
    features: Vec<String>,
    build_native: bool,
    targets: Vec<String>,
    platforms: Vec<String>,
    ipa: bool,
    dmg: bool,
) -> Result<()> {
    // Detect project root (current directory)
    let project_root = env::current_dir()?;
    let lxapp_json_exists = project_root.join("lxapp.json").exists();
    let lxplugin_json_exists = project_root.join("lxplugin.json").exists();

    println!("{}", "🚀 LingXia Build".bold().cyan());
    println!();

    let host_config_exists = project_root.join(HOST_CONFIG_FILE).exists();

    // LxApp or LxPlugin project (no host config)
    if (lxapp_json_exists || lxplugin_json_exists) && !host_config_exists {
        let mut args = vec!["build".to_string()];
        if release {
            args.push("--release".to_string());
        }

        return lxapp::run(&args);
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

    let app = config.app.as_ref().ok_or_else(|| {
        anyhow!(
            "Missing app section in {}.\n\
             Please configure app.productName/app.productVersion/app.platforms/app.homeLxAppID/app.homeLxAppVersion.",
            HOST_CONFIG_FILE
        )
    })?;

    // Determine platforms from config (no auto-detection/fallback).
    let available_platforms: Vec<platform::detector::PlatformType> = app
        .platforms
        .iter()
        .map(|p| p.parse())
        .collect::<Result<Vec<_>, _>>()?;

    if available_platforms.is_empty() {
        return Err(anyhow!(
            "No platform configured in lingxia.config.json.\n\
             Set app.platforms to include at least one of: android, ios, macos, harmony"
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

    // If the user explicitly asked to build iOS/macOS, fail fast on non-macOS hosts
    // (Apple tooling requires macOS).
    if explicit_platforms
        && platforms_to_build.iter().any(|p| {
            matches!(
                p,
                platform::detector::PlatformType::Ios | platform::detector::PlatformType::MacOs
            )
        })
    {
        crate::platform::apple::ensure_macos().map_err(|e| {
            anyhow!(
                "{}\nTip: on non-macOS hosts, pass `--platform android` to build only Android.",
                e
            )
        })?;
    }

    // Parse build profile (cargo-like): debug unless explicitly set to release.
    let build_profile = if release {
        BuildProfile::Release
    } else {
        BuildProfile::Debug
    };

    // Prepare LxApp assets if configured
    prepare_host_assets(
        &project_root,
        &config,
        build_profile,
        &platforms_to_build,
        explicit_platforms,
    )?;

    // Default targets if none specified
    let build_targets = if targets.is_empty() {
        vec!["aarch64-linux-android".to_string()]
    } else {
        targets
    };

    // Build each selected platform
    let mut all_artifacts = Vec::new();

    for platform_type in platforms_to_build {
        let platform = match platform::detector::create_platform(&platform_type) {
            Ok(p) => p,
            Err(e) => {
                if explicit_platforms {
                    return Err(e);
                }
                eprintln!(
                    "{} Skipping {}: {}",
                    "⚠".yellow(),
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
            ipa: ipa && matches!(platform_type, platform::detector::PlatformType::Ios),
            dmg: dmg && matches!(platform_type, platform::detector::PlatformType::MacOs),
        };

        let artifacts = platform.build(&build_config)?;
        all_artifacts.push((platform_type, artifacts));
    }

    if all_artifacts.is_empty() {
        return Err(anyhow!("No supported platforms to build."));
    }

    // Print build summary
    println!();
    for (platform_type, artifacts) in &all_artifacts {
        println!(
            "{} {} → {}",
            "✓".green(),
            platform_type.as_str(),
            artifacts.path().display()
        );
    }

    Ok(())
}

// Asset preparation moved to `crate::host_assets` to keep platform builds consistent.
