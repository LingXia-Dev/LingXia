use crate::commands::rust::resolve_build_profile;
use crate::config::{LingXiaConfig, append_native_features, has_host_config};
use crate::host_assets::{prepare_configured_host_assets, prepare_windows_design_icon_assets};
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
mod lxapp_manifest;
mod server;

mod android;
mod forward;
mod harmony;
mod ios;
mod macos;
mod runner;
mod windows;

const RUNNER_DEV_WS_URL_ENV: &str = "LINGXIA_DEV_WS_URL";

fn dev_native_features(
    config: &LingXiaConfig,
    platform: &str,
    extra_features: &[String],
) -> Vec<String> {
    let mut features = config.native_features_for_platform(platform);
    append_native_features(&mut features, extra_features);
    if !features.iter().any(|feature| feature == "devtools") {
        features.push("devtools".to_string());
    }
    println!(
        "  {} Native features ({}): {}",
        "•".cyan(),
        platform,
        if features.is_empty() {
            "<none>".to_string()
        } else {
            features.join(",")
        }
    );
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
    pub extra_native_features: Vec<String>,
    pub with_provider: Vec<String>,
    pub provider_path: Option<String>,
    pub parallel: bool,
    /// Runner simulator device (macOS lxapp runner only), e.g. `desktop-1440`.
    pub runner_device: Option<String>,
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
    extra_native_features: Vec<String>,
    parallel: bool,
}

fn prepare_dev_host_assets(
    ctx: &DevContext,
    platforms_to_build: &[PlatformType],
    build_targets: &[String],
    dev_ws_url: Option<&str>,
) -> Result<()> {
    prepare_configured_host_assets(
        &ctx.project_root,
        &ctx.config,
        ctx.build_profile,
        ctx.framework,
        ctx.progress.as_deref(),
        platforms_to_build,
        build_targets,
        true,
        dev_ws_url,
        &ctx.resolved_env,
    )?;
    if dev_ws_url.is_some() {
        let dev_manifests =
            lxapp_manifest::write_configured_manifests(&ctx.project_root, &ctx.config)?;
        for manifest in &dev_manifests {
            println!(
                "  {} dev manifest {} ({})",
                "*".cyan(),
                manifest.app_id,
                manifest.dist_hash
            );
        }
    }
    Ok(())
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

    if runner::is_standalone_lxapp_project(&project_root) {
        return runner::execute_lxapp_dev(project_root, options);
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
    crate::commands::build::validate_extra_native_features(
        &project_root,
        &config,
        std::slice::from_ref(&platform_type),
        &options.extra_native_features,
    )?;

    // Inject requested provider crate(s); guard restores on drop after serving
    // stops (ctrlc-driven graceful return).
    let provider_guard = if options.with_provider.is_empty() {
        None
    } else {
        let rust_lib_name = config.get_rust_lib_name().ok_or_else(|| {
            anyhow!("app.projectName or app.rustLibDir is required to inject a provider")
        })?;
        let native_crate_dir = project_root.join(&rust_lib_name);
        crate::commands::provider::inject(
            &native_crate_dir,
            &options.with_provider,
            options.provider_path.as_deref(),
        )?
    };
    let mut extra_native_features = options.extra_native_features;
    if let Some(guard) = &provider_guard {
        for feature in guard.features() {
            if !extra_native_features.contains(feature) {
                extra_native_features.push(feature.clone());
            }
        }
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
        resolved_env,
        extra_native_features,
        parallel: options.parallel,
    };

    let result = match platform_type {
        PlatformType::Android => android::execute_android(ctx, options.abis),
        PlatformType::Ios => ios::execute_ios(ctx),
        PlatformType::MacOs => macos::execute_macos(ctx, options.macos_arch),
        PlatformType::Harmony => harmony::execute_harmony(ctx),
        PlatformType::Windows => windows::execute_windows(ctx),
    };
    drop(provider_guard);
    result
}
fn install_ctrlc_handler(stop_requested: Arc<AtomicBool>) -> Result<()> {
    ctrlc::set_handler(move || {
        stop_requested.store(true, Ordering::Release);
    })
    .context("Failed to install Ctrl+C handler for dev mode")
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

/// Concise "dev started" banner. Session machinery (WS URLs, session id/file, log
/// path) is omitted on purpose — `lxdev` auto-discovers the running session from
/// `.lingxia/sessions/`, so the only thing the user needs is how to drive it.
fn print_dev_banner(label: &str, stop_hint: &str, extra: &[(&str, &str)]) {
    println!();
    println!(
        "{}   {}",
        "Dev workflow started.".green().bold(),
        label.cyan()
    );
    for (key, value) in extra {
        println!("  {}  {}", format!("{key}:").bold(), value.cyan());
    }
    println!(
        "  {}  {}   (run from the project root; --session to pick one)",
        "Control:".bold(),
        "lxdev <logs | lxapp | app | browser>".cyan(),
    );
    println!("  {}  {}", "Stop:".bold(), stop_hint.cyan());
    println!();
}

fn print_mobile_dev_started(platform: &str, extra: &[(&str, &str)]) {
    print_dev_banner(platform, "Ctrl+C", extra);
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
fn canonical_path_or_self(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn process_executable_matches(process_exe: &Path, executable_path: &Path) -> bool {
    canonical_path_or_self(process_exe) == canonical_path_or_self(executable_path)
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

#[cfg(test)]
mod tests {
    use super::process_executable_matches;
    use std::path::Path;

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
