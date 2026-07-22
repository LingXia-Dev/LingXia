use super::forward::DevPortForward;
use super::*;

pub(super) fn execute_android(mut ctx: DevContext) -> Result<()> {
    let platform_name = platform_session_name(PlatformType::Android);
    take_over_target_session(&ctx.project_root, platform_name)?;
    let platform = platform::android::AndroidPlatform::new();
    let (device_id, build_target) =
        platform::android::resolve_dev_device_target(ctx.device.as_deref())?;
    println!(
        "  {} Android device {} ({})",
        "→".cyan(),
        device_id,
        build_target
    );
    ctx.device = Some(device_id);
    let build_targets = vec![build_target];
    let stop_requested = Arc::new(AtomicBool::new(false));
    let server = server::start_server_fixed_with_stop(
        &ctx.project_root,
        "127.0.0.1",
        platform_name,
        stop_requested.clone(),
        None,
    )?;
    let host_ws_url = server.ws_url();
    let device_ws_url = loopback_ws_url(server.port());
    let session = server.session().clone();

    let run_result = (|| -> Result<()> {
        let platforms_to_build = vec![PlatformType::Android];
        prepare_dev_host_assets(
            &ctx,
            &platforms_to_build,
            &build_targets,
            Some(&device_ws_url),
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
            android_aab: false,
            macos_arch: None,
            framework: ctx.framework,
            native_features: dev_native_features(
                &ctx.config,
                "android",
                &ctx.extra_native_features,
            ),
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
        install_ctrlc_handler(stop_requested.clone())?;
        let _session_registration =
            log_store::register_session(&ctx.project_root, &session, platform_name, &host_ws_url);

        let run_config = RunConfig {
            device_id: ctx.device.clone(),
            package_id,
            main_activity: None,
            restart: false,
        };

        platform.run(&run_config)?;

        print_mobile_dev_started("Android", &[]);
        wait_for_interrupt(stop_requested)?;
        Ok(())
    })();

    stop_dev_server(server, run_result)
}
