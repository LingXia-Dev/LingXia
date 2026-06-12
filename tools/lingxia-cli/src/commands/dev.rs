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

/// Windows runner: standalone executable crate built from the LingXia
/// workspace on demand (see `build_windows_runner`).
const RUNNER_WINDOWS_PACKAGE: &str = "lingxia-runner-windows";
const RUNNER_WINDOWS_BIN_NAME: &str = "lingxia-runner";
const RUNNER_WINDOWS_PRODUCT_NAME: &str = "LingXia Runner";

fn dev_native_features(config: &LingXiaConfig, platform: &str) -> Vec<String> {
    let mut features = config.native_features_for_platform(platform);
    if !features.iter().any(|feature| feature == "devtools") {
        features.push("devtools".to_string());
    }
    features
}

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
    pub env_version: Option<String>,
    pub parallel: bool,
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
    resolved_env: crate::config::ResolvedEnv,
    parallel: bool,
}

/// Stable string used for `SessionInfo.platform`. Same set advertised by `lxdev`'s
/// `--platform` selector; keep these in sync.
fn platform_session_name(platform: PlatformType) -> &'static str {
    match platform {
        PlatformType::Android => "android",
        PlatformType::Ios => "ios",
        PlatformType::MacOs => "macos",
        PlatformType::Harmony => "harmony",
        PlatformType::Windows => "windows",
    }
}

/// Refuse to start a new dev session if another live session already exists for
/// the same platform in this project, unless `--parallel` was passed.
///
/// Stale (unreachable) session files are pruned first; only WS-reachable peers
/// count as conflicts. This is the single defense against the "human + agent
/// both ran `lingxia dev -p ios`" footgun.
fn precheck_platform_session(project_root: &Path, platform: &str, parallel: bool) -> Result<()> {
    let _ = log_store::prune_stale(project_root);
    let live = log_store::find_live_for_platform(project_root, platform)?;
    if live.is_empty() || parallel {
        return Ok(());
    }
    let mut msg = format!("Existing {platform} dev session is already running in this project:\n");
    for info in &live {
        msg.push_str(&format!(
            "  {}  pid={}  ws={}\n",
            info.session_id, info.pid, info.ws_url
        ));
    }
    msg.push_str("\nStop it first, or rerun with --parallel to allow multiple ");
    msg.push_str(platform);
    msg.push_str(" sessions.");
    Err(anyhow!(msg))
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
        let has_windows = app
            .platforms
            .iter()
            .any(|p| p.eq_ignore_ascii_case("windows") || p.eq_ignore_ascii_case("win"));
        // Auto-detect: prefer the local desktop platform when available.
        if cfg!(target_os = "windows") && has_windows {
            PlatformType::Windows
        } else if app.platforms.iter().any(|p| p.eq_ignore_ascii_case("ios")) {
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
        } else if has_windows {
            PlatformType::Windows
        } else {
            return Err(anyhow!(
                "No supported platform found in config. Add 'ios', 'macos', 'android', 'harmony', or 'windows' to app.platforms."
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

    let resolved_env =
        crate::commands::build::resolve_build_env(&config, options.env_version.as_deref())?;

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
        resolved_env,
        parallel: options.parallel,
    };

    match platform_type {
        PlatformType::Android => execute_android(ctx, options.abis),
        PlatformType::Ios => execute_ios(ctx),
        PlatformType::MacOs => execute_macos(ctx, options.macos_arch),
        PlatformType::Harmony => execute_harmony(ctx),
        PlatformType::Windows => execute_windows(ctx),
    }
}

fn execute_android(ctx: DevContext, abis: Vec<String>) -> Result<()> {
    let platform_name = platform_session_name(PlatformType::Android);
    precheck_platform_session(&ctx.project_root, platform_name, ctx.parallel)?;
    let platform = platform::android::AndroidPlatform::new();
    let build_targets = crate::platform::android_abis::resolve_android_targets_from_abis(&abis)?;
    let server = server::start_server(&ctx.project_root)?;
    let host_ws_url = server.ws_url();
    let device_ws_url = loopback_ws_url(server.port());
    let session = server.session().clone();

    let run_result = (|| -> Result<()> {
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
            Some(&device_ws_url),
            &ctx.resolved_env,
        )?;

        // Step 1: Build
        println!("{}", "Step 1/4: Building...".bold());
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
            framework: ctx.framework,
            native_features: dev_native_features(&ctx.config, "android"),
            native_default_features: ctx.config.native_default_features_enabled(),
            resolved_env: ctx.resolved_env.clone(),
            skip_native_build: false,
            native_only: false,
        };

        let artifacts = platform.build(&build_config)?;
        let artifact_path = artifacts.path();

        println!();

        // Step 2: Install
        println!("{}", "Step 2/4: Installing...".bold());
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

        // Step 3: Port reverse
        println!("{}", "Step 3/4: Preparing dev connection...".bold());
        let _forward = DevPortForward::android(ctx.device.as_deref(), server.port())?;

        println!();

        // Step 4: Launch app
        println!("{}", "Step 4/4: Launching app...".bold());
        let stop_requested = Arc::new(AtomicBool::new(false));
        install_ctrlc_handler(stop_requested.clone())?;
        log_store::write_session(&ctx.project_root, &session, platform_name, &host_ws_url)?;

        let run_config = RunConfig {
            device_id: ctx.device,
            package_id,
            main_activity: None,
            restart: false,
        };

        platform.run(&run_config)?;

        print_mobile_dev_started(
            "Android",
            artifacts.path(),
            &host_ws_url,
            &device_ws_url,
            &ctx.project_root,
            &session,
        );
        wait_for_interrupt(stop_requested)?;
        Ok(())
    })();

    let _ = log_store::remove_session(&ctx.project_root, &session.session_id);
    stop_dev_server(server, run_result)
}

