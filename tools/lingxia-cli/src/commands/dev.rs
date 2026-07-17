use crate::commands::rust::resolve_build_profile;
use crate::config::{LingXiaConfig, append_native_features, has_host_config};
use crate::host_assets::{prepare_configured_host_assets, prepare_windows_design_icon_assets};
use crate::lxapp::ProjectFramework;
use crate::platform::detector::PlatformType;
use crate::platform::{self, BuildConfig, BuildProfile, InstallConfig, Platform, RunConfig};
use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Local, TimeZone};
use colored::Colorize;
use lingxia_log::now_timestamp_ms;
use std::env;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
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
const BACKGROUND_CHILD_ENV: &str = "LINGXIA_DEV_BACKGROUND_CHILD";
const BACKGROUND_START_TIMEOUT: Duration = Duration::from_secs(600);

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
    pub framework: Option<String>,
    pub progress: Option<String>,
    pub device: Option<String>,
    pub platform_arg: Option<String>,
    pub reinstall: bool,
    pub env_version: Option<String>,
    pub extra_native_features: Vec<String>,
    pub with_provider: Vec<String>,
    pub provider_path: Option<String>,
    /// Runner simulator device (macOS lxapp runner only), e.g. `desktop-1440`.
    pub runner_device: Option<String>,
    pub background: bool,
    pub action: Option<DevSessionAction>,
}

pub enum DevSessionAction {
    Status { json: bool },
    Stop { session: Option<String> },
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
}

/// Per-user token authenticating a physical iOS device's connection to the
/// host-side dev server.
fn persistent_device_token() -> Result<String> {
    let dir = crate::platform::apple::auth::apple_credentials_dir()?;
    std::fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    let path = dir.join("dev-device-token");
    if let Ok(existing) = std::fs::read_to_string(&path) {
        let existing = existing.trim();
        if !existing.is_empty() {
            return Ok(existing.to_string());
        }
    }
    let token = uuid::Uuid::new_v4().simple().to_string();
    std::fs::write(&path, &token)
        .with_context(|| format!("Failed to persist device token at {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(token)
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
        true,
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

/// Stop any live dev session for the same platform in this project before a
/// new one starts. A session is bound to its platform, so a second
/// same-platform session is never wanted — and re-running `lingxia dev` after
/// a host-code edit is the normal inner loop, so the restart intent is
/// unambiguous: take over rather than refuse. Different platforms don't
/// conflict — `lingxia dev -p android` and `-p ios` run side by side.
///
/// Liveness comes from the broker registration: a session that exited (or
/// crashed) has already dropped off the list. Waits until the old session
/// deregisters so the new one never races it.
fn precheck_platform_session(project_root: &Path, platform: &str) -> Result<()> {
    let live = log_store::find_live_for_platform(project_root, platform)?;
    if live.is_empty() {
        return Ok(());
    }
    for info in &live {
        println!(
            "Stopping existing {platform} dev session {} (pid {})...",
            info.session_id, info.pid
        );
        if let Err(err) = log_store::request_shutdown(info) {
            eprintln!("Graceful stop failed ({err:#}); killing pid {}.", info.pid);
            terminate_session_owner(info)?;
        }
    }
    if wait_for_platform_sessions_gone(project_root, platform, Duration::from_secs(10))? {
        return Ok(());
    }
    // Graceful shutdown stalled — escalate to a kill, then give the broker a
    // moment to drop the registrations.
    for info in log_store::find_live_for_platform(project_root, platform)? {
        terminate_session_owner(&info)?;
    }
    if wait_for_platform_sessions_gone(project_root, platform, Duration::from_secs(5))? {
        return Ok(());
    }
    Err(anyhow!(
        "Existing {platform} dev session did not stop after automatic termination."
    ))
}

fn wait_for_platform_sessions_gone(
    project_root: &Path,
    platform: &str,
    timeout: Duration,
) -> Result<bool> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if log_store::find_live_for_platform(project_root, platform)?.is_empty() {
            return Ok(true);
        }
        if std::time::Instant::now() >= deadline {
            return Ok(false);
        }
        thread::sleep(Duration::from_millis(200));
    }
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

    if let Some(action) = options.action {
        return execute_session_action(&project_root, action);
    }

    if options
        .runner_device
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        runner::print_runner_devices()?;
        return Ok(());
    }

    if options.background && env::var_os(BACKGROUND_CHILD_ENV).is_none() {
        return spawn_background_dev(&project_root);
    }

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
    };

    let result = match platform_type {
        PlatformType::Android => android::execute_android(ctx),
        PlatformType::Ios => ios::execute_ios(ctx),
        PlatformType::MacOs => macos::execute_macos(ctx),
        PlatformType::Harmony => harmony::execute_harmony(ctx),
        PlatformType::Windows => windows::execute_windows(ctx),
    };
    drop(provider_guard);
    result
}

