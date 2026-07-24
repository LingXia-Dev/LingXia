use super::*;

pub(super) fn execute_macos(ctx: DevContext) -> Result<()> {
    let platform_name = platform_session_name(PlatformType::MacOs);
    take_over_target_session(&ctx.project_root, platform_name)?;
    use std::process::Command;

    let platform = platform::macos::MacosPlatform::new();
    let stop_requested = ctx.stop_requested.clone();
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
        let platforms_to_build = vec![PlatformType::MacOs];
        prepare_dev_host_assets(&ctx, &platforms_to_build, &[], Some(&ws_url))?;

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
            android_aab: false,
            macos_arch: None,
            framework: ctx.framework,
            native_features: dev_native_features(&ctx.config, "macos", &ctx.extra_native_features),
            native_default_features: ctx.config.native_default_features_enabled(),
            resolved_env: ctx.resolved_env.clone(),
            skip_native_build: false,
            native_only: false,
        };

        let artifacts = platform.build(&build_config)?;
        let app_path = artifacts.path().to_path_buf();
        let exe = platform::macos::app_bundle_executable(&app_path)?;
        println!();

        let _session_registration =
            log_store::register_session(&ctx.project_root, &session, platform_name, &ws_url);

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

        print_dev_banner("macOS", "Ctrl+C or close app", &[]);

        wait_for_child_or_interrupt(&mut child, stop_requested, "macOS app")?;
        Ok(())
    })();

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
