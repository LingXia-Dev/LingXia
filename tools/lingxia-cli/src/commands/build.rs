use crate::commands::rust::resolve_build_profile;
use crate::config::{HOST_CONFIG_FILE, LXAPP_BUILD_CONFIG_FILE, LingXiaConfig};
use crate::host_assets::prepare_configured_host_assets;
use crate::lxapp;
use crate::lxapp::ProjectFramework;
use crate::platform::detector::PlatformType;
use crate::platform::{self, BuildConfig};
use anyhow::{Result, anyhow};
use colored::Colorize;
use std::env;

pub struct BuildExecuteOptions {
    pub release: bool,
    pub build_native: bool,
    pub abis: Vec<String>,
    pub macos_arch: Option<String>,
    pub framework: Option<String>,
    pub progress: Option<String>,
    pub platforms: Vec<String>,
    pub all_platforms: bool,
    pub ipa: bool,
    pub dmg: bool,
    pub package: bool,
}

/// Execute the build command
///
/// Builds the project using the detected platform's build system.
/// Supports debug and release profiles and multi-target builds.
pub fn execute(options: BuildExecuteOptions) -> Result<()> {
    let BuildExecuteOptions {
        release,
        build_native,
        abis,
        macos_arch,
        framework,
        progress,
        platforms,
        all_platforms,
        ipa,
        dmg,
        package,
    } = options;

    // Detect project root (current directory)
    let current_dir = env::current_dir()?;
    let mut project_root = current_dir.clone();
    let mut inferred_platform_from_subdir = None;
    let mut standalone_apple_swift_package = false;
    let lxapp_json_exists = current_dir.join("lxapp.json").exists();
    let lxplugin_json_exists = current_dir.join("lxplugin.json").exists();

    let host_config_exists = current_dir.join(HOST_CONFIG_FILE).exists();

    // LxApp or LxPlugin project (no host config)
    if (lxapp_json_exists || lxplugin_json_exists) && !host_config_exists {
        if package && !release {
            return Err(anyhow!(
                "Packaging requires a release build for LxApp/LxPlugin projects."
            ));
        }
        let mut args = vec!["build".to_string()];
        if release {
            args.push("--release".to_string());
        }
        if package {
            args.push("--package".to_string());
        }
        if let Some(framework) = framework.as_deref() {
            args.push("--framework".to_string());
            args.push(framework.to_string());
        }
        if let Some(progress) = progress.as_deref() {
            args.push("--progress".to_string());
            args.push(progress.to_string());
        }

        return lxapp::run(&args);
    }

    if !host_config_exists {
        if let Some(ctx) =
            platform::spm::find_apple_swift_package_context(&current_dir, HOST_CONFIG_FILE)?
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
            if let Some(inferred_platform) =
                platform::spm::detect_local_apple_swift_package_platform(&current_dir)?
            {
                println!(
                    "{} Detected standalone Apple Swift Package in {}",
                    "ℹ".blue(),
                    current_dir.display()
                );
                println!("  {} Platform: {}", "•".cyan(), inferred_platform.as_str());
                println!(
                    "  {} lingxia.yaml: not required for standalone Swift Package builds",
                    "•".cyan()
                );
                println!();

                project_root = current_dir.clone();
                inferred_platform_from_subdir = Some(inferred_platform);
                standalone_apple_swift_package = true;
            } else {
                return Err(anyhow!(
                    "No config file found in {}.\n\
                     Expected one of:\n\
                     - {} (native host project)\n\
                     - lxapp.json + {} (LxApp project)\n\
                     Tip: run from a host project, one of its platform subdirectories, or a standalone Apple Swift Package.",
                    current_dir.display(),
                    HOST_CONFIG_FILE,
                    LXAPP_BUILD_CONFIG_FILE
                ));
            }
        }
    }

    if standalone_apple_swift_package {
        return build_standalone_apple_swift_package(
            &project_root,
            inferred_platform_from_subdir,
            build_native,
            release,
            macos_arch,
            platforms,
            all_platforms,
            ipa,
            dmg,
            package,
        );
    }

    // Host/native build
    if package && !release {
        return Err(anyhow!("Packaging requires a release build."));
    }
    let config = LingXiaConfig::load(&project_root)?;

    let app = config.app.as_ref().ok_or_else(|| {
        anyhow!(
            "Missing app section in {}.\n\
             Please configure app.productName/app.productVersion/app.platforms.",
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
        && !all_platforms
        && let Some(inferred_platform) = inferred_platform_from_subdir.as_ref()
    {
        requested_platforms.push(inferred_platform.as_str().to_string());
    }
    let (platforms_to_build, constrained_platforms): (Vec<platform::detector::PlatformType>, bool) =
        if !requested_platforms.is_empty() {
            let mut selected = Vec::new();
            for p in requested_platforms {
                let platform_type: platform::detector::PlatformType = p.parse()?;
                if !available_platforms.contains(&platform_type) {
                    let configured = available_platforms
                        .iter()
                        .map(|p| p.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(anyhow!(
                        "Platform '{}' is not configured in {} (app.platforms).\n\
Configured platforms: {}",
                        platform_type.as_str(),
                        HOST_CONFIG_FILE,
                        configured
                    ));
                }
                if !selected.contains(&platform_type) {
                    selected.push(platform_type);
                }
            }
            (selected, true)
        } else if all_platforms || available_platforms.len() == 1 {
            (available_platforms.clone(), true)
        } else {
            let available = available_platforms
                .iter()
                .map(|p| p.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(anyhow!(
                "Multiple platforms are configured: {available}\n\
Specify one with `--platform <name>` or build all with `--all-platforms`."
            ));
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
    let build_profile = resolve_build_profile(release);

    let has_android = platforms_to_build
        .iter()
        .any(|p| matches!(p, platform::detector::PlatformType::Android));
    let build_targets = if has_android {
        crate::platform::android_abis::resolve_android_targets_from_abis(&abis)?
    } else {
        if !abis.is_empty() {
            println!(
                "{} Ignoring --abis because Android is not in selected platforms",
                "ℹ".blue()
            );
        }
        Vec::new()
    };

    prepare_configured_host_assets(
        &project_root,
        &config,
        build_profile,
        framework
            .as_deref()
            .map(parse_lxapp_framework)
            .transpose()?,
        progress.as_deref(),
        &platforms_to_build,
        &build_targets,
        constrained_platforms,
        None,
    )?;

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
            build_native,
            targets: if matches!(platform_type, platform::detector::PlatformType::Android) {
                build_targets.clone()
            } else {
                Vec::new()
            },
            lingxia_config: Some(config.clone()),
            ipa: ipa && matches!(platform_type, platform::detector::PlatformType::Ios),
            package: package && matches!(platform_type, platform::detector::PlatformType::MacOs),
            dmg: dmg && matches!(platform_type, platform::detector::PlatformType::MacOs),
            macos_arch: if matches!(platform_type, platform::detector::PlatformType::MacOs) {
                macos_arch.clone()
            } else {
                None
            },
            native_features: config.native_features_for_platform(platform_type.as_str()),
            native_default_features: config.native_default_features_enabled(),
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

fn parse_lxapp_framework(value: &str) -> Result<ProjectFramework> {
    match value {
        "react" => Ok(ProjectFramework::React),
        "vue" => Ok(ProjectFramework::Vue),
        "html" => Ok(ProjectFramework::Html),
        _ => Err(anyhow!(
            "Unsupported framework {value:?}; expected react, vue, or html"
        )),
    }
}

fn build_standalone_apple_swift_package(
    project_root: &std::path::Path,
    inferred_platform: Option<PlatformType>,
    build_native: bool,
    release: bool,
    macos_arch: Option<String>,
    platforms: Vec<String>,
    all_platforms: bool,
    ipa: bool,
    dmg: bool,
    package: bool,
) -> Result<()> {
    if package && !release {
        return Err(anyhow!(
            "Packaging requires a release build for standalone Apple Swift Package projects."
        ));
    }

    let inferred_platform = inferred_platform
        .ok_or_else(|| anyhow!("Failed to infer platform for standalone Apple Swift Package"))?;

    let build_profile = resolve_build_profile(release);
    let mut requested_platforms = platforms;
    if requested_platforms.is_empty() && !all_platforms {
        requested_platforms.push(inferred_platform.as_str().to_string());
    }

    let platforms_to_build = if !requested_platforms.is_empty() {
        let mut selected = Vec::new();
        for p in requested_platforms {
            let platform_type: PlatformType = p.parse()?;
            if !matches!(platform_type, PlatformType::MacOs) {
                return Err(anyhow!(
                    "Standalone Apple Swift Package without {} only supports macos builds, got '{}'",
                    HOST_CONFIG_FILE,
                    platform_type.as_str()
                ));
            }
            if !selected.contains(&platform_type) {
                selected.push(platform_type);
            }
        }
        selected
    } else {
        vec![inferred_platform]
    };

    crate::platform::apple::ensure_macos()?;

    let mut all_artifacts = Vec::new();
    for platform_type in platforms_to_build {
        let platform = platform::detector::create_platform(&platform_type)?;
        let build_config = BuildConfig {
            project_root: project_root.to_path_buf(),
            profile: build_profile,
            build_native,
            targets: Vec::new(),
            lingxia_config: None,
            ipa: ipa && matches!(platform_type, PlatformType::Ios),
            package: package && matches!(platform_type, PlatformType::MacOs),
            dmg: dmg && matches!(platform_type, PlatformType::MacOs),
            macos_arch: if matches!(platform_type, PlatformType::MacOs) {
                macos_arch.clone()
            } else {
                None
            },
            native_features: if matches!(platform_type, PlatformType::MacOs) {
                vec!["shell-runtime".to_string(), "webview-input".to_string()]
            } else {
                Vec::new()
            },
            native_default_features: true,
        };

        let artifacts = platform.build(&build_config)?;
        all_artifacts.push((platform_type, artifacts));
    }

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
