use super::*;

#[cfg(target_os = "windows")]
mod windows_interactive;

#[cfg(target_os = "windows")]
pub(super) fn focus_ssh_runner_window(pid: u32) -> Result<()> {
    windows_interactive::focus_windows_process(pid)
}

const RUNNER_APP_NAME: &str = "LingXia Runner.app";
const RUNNER_EXECUTABLE_NAME: &str = "LingXiaRunner";
const RUNNER_LXAPP_PATH_ENV: &str = "LINGXIA_LXAPP_PATH";
const RUNNER_DEV_WS_URL_ENV: &str = "LINGXIA_DEV_WS_URL";
const RUNNER_LINGXIAO_MOCK_DIR_ENV: &str = "LINGXIAO_MOCK_DIR";
const RUNNER_ENV_ENV: &str = "LINGXIA_RUNNER_ENV";
/// Marks the child process as the LingXia Runner (vs a real host app). The core
/// runtime injects `runner:true` into `__LX_BRIDGE_CFG` so the View bridge can
/// expose `platform.isRunner()`; the Runner lacks host-declared surfaces like
/// the terminal, so apps use this to hide those affordances.
const RUNNER_MARKER_ENV: &str = "LINGXIA_RUNNER";
/// Per-project file the Runner writes its own pid into on startup. Lets the CLI
/// terminate *this* project's Runner without touching Runners from other
/// projects — so `lingxia dev` in two lxapp dirs runs two Runners in parallel.
const RUNNER_PID_FILE_ENV: &str = "LINGXIA_RUNNER_PID_FILE";
/// Stable-per-project id that isolates each Runner instance's on-disk state
/// (metadata DB, caches, WebView data) so parallel Runners don't collide.
const RUNNER_INSTANCE_ENV: &str = "LINGXIA_RUNNER_INSTANCE";
const REQUIRED_RUNNER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Windows runner: standalone executable installed by
/// `tools/lingxia-runner/windows/install-local-runner.ps1`.
const RUNNER_WINDOWS_BIN_NAME: &str = "lingxia-runner";
const RUNNER_WINDOWS_PRODUCT_NAME: &str = "LingXia Runner";
const RUNNER_WINDOWS_APP_ID: &str = "app.lingxia.runner";
const RUNNER_DEVICES_JSON: &str = include_str!("../../../../lingxia-runner/devices.json");

#[derive(Debug, serde::Deserialize)]
struct RunnerDevicesManifest {
    default: String,
    devices: Vec<RunnerDevicePreset>,
}

#[derive(Debug, serde::Deserialize)]
struct RunnerDevicePreset {
    id: String,
    group: String,
    name: String,
    width: u32,
    height: u32,
}

pub(super) fn print_runner_devices() -> Result<()> {
    println!("{}", render_runner_devices()?);
    Ok(())
}

fn render_runner_devices() -> Result<String> {
    let manifest: RunnerDevicesManifest =
        serde_json::from_str(RUNNER_DEVICES_JSON).context("runner devices.json must be valid")?;
    let id_width = manifest
        .devices
        .iter()
        .map(|device| device.id.len())
        .max()
        .unwrap_or("device".len())
        .max("device".len());
    let group_width = manifest
        .devices
        .iter()
        .map(|device| device.group.len())
        .max()
        .unwrap_or("group".len())
        .max("group".len());

    let mut out = String::from("Runner devices:\n");
    out.push_str(&format!(
        "  {:id_width$}  {:group_width$}  {:>11}  {}\n",
        "device",
        "group",
        "size",
        "name",
        id_width = id_width,
        group_width = group_width
    ));
    for device in &manifest.devices {
        let marker = if device.id == manifest.default {
            " (default)"
        } else {
            ""
        };
        out.push_str(&format!(
            "  {:id_width$}  {:group_width$}  {:>4} x {:<4}  {}{}\n",
            device.id,
            device.group,
            device.width,
            device.height,
            device.name,
            marker,
            id_width = id_width,
            group_width = group_width
        ));
    }
    out.push_str("\nUse `lingxia dev --runner <device>` to launch a specific runner device.");
    Ok(out)
}