fn execute_ios(ctx: DevContext) -> Result<()> {
    let platform_name = platform_session_name(PlatformType::Ios);
    precheck_platform_session(&ctx.project_root, platform_name, ctx.parallel)?;
    let platform = platform::ios::IosPlatform::new();
    let server = server::start_server_on(&ctx.project_root, "0.0.0.0:0")?;
    let host_ws_url = loopback_ws_url(server.port());
    let device_ws_url = lan_ws_url(server.port())?;
    let session = server.session().clone();

    let run_result = (|| -> Result<()> {
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
            Some(&device_ws_url),
            &ctx.resolved_env,
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
            framework: ctx.framework,
            native_features: dev_native_features(&ctx.config, "ios"),
            native_default_features: ctx.config.native_default_features_enabled(),
            resolved_env: ctx.resolved_env.clone(),
            skip_native_build: false,
            native_only: false,
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
        let stop_requested = Arc::new(AtomicBool::new(false));
        install_ctrlc_handler(stop_requested.clone())?;
        log_store::write_session(&ctx.project_root, &session, platform_name, &host_ws_url)?;

        // Read bundle ID from the signed app (signing may change it for free accounts)
        let bundle_id = platform::ios::read_bundle_id(app_path)?;

        let run_config = RunConfig {
            package_id: bundle_id.clone(),
            main_activity: None,
            device_id: ctx.device,
            restart: false,
        };
        platform.run(&run_config)?;

        print_mobile_dev_started(
            "iOS",
            app_path,
            &host_ws_url,
            &device_ws_url,
            &ctx.project_root,
            &session,
        );
        println!("  {} Bundle ID: {}", "*".bold(), bundle_id);
        wait_for_interrupt(stop_requested)?;
        Ok(())
    })();

    let _ = log_store::remove_session(&ctx.project_root, &session.session_id);
    stop_dev_server(server, run_result)
}

