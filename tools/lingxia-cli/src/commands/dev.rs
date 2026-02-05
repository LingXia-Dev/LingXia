use crate::config::LingXiaConfig;
use crate::host_assets::prepare_host_assets;
use crate::platform::detector::PlatformType;
use crate::platform::{self, BuildConfig, BuildProfile, InstallConfig, Platform, RunConfig};
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use std::env;

/// Execute the dev command
///
/// Runs the complete development workflow:
/// 1. Build the project
/// 2. Install to device
/// 3. Launch the application
pub fn execute(
    release: bool,
    features: Vec<String>,
    build_native: bool,
    targets: Vec<String>,
    device: Option<String>,
    platform_arg: Option<String>,
) -> Result<()> {
    println!();
    println!(
        "{}",
        "Development Mode: Build -> Install -> Launch".bold().cyan()
    );
    println!();

    // Detect project root (current directory)
    let project_root = env::current_dir()?;

    // Config is required for all project commands.
    let config = LingXiaConfig::load(&project_root)?;

    // Parse build profile (cargo-like): debug unless explicitly set to release.
    let build_profile = if release {
        BuildProfile::Release
    } else {
        BuildProfile::Debug
    };

    // Determine platform from argument or config
    let app = config.app.as_ref().ok_or_else(|| {
        anyhow!(
            "Missing app section in lingxia.config.json.\n\
             Please configure app.platforms."
        )
    })?;

    let platform_type = if let Some(ref p) = platform_arg {
        p.parse::<PlatformType>()?
    } else {
        // Auto-detect: prefer iOS, then macOS, then Android
        if app.platforms.iter().any(|p| p.eq_ignore_ascii_case("ios")) {
            PlatformType::Ios
        } else if app
            .platforms
            .iter()
            .any(|p| p.eq_ignore_ascii_case("macos"))
        {
            PlatformType::MacOs
        } else if app
            .platforms
            .iter()
            .any(|p| p.eq_ignore_ascii_case("android"))
        {
            PlatformType::Android
        } else {
            return Err(anyhow!(
                "No supported platform found in config. Add 'ios', 'macos', or 'android' to app.platforms."
            ));
        }
    };

    // iOS/macOS dev workflow requires macOS host (uses Xcode tooling).
    if matches!(platform_type, PlatformType::Ios | PlatformType::MacOs) {
        crate::platform::apple::ensure_macos().map_err(|e| {
            anyhow!(
                "{}\nTip: on non-macOS hosts, pass `--platform android` to use Android dev.",
                e
            )
        })?;
    }

    match platform_type {
        PlatformType::Android => execute_android(
            project_root,
            config,
            build_profile,
            features,
            build_native,
            targets,
            device,
        ),
        PlatformType::Ios => execute_ios(
            project_root,
            config,
            build_profile,
            features,
            build_native,
            device,
        ),
        PlatformType::MacOs => {
            execute_macos(project_root, config, build_profile, features, build_native)
        }
        PlatformType::Harmony => Err(anyhow!("HarmonyOS dev mode is not yet supported.")),
    }
}

fn execute_android(
    project_root: std::path::PathBuf,
    config: LingXiaConfig,
    build_profile: BuildProfile,
    features: Vec<String>,
    build_native: bool,
    targets: Vec<String>,
    device: Option<String>,
) -> Result<()> {
    let platform = platform::android::AndroidPlatform::new();

    // Default targets if none specified
    let build_targets = if targets.is_empty() {
        vec!["aarch64-linux-android".to_string()]
    } else {
        targets
    };

    // Generate app.json and embed LxApp assets
    let platforms_to_build = vec![PlatformType::Android];
    prepare_host_assets(
        &project_root,
        &config,
        build_profile,
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
        ipa: false,
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
        .ok_or_else(|| anyhow!("Missing android.packageId in lingxia.config.json"))?;

    let run_config = RunConfig {
        device_id: device,
        package_id,
        main_activity: None,
    };

    platform.run(&run_config)?;

    println!();
    println!("{}", "Dev workflow complete!".green().bold());
    println!("  {} Platform: {}", "*".bold(), "Android".cyan());
    println!("  {} Artifact: {}", "*".bold(), artifacts.path().display());
    println!();

    Ok(())
}

fn execute_ios(
    project_root: std::path::PathBuf,
    config: LingXiaConfig,
    build_profile: BuildProfile,
    features: Vec<String>,
    build_native: bool,
    device: Option<String>,
) -> Result<()> {
    let platform = platform::ios::IosPlatform::new();

    // Generate app.json and embed LxApp assets
    let platforms_to_build = vec![PlatformType::Ios];
    prepare_host_assets(
        &project_root,
        &config,
        build_profile,
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
        targets: vec![],
        lingxia_config: Some(config.clone()),
        ipa: false,
    };

    let artifacts = platform.build(&build_config)?;
    let app_path = artifacts.path();

    println!();

    // Step 2: Sign + Install
    println!("{}", "Step 2/3: Installing...".bold());
    let install_config = InstallConfig {
        project_root: project_root.clone(),
        artifact_path: Some(app_path.to_path_buf()),
        device_id: device.clone(),
    };
    platform.install(&install_config)?;

    println!();

    // Step 3: Launch app
    println!("{}", "Step 3/3: Launching...".bold());

    // Read bundle ID from the signed app (signing may change it for free accounts)
    let bundle_id = platform::ios::read_bundle_id(app_path)?;

    let run_config = RunConfig {
        package_id: bundle_id.clone(),
        main_activity: None,
        device_id: device,
    };
    platform.run(&run_config)?;

    println!();
    println!("{}", "Dev workflow complete!".green().bold());
    println!("  {} Platform: {}", "*".bold(), "iOS".cyan());
    println!("  {} Bundle ID: {}", "*".bold(), bundle_id);
    println!("  {} Artifact: {}", "*".bold(), app_path.display());
    println!();

    Ok(())
}

fn execute_macos(
    project_root: std::path::PathBuf,
    config: LingXiaConfig,
    build_profile: BuildProfile,
    features: Vec<String>,
    build_native: bool,
) -> Result<()> {
    use std::process::Command;

    let platform = platform::macos::MacosPlatform::new();

    // Generate app.json and embed LxApp assets (macOS build prepares resources itself)
    let platforms_to_build = vec![PlatformType::MacOs];
    prepare_host_assets(
        &project_root,
        &config,
        build_profile,
        &platforms_to_build,
        true,
    )?;

    // Step 1: Build
    println!("{}", "Step 1/2: Building...".bold());
    let build_config = BuildConfig {
        project_root: project_root.clone(),
        profile: build_profile,
        features,
        build_native,
        targets: vec![],
        lingxia_config: Some(config.clone()),
        ipa: false,
    };

    let artifacts = platform.build(&build_config)?;
    let exe = artifacts.path().to_path_buf();

    println!();

    // Step 2: Run (run the built executable directly)
    println!("{}", "Step 2/2: Running...".bold());
    let status = Command::new(&exe)
        .status()
        .with_context(|| format!("Failed to run {}", exe.display()))?;
    if !status.success() {
        return Err(anyhow!("macOS app exited with non-zero status"));
    }

    println!();
    println!("{}", "Dev workflow complete!".green().bold());
    println!("  {} Platform: {}", "*".bold(), "macOS".cyan());
    println!("  {} Artifact: {}", "*".bold(), exe.display());
    println!();

    Ok(())
}