#[cfg(target_os = "windows")]
pub fn focus_windows_launch(executable: &Path, excluded_pids: &str) -> Result<()> {
    runner::focus_windows_launch(executable, excluded_pids)
}

fn execute_session_action(project_root: &Path, action: DevSessionAction) -> Result<()> {
    match action {
        DevSessionAction::Status { json } => print_session_status(project_root, json),
        DevSessionAction::Stop { session } => stop_session(project_root, session),
    }
}

fn spawn_background_dev(project_root: &Path) -> Result<()> {
    let log_dir = log_store::dev_dir(project_root).join("background");
    fs::create_dir_all(&log_dir)
        .with_context(|| format!("Failed to create {}", log_dir.display()))?;
    // Prune old background launch logs under the same retention as session logs
    // so repeated `--background` starts don't grow `.lingxia/dev/background/`
    // without bound.
    let _ = log_store::cleanup_old_logs(&log_dir, log_store::DEFAULT_LOG_RETENTION_DAYS);
    let started_at = now_timestamp_ms();
    let log_path = log_dir.join(format!("dev-{started_at}.log"));
    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("Failed to open {}", log_path.display()))?;
    let stderr = stdout
        .try_clone()
        .with_context(|| format!("Failed to clone {}", log_path.display()))?;

    let mut command = Command::new(env::current_exe().context("Failed to resolve current exe")?);
    command
        .args(background_child_args())
        .current_dir(project_root)
        .env(BACKGROUND_CHILD_ENV, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    configure_background_process(&mut command);

    let mut child = command.spawn().with_context(|| {
        format!(
            "Failed to start background dev process; log path {}",
            log_path.display()
        )
    })?;
    let pid = child.id();
    println!("Started background dev process pid {pid}.");
    println!("  log: {}", log_path.display());

    match wait_for_background_session(project_root, &mut child, started_at)? {
        Some(session) => {
            println!("Dev session is ready.");
            println!("  id: {}", session.session_id);
            println!("  target: {}", session.target);
            println!("  ws: {}", session.ws_url);
            println!("  session log: {}", session.log_file);
            println!("Use `lxdev logs -f` to follow logs.");
            println!("Use `lingxia dev stop {}` to stop it.", session.session_id);
        }
        None => {
            println!("Background dev process is still starting.");
            println!("Use `lingxia dev status` to check readiness.");
        }
    }
    Ok(())
}

