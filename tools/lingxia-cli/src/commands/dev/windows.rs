use super::*;

/// Overrides the launcher icon `lingxia-windows-sdk` loads, so `lingxia dev`
/// can show the env badge without touching the prepared Windows assets.
/// Must match the env var read in `lingxia-windows-sdk`'s `resolve_app_icon_path`.
const WINDOWS_APP_ICON_PATH_ENV: &str = "LINGXIA_APP_ICON_PATH";

pub(super) fn execute_windows(ctx: DevContext) -> Result<()> {
    let platform_name = platform_session_name(PlatformType::Windows);
    take_over_target_session(&ctx.project_root, platform_name)?;
    let platform = platform::windows::WindowsPlatform::new();
    let stop_requested = Arc::new(AtomicBool::new(false));
    let server = server::start_server_fixed_with_stop(
        &ctx.project_root,
        "127.0.0.1",
        platform_name,
        stop_requested.clone(),
        None,
    )?;
    let ws_url = server.ws_url();
    let session = server.session().clone();

    let run_result = (|| -> Result<()> {
        let platforms_to_build = vec![PlatformType::Windows];
        prepare_dev_host_assets(&ctx, &platforms_to_build, &[], Some(&ws_url))?;

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
            android_aab: false,
            macos_arch: None,
            framework: ctx.framework,
            native_features: dev_native_features(
                &ctx.config,
                "windows",
                &ctx.extra_native_features,
            ),
            native_default_features: ctx.config.native_default_features_enabled(),
            resolved_env: ctx.resolved_env.clone(),
            skip_native_build: false,
            native_only: false,
        };

        let artifacts = platform.build(&build_config)?;
        let exe_path = artifacts.path().to_path_buf();

        // dev/preview: stage a badged copy of the launcher icon and point the
        // SDK at it via env, so the running window/taskbar shows the D/P badge
        // without mutating the prepared assets icon (which a later
        // `lingxia build` copies into its dist).
        let windows_build_dir = platform::windows::resolve_windows_build_dir(&ctx.project_root)?;
        let staged_icon = crate::platform::windows::env_icon::stage_dev_badged_icon(
            &platform::windows::resolve_windows_assets_dir(&ctx.project_root)?,
            ctx.config.app.as_ref().map(|app| app.home_app_id.as_str()),
            &windows_build_dir
                .join("overlay")
                .join(ctx.resolved_env.version.as_str()),
            ctx.resolved_env.version,
        )?;
        println!();

        println!("{}", "Step 2/2: Running...".bold());
        install_ctrlc_handler(stop_requested.clone())?;
        let _session_registration =
            log_store::register_session(&ctx.project_root, &session, platform_name, &ws_url);

        launch_and_wait_windows_app(
            &exe_path,
            &ctx.project_root,
            &ws_url,
            staged_icon.as_deref(),
            stop_requested,
        )?;
        Ok(())
    })();

    stop_dev_server(server, run_result)
}

fn launch_and_wait_windows_app(
    exe_path: &Path,
    project_root: &Path,
    ws_url: &str,
    staged_icon: Option<&Path>,
    stop_requested: Arc<AtomicBool>,
) -> Result<()> {
    #[cfg(not(target_os = "windows"))]
    let _ = project_root;

    #[cfg(target_os = "windows")]
    if runner::windows_interactive::is_ssh_session() {
        let mut environment = vec![(RUNNER_DEV_WS_URL_ENV.to_string(), ws_url.to_string())];
        if let Some(icon) = staged_icon {
            environment.push((
                WINDOWS_APP_ICON_PATH_ENV.to_string(),
                icon.display().to_string(),
            ));
        }
        let mut launch = runner::windows_interactive::launch_app(
            exe_path,
            project_root,
            &environment,
            &log_store::dev_dir(project_root).join("app"),
        )?;
        println!("Bootstrapped app in the interactive Windows desktop");
        print_dev_banner("Windows", "Ctrl+C or close app", &[]);
        return wait_for_interactive_app_or_interrupt(&mut launch, stop_requested);
    }

    let mut command = Command::new(exe_path);
    command.env(RUNNER_DEV_WS_URL_ENV, ws_url);
    if let Some(icon) = staged_icon {
        command.env(WINDOWS_APP_ICON_PATH_ENV, icon);
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("Failed to run {}", exe_path.display()))?;

    print_dev_banner("Windows", "Ctrl+C or close app", &[]);
    wait_for_child_or_interrupt(&mut child, stop_requested, "Windows app")
}

#[cfg(target_os = "windows")]
fn wait_for_interactive_app_or_interrupt(
    launch: &mut runner::windows_interactive::InteractiveLaunch,
    stop_requested: Arc<AtomicBool>,
) -> Result<()> {
    loop {
        if stop_requested.load(Ordering::Acquire) {
            launch.terminate("Windows app")?;
            println!();
            println!("{}", "Dev workflow stopped.".yellow().bold());
            return Ok(());
        }

        if let Some(code) = launch.exit_code()? {
            println!();
            println!("{}", "Windows app exited.".yellow().bold());
            if code != 0 {
                let log = launch
                    .output_log()
                    .map(|path| format!("; output: {}", path.display()))
                    .unwrap_or_default();
                return Err(anyhow!(
                    "Windows app exited with non-zero status {code}{log}"
                ));
            }
            return Ok(());
        }

        thread::sleep(Duration::from_millis(150));
    }
}