pub(super) fn execute_lxapp_dev(project_root: PathBuf, options: DevExecuteOptions) -> Result<()> {
    let runner_host = LxAppRunnerHost::detect()?;
    let runner_env = options
        .env_version
        .as_deref()
        .map(crate::config::EnvVersion::parse_cli)
        .transpose()?
        .unwrap_or(crate::config::EnvVersion::Developer);

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
    precheck_platform_session(&project_root, platform_name)?;

    println!();
    println!("{}", "Development Mode: LxApp -> Runner".bold().cyan());
    println!();

    let stop_requested = Arc::new(AtomicBool::new(false));
    let server = server::start_server_fixed_with_stop(
        &project_root,
        "127.0.0.1",
        platform_name,
        stop_requested.clone(),
        None,
    )?;
    let ws_url = server.ws_url();
    let session = server.session().clone();

    let build_args = lxapp_runner_build_args(
        options.release,
        options.framework.as_deref(),
        options.progress.as_deref(),
        runner_env,
    );

    let run_result = (|| -> Result<()> {
        println!("{}", "Step 1/2: Building lxapp...".bold());
        crate::lxapp::run_in_dir(&build_args, &project_root)?;

        install_ctrlc_handler(stop_requested.clone())?;
        let _session_registration =
            log_store::register_session(&project_root, &session, platform_name, &ws_url);

        println!();
        println!("{}", "Step 2/2: Launching Runner...".bold());
        let mut runner = match runner_host {
            LxAppRunnerHost::MacOs => launch_runner_for_lxapp(
                &project_root,
                &ws_url,
                options.runner_device.as_deref(),
                runner_env,
            )?,
            LxAppRunnerHost::Windows => launch_windows_runner_for_lxapp(
                &project_root,
                &ws_url,
                options.runner_device.as_deref(),
                runner_env,
            )?,
        };

        print_dev_banner("LxApp Runner", "Ctrl+C or `lingxia dev stop`", &[]);

        wait_for_runner_or_interrupt(&mut runner, stop_requested)?;
        Ok(())
    })();

    let stop_result = server.stop();
    match (run_result, stop_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(err), Ok(())) => Err(err),
        (Ok(()), Err(err)) => Err(err),
        (Err(run_err), Err(stop_err)) => {
            eprintln!("Also failed to stop dev server: {stop_err:#}");
            Err(run_err)
        }
    }
}

fn lxapp_runner_build_args(
    release: bool,
    framework: Option<&str>,
    progress: Option<&str>,
    runner_env: crate::config::EnvVersion,
) -> Vec<String> {
    let mut args = vec![
        "build".to_string(),
        "--env".to_string(),
        runner_env.as_str().to_string(),
    ];
    if release {
        args.push("--release".to_string());
    }
    if let Some(framework) = framework {
        args.push("--framework".to_string());
        args.push(framework.to_string());
    }
    if let Some(progress) = progress {
        args.push("--progress".to_string());
        args.push(progress.to_string());
    }
    args
}