#[cfg(unix)]
fn configure_background_process(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

#[cfg(windows)]
fn configure_background_process(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    // DETACHED_PROCESS can still flash a console when this console-subsystem
    // child launches build tools. CREATE_NO_WINDOW keeps the background owner
    // headless while the independent process group preserves stop isolation.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
}

fn background_child_args() -> Vec<OsString> {
    env::args_os()
        .skip(1)
        .filter(|arg| {
            let value = arg.to_string_lossy();
            value != "--background" && !value.starts_with("--background=")
        })
        .collect()
}

fn wait_for_background_session(
    project_root: &Path,
    child: &mut Child,
    started_at: u64,
) -> Result<Option<log_store::SessionInfo>> {
    let deadline = std::time::Instant::now() + BACKGROUND_START_TIMEOUT;
    loop {
        if let Some(status) = child
            .try_wait()
            .context("Failed to poll background dev process")?
        {
            return Err(anyhow!(
                "Background dev process exited before it became ready: {status}"
            ));
        }

        for session in log_store::list_sessions(project_root)? {
            if session.started_at >= started_at
                && log_store::session_state(&session) == log_store::DevSessionState::Ready
            {
                return Ok(Some(session));
            }
        }

        if std::time::Instant::now() >= deadline {
            return Ok(None);
        }

        thread::sleep(Duration::from_millis(250));
    }
}

fn print_session_status(project_root: &Path, json_output: bool) -> Result<()> {
    let sessions = log_store::list_sessions(project_root)?;
    if json_output {
        let values: Vec<serde_json::Value> = sessions
            .iter()
            .map(|session| {
                let state = log_store::session_state(session);
                serde_json::json!({
                    "session_id": session.session_id,
                    "pid": session.pid,
                    "target": session.target,
                    "started_at": session.started_at,
                    "ws_url": session.ws_url,
                    "log_file": session.log_file,
                    "state": state.as_str(),
                    "runtime_connected": state == log_store::DevSessionState::Ready,
                    "stale": state == log_store::DevSessionState::Stale,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&values)?);
        return Ok(());
    }

    if sessions.is_empty() {
        println!("No active dev sessions.");
        return Ok(());
    }

    println!(
        "{:<8}  {:<8}  {:<8}  {:<19}  {:<22}  PID",
        "ID", "STATE", "TARGET", "STARTED", "WS"
    );
    for session in &sessions {
        let state = log_store::session_state(session).as_str();
        println!(
            "{:<8}  {:<8}  {:<8}  {:<19}  {:<22}  {}",
            session.session_id,
            state,
            session.target,
            format_started(session.started_at),
            session.ws_url,
            session.pid,
        );
        println!("  log: {}", session.log_file);
    }
    println!();
    println!("Use `lxdev logs -f` to follow session logs.");
    Ok(())
}

fn stop_session(project_root: &Path, selector: Option<String>) -> Result<()> {
    let session = log_store::resolve_session(project_root, selector.as_deref())?;
    println!(
        "Stopping {} dev session {}...",
        session.target, session.session_id
    );
    match log_store::request_shutdown(&session) {
        Ok(()) => {
            if wait_for_pid_exit(sysinfo::Pid::from_u32(session.pid), Duration::from_secs(5)) {
                println!(
                    "Stopped {} dev session {}.",
                    session.target, session.session_id
                );
                return Ok(());
            }
            eprintln!(
                "Graceful shutdown timed out; terminating session owner pid {}.",
                session.pid
            );
        }
        Err(err) => eprintln!(
            "Graceful shutdown unavailable ({err:#}); terminating session owner pid {}.",
            session.pid
        ),
    }
    terminate_session_owner(&session)
}

fn terminate_session_owner(session: &log_store::SessionInfo) -> Result<()> {
    let pid = sysinfo::Pid::from_u32(session.pid);
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);

    let Some(process) = system.process(pid) else {
        println!("Session process {} is not running.", session.pid);
        return Ok(());
    };

    // Guard against PID reuse: only kill if this really is the `lingxia dev`
    // process that registered the session.
    if !is_owning_dev_process(process, session) {
        return Err(anyhow!(
            "Refusing to terminate pid {} because it no longer matches dev session {}",
            session.pid,
            session.session_id
        ));
    }

    // Prefer a graceful interrupt: the dev process's Ctrl+C handler runs its own
    // teardown (terminates the app/Runner, drops port forwards), so children
    // aren't orphaned. Fall back to SIGKILL only if
    // it doesn't exit in time or the platform can't deliver the signal.
    let interrupted = process.kill_with(Signal::Interrupt).unwrap_or(false);
    if !(interrupted && wait_for_pid_exit(pid, Duration::from_secs(3))) {
        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
        if let Some(process) = system.process(pid) {
            process.kill();
        }
        if !wait_for_pid_exit(pid, Duration::from_secs(2)) {
            return Err(anyhow!("Failed to kill session pid {}", session.pid));
        }
    }

    println!(
        "Terminated {} dev session {} (pid {}).",
        session.target, session.session_id, session.pid
    );
    Ok(())
}

/// Whether `process` is still the `lingxia dev` process that wrote `session`.
/// Defends `terminate_session_owner` against PID reuse by requiring the same
/// executable and a process start time compatible with the recorded session.
fn is_owning_dev_process(process: &sysinfo::Process, session: &log_store::SessionInfo) -> bool {
    let Some(exe) = process.exe() else {
        return false;
    };
    let expected = if session.executable.is_empty() {
        let Ok(current) = env::current_exe() else {
            return false;
        };
        current
    } else {
        PathBuf::from(&session.executable)
    };
    if !process_executable_matches(exe, &expected) {
        return false;
    }
    let started_at = process.start_time();
    if started_at == 0 {
        return false;
    }
    // A process reusing this PID must have started after the original session
    // registered. Allow only sub-second timestamp slack around registration;
    // the real owner may be much older because it registers after its build.
    started_at <= session.started_at / 1000 + 2
}

/// Poll until the given pid is gone (or reaped to a zombie), up to `timeout`.
fn wait_for_pid_exit(pid: sysinfo::Pid, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
        match system.process(pid) {
            None => return true,
            Some(process) if process.status() == sysinfo::ProcessStatus::Zombie => return true,
            Some(_) => {}
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn format_started(started_at: u64) -> String {
    let secs = (started_at / 1000) as i64;
    let nsecs = ((started_at % 1000) * 1_000_000) as u32;
    match Local.timestamp_opt(secs, nsecs).single() {
        Some(dt) => {
            let dt: DateTime<Local> = dt;
            dt.format("%Y-%m-%d %H:%M:%S").to_string()
        }
        None => started_at.to_string(),
    }
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
    match detect_lan_ip() {
        Some(ip) => Ok(format!("ws://{ip}:{port}")),
        None => {
            eprintln!(
                "{} Failed to detect a LAN address; falling back to localhost. Use a reachable host address if your device cannot connect.",
                "Warning:".yellow(),
            );
            Ok(loopback_ws_url(port))
        }
    }
}

/// Address LAN peers should dial. Prefers a private (RFC1918) interface: the
/// default-route probe follows VPN/proxy tunnels whose address LAN peers
/// cannot reach, so a non-private result only wins when nothing private
/// exists. Among private candidates, real-LAN ranges beat the 172.16/12
/// block that virtual adapters (WSL, Docker) commonly squat on.
fn detect_lan_ip() -> Option<std::net::IpAddr> {
    let default_route_ip = std::net::UdpSocket::bind("0.0.0.0:0")
        .ok()
        .and_then(|socket| {
            socket.connect("8.8.8.8:80").ok()?;
            Some(socket.local_addr().ok()?.ip())
        });
    if let Some(ip) = default_route_ip
        && is_private_v4(&ip)
    {
        return Some(ip);
    }

    let hostname = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()?;
    let candidates: Vec<std::net::IpAddr> =
        std::net::ToSocketAddrs::to_socket_addrs(&(hostname.as_str(), 0u16))
            .ok()?
            .map(|addr| addr.ip())
            .filter(is_private_v4)
            .collect();
    let preferred = |prefix: fn(&std::net::Ipv4Addr) -> bool| {
        candidates.iter().copied().find(|ip| match ip {
            std::net::IpAddr::V4(v4) => prefix(v4),
            std::net::IpAddr::V6(_) => false,
        })
    };
    preferred(|v4| v4.octets()[0] == 192)
        .or_else(|| preferred(|v4| v4.octets()[0] == 10))
        .or_else(|| candidates.first().copied())
        .or(default_route_ip)
}

fn is_private_v4(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => v4.is_private(),
        std::net::IpAddr::V6(_) => false,
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
