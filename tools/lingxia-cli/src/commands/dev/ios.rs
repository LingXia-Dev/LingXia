use super::*;

pub(super) fn execute_ios(ctx: DevContext) -> Result<()> {
    let platform_name = platform_session_name(PlatformType::Ios);
    take_over_target_session(&ctx.project_root, platform_name)?;
    let platform = platform::ios::IosPlatform::new();
    let stop_requested = Arc::new(AtomicBool::new(false));
    // The iOS device reaches the host over Wi-Fi, so this bind was always
    // 0.0.0.0 — it now carries the persistent session token so the open bind
    // is no longer unauthenticated.
    let auth_token = persistent_device_token()?;
    let server = server::start_server_fixed_with_stop(
        &ctx.project_root,
        "0.0.0.0",
        platform_name,
        stop_requested.clone(),
        Some(auth_token.clone()),
    )?;
    let host_ws_url =
        lingxia_devtool_protocol::ws_url_with_token(&loopback_ws_url(server.port()), &auth_token);
    let device_ws_url =
        lingxia_devtool_protocol::ws_url_with_token(&lan_ws_url(server.port())?, &auth_token);
    let session = server.session().clone();

    let run_result = (|| -> Result<()> {
        let platforms_to_build = vec![PlatformType::Ios];
        prepare_dev_host_assets(&ctx, &platforms_to_build, &[], Some(&device_ws_url))?;

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
            android_aab: false,
            macos_arch: None,
            framework: ctx.framework,
            native_features: dev_native_features(&ctx.config, "ios", &ctx.extra_native_features),
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
        install_ctrlc_handler(stop_requested.clone())?;
        let _session_registration =
            log_store::register_session(&ctx.project_root, &session, platform_name, &host_ws_url);

        // Read bundle ID from the signed app (signing may change it for free accounts)
        let bundle_id = platform::ios::read_bundle_id(app_path)?;

        let run_config = RunConfig {
            package_id: bundle_id.clone(),
            main_activity: None,
            device_id: ctx.device.clone(),
            restart: false,
        };
        platform.run(&run_config)?;

        print_mobile_dev_started("iOS", &[("Bundle ID", bundle_id.as_str())]);
        wait_for_interrupt(stop_requested)?;
        Ok(())
    })();

    stop_dev_server(server, run_result)
}
