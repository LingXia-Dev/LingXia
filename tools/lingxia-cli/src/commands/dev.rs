use super::build::prepare_host_assets;
use crate::config::LingXiaConfig;
use crate::platform::{self, BuildConfig, BuildProfile, InstallConfig, Platform, RunConfig};
use anyhow::{anyhow, Result};
use colored::Colorize;
use std::env;

/// Execute the dev command
///
/// Runs the complete development workflow:
/// 1. Build the project
/// 2. Install to device
/// 3. Launch the application
pub fn execute(
    profile: Option<String>,
    features: Vec<String>,
    build_native: bool,
    targets: Vec<String>,
    device: Option<String>,
) -> Result<()> {
    println!();
    println!(
        "{}",
        "🚀 Development Mode: Build → Install → Launch"
            .bold()
            .cyan()
    );
    println!();

    // Detect project root (current directory)
    let project_root = env::current_dir()?;

    // Config is required for all project commands.
    let config = LingXiaConfig::load(&project_root)?;

    // Log config status
    println!("  📄 Using lingxia.config.json");
    if let Some(ref android) = config.android {
        println!(
            "  📱 Android SDK: min={}, target={}, compile={}",
            android.min_sdk.unwrap_or(28),
            android.target_sdk.unwrap_or(35),
            android.compile_sdk.unwrap_or(35)
        );
    }

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

    // Determine platform from config (no auto-detection/fallback).
    if !config
        .project
        .platforms
        .iter()
        .any(|p| p.eq_ignore_ascii_case("android"))
    {
        return Err(anyhow!(
            "This command currently supports Android projects only. Add 'android' to project.platforms in lingxia.config.json."
        ));
    }
    let platform = platform::android::AndroidPlatform::new();

    // Generate app.json and embed LxApp assets (assets are not tracked by git).
    let platforms_to_build = vec![platform::detector::PlatformType::Android];
    prepare_host_assets(
        &project_root,
        &config,
        build_profile,
        false,
        false,
        &platforms_to_build,
        true,
    )?;

    // Step 1: Build
    println!("{}", "Step 1/3: Building...".bold());
    let build_config = BuildConfig {
        project_root: project_root.clone(),
        profile: build_profile,
        features,
        build_native,
        targets: build_targets,
        lingxia_config: Some(config.clone()),
    };

    let artifacts = platform.build(&build_config)?;
    let artifact_path = artifacts.path();

    println!();

    // Step 2: Install
    println!("{}", "Step 2/3: Installing...".bold());
    let install_config = InstallConfig {
        project_root: project_root.clone(),
        artifact_path: Some(artifact_path.to_path_buf()),
        device_id: device.clone(),
    };

    platform.install(&install_config)?;

    println!();

    // Step 3: Launch app
    println!("{}", "Step 3/3: Launching app...".bold());

    let package_id = config
        .android
        .as_ref()
        .map(|android| android.package_id.clone())
        .ok_or_else(|| {
            anyhow!(
                "Missing android.packageId in lingxia.config.json (required to launch the app)."
            )
        })?;

    let run_config = RunConfig {
        device_id: device,
        package_id: package_id.clone(),
        main_activity: None, // Will use default MainActivity
    };

    platform.run(&run_config)?;

    println!();
    println!("{}", "✅ Dev workflow complete!".green().bold());
    println!();
    println!(
        "  {} Platform: {}",
        "📦".bold(),
        artifacts.platform_name().cyan()
    );
    println!("  {} Artifact: {}", "📦".bold(), artifacts.path().display());
    println!();

    Ok(())
}
