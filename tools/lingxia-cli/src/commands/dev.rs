use crate::commands::rust::{resolve_build_profile, resolve_platform_features};
use crate::config::{HOST_CONFIG_FILE, LingXiaConfig};
use crate::host_assets::prepare_host_assets;
use crate::lxapp::ProjectFramework;
use crate::platform::detector::PlatformType;
use crate::platform::{self, BuildConfig, BuildProfile, InstallConfig, Platform, RunConfig};
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

const RUNNER_APP_NAME: &str = "LingXia Runner.app";
const RUNNER_EXECUTABLE_NAME: &str = "LingXiaRunner";
const RUNNER_LXAPP_PATH_ENV: &str = "LINGXIA_LXAPP_PATH";
const REQUIRED_RUNNER_VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct DevExecuteOptions {
    pub release: bool,
    pub features: Vec<String>,
    pub build_native: bool,
    pub abis: Vec<String>,
    pub macos_arch: Option<String>,
    pub framework: Option<String>,
    pub progress: Option<String>,
    pub device: Option<String>,
    pub platform_arg: Option<String>,
    pub reinstall: bool,
}

#[derive(Clone)]
struct DevContext {
    project_root: std::path::PathBuf,
    config: LingXiaConfig,
    build_profile: BuildProfile,
    features: Vec<String>,
    build_native: bool,
    framework: Option<ProjectFramework>,
    progress: Option<String>,
    device: Option<String>,
    reinstall: bool,
}

