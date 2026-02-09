use crate::config::{HOST_CONFIG_FILE, LXAPP_BUILD_CONFIG_FILE, LingXiaConfig};
use crate::host_assets::prepare_host_assets;
use crate::lxapp;
use crate::platform::{self, BuildConfig, BuildProfile};
use anyhow::{Result, anyhow};
use colored::Colorize;
use std::env;

pub struct BuildExecuteOptions {
    pub release: bool,
    pub features: Vec<String>,
    pub build_native: bool,
    pub targets: Vec<String>,
    pub platforms: Vec<String>,
    pub ipa: bool,
    pub dmg: bool,
    pub sign: bool,
}

/// Execute the build command
///
/// Builds the project using the detected platform's build system.
/// Supports debug and release profiles, custom features, and multi-target builds.
pub fn execute(options: BuildExecuteOptions) -> Result<()> {
    let BuildExecuteOptions {
        release,
        features,
        build_native,
        targets,
        platforms,
        ipa,
        dmg,
        sign,
    } = options;

    // Detect project root (current directory)
    let current_dir = env::current_dir()?;
    let mut project_root = current_dir.clone();
    let mut inferred_platform_from_subdir = None;
    let mut skip_host_assets = false;
    let lxapp_json_exists = current_dir.join("lxapp.json").exists();
    let lxplugin_json_exists = current_dir.join("lxplugin.json").exists();

    println!("{}", "🚀 LingXia Build".bold().cyan());
    println!();

    let host_config_exists = current_dir.join(HOST_CONFIG_FILE).exists();

    // LxApp or LxPlugin project (no host config)
    if (lxapp_json_exists || lxplugin_json_exists) && !host_config_exists {
        let mut args = vec!["build".to_string()];
        if release {
            args.push("--release".to_string());
        }

        return lxapp::run(&args);
    }

    if !host_config_exists {
        if let Some(ctx) =
            platform::detector::find_apple_swift_package_context(&current_dir, HOST_CONFIG_FILE)?
        {
            println!(
                "{} Detected Apple Swift Package in {}",
                "ℹ".blue(),
                current_dir.display()
            );
            println!(
                "  {} Host project: {}",
                "•".cyan(),
                ctx.host_project_root.display()
            );
            println!(
                "  {} Default platform: {}",
                "•".cyan(),
                ctx.inferred_platform.as_str()
            );
            println!();

            project_root = ctx.host_project_root;
            inferred_platform_from_subdir = Some(ctx.inferred_platform);
            skip_host_assets = true;
        } else if let Some(host_root) =
            platform::detector::find_host_project_root(&current_dir, HOST_CONFIG_FILE)
        {
            if let Ok(inferred_platform) = platform::detector::detect_platform_type(&current_dir) {
                println!(
                    "{} Detected {} project in {}",
                    "ℹ".blue(),
                    inferred_platform.as_str(),
                    current_dir.display()
                );
                println!("  {} Host project: {}", "•".cyan(), host_root.display());
                println!();

                project_root = host_root;
                inferred_platform_from_subdir = Some(inferred_platform);
                skip_host_assets = true;
            } else {
                return Err(anyhow!(
                    "No config file found in {}.\n\
                     Expected one of:\n\
                     - {} (native host project)\n\
                     - lxapp.json + {} (LxApp project)",
                    current_dir.display(),
                    HOST_CONFIG_FILE,
                    LXAPP_BUILD_CONFIG_FILE
                ));
            }
        } else {
            return Err(anyhow!(
                "No config file found in {}.\n\
                 Expected one of:\n\
                 - {} (native host project)\n\
                 - lxapp.json + {} (LxApp project)\n\
                 Tip: run from a host project or one of its platform subdirectories.",
                current_dir.display(),
                HOST_CONFIG_FILE,
                LXAPP_BUILD_CONFIG_FILE
            ));
        }
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

    // Determine which platforms to build.
    let mut requested_platforms = platforms;
    if requested_platforms.is_empty()
        && let Some(inferred_platform) = inferred_platform_from_subdir.as_ref()
    {
        requested_platforms.push(inferred_platform.as_str().to_string());
    }
    let constrained_platforms = !requested_platforms.is_empty();
    let platforms_to_build: Vec<platform::detector::PlatformType> = if constrained_platforms {
        let mut selected = Vec::new();
        for p in requested_platforms {
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
    if constrained_platforms
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

    // Prepare host assets unless we're building from a platform subdirectory.
    if skip_host_assets {
        println!(
            "{} Skipping host assets (building from platform subdirectory)",
            "ℹ".blue()
        );
    } else {
        prepare_host_assets(
            &project_root,
            &config,
            build_profile,
            &platforms_to_build,
            constrained_platforms,
        )?;
    }

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
                if constrained_platforms {
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
            sign: sign && matches!(platform_type, platform::detector::PlatformType::Harmony),
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