pub(super) fn is_standalone_lxapp_project(project_root: &Path) -> bool {
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

fn launch_runner_for_lxapp(
    lxapp_path: &Path,
    ws_url: &str,
    runner_device: Option<&str>,
    runner_env: crate::config::EnvVersion,
) -> Result<RunnerProcess> {
    platform::apple::ensure_macos()?;
    ensure_valid_lxapp_dir(lxapp_path)?;
    // Provision the runner from the matching release if it isn't installed yet
    // (end users install only the CLI; this self-heals the first `lingxia dev`).
    crate::runner_cache::ensure_runner(REQUIRED_RUNNER_VERSION, false)?;
    let app_path = installed_runner_app_path()?;
    ensure_runner_matches_cli(&app_path)?;

    // Replace only *this* project's prior Runner (a leftover from a crashed
    // session); Runners started for other projects keep running in parallel.
    let pid_file = runner_pid_file(lxapp_path);
    terminate_runner_from_pid_file(&pid_file);

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
    command.env(RUNNER_MARKER_ENV, "1");
    command.env(RUNNER_LXAPP_PATH_ENV, lxapp_path);
    command.env(RUNNER_DEV_WS_URL_ENV, ws_url);
    command.env(RUNNER_ENV_ENV, runner_env.as_str());
    command.env(RUNNER_PID_FILE_ENV, &pid_file);
    command.env(RUNNER_INSTANCE_ENV, runner_instance_id(lxapp_path));
    if let Some(device) = runner_device.map(str::trim).filter(|s| !s.is_empty()) {
        command.env("LINGXIA_RUNNER_DEVICE", device);
    }
    // Cloud worker: transpile mocks + generate typed `lx.cloud.invoke`, then
    // point the runner at the loadable mock dir (it reads routing from worker.json).
    if let Some(mock_dir) = crate::lxapp::worker::prepare_dev(lxapp_path) {
        command.env(RUNNER_LINGXIAO_MOCK_DIR_ENV, &mock_dir);
        println!(
            "  {} Cloud functions (mock): {}",
            "*".cyan(),
            mock_dir.display()
        );
    }
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
    Ok(RunnerProcess::MacOsApp {
        child,
        pid_file,
        seen_running: false,
        handoff_deadline: std::time::Instant::now() + Duration::from_secs(5),
    })
}

/// Identity of the lxapp the Windows runner hosts, read from the built
/// bundle manifest (`dist/lxapp.json` preferred, project `lxapp.json`
/// otherwise — same resolution as the runtime's dev bundle source).
struct WindowsRunnerLxAppIdentity {
    app_id: String,
    version: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct WindowsRunnerResourceLxAppPath {
    app_id: String,
    path: String,
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
    runner_env: crate::config::EnvVersion,
) -> Result<PathBuf> {
    let assets_dir = log_store::dev_dir(lxapp_path)
        .join("runner")
        .join("windows-assets");
    if assets_dir.exists() {
        std::fs::remove_dir_all(&assets_dir)
            .with_context(|| format!("Failed to clear {}", assets_dir.display()))?;
    }
    std::fs::create_dir_all(&assets_dir)
        .with_context(|| format!("Failed to create {}", assets_dir.display()))?;

    // Per-lxapp identity: the window title / About dialog name which app this
    // runner hosts, and the windowsAppId becomes the process's taskbar
    // AppUserModelID — so two runners (different projects) show as two separate
    // taskbar apps instead of grouping under the shared runner executable.
    let app_json = serde_json::json!({
        "productName": format!("{} - {RUNNER_WINDOWS_PRODUCT_NAME}", identity.app_id),
        "productVersion": REQUIRED_RUNNER_VERSION,
        "envVersion": runner_env.as_str(),
        "windowsAppId": format!("{RUNNER_WINDOWS_APP_ID}.{}", identity.app_id),
        "homeAppId": identity.app_id,
        "homeAppVersion": identity.version,
        "devWsUrl": ws_url,
    });
    let app_json_path = assets_dir.join("app.json");
    std::fs::write(&app_json_path, serde_json::to_vec_pretty(&app_json)?)
        .with_context(|| format!("Failed to write {}", app_json_path.display()))?;

    let ui_json = windows_runner_ui_json(identity);
    let ui_json_path = assets_dir.join("ui.json");
    std::fs::write(&ui_json_path, serde_json::to_vec_pretty(&ui_json)?)
        .with_context(|| format!("Failed to write {}", ui_json_path.display()))?;

    let runtime = crate::runtime::embedded_runtime(crate::runtime::RuntimeEcmaTarget::Es2020);
    let runtime_path = assets_dir.join("bridge-runtime.js");
    std::fs::write(&runtime_path, runtime.bytes)
        .with_context(|| format!("Failed to write {}", runtime_path.display()))?;

    // Runner window/taskbar icon: the LingXia vessel mark, embedded in the
    // CLI so published builds don't depend on the repo's design sources.
    // `lingxia-windows-sdk` picks `<assets>/AppIcon.png` up automatically.
    let icon_path = assets_dir.join("AppIcon.png");
    std::fs::write(
        &icon_path,
        include_bytes!("../../../assets/runner-icon.png"),
    )
    .with_context(|| format!("Failed to write {}", icon_path.display()))?;
    // The shell's default sidebar icon (lxapp rows / browser tabs with no
    // icon of their own) loads from `<assets>/icons/lingxia.png`; stage the
    // same mark there so the dev runner has the fallback too.
    let icons_dir = assets_dir.join("icons");
    std::fs::create_dir_all(&icons_dir)
        .with_context(|| format!("Failed to create {}", icons_dir.display()))?;
    let default_icon_path = icons_dir.join("lingxia.png");
    std::fs::write(
        &default_icon_path,
        include_bytes!("../../../assets/runner-icon.png"),
    )
    .with_context(|| format!("Failed to write {}", default_icon_path.display()))?;
    prepare_windows_design_icon_assets(&assets_dir)?;

    Ok(assets_dir)
}

fn windows_runner_ui_json(identity: &WindowsRunnerLxAppIdentity) -> serde_json::Value {
    serde_json::json!({
        "launch": {
            "initialSurface": identity.app_id,
            "openOnLaunch": true
        },
        "surfaces": [{
            "id": identity.app_id,
            "role": "main",
            "content": {
                "kind": "lxapp",
                "appId": identity.app_id
            }
        }],
        "activators": []
    })
}

fn installed_windows_runner_exe_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Failed to resolve home directory"))?;
    let runner_dir = home
        .join(".lingxia")
        .join("runner")
        .join(REQUIRED_RUNNER_VERSION);
    let exe_path = runner_dir.join(format!("{RUNNER_WINDOWS_BIN_NAME}.exe"));
    if !exe_path.exists() {
        return Err(anyhow!(
            "Windows LingXia Runner {} is not installed at {}.\n\
             Install it with:\n  lingxia runner install",
            REQUIRED_RUNNER_VERSION,
            exe_path.display()
        ));
    }
    Ok(exe_path)
}

/// Windows counterpart of `launch_runner_for_lxapp`: prepares the runner
/// asset layout and spawns the installed runner against the dev server.
/// Launch contract: `--asset-dir` (host assets), `LINGXIA_LXAPP_PATH`
/// (live lxapp bundle), and `LINGXIA_DEV_WS_URL` (devtool bridge).
/// Host identity is generated into `app.json`.
/// `runner_device` (the `--runner` flag) picks the simulated device, same
/// id contract as the macOS runner's `LINGXIA_RUNNER_DEVICE`.
fn launch_windows_runner_for_lxapp(
    lxapp_path: &Path,
    ws_url: &str,
    runner_device: Option<&str>,
    runner_env: crate::config::EnvVersion,
) -> Result<RunnerProcess> {
    platform::host_support::ensure_supported_host(&PlatformType::Windows)?;
    ensure_valid_lxapp_dir(lxapp_path)?;
    // Provision the runner from the matching release if it isn't installed yet.
    crate::runner_cache::ensure_runner(REQUIRED_RUNNER_VERSION, false)?;
    let identity = read_windows_runner_lxapp_identity(lxapp_path)?;
    let assets_dir = prepare_windows_runner_assets(lxapp_path, &identity, ws_url, runner_env)?;
    let resource_lxapp_paths = windows_runner_resource_lxapp_paths(lxapp_path, &identity)?;
    let exe_path = installed_windows_runner_exe_path()?;
    terminate_existing_windows_runner_processes(&exe_path, ws_url)?;
    let mock_dir = crate::lxapp::worker::prepare_dev(lxapp_path);
    if let Some(mock_dir) = &mock_dir {
        println!(
            "  {} Cloud functions (mock): {}",
            "*".cyan(),
            mock_dir.display()
        );
    }

    let launch_args = windows_runner_launch_args(
        lxapp_path,
        &assets_dir,
        ws_url,
        mock_dir.as_deref(),
        runner_device,
        runner_env,
        &resource_lxapp_paths,
    )?;

    #[cfg(target_os = "windows")]
    let child = if windows_interactive::is_ssh_session() {
        let launch = windows_interactive::launch_runner(
            &exe_path,
            lxapp_path,
            &launch_args,
            &log_store::dev_dir(lxapp_path).join("runner"),
        )?;
        println!(
            "{} Bootstrapped Runner in the interactive Windows desktop",
            "[runner]".cyan()
        );
        RunnerProcess::WindowsShell(WindowsShellRunnerProcess {
            handle: launch.handle,
            _interactive_cleanup: Some(launch.cleanup),
        })
    } else {
        shell_execute_windows_runner(&exe_path, lxapp_path, &launch_args)?
    };

    #[cfg(not(target_os = "windows"))]
    let child = {
        let mut command = Command::new(&exe_path);
        command.env(RUNNER_MARKER_ENV, "1");
        command.args(&launch_args);
        command.stdin(Stdio::null());
        command.stdout(Stdio::inherit());
        command.stderr(Stdio::inherit());

        RunnerProcess::Child(command.spawn().with_context(|| {
            format!(
                "Failed to launch Windows Runner executable: {}",
                exe_path.display()
            )
        })?)
    };

    println!("{} Launched {}", "[runner]".cyan(), exe_path.display());
    Ok(child)
}

fn windows_runner_launch_args(
    lxapp_path: &Path,
    assets_dir: &Path,
    ws_url: &str,
    mock_dir: Option<&Path>,
    runner_device: Option<&str>,
    runner_env: crate::config::EnvVersion,
    resource_lxapp_paths: &[WindowsRunnerResourceLxAppPath],
) -> Result<Vec<String>> {
    let mut args = vec![
        "--lxapp-path".to_string(),
        lxapp_path.display().to_string(),
        "--dev-ws-url".to_string(),
        ws_url.to_string(),
        "--runner-env".to_string(),
        runner_env.as_str().to_string(),
        "--asset-dir".to_string(),
        assets_dir.display().to_string(),
    ];
    if let Some(mock_dir) = mock_dir {
        args.push("--lingxiao-mock-dir".to_string());
        args.push(mock_dir.display().to_string());
    }
    if let Some(device) = runner_device.map(str::trim).filter(|s| !s.is_empty()) {
        args.push("--runner-device".to_string());
        args.push(device.to_string());
    }
    if !resource_lxapp_paths.is_empty() {
        args.push("--resource-lxapp-paths".to_string());
        args.push(serde_json::to_string(resource_lxapp_paths)?);
    }
    Ok(args)
}

fn windows_runner_resource_lxapp_paths(
    lxapp_path: &Path,
    identity: &WindowsRunnerLxAppIdentity,
) -> Result<Vec<WindowsRunnerResourceLxAppPath>> {
    let Some(host_root) = lxapp_path
        .ancestors()
        .skip(1)
        .find(|root| root.join(crate::config::HOST_CONFIG_FILE).is_file())
    else {
        return Ok(Vec::new());
    };
    let config = LingXiaConfig::load(host_root)?;
    let Some(resources) = config.resources.as_ref() else {
        return Ok(Vec::new());
    };

    let mut paths = Vec::new();
    for bundle in &resources.bundles {
        if bundle.app_id == identity.app_id {
            continue;
        }
        let Some(path) = bundle
            .path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let bundle_root = host_root.join(path);
        let runnable = resolve_windows_runner_resource_lxapp_path(&bundle_root);
        if !runnable.join("lxapp.json").is_file() {
            continue;
        }
        paths.push(WindowsRunnerResourceLxAppPath {
            app_id: bundle.app_id.clone(),
            path: runnable.display().to_string(),
        });
    }
    Ok(paths)
}

fn resolve_windows_runner_resource_lxapp_path(bundle_root: &Path) -> PathBuf {
    let dist = bundle_root.join("dist");
    if dist.join("lxapp.json").is_file() {
        dist
    } else {
        bundle_root.to_path_buf()
    }
}

enum RunnerProcess {
    #[cfg(not(target_os = "windows"))]
    Child(Child),
    /// The macOS Runner: `child` is the executable we spawned, but it may re-exec
    /// itself through LaunchServices and exit 0 while the real app keeps running
    /// under a separate pid. Liveness is therefore tracked by the Runner's own
    /// `pid_file` (which it writes on startup), not by the child handle — and
    /// per-file so one project's Runner is distinguished from another's.
    MacOsApp {
        child: Child,
        pid_file: PathBuf,
        /// Set once we've observed a live Runner, so the initial hand-off window
        /// (child exited, pid-file not yet written) isn't mistaken for exit.
        seen_running: bool,
        handoff_deadline: std::time::Instant,
    },
    #[cfg(target_os = "windows")]
    WindowsShell(WindowsShellRunnerProcess),
}

struct RunnerExitStatus {
    success: bool,
    code: Option<i32>,
}

impl RunnerProcess {
    fn try_wait(&mut self) -> Result<Option<RunnerExitStatus>> {
        match self {
            #[cfg(not(target_os = "windows"))]
            RunnerProcess::Child(child) => child
                .try_wait()
                .context("Failed to poll LingXia Runner")
                .map(|status| {
                    status.map(|status| RunnerExitStatus {
                        success: status.success(),
                        code: status.code(),
                    })
                }),
            RunnerProcess::MacOsApp {
                child,
                pid_file,
                seen_running,
                handoff_deadline,
            } => {
                match child.try_wait().context("Failed to poll LingXia Runner")? {
                    // A non-zero exit is a genuine launch/runtime failure.
                    Some(status) if !status.success() => Ok(Some(RunnerExitStatus {
                        success: false,
                        code: status.code(),
                    })),
                    // Child exited 0 (either a clean quit or a LaunchServices
                    // hand-off) — the real Runner's liveness is its pid-file.
                    Some(_) => {
                        if runner_process_running(pid_file) {
                            *seen_running = true;
                            Ok(None)
                        } else if *seen_running {
                            Ok(Some(RunnerExitStatus {
                                success: true,
                                code: Some(0),
                            }))
                        } else if std::time::Instant::now() >= *handoff_deadline {
                            Ok(Some(RunnerExitStatus {
                                success: false,
                                code: None,
                            }))
                        } else {
                            // Hand-off may still be in flight; keep waiting.
                            Ok(None)
                        }
                    }
                    // Child still running == Runner running (no hand-off yet).
                    None => {
                        *seen_running = true;
                        Ok(None)
                    }
                }
            }
            #[cfg(target_os = "windows")]
            RunnerProcess::WindowsShell(process) => process.try_wait(),
        }
    }

    fn terminate(&mut self) -> Result<()> {
        match self {
            #[cfg(not(target_os = "windows"))]
            RunnerProcess::Child(child) => terminate_child(child, "LingXia Runner"),
            RunnerProcess::MacOsApp {
                child, pid_file, ..
            } => {
                let _ = terminate_child(child, "LingXia Runner");
                terminate_runner_from_pid_file(pid_file);
                Ok(())
            }
            #[cfg(target_os = "windows")]
            RunnerProcess::WindowsShell(process) => process.terminate(),
        }
    }
}

#[cfg(target_os = "windows")]
struct WindowsShellRunnerProcess {
    handle: ::windows::Win32::Foundation::HANDLE,
    _interactive_cleanup: Option<windows_interactive::InteractiveRunnerCleanup>,
}

#[cfg(target_os = "windows")]
impl WindowsShellRunnerProcess {
    fn try_wait(&self) -> Result<Option<RunnerExitStatus>> {
        use ::windows::Win32::Foundation::{WAIT_OBJECT_0, WAIT_TIMEOUT};
        use ::windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};

        let wait = unsafe { WaitForSingleObject(self.handle, 0) };
        if wait == WAIT_TIMEOUT {
            return Ok(None);
        }
        if wait != WAIT_OBJECT_0 {
            return Err(anyhow!("Failed to wait for LingXia Runner: {wait:?}"));
        }

        let mut code = 0u32;
        unsafe { GetExitCodeProcess(self.handle, &mut code) }
            .context("Failed to read LingXia Runner exit code")?;
        Ok(Some(RunnerExitStatus {
            success: code == 0,
            code: Some(code as i32),
        }))
    }

    fn terminate(&mut self) -> Result<()> {
        use ::windows::Win32::System::Threading::TerminateProcess;

        unsafe { TerminateProcess(self.handle, 1) }
            .context("Failed to terminate LingXia Runner")?;
        Ok(())
    }
}