/// Execute the dev command.
///
/// For app projects, runs the complete development workflow:
/// 1. Build the project
/// 2. Install to device
/// 3. Launch the application
///
/// For standalone lxapp projects, builds the lxapp and launches LingXia Runner.
pub fn execute(options: DevExecuteOptions) -> Result<()> {
    // Detect project root (current directory)
    let project_root = env::current_dir()?;

    if is_standalone_lxapp_project(&project_root) {
        return execute_lxapp_dev(project_root, options);
    }

    println!();
    println!(
        "{}",
        "Development Mode: Build -> Install -> Launch".bold().cyan()
    );
    println!();

    // Config is required for all project commands.
    let config = LingXiaConfig::load(&project_root)?;

    // Parse build profile (cargo-like): debug unless explicitly set to release.
    let build_profile = resolve_build_profile(options.release);

    // Determine platform from argument or config
    let app = config.app.as_ref().ok_or_else(|| {
        anyhow!(
            "Missing app section in lingxia.config.json.\n\
             Please configure app.platforms."
        )
    })?;

    let platform_type = if let Some(ref p) = options.platform_arg {
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

    // iOS/macOS run workflow requires macOS host (uses Xcode tooling).
    if matches!(platform_type, PlatformType::Ios | PlatformType::MacOs) {
        crate::platform::apple::ensure_macos().map_err(|e| {
            anyhow!(
                "{}\nTip: on non-macOS hosts, pass `--platform android` to use Android run.",
                e
            )
        })?;
    }

    let ctx = DevContext {
        project_root,
        config,
        build_profile,
        features: options.features,
        build_native: options.build_native,
        framework: options
            .framework
            .as_deref()
            .map(parse_lxapp_framework)
            .transpose()?,
        progress: options.progress,
        device: options.device,
        reinstall: options.reinstall,
    };

    match platform_type {
        PlatformType::Android => execute_android(ctx, options.abis),
        PlatformType::Ios => execute_ios(ctx),
        PlatformType::MacOs => execute_macos(ctx, options.macos_arch),
        PlatformType::Harmony => execute_harmony(ctx),
    }
}

fn execute_android(ctx: DevContext, abis: Vec<String>) -> Result<()> {
    let platform = platform::android::AndroidPlatform::new();
    let platform_features = resolve_platform_features(&ctx.features, &PlatformType::Android)?;

    let build_targets = crate::platform::android_abis::resolve_android_targets_from_abis(&abis)?;

    // Generate app.json and embed LxApp assets
    let platforms_to_build = vec![PlatformType::Android];
    prepare_host_assets(
        &ctx.project_root,
        &ctx.config,
        ctx.build_profile,
        ctx.framework,
        ctx.progress.as_deref(),
        &platforms_to_build,
        &build_targets,
        true,
    )?;

    // Step 1: Build
    println!("{}", "Step 1/3: Building...".bold());
    let build_config = BuildConfig {
        project_root: ctx.project_root.clone(),
        profile: ctx.build_profile,
        features: platform_features,
        build_native: ctx.build_native,
        targets: build_targets,
        lingxia_config: Some(ctx.config.clone()),
        ipa: false,
        package: false,
        dmg: false,
        macos_arch: None,
    };

    let artifacts = platform.build(&build_config)?;
    let artifact_path = artifacts.path();

    println!();

    // Step 2: Install
    println!("{}", "Step 2/3: Installing...".bold());
    let package_id = ctx
        .config
        .android
        .as_ref()
        .map(|android| android.package_id.clone())
        .ok_or_else(|| anyhow!("Missing android.packageId in lingxia.config.json"))?;
    let install_config = InstallConfig {
        project_root: ctx.project_root.clone(),
        artifact_path: Some(artifact_path.to_path_buf()),
        device_id: ctx.device.clone(),
        reinstall: ctx.reinstall,
    };

    platform.install(&install_config)?;

    println!();

    // Step 3: Launch app
    println!("{}", "Step 3/3: Launching app...".bold());

    let run_config = RunConfig {
        device_id: ctx.device,
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

fn execute_ios(ctx: DevContext) -> Result<()> {
    let platform = platform::ios::IosPlatform::new();
    let platform_features = resolve_platform_features(&ctx.features, &PlatformType::Ios)?;

    // Generate app.json and embed LxApp assets
    let platforms_to_build = vec![PlatformType::Ios];
    prepare_host_assets(
        &ctx.project_root,
        &ctx.config,
        ctx.build_profile,
        ctx.framework,
        ctx.progress.as_deref(),
        &platforms_to_build,
        &[],
        true,
    )?;

    // Step 1: Build
    println!("{}", "Step 1/3: Building...".bold());
    let build_config = BuildConfig {
        project_root: ctx.project_root.clone(),
        profile: ctx.build_profile,
        features: platform_features,
        build_native: ctx.build_native,
        targets: vec![],
        lingxia_config: Some(ctx.config.clone()),
        ipa: false,
        package: false,
        dmg: false,
        macos_arch: None,
    };

    let artifacts = platform.build(&build_config)?;
    let app_path = artifacts.path();

    println!();

    // Step 2: Sign + Install
    println!("{}", "Step 2/3: Installing...".bold());
    let install_config = InstallConfig {
        project_root: ctx.project_root.clone(),
        artifact_path: Some(app_path.to_path_buf()),
        device_id: ctx.device.clone(),
        reinstall: ctx.reinstall,
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
        device_id: ctx.device,
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

fn execute_macos(ctx: DevContext, macos_arch: Option<String>) -> Result<()> {
    use std::process::Command;

    let platform = platform::macos::MacosPlatform::new();
    let platform_features = resolve_platform_features(&ctx.features, &PlatformType::MacOs)?;
    let host_arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x86_64"
    };
    if let Some(ref requested_arch) = macos_arch
        && requested_arch != host_arch
    {
        return Err(anyhow!(
            "`lingxia dev --platform macos` launches the app locally and requires host arch `{}`.\n\
Use `lingxia build --platform macos --macos-arch {}` for cross-arch builds.",
            host_arch,
            requested_arch
        ));
    }

    // Generate app.json and embed LxApp assets (macOS build prepares resources itself)
    let platforms_to_build = vec![PlatformType::MacOs];
    prepare_host_assets(
        &ctx.project_root,
        &ctx.config,
        ctx.build_profile,
        ctx.framework,
        ctx.progress.as_deref(),
        &platforms_to_build,
        &[],
        true,
    )?;

    // Step 1: Build
    println!("{}", "Step 1/2: Building...".bold());
    let build_config = BuildConfig {
        project_root: ctx.project_root.clone(),
        profile: ctx.build_profile,
        features: platform_features,
        build_native: ctx.build_native,
        targets: vec![],
        lingxia_config: Some(ctx.config.clone()),
        ipa: false,
        package: false,
        dmg: false,
        macos_arch,
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

fn execute_harmony(ctx: DevContext) -> Result<()> {
    let harmony_platform = platform::harmony::HarmonyPlatform::new();
    let platform_features = resolve_platform_features(&ctx.features, &PlatformType::Harmony)?;

    // Generate app.json and embed LxApp assets
    let platforms_to_build = vec![PlatformType::Harmony];
    prepare_host_assets(
        &ctx.project_root,
        &ctx.config,
        ctx.build_profile,
        ctx.framework,
        ctx.progress.as_deref(),
        &platforms_to_build,
        &[],
        true,
    )?;

    // Step 1: Build
    println!("{}", "Step 1/3: Building...".bold());
    let build_config = BuildConfig {
        project_root: ctx.project_root.clone(),
        profile: ctx.build_profile,
        features: platform_features,
        build_native: ctx.build_native,
        targets: vec![],
        lingxia_config: Some(ctx.config.clone()),
        ipa: false,
        package: false,
        dmg: false,
        macos_arch: None,
    };

    let artifacts = harmony_platform.build(&build_config)?;
    let built_hap_path = artifacts.path().to_path_buf();

    println!();

    // Step 2: Install
    println!("{}", "Step 2/3: Installing...".bold());
    let harmony_dir =
        platform::harmony::resolve_harmony_dir(&ctx.project_root, ctx.config.harmony.as_ref())?;
    let bundle_name = platform::harmony::read_bundle_name(&harmony_dir)?;
    let install_config = InstallConfig {
        project_root: ctx.project_root.clone(),
        artifact_path: Some(built_hap_path.clone()),
        device_id: ctx.device.clone(),
        reinstall: ctx.reinstall,
    };

    harmony_platform.install(&install_config)?;
    let installed_hap_path = resolve_installed_harmony_hap(&built_hap_path);

    println!();

    // Step 3: Launch app
    println!("{}", "Step 3/3: Launching app...".bold());

    // Read bundleName from app.json5 (authoritative source).
    let run_config = RunConfig {
        package_id: bundle_name.clone(),
        main_activity: None, // defaults to "EntryAbility" in harmony platform
        device_id: ctx.device,
    };

    harmony_platform.run(&run_config)?;

    println!();
    println!("{}", "Dev workflow complete!".green().bold());
    println!("  {} Platform: {}", "*".bold(), "HarmonyOS".cyan());
    println!("  {} Bundle: {}", "*".bold(), bundle_name);
    println!(
        "  {} Artifact: {}",
        "*".bold(),
        installed_hap_path.display()
    );
    println!();

    Ok(())
}

fn execute_lxapp_dev(project_root: PathBuf, options: DevExecuteOptions) -> Result<()> {
    platform::apple::ensure_macos().map_err(|e| {
        anyhow!(
            "{}\nTip: `lingxia dev` for lxapp currently only supports macOS Runner.",
            e
        )
    })?;

    if let Some(platform) = options.platform_arg.as_deref() {
        let parsed = platform.parse::<PlatformType>()?;
        if parsed != PlatformType::MacOs {
            return Err(anyhow!(
                "`lingxia dev` for lxapp currently only supports macOS Runner.\nDo not pass `--platform {}`.",
                parsed.as_str()
            ));
        }
    }

    if options.device.is_some() {
        return Err(anyhow!(
            "`--device` is not supported for lxapp dev.\nRun `lingxia dev` directly inside the lxapp project."
        ));
    }

    if !options.abis.is_empty() {
        return Err(anyhow!(
            "`--abis` is not supported for lxapp dev.\nRun `lingxia dev` directly inside the lxapp project."
        ));
    }

    if options.macos_arch.is_some() {
        return Err(anyhow!(
            "`--macos-arch` is not supported for lxapp dev.\nRunner always launches locally on the current Mac."
        ));
    }

    println!();
    println!("{}", "Development Mode: LxApp -> Runner".bold().cyan());
    println!();

    let mut build_args = vec!["build".to_string()];
    if options.release {
        build_args.push("--release".to_string());
    }
    if let Some(framework) = options.framework.as_deref() {
        build_args.push("--framework".to_string());
        build_args.push(framework.to_string());
    }
    if let Some(progress) = options.progress.as_deref() {
        build_args.push("--progress".to_string());
        build_args.push(progress.to_string());
    }

    println!("{}", "Step 1/2: Building lxapp...".bold());
    crate::lxapp::run_in_dir(&build_args, &project_root)?;

    println!();
    println!("{}", "Step 2/2: Launching Runner...".bold());
    launch_runner_for_lxapp(&project_root)?;

    println!();
    println!("{}", "Dev workflow started.".green().bold());
    println!("  {} Mode: {}", "*".bold(), "LxApp Runner".cyan());
    println!("  {} Project: {}", "*".bold(), project_root.display());
    println!();

    Ok(())
}

fn is_standalone_lxapp_project(project_root: &Path) -> bool {
    project_root.join("lxapp.json").exists() && !project_root.join(HOST_CONFIG_FILE).exists()
}

fn ensure_valid_lxapp_dir(path: &Path) -> Result<()> {
    if path.join("lxapp.json").exists() || path.join("dist").join("lxapp.json").exists() {
        return Ok(());
    }
    Err(anyhow!(
        "lxapp.json not found in {} or {}/dist",
        path.display(),
        path.display()
    ))
}

fn launch_runner_for_lxapp(lxapp_path: &Path) -> Result<()> {
    platform::apple::ensure_macos()?;
    ensure_valid_lxapp_dir(lxapp_path)?;
    let app_path = installed_runner_app_path()?;
    ensure_runner_matches_cli(&app_path)?;

    let executable_path = app_path
        .join("Contents")
        .join("MacOS")
        .join(RUNNER_EXECUTABLE_NAME);
    if !executable_path.exists() {
        return Err(anyhow!(
            "Runner executable not found in installed app bundle: {}",
            executable_path.display()
        ));
    }

    let mut command = Command::new(&executable_path);
    command.env(RUNNER_LXAPP_PATH_ENV, lxapp_path);
    command.spawn().with_context(|| {
        format!(
            "Failed to launch installed Runner executable: {}",
            executable_path.display()
        )
    })?;

    println!("{} Launched {}", "[runner]".cyan(), app_path.display());
    Ok(())
}

fn installed_runner_app_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Failed to resolve home directory"))?;
    let path = home
        .join(".lingxia")
        .join("runner")
        .join(REQUIRED_RUNNER_VERSION)
        .join(RUNNER_APP_NAME);
    if !path.exists() {
        return Err(anyhow!(
            "LingXia Runner {} is not installed at {}.",
            REQUIRED_RUNNER_VERSION,
            path.display()
        ));
    }
    Ok(path)
}

fn ensure_runner_matches_cli(app_path: &Path) -> Result<()> {
    let installed_version = installed_runner_version(app_path)?;
    let installed = installed_version
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown");

    if installed != REQUIRED_RUNNER_VERSION {
        return Err(anyhow!(
            "Installed Runner version {} does not match CLI version {}.\nRunner path: {}",
            installed,
            REQUIRED_RUNNER_VERSION,
            app_path.display()
        ));
    }

    Ok(())
}

fn installed_runner_version(app_path: &Path) -> Result<Option<String>> {
    let info_path = app_path.join("Contents").join("Info.plist");
    if !info_path.exists() {
        return Ok(None);
    }

    let info: plist::Dictionary = plist::from_file(&info_path)
        .with_context(|| format!("Failed to parse {}", info_path.display()))?;
    Ok(info
        .get("CFBundleShortVersionString")
        .and_then(|value| value.as_string())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned))
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

#[cfg(test)]
mod tests {
    use super::is_standalone_lxapp_project;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn standalone_lxapp_project_is_detected() {
        let temp = tempdir().unwrap();
        fs::write(temp.path().join("lxapp.json"), "{}").unwrap();

        assert!(is_standalone_lxapp_project(temp.path()));
    }

    #[test]
    fn host_project_is_not_treated_as_standalone_lxapp() {
        let temp = tempdir().unwrap();
        fs::write(temp.path().join("lxapp.json"), "{}").unwrap();
        fs::write(temp.path().join("lingxia.config.json"), "{}").unwrap();

        assert!(!is_standalone_lxapp_project(temp.path()));
    }
}
