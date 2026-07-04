use super::*;

const RUNNER_APP_NAME: &str = "LingXia Runner.app";
const RUNNER_EXECUTABLE_NAME: &str = "LingXiaRunner";
const RUNNER_LXAPP_PATH_ENV: &str = "LINGXIA_LXAPP_PATH";
const RUNNER_DEV_WS_URL_ENV: &str = "LINGXIA_DEV_WS_URL";
const RUNNER_LINGXIAO_MOCK_DIR_ENV: &str = "LINGXIAO_MOCK_DIR";
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

    let stop_requested = Arc::new(AtomicBool::new(false));
    let server = server::start_server_fixed_with_stop(
        &project_root,
        "127.0.0.1",
        platform_name,
        stop_requested.clone(),
    )?;
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

        install_ctrlc_handler(stop_requested.clone())?;
        log_store::write_session(&project_root, &session, platform_name, &ws_url)?;

        println!();
        println!("{}", "Step 2/2: Launching Runner...".bold());
        let mut runner = match runner_host {
            LxAppRunnerHost::MacOs => {
                launch_runner_for_lxapp(&project_root, &ws_url, options.runner_device.as_deref())?
            }
            LxAppRunnerHost::Windows => launch_windows_runner_for_lxapp(
                &project_root,
                &ws_url,
                options.runner_device.as_deref(),
            )?,
        };

        print_dev_banner("LxApp Runner", "Ctrl+C or `lingxia dev stop`", &[]);

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
) -> Result<RunnerProcess> {
    platform::apple::ensure_macos()?;
    ensure_valid_lxapp_dir(lxapp_path)?;
    // Provision the runner from the matching release if it isn't installed yet
    // (end users install only the CLI; this self-heals the first `lingxia dev`).
    crate::runner_cache::ensure_runner(REQUIRED_RUNNER_VERSION, false)?;
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
    if let Some(device) = runner_device.map(str::trim).filter(|s| !s.is_empty()) {
        command.env("LINGXIA_RUNNER_DEVICE", device);
    }
    // Cloud functions: transpile mocks + generate typed `lx.cloud.invoke`, then
    // point the runner at the loadable mock dir (it reads routing from functions.json).
    if let Some(mock_dir) = crate::lxapp::functions::prepare_dev(lxapp_path) {
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
        "windowsAppId": RUNNER_WINDOWS_APP_ID,
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
/// asset layout and spawns the installed runner against the dev server. Env
/// contract: `LINGXIA_ASSET_DIR` (host assets),
/// `LINGXIA_LXAPP_PATH` (live lxapp bundle), and `LINGXIA_DEV_WS_URL`
/// (devtool bridge). Host identity is generated into `app.json`.
/// `runner_device` (the `--runner` flag) picks the simulated device, same
/// id contract as the macOS runner's `LINGXIA_RUNNER_DEVICE`.
fn launch_windows_runner_for_lxapp(
    lxapp_path: &Path,
    ws_url: &str,
    runner_device: Option<&str>,
) -> Result<RunnerProcess> {
    ensure_valid_lxapp_dir(lxapp_path)?;
    // Provision the runner from the matching release if it isn't installed yet.
    crate::runner_cache::ensure_runner(REQUIRED_RUNNER_VERSION, false)?;
    let identity = read_windows_runner_lxapp_identity(lxapp_path)?;
    let assets_dir = prepare_windows_runner_assets(lxapp_path, &identity, ws_url)?;
    let resource_lxapp_paths = windows_runner_resource_lxapp_paths(lxapp_path, &identity)?;
    let exe_path = installed_windows_runner_exe_path()?;
    let mock_dir = crate::lxapp::functions::prepare_dev(lxapp_path);
    if let Some(mock_dir) = &mock_dir {
        println!(
            "  {} Cloud functions (mock): {}",
            "*".cyan(),
            mock_dir.display()
        );
    }

    #[cfg(target_os = "windows")]
    let child = shell_execute_windows_runner(
        &exe_path,
        lxapp_path,
        &assets_dir,
        ws_url,
        mock_dir.as_deref(),
        runner_device,
        &resource_lxapp_paths,
    )?;

    #[cfg(not(target_os = "windows"))]
    let child = {
        let mut command = Command::new(&exe_path);
        command.arg("--lxapp-path").arg(lxapp_path);
        command.arg("--dev-ws-url").arg(ws_url);
        command.arg("--asset-dir").arg(&assets_dir);
        if let Some(mock_dir) = &mock_dir {
            command.arg("--lingxiao-mock-dir").arg(mock_dir);
        }
        if let Some(device) = runner_device.map(str::trim).filter(|s| !s.is_empty()) {
            command.arg("--runner-device").arg(device);
        }
        if !resource_lxapp_paths.is_empty() {
            command
                .arg("--resource-lxapp-paths")
                .arg(serde_json::to_string(&resource_lxapp_paths)?);
        }
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
    /// under a same-named process. Liveness is therefore tracked by process name,
    /// not by the child handle (which would spin forever on a clean hand-off).
    MacOsApp {
        child: Child,
        /// Set once we've observed a live Runner, so the initial hand-off window
        /// (child exited, named process not yet visible) isn't mistaken for exit.
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
                    // hand-off) — the real Runner's liveness is decided by name.
                    Some(_) => {
                        if runner_process_running() {
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
            RunnerProcess::MacOsApp { child, .. } => {
                let _ = terminate_child(child, "LingXia Runner");
                terminate_existing_runner_processes()
            }
            #[cfg(target_os = "windows")]
            RunnerProcess::WindowsShell(process) => process.terminate(),
        }
    }
}

#[cfg(target_os = "windows")]
struct WindowsShellRunnerProcess {
    handle: ::windows::Win32::Foundation::HANDLE,
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
    assets_dir: &Path,
    ws_url: &str,
    mock_dir: Option<&Path>,
    runner_device: Option<&str>,
    resource_lxapp_paths: &[WindowsRunnerResourceLxAppPath],
) -> Result<RunnerProcess> {
    use ::windows::Win32::Foundation::HANDLE;
    use ::windows::Win32::System::Threading::GetProcessId;
    use ::windows::Win32::UI::Shell::{
        SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
    };
    use ::windows::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, SW_SHOWNORMAL};
    use ::windows::core::PCWSTR;
    use std::mem::size_of;

    let mut params = vec![
        "--lxapp-path".to_string(),
        lxapp_path.display().to_string(),
        "--dev-ws-url".to_string(),
        ws_url.to_string(),
        "--asset-dir".to_string(),
        assets_dir.display().to_string(),
    ];
    if let Some(mock_dir) = mock_dir {
        params.push("--lingxiao-mock-dir".to_string());
        params.push(mock_dir.display().to_string());
    }
    if let Some(device) = runner_device.map(str::trim).filter(|s| !s.is_empty()) {
        params.push("--runner-device".to_string());
        params.push(device.to_string());
    }
    if !resource_lxapp_paths.is_empty() {
        params.push("--resource-lxapp-paths".to_string());
        params.push(serde_json::to_string(resource_lxapp_paths)?);
    }
    let params = params
        .into_iter()
        .map(|arg| quote_windows_arg(&arg))
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
    }))
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

/// Whether any LingXia Runner process is currently alive (by executable name).
/// Used to track the macOS Runner across a LaunchServices hand-off, where the
/// process we spawned exits and the real app runs under a separate pid.
fn runner_process_running() -> bool {
    Command::new("pgrep")
        .args(["-x", RUNNER_EXECUTABLE_NAME])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
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
        WindowsRunnerLxAppIdentity, is_standalone_lxapp_project, render_runner_devices,
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
}