fn execute_macos(ctx: DevContext, macos_arch: Option<String>) -> Result<()> {
    let platform_name = platform_session_name(PlatformType::MacOs);
    precheck_platform_session(&ctx.project_root, platform_name, ctx.parallel)?;
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
        None,
        &ctx.resolved_env,
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
        framework: ctx.framework,
        native_features: dev_native_features(&ctx.config, "macos"),
        native_default_features: ctx.config.native_default_features_enabled(),
        resolved_env: ctx.resolved_env.clone(),
        skip_native_build: false,
        native_only: false,
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
        log_store::write_session(&ctx.project_root, &session, platform_name, &ws_url)?;

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
            "  {} Session file: {}",
            "*".bold(),
            log_store::session_file_path(&ctx.project_root, &session.session_id).display()
        );
        println!("  {} WS: {}", "*".bold(), ws_url);
        println!("  {} Log file: {}", "*".bold(), session.log_file.display());
        println!("  {} Session: {}", "*".bold(), session.session_id);
        println!("  {} Stop: {}", "*".bold(), "Ctrl+C or close app".cyan());
        println!();

        wait_for_child_or_interrupt(&mut child, stop_requested, "macOS app")?;
        Ok(())
    })();

    let _ = log_store::remove_session(&ctx.project_root, &session.session_id);
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
    let platform_name = platform_session_name(PlatformType::Harmony);
    precheck_platform_session(&ctx.project_root, platform_name, ctx.parallel)?;
    let harmony_platform = platform::harmony::HarmonyPlatform::new();
    let server = server::start_server(&ctx.project_root)?;
    let host_ws_url = server.ws_url();
    let device_ws_url = loopback_ws_url(server.port());
    let session = server.session().clone();

    let run_result = (|| -> Result<()> {
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
            Some(&device_ws_url),
            &ctx.resolved_env,
        )?;

        // Step 1: Build
        println!("{}", "Step 1/4: Building...".bold());
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
            framework: ctx.framework,
            native_features: dev_native_features(&ctx.config, "harmony"),
            native_default_features: ctx.config.native_default_features_enabled(),
            resolved_env: ctx.resolved_env.clone(),
            skip_native_build: false,
            native_only: false,
        };

        let artifacts = harmony_platform.build(&build_config)?;
        let built_hap_path = artifacts.path().to_path_buf();

        println!();

        // Step 2: Install
        println!("{}", "Step 2/4: Installing...".bold());
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

        // Step 3: Port reverse
        println!("{}", "Step 3/4: Preparing dev connection...".bold());
        let _forward = DevPortForward::harmony(ctx.device.as_deref(), server.port())?;

        println!();

        // Step 4: Launch app
        println!("{}", "Step 4/4: Launching app...".bold());
        let stop_requested = Arc::new(AtomicBool::new(false));
        install_ctrlc_handler(stop_requested.clone())?;
        log_store::write_session(&ctx.project_root, &session, platform_name, &host_ws_url)?;

        // Read bundleName from app.json5 (authoritative source).
        let run_config = RunConfig {
            package_id: bundle_name.clone(),
            main_activity: None, // defaults to "EntryAbility" in harmony platform
            device_id: ctx.device,
            restart: false,
        };

        harmony_platform.run(&run_config)?;

        print_mobile_dev_started(
            "HarmonyOS",
            &installed_hap_path,
            &host_ws_url,
            &device_ws_url,
            &ctx.project_root,
            &session,
        );
        println!("  {} Bundle: {}", "*".bold(), bundle_name);
        wait_for_interrupt(stop_requested)?;
        Ok(())
    })();

    let _ = log_store::remove_session(&ctx.project_root, &session.session_id);
    stop_dev_server(server, run_result)
}

fn execute_windows(ctx: DevContext) -> Result<()> {
    let platform_name = platform_session_name(PlatformType::Windows);
    precheck_platform_session(&ctx.project_root, platform_name, ctx.parallel)?;
    let platform = platform::windows::WindowsPlatform::new();
    let server = server::start_server(&ctx.project_root)?;
    let ws_url = server.ws_url();
    let session = server.session().clone();

    let run_result = (|| -> Result<()> {
        let platforms_to_build = vec![PlatformType::Windows];
        prepare_configured_host_assets(
            &ctx.project_root,
            &ctx.config,
            ctx.build_profile,
            ctx.framework,
            ctx.progress.as_deref(),
            &platforms_to_build,
            &[],
            true,
            Some(&ws_url),
            &ctx.resolved_env,
        )?;

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
            macos_arch: None,
            framework: ctx.framework,
            native_features: dev_native_features(&ctx.config, "windows"),
            native_default_features: ctx.config.native_default_features_enabled(),
            resolved_env: ctx.resolved_env.clone(),
            skip_native_build: false,
            native_only: false,
        };

        let artifacts = platform.build(&build_config)?;
        let exe_path = artifacts.path().to_path_buf();
        let runtime_env = platform::windows::windows_runtime_env(
            &ctx.project_root,
            &ctx.config,
            &ctx.resolved_env,
        )?;
        println!();

        println!("{}", "Step 2/2: Running...".bold());
        let stop_requested = Arc::new(AtomicBool::new(false));
        install_ctrlc_handler(stop_requested.clone())?;
        log_store::write_session(&ctx.project_root, &session, platform_name, &ws_url)?;

        let mut command = Command::new(&exe_path);
        command.env(RUNNER_DEV_WS_URL_ENV, &ws_url);
        for (key, value) in &runtime_env {
            command.env(key, value);
        }
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| format!("Failed to run {}", exe_path.display()))?;

        println!();
        println!("{}", "Dev workflow started.".green().bold());
        println!("  {} Platform: {}", "*".bold(), "Windows".cyan());
        println!("  {} Artifact: {}", "*".bold(), exe_path.display());
        println!(
            "  {} Session file: {}",
            "*".bold(),
            log_store::session_file_path(&ctx.project_root, &session.session_id).display()
        );
        println!("  {} WS: {}", "*".bold(), ws_url);
        println!("  {} Log file: {}", "*".bold(), session.log_file.display());
        println!("  {} Session: {}", "*".bold(), session.session_id);
        println!("  {} Stop: {}", "*".bold(), "Ctrl+C or close app".cyan());
        println!();

        wait_for_child_or_interrupt(&mut child, stop_requested, "Windows app")?;
        Ok(())
    })();

    let _ = log_store::remove_session(&ctx.project_root, &session.session_id);
    stop_dev_server(server, run_result)
}