#[cfg(target_os = "windows")]
impl Drop for WindowsShellRunnerProcess {
    fn drop(&mut self) {
        unsafe {
            let _ = ::windows::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

#[cfg(target_os = "windows")]
fn shell_execute_windows_runner(
    exe_path: &Path,
    lxapp_path: &Path,
    launch_args: &[String],
) -> Result<RunnerProcess> {
    use ::windows::Win32::Foundation::HANDLE;
    use ::windows::Win32::System::Threading::GetProcessId;
    use ::windows::Win32::UI::Shell::{
        SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
    };
    use ::windows::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, SW_SHOWNORMAL};
    use ::windows::core::PCWSTR;
    use std::mem::size_of;

    // ShellExecuteExW gives the child our environment block (no per-child env
    // param), so stamp the runner marker on this process. Safe in practice: a
    // single write of a var no other thread reads, done just before launch.
    unsafe { std::env::set_var(RUNNER_MARKER_ENV, "1") };

    let params = launch_args
        .iter()
        .map(|arg| quote_windows_arg(arg))
        .collect::<Vec<_>>()
        .join(" ");

    let file = wide_null(&exe_path.display().to_string());
    let parameters = wide_null(&params);
    let directory = wide_null(
        &lxapp_path
            .canonicalize()
            .unwrap_or_else(|_| lxapp_path.to_path_buf())
            .display()
            .to_string(),
    );

    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(parameters.as_ptr()),
        lpDirectory: PCWSTR(directory.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };

    unsafe { ShellExecuteExW(&mut info) }.with_context(|| {
        format!(
            "Failed to launch Windows Runner executable: {}",
            exe_path.display()
        )
    })?;
    if info.hProcess == HANDLE::default() {
        return Err(anyhow!(
            "Windows Runner launch did not return a process handle"
        ));
    }
    let runner_pid = unsafe { GetProcessId(info.hProcess) };
    if runner_pid != 0 {
        unsafe {
            let _ = AllowSetForegroundWindow(runner_pid);
        }
    }

    Ok(RunnerProcess::WindowsShell(WindowsShellRunnerProcess {
        handle: info.hProcess,
        _interactive_cleanup: None,
    }))
}

#[cfg(target_os = "windows")]
fn terminate_existing_windows_runner_processes(exe_path: &Path, dev_ws_url: &str) -> Result<()> {
    use sysinfo::{ProcessRefreshKind, UpdateKind};

    let mut system = System::new();
    // The default process refresh leaves `cmd()` empty; ask for it explicitly —
    // matching on the command line is what scopes the kill to this session.
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_exe(UpdateKind::Always)
            .with_cmd(UpdateKind::Always),
    );

    let mut pids = Vec::new();
    for process in system.processes().values() {
        let Some(process_exe) = process.exe() else {
            continue;
        };
        if !process_executable_matches(process_exe, exe_path) {
            continue;
        }
        // Reclaim only a stale runner of THIS dev session — one launched with the
        // same `--dev-ws-url`. The ws port is deterministic per project+platform
        // (`dev_port`), so any lingering holder of this url is a previous run of
        // this same project; runners for other projects use a different url and
        // must survive so two instances can run at once.
        let owns_session = process
            .cmd()
            .iter()
            .any(|arg| arg.to_string_lossy() == dev_ws_url);
        if owns_session {
            pids.push(process.pid());
        }
    }

    for pid in pids {
        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
        let Some(process) = system.process(pid) else {
            continue;
        };
        let _ = process.kill();
        if !wait_for_pid_exit(pid, Duration::from_secs(3)) {
            return Err(anyhow!(
                "Failed to terminate existing LingXia Runner process {}",
                pid
            ));
        }
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn terminate_existing_windows_runner_processes(_exe_path: &Path, _dev_ws_url: &str) -> Result<()> {
    Ok(())
}

#[cfg(target_os = "windows")]
fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(target_os = "windows")]
fn quote_windows_arg(value: &str) -> String {
    if !value.is_empty()
        && !value
            .chars()
            .any(|ch| ch.is_whitespace() || matches!(ch, '"' | '\\'))
    {
        return value.to_string();
    }

    let mut quoted = String::from("\"");
    let mut backslashes = 0usize;
    for ch in value.chars() {
        match ch {
            '\\' => backslashes += 1,
            '"' => {
                quoted.push_str(&"\\".repeat(backslashes * 2 + 1));
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                quoted.push_str(&"\\".repeat(backslashes));
                backslashes = 0;
                quoted.push(ch);
            }
        }
    }
    quoted.push_str(&"\\".repeat(backslashes * 2));
    quoted.push('"');
    quoted
}

fn wait_for_runner_or_interrupt(
    runner: &mut RunnerProcess,
    stop_requested: Arc<AtomicBool>,
) -> Result<()> {
    loop {
        if stop_requested.load(Ordering::Acquire) {
            runner.terminate()?;
            println!();
            println!("{}", "Dev workflow stopped.".yellow().bold());
            return Ok(());
        }

        if let Some(status) = runner.try_wait()? {
            println!();
            println!("{}", "LingXia Runner exited.".yellow().bold());
            if !status.success {
                return Err(match status.code {
                    Some(code) => anyhow!("LingXia Runner exited with non-zero status: {code}"),
                    None => anyhow!("LingXia Runner exited with non-zero status"),
                });
            }
            return Ok(());
        }

        thread::sleep(Duration::from_millis(150));
    }
}

/// This project's Runner pid-file. Each Runner writes its own pid here, so the
/// CLI can act on exactly its own Runner and leave other projects' alone.
fn runner_pid_file(project_root: &Path) -> PathBuf {
    log_store::dev_dir(project_root).join("runner.pid")
}

/// Stable, filesystem-safe id for a project, so its Runner reuses the same
/// isolated data subtree across restarts while staying distinct from other
/// projects. `<dir name>-<hash of the canonical path>`.
fn runner_instance_id(lxapp_path: &Path) -> String {
    use std::hash::{Hash, Hasher};
    let canonical = lxapp_path
        .canonicalize()
        .unwrap_or_else(|_| lxapp_path.to_path_buf());
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    canonical.hash(&mut hasher);
    let hash = hasher.finish();
    let name = lxapp_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("lxapp");
    let slug: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    format!("{slug}-{hash:x}")
}

fn read_runner_pid(pid_file: &Path) -> Option<i32> {
    std::fs::read_to_string(pid_file)
        .ok()?
        .trim()
        .parse::<i32>()
        .ok()
        .filter(|pid| *pid > 0)
}

/// Whether `pid` is a live process (`kill(pid, 0)`, no signal delivered). The
/// pid-file scheme is only used by the macOS Runner, so the check is a no-op on
/// non-Unix targets (this file still compiles for the Windows Runner path).
fn pid_alive(pid: i32) -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::kill(pid, 0) == 0 }
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

/// Whether *this project's* Runner is alive, by its pid-file. Replaces the
/// name-based check, which couldn't tell one project's Runner from another's.
fn runner_process_running(pid_file: &Path) -> bool {
    read_runner_pid(pid_file).map(pid_alive).unwrap_or(false)
}

/// Terminate the Runner recorded in `pid_file` (if any) and clear the file.
/// Scoped to one project — never touches Runners started for other projects.
fn terminate_runner_from_pid_file(pid_file: &Path) {
    if let Some(_pid) = read_runner_pid(pid_file).filter(|pid| pid_alive(*pid)) {
        #[cfg(unix)]
        unsafe {
            libc::kill(_pid, libc::SIGTERM);
        }
    }
    let _ = std::fs::remove_file(pid_file);
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
            "LingXia Runner {} is not installed at {}.\n\
             Install it with:\n  lingxia runner install",
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

#[cfg(test)]
mod tests {
    use super::{
        WindowsRunnerLxAppIdentity, is_standalone_lxapp_project, lxapp_runner_build_args,
        prepare_windows_runner_assets, render_runner_devices, windows_runner_launch_args,
        windows_runner_ui_json,
    };
    use crate::config::HOST_CONFIG_FILE;
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
        fs::write(temp.path().join(HOST_CONFIG_FILE), "").unwrap();

        assert!(!is_standalone_lxapp_project(temp.path()));
    }

    #[test]
    fn runner_device_list_uses_shared_manifest() {
        let list = render_runner_devices().unwrap();

        assert!(list.contains("iphone-15-pro"));
        assert!(list.contains("(default)"));
        assert!(list.contains("lingxia dev --runner <device>"));
    }

    #[test]
    fn lxapp_runner_build_args_include_selected_env() {
        let args = lxapp_runner_build_args(
            true,
            Some("react"),
            Some("plain"),
            crate::config::EnvVersion::Preview,
        );

        assert_eq!(
            args,
            vec![
                "build",
                "--env",
                "preview",
                "--release",
                "--framework",
                "react",
                "--progress",
                "plain"
            ]
        );
    }

    #[test]
    fn windows_runner_ui_json_declares_home_surface_only() {
        let identity = WindowsRunnerLxAppIdentity {
            app_id: "com.example.demo".to_string(),
            version: "1.0.0".to_string(),
        };

        let ui = windows_runner_ui_json(&identity);

        assert_eq!(ui["launch"]["initialSurface"], "com.example.demo");
        assert_eq!(ui["launch"]["openOnLaunch"], true);
        assert_eq!(ui["surfaces"][0]["id"], "com.example.demo");
        assert_eq!(ui["surfaces"][0]["role"], "main");
        assert_eq!(ui["surfaces"][0]["content"]["kind"], "lxapp");
        assert_eq!(ui["surfaces"][0]["content"]["appId"], "com.example.demo");
        assert_eq!(ui["activators"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn windows_runner_app_json_uses_selected_env() {
        let temp = tempdir().unwrap();
        let identity = WindowsRunnerLxAppIdentity {
            app_id: "com.example.demo".to_string(),
            version: "1.0.0".to_string(),
        };

        let assets = prepare_windows_runner_assets(
            temp.path(),
            &identity,
            "ws://127.0.0.1:3000",
            crate::config::EnvVersion::Preview,
        )
        .unwrap();
        let app_json: serde_json::Value =
            serde_json::from_slice(&fs::read(assets.join("app.json")).unwrap()).unwrap();

        assert_eq!(app_json["envVersion"], "preview");
    }

    #[test]
    fn windows_runner_launch_args_preserve_all_runtime_inputs() {
        let resources = vec![super::WindowsRunnerResourceLxAppPath {
            app_id: "com.example.extra".to_string(),
            path: r"D:\apps\extra".to_string(),
        }];

        let args = windows_runner_launch_args(
            std::path::Path::new(r"D:\apps\home"),
            std::path::Path::new(r"D:\apps\assets"),
            "ws://127.0.0.1:39000/?token=abc",
            Some(std::path::Path::new(r"D:\apps\mock")),
            Some("desktop-1440"),
            crate::config::EnvVersion::Developer,
            &resources,
        )
        .unwrap();

        assert!(
            args.windows(2)
                .any(|pair| pair == ["--lxapp-path", r"D:\apps\home"])
        );
        assert!(
            args.windows(2)
                .any(|pair| { pair == ["--dev-ws-url", "ws://127.0.0.1:39000/?token=abc"] })
        );
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--runner-device", "desktop-1440"])
        );
        assert!(args.iter().any(|arg| arg.contains("com.example.extra")));
    }
}
