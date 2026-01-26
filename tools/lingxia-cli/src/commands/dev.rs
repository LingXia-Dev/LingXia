use crate::config::LingXiaConfig;
use crate::platform::{self, BuildConfig, BuildProfile, InstallConfig, RunConfig};
use anyhow::{anyhow, Context, Result};
use colored::Colorize;
use std::env;
use std::fs;
use std::path::PathBuf;

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
    println!("{}", "🚀 Development Mode: Build → Install → Launch".bold().cyan());
    println!();

    // Detect project root (current directory)
    let project_root = env::current_dir()?;

    // Try to load config file
    let config = LingXiaConfig::try_load(&project_root);

    // Log config status
    if let Some(ref cfg) = config {
        println!("  📄 Using lingxia.config.json");
        if let Some(ref android) = cfg.android {
            println!(
                "  📱 Android SDK: min={}, target={}, compile={}",
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

    // Detect platform
    let platform = platform::detector::detect_platform(&project_root)?;

    // Step 1: Build
    println!("{}", "Step 1/3: Building...".bold());
    let build_config = BuildConfig {
        project_root: project_root.clone(),
        profile: build_profile,
        features,
        build_native,
        targets: build_targets,
        lingxia_config: config.clone(),
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

    // Auto-detect package ID from Android Gradle project
    let android_root = platform::detector::resolve_android_dir(&project_root);
    let package_id = detect_package_id(&android_root)?;
    println!("  Detected package: {}", package_id.cyan());

    let run_config = RunConfig {
        device_id: device,
        package_id: package_id.clone(),
        main_activity: None, // Will use default MainActivity
    };

    platform.run(&run_config)?;

    println!();
    println!("{}", "✅ Dev workflow complete!".green().bold());
    println!();
    println!("  {} Platform: {}", "📦".bold(), artifacts.platform_name().cyan());
    println!("  {} Artifact: {}", "📦".bold(), artifacts.path().display());
    println!("  {} Package: {}", "📱".bold(), package_id.cyan());
    println!();

    Ok(())
}

/// Detect package ID from build.gradle.kts
fn detect_package_id(project_root: &PathBuf) -> Result<String> {
    // Try app/build.gradle.kts first
    let gradle_kts = project_root.join("app").join("build.gradle.kts");
    let gradle = project_root.join("app").join("build.gradle");

    let content = if gradle_kts.exists() {
        fs::read_to_string(&gradle_kts)
            .context("Failed to read app/build.gradle.kts")?
    } else if gradle.exists() {
        fs::read_to_string(&gradle)
            .context("Failed to read app/build.gradle")?
    } else {
        return Err(anyhow!("Could not find build.gradle or build.gradle.kts in app/"));
    };

    // Parse applicationId from gradle file
    // Look for: applicationId = "com.example.app" or applicationId "com.example.app"
    for line in content.lines() {
        let trimmed = line.trim();

        // Kotlin DSL: applicationId = "..."
        if let Some(stripped) = trimmed.strip_prefix("applicationId") {
            let rest = stripped.trim();
            if let Some(rest) = rest.strip_prefix('=') {
                let rest = rest.trim();
                if let Some(id) = extract_quoted_string(rest) {
                    return Ok(id);
                }
            } else if let Some(id) = extract_quoted_string(rest) {
                // Groovy DSL: applicationId "..."
                return Ok(id);
            }
        }
    }

    Err(anyhow!(
        "Could not find applicationId in build.gradle.kts. Please ensure it's defined in android.defaultConfig block."
    ))
}

/// Extract string from quotes (handles both "..." and '...')
fn extract_quoted_string(s: &str) -> Option<String> {
    let s = s.trim();
    if s.starts_with('"') && s.contains('"') {
        let start = s.find('"')? + 1;
        let end = s[start..].find('"')? + start;
        Some(s[start..end].to_string())
    } else if s.starts_with('\'') && s.contains('\'') {
        let start = s.find('\'')? + 1;
        let end = s[start..].find('\'')? + start;
        Some(s[start..end].to_string())
    } else {
        None
    }
}