fn execute_lxapp_dev(project_root: PathBuf, options: DevExecuteOptions) -> Result<()> {
    let runner_host = LxAppRunnerHost::detect()?;

    if let Some(platform) = options.platform_arg.as_deref() {
        let parsed = platform.parse::<PlatformType>()?;
        if parsed != runner_host.platform_type() {
            return Err(anyhow!(
                "`lingxia dev` for lxapp launches the local {} Runner.\nDo not pass `--platform {}`.",
                runner_host.display_name(),
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
            "`--macos-arch` is not supported for lxapp dev.\nRunner always launches locally on the current machine."
        ));
    }

    let platform_name = "lxapp";
    precheck_platform_session(&project_root, platform_name, options.parallel)?;

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
        log_store::write_session(&project_root, &session, platform_name, &ws_url)?;

        println!();
        println!("{}", "Step 2/2: Launching Runner...".bold());
        let mut runner = match runner_host {
            LxAppRunnerHost::MacOs => launch_runner_for_lxapp(&project_root, &ws_url)?,
            LxAppRunnerHost::Windows => launch_windows_runner_for_lxapp(&project_root, &ws_url)?,
        };

        println!();
        println!("{}", "Dev workflow started.".green().bold());
        println!("  {} Mode: {}", "*".bold(), "LxApp Runner".cyan());
        println!("  {} Project: {}", "*".bold(), project_root.display());
        println!(
            "  {} Session file: {}",
            "*".bold(),
            log_store::session_file_path(&project_root, &session.session_id).display()
        );
        println!("  {} WS: {}", "*".bold(), ws_url);
        println!("  {} Log file: {}", "*".bold(), session.log_file.display());
        println!("  {} Session: {}", "*".bold(), session.session_id);
        println!("  {} Stop: {}", "*".bold(), "Ctrl+C or close Runner".cyan());
        println!();

        wait_for_runner_or_interrupt(&mut runner, stop_requested)?;
        Ok(())
    })();

    let _ = log_store::remove_session(&project_root, &session.session_id);
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

/// Local desktop host that runs the standalone-lxapp dev Runner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LxAppRunnerHost {
    MacOs,
    Windows,
}

impl LxAppRunnerHost {
    fn detect() -> Result<Self> {
        if cfg!(target_os = "macos") {
            Ok(Self::MacOs)
        } else if cfg!(target_os = "windows") {
            Ok(Self::Windows)
        } else {
            Err(anyhow!(
                "`lingxia dev` for a standalone lxapp project requires macOS or Windows."
            ))
        }
    }

    fn platform_type(self) -> PlatformType {
        match self {
            Self::MacOs => PlatformType::MacOs,
            Self::Windows => PlatformType::Windows,
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            Self::MacOs => "macOS",
            Self::Windows => "Windows",
        }
    }
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

/// Identity of the lxapp the Windows runner hosts, read from the built
/// bundle manifest (`dist/lxapp.json` preferred, project `lxapp.json`
/// otherwise — same resolution as the runtime's dev bundle source).
struct WindowsRunnerLxAppIdentity {
    app_id: String,
    version: String,
}

fn read_windows_runner_lxapp_identity(lxapp_path: &Path) -> Result<WindowsRunnerLxAppIdentity> {
    let dist_manifest = lxapp_path.join("dist").join("lxapp.json");
    let manifest_path = if dist_manifest.exists() {
        dist_manifest
    } else {
        lxapp_path.join("lxapp.json")
    };
    let content = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("Failed to read {}", manifest_path.display()))?;
    let manifest: serde_json::Value = serde_json::from_str(&content)
        .with_context(|| format!("Invalid JSON in {}", manifest_path.display()))?;
    let field = |name: &str| -> Result<String> {
        manifest
            .get(name)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .ok_or_else(|| anyhow!("Missing or empty \"{name}\" in {}", manifest_path.display()))
    };
    Ok(WindowsRunnerLxAppIdentity {
        app_id: field("appId")?,
        version: field("version")?,
    })
}

