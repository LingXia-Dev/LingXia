use crate::commands::rust::resolve_build_profile;
use crate::config::{LingXiaConfig, has_host_config};
use crate::host_assets::prepare_configured_host_assets;
use crate::lxapp::ProjectFramework;
use crate::platform::detector::PlatformType;
use crate::platform::{self, BuildConfig, BuildProfile, InstallConfig, Platform, RunConfig};
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use sysinfo::{ProcessesToUpdate, Signal, System};

pub(crate) mod log_store;
mod server;

const RUNNER_APP_NAME: &str = "LingXia Runner.app";
const RUNNER_EXECUTABLE_NAME: &str = "LingXiaRunner";
const RUNNER_LXAPP_PATH_ENV: &str = "LINGXIA_LXAPP_PATH";
const RUNNER_DEV_WS_URL_ENV: &str = "LINGXIA_DEV_WS_URL";
const REQUIRED_RUNNER_VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct DevExecuteOptions {
    pub release: bool,
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
            "Missing app section in lingxia.yaml.\n\
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

    let build_targets = crate::platform::android_abis::resolve_android_targets_from_abis(&abis)?;

    // Generate app.json and embed LxApp assets
    let platforms_to_build = vec![PlatformType::Android];
    prepare_configured_host_assets(
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
        build_native: ctx.build_native,
        targets: build_targets,
        lingxia_config: Some(ctx.config.clone()),
        ipa: false,
        package: false,
        dmg: false,
        macos_arch: None,
        native_features: Vec::new(),
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
        .ok_or_else(|| anyhow!("Missing android.packageId in lingxia.yaml"))?;
    let install_config = InstallConfig {
        project_root: ctx.project_root.clone(),
        artifact_path: Some(artifact_path.to_path_buf()),
        device_id: ctx.device.clone(),
        reinstall: ctx.reinstall,
        quiet: false,
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

    // Generate app.json and embed LxApp assets
    let platforms_to_build = vec![PlatformType::Ios];
    prepare_configured_host_assets(
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
        build_native: ctx.build_native,
        targets: vec![],
        lingxia_config: Some(ctx.config.clone()),
        ipa: false,
        package: false,
        dmg: false,
        macos_arch: None,
        native_features: Vec::new(),
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
        quiet: false,
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
    prepare_configured_host_assets(
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
        build_native: ctx.build_native,
        targets: vec![],
        lingxia_config: Some(ctx.config.clone()),
        ipa: false,
        package: false,
        dmg: false,
        macos_arch,
        native_features: vec![
            "shell".to_string(),
            "devtools".to_string(),
            "webview-input".to_string(),
        ],
    };

    let artifacts = platform.build(&build_config)?;
    let app_path = artifacts.path().to_path_buf();
    let exe = platform::macos::app_bundle_executable(&app_path)?;
    println!();

    let server = server::start_server(&ctx.project_root)?;
    let ws_url = server.ws_url();
    let session = server.session().clone();

    let run_result = (|| -> Result<()> {
        let stop_requested = Arc::new(AtomicBool::new(false));
        install_ctrlc_handler(stop_requested.clone())?;
        log_store::write_dev_info(&ctx.project_root, &session, &ws_url)?;

        // Step 2: Run (run the built executable directly)
        println!("{}", "Step 2/2: Running...".bold());
        terminate_existing_macos_app_processes(&exe)?;
        let mut child = Command::new(&exe)
            .env(RUNNER_DEV_WS_URL_ENV, &ws_url)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| format!("Failed to run {}", exe.display()))?;

        println!();
        println!("{}", "Dev workflow started.".green().bold());
        println!("  {} Platform: {}", "*".bold(), "macOS".cyan());
        println!("  {} Artifact: {}", "*".bold(), app_path.display());
        println!(
            "  {} Dev info: {}",
            "*".bold(),
            log_store::dev_info_path(&ctx.project_root).display()
        );
        println!("  {} WS: {}", "*".bold(), ws_url);
        println!("  {} Log file: {}", "*".bold(), session.log_file.display());
        println!("  {} Session: {}", "*".bold(), session.session_id);
        println!("  {} Stop: {}", "*".bold(), "Ctrl+C or close app".cyan());
        println!();

        wait_for_child_or_interrupt(&mut child, stop_requested, "macOS app")?;
        Ok(())
    })();

    let _ = log_store::remove_dev_info(&ctx.project_root);
    let stop_result = server.stop();
    match (run_result, stop_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(err), Ok(())) => Err(err),
        (Ok(()), Err(err)) => Err(err),
        (Err(run_err), Err(stop_err)) => Err(anyhow!(
            "{}\nAlso failed to stop dev server: {}",
            run_err,
            stop_err
        )),
    }
}

fn execute_harmony(ctx: DevContext) -> Result<()> {
    let harmony_platform = platform::harmony::HarmonyPlatform::new();

    // Generate app.json and embed LxApp assets
    let platforms_to_build = vec![PlatformType::Harmony];
    prepare_configured_host_assets(
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
        build_native: ctx.build_native,
        targets: vec![],
        lingxia_config: Some(ctx.config.clone()),
        ipa: false,
        package: false,
        dmg: false,
        macos_arch: None,
        native_features: Vec::new(),
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
        quiet: false,
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

    let server = server::start_server(&project_root)?;
    let ws_url = server.ws_url();
    let session = server.session().clone();

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

    let run_result = (|| -> Result<()> {
        println!("{}", "Step 1/2: Building lxapp...".bold());
        crate::lxapp::run_in_dir(&build_args, &project_root)?;

        let stop_requested = Arc::new(AtomicBool::new(false));
        install_ctrlc_handler(stop_requested.clone())?;
        log_store::write_dev_info(&project_root, &session, &ws_url)?;

        println!();
        println!("{}", "Step 2/2: Launching Runner...".bold());
        let mut runner = launch_runner_for_lxapp(&project_root, &ws_url)?;

        println!();
        println!("{}", "Dev workflow started.".green().bold());
        println!("  {} Mode: {}", "*".bold(), "LxApp Runner".cyan());
        println!("  {} Project: {}", "*".bold(), project_root.display());
        println!(
            "  {} Dev info: {}",
            "*".bold(),
            log_store::dev_info_path(&project_root).display()
        );
        println!("  {} WS: {}", "*".bold(), ws_url);
        println!("  {} Log file: {}", "*".bold(), session.log_file.display());
        println!("  {} Session: {}", "*".bold(), session.session_id);
        println!("  {} Stop: {}", "*".bold(), "Ctrl+C or close Runner".cyan());
        println!();

        wait_for_runner_or_interrupt(&mut runner, stop_requested)?;
        Ok(())
    })();

    let _ = log_store::remove_dev_info(&project_root);
    let stop_result = server.stop();
    match (run_result, stop_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(err), Ok(())) => Err(err),
        (Ok(()), Err(err)) => Err(err),
        (Err(run_err), Err(stop_err)) => Err(anyhow!(
            "{}\nAlso failed to stop dev server: {}",
            run_err,
            stop_err
        )),
    }
}

fn is_standalone_lxapp_project(project_root: &Path) -> bool {
    project_root.join("lxapp.json").exists() && !has_host_config(project_root)
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

fn launch_runner_for_lxapp(lxapp_path: &Path, ws_url: &str) -> Result<Child> {
    platform::apple::ensure_macos()?;
    ensure_valid_lxapp_dir(lxapp_path)?;
    let app_path = installed_runner_app_path()?;
    ensure_runner_matches_cli(&app_path)?;
    terminate_existing_runner_processes()?;

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
    command.env(RUNNER_DEV_WS_URL_ENV, ws_url);
    command.stdin(Stdio::null());
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());

    let child = command.spawn().with_context(|| {
        format!(
            "Failed to launch installed Runner executable: {}",
            executable_path.display()
        )
    })?;

    println!("{} Launched {}", "[runner]".cyan(), app_path.display());
    Ok(child)
}

fn install_ctrlc_handler(stop_requested: Arc<AtomicBool>) -> Result<()> {
    ctrlc::set_handler(move || {
        stop_requested.store(true, Ordering::Release);
    })
    .context("Failed to install Ctrl+C handler for dev mode")
}

fn wait_for_runner_or_interrupt(runner: &mut Child, stop_requested: Arc<AtomicBool>) -> Result<()> {
    wait_for_child_or_interrupt(runner, stop_requested, "LingXia Runner")
}

fn wait_for_child_or_interrupt(
    child: &mut Child,
    stop_requested: Arc<AtomicBool>,
    label: &str,
) -> Result<()> {
    loop {
        if stop_requested.load(Ordering::Acquire) {
            terminate_child(child, label)?;
            println!();
            println!("{}", "Dev workflow stopped.".yellow().bold());
            return Ok(());
        }

        if let Some(status) = child
            .try_wait()
            .with_context(|| format!("Failed to poll {}", label))?
        {
            println!();
            println!("{}", format!("{} exited.", label).yellow().bold());
            if !status.success() {
                return Err(anyhow!("{} exited with non-zero status", label));
            }
            return Ok(());
        }

        thread::sleep(Duration::from_millis(150));
    }
}

fn terminate_child(child: &mut Child, label: &str) -> Result<()> {
    if child.try_wait()?.is_some() {
        return Ok(());
    }
    child
        .kill()
        .with_context(|| format!("Failed to terminate {}", label))?;
    let _ = child.wait();
    Ok(())
}

fn terminate_existing_runner_processes() -> Result<()> {
    let status = Command::new("pkill")
        .args(["-x", RUNNER_EXECUTABLE_NAME])
        .status()
        .context("Failed to execute pkill for LingXia Runner")?;

    if let Some(1) = status.code() {
        return Ok(());
    }

    if !status.success() {
        return Err(anyhow!(
            "Failed to terminate existing LingXia Runner processes"
        ));
    }

    std::thread::sleep(std::time::Duration::from_millis(300));
    Ok(())
}

fn terminate_existing_macos_app_processes(executable_path: &Path) -> Result<()> {
    let executable_path = canonical_path_or_self(executable_path);
    let mut system = System::new_all();
    system.refresh_processes(ProcessesToUpdate::All, true);
    let mut terminated = false;

    for (pid, process) in system.processes() {
        let Some(process_exe) = process.exe() else {
            continue;
        };
        if !process_executable_matches(process_exe, &executable_path) {
            continue;
        }

        let killed = process
            .kill_with(Signal::Term)
            .unwrap_or_else(|| process.kill());
        if !killed {
            return Err(anyhow!(
                "Failed to terminate existing macOS app process {} ({})",
                pid,
                executable_path.display()
            ));
        }
        terminated = true;
    }

    if terminated {
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
    Ok(())
}

fn canonical_path_or_self(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn process_executable_matches(process_exe: &Path, executable_path: &Path) -> bool {
    canonical_path_or_self(process_exe) == canonical_path_or_self(executable_path)
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
    use super::{is_standalone_lxapp_project, process_executable_matches};
    use crate::config::HOST_CONFIG_FILE;
    use std::fs;
    use std::path::Path;
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
        fs::write(temp.path().join(HOST_CONFIG_FILE), "").unwrap();

        assert!(!is_standalone_lxapp_project(temp.path()));
    }

    #[test]
    fn process_match_requires_exact_executable_path() {
        let exe = Path::new("/tmp/LingXia Demo.app/Contents/MacOS/Demo");

        assert!(process_executable_matches(exe, exe));
        assert!(!process_executable_matches(
            Path::new("/tmp/LingXia Demo.app/Contents/MacOS/DemoOther"),
            exe
        ));
        assert!(!process_executable_matches(
            Path::new("/Applications/Other.app/Contents/MacOS/Demo"),
            exe
        ));
    }
}
