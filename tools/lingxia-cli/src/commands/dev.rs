use crate::config::LingXiaConfig;
use crate::host_assets::prepare_host_assets;
use crate::platform::detector::PlatformType;
use crate::platform::{self, BuildConfig, BuildProfile, InstallConfig, Platform, RunConfig};
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use std::env;
use std::path::{Path, PathBuf};

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
        } else if app
            .platforms
            .iter()
            .any(|p| p.eq_ignore_ascii_case("harmony") || p.eq_ignore_ascii_case("harmonyos"))
        {
            PlatformType::Harmony
        } else {
            return Err(anyhow!(
                "No supported platform found in config. Add 'ios', 'macos', 'android', or 'harmony' to app.platforms."
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
        PlatformType::Harmony => execute_harmony(
            project_root,
            config,
            build_profile,
            features,
            build_native,
            device,
        ),
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
        dmg: false,
        sign: false,
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
        dmg: false,
        sign: false,
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
        dmg: false,
        sign: false,
    };

    let artifacts = platform.build(&build_config)?;
    let app_path = artifacts.path().to_path_buf();
    let exe = platform::macos::app_bundle_executable(&app_path)?;

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
    println!("  {} Artifact: {}", "*".bold(), app_path.display());
    println!();

    Ok(())
}

fn execute_harmony(
    project_root: std::path::PathBuf,
    config: LingXiaConfig,
    build_profile: BuildProfile,
    features: Vec<String>,
    build_native: bool,
    device: Option<String>,
) -> Result<()> {
    let harmony_platform = platform::harmony::HarmonyPlatform::new();

    // Generate app.json and embed LxApp assets
    let platforms_to_build = vec![PlatformType::Harmony];
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
        dmg: false,
        sign: false,
    };

    let artifacts = harmony_platform.build(&build_config)?;
    let built_hap_path = artifacts.path().to_path_buf();

    println!();

    // Step 2: Install
    println!("{}", "Step 2/3: Installing...".bold());
    let install_config = InstallConfig {
        project_root: project_root.clone(),
        artifact_path: Some(built_hap_path.clone()),
        device_id: device.clone(),
    };

    harmony_platform.install(&install_config)?;
    let installed_hap_path = resolve_installed_harmony_hap(&built_hap_path);

    println!();

    // Step 3: Launch app
    println!("{}", "Step 3/3: Launching app...".bold());

    // Read bundleName from app.json5 (authoritative source).
    let harmony_dir =
        platform::harmony::resolve_harmony_dir(&project_root, config.harmony.as_ref())?;
    let bundle_name = platform::harmony::read_bundle_name(&harmony_dir)?;

    let run_config = RunConfig {
        package_id: bundle_name.clone(),
        main_activity: None, // defaults to "EntryAbility" in harmony platform
        device_id: device,
    };

    harmony_platform.run(&run_config)?;

    println!();
    println!("{}", "Dev workflow complete!".green().bold());
    println!("  {} Platform: {}", "*".bold(), "HarmonyOS".cyan());
    println!("  {} Bundle: {}", "*".bold(), bundle_name);
    println!("  {} Artifact: {}", "*".bold(), installed_hap_path.display());
    println!();

    Ok(())
}

fn resolve_installed_harmony_hap(built_hap: &Path) -> PathBuf {
    for candidate in harmony_install_signed_candidates(built_hap) {
        if candidate.exists() {
            return candidate;
        }
    }
    built_hap.to_path_buf()
}

fn harmony_install_signed_candidates(input_hap: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    candidates.push(install_signed_output_path(input_hap));

    let stem = input_hap
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    if let Some(base) = stem.strip_suffix("-unsigned") {
        let signed_source = input_hap.with_file_name(format!("{base}-signed.hap"));
        candidates.push(install_signed_output_path(&signed_source));
    }

    candidates
}

fn install_signed_output_path(input_hap: &Path) -> PathBuf {
    let stem = input_hap
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("app")
        .trim_end_matches("-unsigned")
        .trim_end_matches("-signed")
        .trim_end_matches("-install-signed");
    input_hap.with_file_name(format!("{stem}-install-signed.hap"))
}