/// Prepares the Windows runner's host asset directory under
/// `.lingxia/runner/windows-assets`: an `app.json` with the lxapp as home
/// app and the embedded `bridge-runtime.js`. The lxapp bundle itself is not
/// copied — the runner serves it live from the project's `dist/` via
/// `LINGXIA_LXAPP_PATH` (same dev bundle source as the macOS Runner).
fn prepare_windows_runner_assets(
    lxapp_path: &Path,
    identity: &WindowsRunnerLxAppIdentity,
    ws_url: &str,
) -> Result<PathBuf> {
    let assets_dir = log_store::dev_dir(lxapp_path)
        .join("runner")
        .join("windows-assets");
    std::fs::create_dir_all(&assets_dir)
        .with_context(|| format!("Failed to create {}", assets_dir.display()))?;

    let app_json = serde_json::json!({
        "productName": RUNNER_WINDOWS_PRODUCT_NAME,
        "productVersion": REQUIRED_RUNNER_VERSION,
        "envVersion": "developer",
        "homeAppId": identity.app_id,
        "homeAppVersion": identity.version,
        "devWsUrl": ws_url,
    });
    let app_json_path = assets_dir.join("app.json");
    std::fs::write(&app_json_path, serde_json::to_vec_pretty(&app_json)?)
        .with_context(|| format!("Failed to write {}", app_json_path.display()))?;

    let runtime = crate::runtime::embedded_runtime(crate::runtime::RuntimeEcmaTarget::Es2020);
    let runtime_path = assets_dir.join("bridge-runtime.js");
    std::fs::write(&runtime_path, runtime.bytes)
        .with_context(|| format!("Failed to write {}", runtime_path.display()))?;

    // Runner window/taskbar icon: the LingXia vessel mark, embedded in the
    // CLI so published builds don't depend on the repo's design sources.
    // `lingxia-windows` picks `<assets>/AppIcon.png` up automatically.
    let icon_path = assets_dir.join("AppIcon.png");
    std::fs::write(&icon_path, include_bytes!("../../assets/runner-icon.png"))
        .with_context(|| format!("Failed to write {}", icon_path.display()))?;

    // The runtime's home-app bootstrap installs from `<assets>/<appid>/`
    // before the dev-config override kicks in, so the built bundle is
    // mirrored into the assets as the install source; live edits still
    // come from `dist/` via `LINGXIA_LXAPP_PATH`.
    let bundle_src = lxapp_path.join("dist");
    let bundle_dst = assets_dir.join(&identity.app_id);
    if bundle_dst.exists() {
        std::fs::remove_dir_all(&bundle_dst)
            .with_context(|| format!("Failed to clear {}", bundle_dst.display()))?;
    }
    crate::platform::apple::copy_dir_recursive(&bundle_src, &bundle_dst)?;

    Ok(assets_dir)
}

/// LingXia workspace the Windows runner sources are built from. The CLI is
/// compiled inside that workspace, so `CARGO_MANIFEST_DIR` (= `<workspace>/
/// tools/lingxia-cli` at build time) anchors the lookup.
fn windows_runner_workspace_root() -> Result<PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .ancestors()
        .nth(2)
        .ok_or_else(|| anyhow!("Failed to resolve the LingXia workspace root"))?;
    let runner_manifest = workspace_root
        .join("tools")
        .join("lingxia-runner")
        .join("windows")
        .join("Cargo.toml");
    if !runner_manifest.exists() {
        return Err(anyhow!(
            "LingXia Runner sources not found: {}\n\
             The Windows runner is built from the LingXia workspace this CLI was compiled in;\n\
             keep that checkout available (or rebuild the CLI from your checkout).",
            runner_manifest.display()
        ));
    }
    Ok(workspace_root.to_path_buf())
}

