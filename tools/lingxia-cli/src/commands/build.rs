use crate::config::LingXiaConfig;
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
    features: Vec<String>,
    skip_native: bool,
    targets: Vec<String>,
) -> Result<()> {
    // Detect project root (current directory)
    let project_root = env::current_dir()?;

    // Try to load config file
    let config = LingXiaConfig::try_load(&project_root);

    // Log config status
    if let Some(ref cfg) = config {
        println!("  Using lingxia.config.json");
        if let Some(ref android) = cfg.android {
            println!(
                "  Android SDK: min={}, target={}, compile={}",
                android.min_sdk.unwrap_or(28),
                android.target_sdk.unwrap_or(35),
                android.compile_sdk.unwrap_or(35)
            );
        }
    }

    // Parse build profile
    let build_profile = match profile.as_deref() {
        Some("debug") | None => BuildProfile::Debug,
        Some("release") => BuildProfile::Release,
        Some(p) => {
            return Err(anyhow!(
                "Invalid profile: {}. Use 'debug' or 'release'",
                p
            ))
        }
    };

    // Default targets if none specified
    let build_targets = if targets.is_empty() {
        vec!["aarch64-linux-android".to_string()]
    } else {
        targets
    };

    let build_config = BuildConfig {
        project_root: project_root.clone(),
        profile: build_profile,
        features,
        skip_native,
        targets: build_targets,
        lingxia_config: config,
    };

    // Detect platform and build
    let platform = platform::detector::detect_platform(&project_root)?;
    let artifacts = platform.build(&build_config)?;

    // Print build summary
    println!();
    println!("{}", "📊 Build Summary:".bold());
    println!("  Platform: {}", artifacts.platform_name().cyan());
    println!("  Artifact: {}", artifacts.path().display().to_string().cyan());

    Ok(())
}
