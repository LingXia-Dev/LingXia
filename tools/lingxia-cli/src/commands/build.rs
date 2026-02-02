use crate::config::{HOST_CONFIG_FILE, LXAPP_BUILD_CONFIG_FILE, LingXiaConfig};
use crate::lxapp;
use crate::platform::{self, BuildConfig, BuildProfile};
use anyhow::{Result, anyhow};
use colored::Colorize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

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
             Set app.platforms to include at least one of: android, ios, harmony"
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
        };

        let artifacts = platform.build(&build_config)?;
        all_artifacts.push((platform_type, artifacts));
    }

    if all_artifacts.is_empty() {
        return Err(anyhow!("No supported platforms to build."));
    }

    // Print build summary
    println!();
    for (platform_type, artifacts) in all_artifacts {
        println!(
            "{} {} → {}",
            "✓".green(),
            platform_type.as_str(),
            artifacts.path().display()
        );
    }

    Ok(())
}

pub(crate) fn prepare_host_assets(
    project_root: &Path,
    config: &LingXiaConfig,
    build_profile: BuildProfile,
    platforms: &[platform::detector::PlatformType],
    explicit_platforms: bool,
) -> Result<()> {
    let prepared_lxapp_assets = if platforms
        .iter()
        .any(|p| matches!(p, platform::detector::PlatformType::Android))
    {
        prepare_embedded_lxapp_assets(project_root, config, build_profile)?
    } else {
        None
    };

    for platform in platforms {
        match platform {
            platform::detector::PlatformType::Android => {
                let assets_root = platform::detector::resolve_android_assets_dir(project_root);
                fs::create_dir_all(&assets_root)?;

                ensure_host_app_json(config, &assets_root)?;

                if let Some(ref lxapp_assets) = prepared_lxapp_assets {
                    let target_dir = assets_root.join(&lxapp_assets.asset_name);
                    if target_dir.exists() {
                        fs::remove_dir_all(&target_dir)?;
                    }
                    copy_dir_recursive(&lxapp_assets.dist_dir, &target_dir)?;
                    println!("  {} LxApp assets → {}", "✓".green(), target_dir.display());
                }
            }
            platform::detector::PlatformType::Ios | platform::detector::PlatformType::Harmony => {
                if explicit_platforms && prepared_lxapp_assets.is_some() {
                    println!(
                        "  {} LxApp embedding for {} not yet supported.",
                        "⚠️".yellow(),
                        platform.as_str()
                    );
                }
            }
        }
    }

    Ok(())
}

struct PreparedLxAppAssets {
    dist_dir: PathBuf,
    asset_name: String,
}

fn prepare_embedded_lxapp_assets(
    project_root: &Path,
    config: &LingXiaConfig,
    build_profile: BuildProfile,
) -> Result<Option<PreparedLxAppAssets>> {
    let Some(app) = &config.app else {
        return Ok(None);
    };

    let lxapp_dir = project_root.join(&app.home_lxapp_id);
    if !lxapp_dir.exists() {
        return Ok(None);
    }

    let lxapp_json = lxapp_dir.join("lxapp.json");
    let lxapp_build_config = lxapp_dir.join(LXAPP_BUILD_CONFIG_FILE);
    if !lxapp_json.exists() || !lxapp_build_config.exists() {
        return Err(anyhow!(
            "LxApp project must include lxapp.json and {} in {}",
            LXAPP_BUILD_CONFIG_FILE,
            lxapp_dir.display()
        ));
    }

    println!("{}", "Building LxApp...".bold());

    let mut args = vec!["build".to_string()];
    if matches!(build_profile, BuildProfile::Release) {
        args.push("--release".to_string());
    }
    lxapp::run_in_dir(&args, &lxapp_dir)?;

    let dist_dir = lxapp_dir.join("dist");
    if !dist_dir.exists() {
        return Err(anyhow!(
            "LxApp build output not found: {}",
            dist_dir.display()
        ));
    }

    let asset_name = resolve_lxapp_id(&lxapp_json).unwrap_or_else(|_| app.home_lxapp_id.clone());

    Ok(Some(PreparedLxAppAssets {
        dist_dir,
        asset_name,
    }))
}

fn ensure_host_app_json(config: &LingXiaConfig, assets_root: &Path) -> Result<()> {
    write_app_json_from_config(config, assets_root)
}

fn write_app_json_from_config(config: &LingXiaConfig, assets_root: &Path) -> Result<()> {
    let app = config
        .app
        .as_ref()
        .ok_or_else(|| anyhow!("Missing app settings in {}", HOST_CONFIG_FILE))?;

    // Read apiKey/apiSecret from environment variables (CI-friendly)
    let api_key = env::var("LINGXIA_API_KEY")
        .ok()
        .filter(|s| !s.trim().is_empty());
    let api_secret = env::var("LINGXIA_API_SECRET")
        .ok()
        .filter(|s| !s.trim().is_empty());

    let mut obj = serde_json::Map::new();
    obj.insert(
        "productName".to_string(),
        serde_json::json!(app.product_name),
    );
    obj.insert(
        "productVersion".to_string(),
        serde_json::json!(app.product_version),
    );

    if let Some(api_server) = app.api_server.as_ref().filter(|s| !s.trim().is_empty()) {
        obj.insert("apiServer".to_string(), serde_json::json!(api_server));
    }
    if let Some(api_key) = api_key {
        obj.insert("apiKey".to_string(), serde_json::json!(api_key));
    }
    if let Some(api_secret) = api_secret {
        obj.insert("apiSecret".to_string(), serde_json::json!(api_secret));
    }

    obj.insert(
        "homeLxAppID".to_string(),
        serde_json::json!(app.home_lxapp_id),
    );
    obj.insert(
        "homeLxAppVersion".to_string(),
        serde_json::json!(app.home_lxapp_version),
    );
    if let Some(max_age) = app.cache_max_age_days {
        obj.insert("cacheMaxAgeDays".to_string(), serde_json::json!(max_age));
    }

    let app_json_path = assets_root.join("app.json");
    fs::write(
        app_json_path,
        serde_json::to_string_pretty(&serde_json::Value::Object(obj))?,
    )?;
    Ok(())
}

fn resolve_lxapp_id(path: &PathBuf) -> Result<String> {
    let content = fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&content)?;
    let app_id = value
        .get("lxAppId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("lxAppId missing in lxapp.json"))?;
    Ok(app_id.to_string())
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
    if !dest.exists() {
        fs::create_dir_all(dest)?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let target = dest.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &target)?;
        } else {
            fs::copy(&path, &target)?;
        }
    }
    Ok(())
}