/// Builds the Windows runner bin in release mode from the LingXia workspace
/// and returns the executable path. Respects `CARGO_TARGET_DIR` (and the
/// workspace's `.cargo/config.toml`) through `resolve_cargo_target_dir`.
fn build_windows_runner() -> Result<PathBuf> {
    let workspace_root = windows_runner_workspace_root()?;
    let cargo_target_dir = crate::platform::resolve_cargo_target_dir(&workspace_root);

    println!(
        "{} Building Windows Runner from {}",
        "[runner]".cyan(),
        workspace_root.display()
    );
    let status = Command::new("cargo")
        .current_dir(&workspace_root)
        .env("CARGO_TARGET_DIR", &cargo_target_dir)
        .args([
            "build",
            "--release",
            "--package",
            RUNNER_WINDOWS_PACKAGE,
            "--bin",
            RUNNER_WINDOWS_BIN_NAME,
        ])
        .status()
        .context("Failed to execute cargo build for the Windows Runner")?;
    if !status.success() {
        return Err(anyhow!("Windows Runner cargo build failed"));
    }

    let exe_path = cargo_target_dir
        .join("release")
        .join(format!("{RUNNER_WINDOWS_BIN_NAME}.exe"));
    if !exe_path.exists() {
        return Err(anyhow!(
            "Windows Runner executable not found after build: {}",
            exe_path.display()
        ));
    }
    Ok(exe_path)
}

/// Windows counterpart of `launch_runner_for_lxapp`: prepares the runner
/// asset layout, builds the runner bin, and spawns it against the dev
/// server. Env contract: `LINGXIA_ASSET_DIR` (host assets),
/// `LINGXIA_LXAPP_PATH` (live lxapp bundle), `LINGXIA_DEV_WS_URL`
/// (devtool bridge), `LINGXIA_APP_ID` / `LINGXIA_PRODUCT_NAME` (identity
/// and state-root scoping).
fn launch_windows_runner_for_lxapp(lxapp_path: &Path, ws_url: &str) -> Result<Child> {
    ensure_valid_lxapp_dir(lxapp_path)?;
    let identity = read_windows_runner_lxapp_identity(lxapp_path)?;
    let assets_dir = prepare_windows_runner_assets(lxapp_path, &identity, ws_url)?;
    let exe_path = build_windows_runner()?;

    let mut command = Command::new(&exe_path);
    command.env(RUNNER_LXAPP_PATH_ENV, lxapp_path);
    command.env(RUNNER_DEV_WS_URL_ENV, ws_url);
    command.env("LINGXIA_ASSET_DIR", &assets_dir);
    command.env("LINGXIA_APP_ID", &identity.app_id);
    command.env("LINGXIA_PRODUCT_NAME", RUNNER_WINDOWS_PRODUCT_NAME);
    command.stdin(Stdio::null());
    command.stdout(Stdio::inherit());
    command.stderr(Stdio::inherit());

    let child = command.spawn().with_context(|| {
        format!(
            "Failed to launch Windows Runner executable: {}",
            exe_path.display()
        )
    })?;

    println!("{} Launched {}", "[runner]".cyan(), exe_path.display());
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

fn wait_for_interrupt(stop_requested: Arc<AtomicBool>) -> Result<()> {
    while !stop_requested.load(Ordering::Acquire) {
        thread::sleep(Duration::from_millis(150));
    }
    println!();
    println!("{}", "Dev workflow stopped.".yellow().bold());
    Ok(())
}

fn stop_dev_server(server: server::DevServerHandle, run_result: Result<()>) -> Result<()> {
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

fn loopback_ws_url(port: u16) -> String {
    format!("ws://127.0.0.1:{port}")
}

fn lan_ws_url(port: u16) -> Result<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0")
        .context("Failed to create UDP socket for host address detection")?;
    if let Err(err) = socket.connect("8.8.8.8:80") {
        eprintln!(
            "{} Failed to detect LAN address ({}); falling back to localhost. Use a reachable host address if your device cannot connect.",
            "Warning:".yellow(),
            err
        );
        return Ok(loopback_ws_url(port));
    }
    match socket.local_addr() {
        Ok(addr) => Ok(format!("ws://{}:{port}", addr.ip())),
        Err(err) => {
            eprintln!(
                "{} Failed to read LAN address ({}); falling back to localhost. Use a reachable host address if your device cannot connect.",
                "Warning:".yellow(),
                err
            );
            Ok(loopback_ws_url(port))
        }
    }
}

fn print_mobile_dev_started(
    platform: &str,
    artifact: &Path,
    host_ws_url: &str,
    device_ws_url: &str,
    project_root: &Path,
    session: &log_store::DevLogSession,
) {
    println!();
    println!("{}", "Dev workflow started.".green().bold());
    println!("  {} Platform: {}", "*".bold(), platform.cyan());
    println!("  {} Artifact: {}", "*".bold(), artifact.display());
    println!(
        "  {} Session file: {}",
        "*".bold(),
        log_store::session_file_path(project_root, &session.session_id).display()
    );
    println!("  {} Host WS: {}", "*".bold(), host_ws_url);
    println!("  {} Device WS: {}", "*".bold(), device_ws_url);
    println!("  {} Log file: {}", "*".bold(), session.log_file.display());
    println!("  {} Session: {}", "*".bold(), session.session_id);
    println!("  {} Stop: {}", "*".bold(), "Ctrl+C".cyan());
    println!();
}

struct DevPortForward {
    cleanup: Option<PortForwardCleanup>,
}

enum PortForwardCleanup {
    Android { device: Option<String>, port: u16 },
    Harmony { device: Option<String>, port: u16 },
}

impl DevPortForward {
    fn android(device: Option<&str>, port: u16) -> Result<Self> {
        let _ = run_adb_reverse_remove(device, port);
        run_adb_reverse(device, port)?;
        println!("  {} adb reverse tcp:{port} -> tcp:{port}", "✓".green());
        Ok(Self {
            cleanup: Some(PortForwardCleanup::Android {
                device: device.map(ToOwned::to_owned),
                port,
            }),
        })
    }

    fn harmony(device: Option<&str>, port: u16) -> Result<Self> {
        let _ = run_hdc_reverse_remove(device, port);
        run_hdc_reverse(device, port)?;
        println!("  {} hdc rport tcp:{port} -> tcp:{port}", "✓".green());
        Ok(Self {
            cleanup: Some(PortForwardCleanup::Harmony {
                device: device.map(ToOwned::to_owned),
                port,
            }),
        })
    }
}

impl Drop for DevPortForward {
    fn drop(&mut self) {
        match self.cleanup.take() {
            Some(PortForwardCleanup::Android { device, port }) => {
                let _ = run_adb_reverse_remove(device.as_deref(), port);
            }
            Some(PortForwardCleanup::Harmony { device, port }) => {
                let _ = run_hdc_reverse_remove(device.as_deref(), port);
            }
            None => {}
        }
    }
}

fn adb_command(device: Option<&str>) -> Command {
    let mut command = Command::new("adb");
    if let Some(device) = device {
        command.arg("-s").arg(device);
    }
    command
}

fn run_adb_reverse(device: Option<&str>, port: u16) -> Result<()> {
    let output = adb_command(device)
        .args(["reverse", &format!("tcp:{port}"), &format!("tcp:{port}")])
        .output()
        .context("Failed to execute adb reverse")?;
    ensure_command_success(output, "adb reverse")
}

fn run_adb_reverse_remove(device: Option<&str>, port: u16) -> Result<()> {
    let output = adb_command(device)
        .args(["reverse", "--remove", &format!("tcp:{port}")])
        .output()
        .context("Failed to execute adb reverse --remove")?;
    ensure_command_success(output, "adb reverse --remove")
}

fn hdc_command(device: Option<&str>) -> Command {
    let mut command = Command::new("hdc");
    if let Some(device) = device {
        command.arg("-t").arg(device);
    }
    command
}

fn run_hdc_reverse(device: Option<&str>, port: u16) -> Result<()> {
    let output = hdc_command(device)
        .args(["rport", &format!("tcp:{port}"), &format!("tcp:{port}")])
        .output()
        .context("Failed to execute hdc rport")?;
    ensure_command_success(output, "hdc rport")
}

fn run_hdc_reverse_remove(device: Option<&str>, port: u16) -> Result<()> {
    let output = hdc_command(device)
        .args(["fport", "rm", &format!("tcp:{port} tcp:{port}")])
        .output()
        .context("Failed to execute hdc fport rm")?;
    ensure_command_success(output, "hdc fport rm")
}

fn ensure_command_success(output: std::process::Output, label: &str) -> Result<()> {
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    Err(anyhow!(
        "{label} failed\nstdout: {}\nstderr: {}",
        stdout.trim(),
        stderr.trim()
    ))
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
