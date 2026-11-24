use super::*;

pub(super) fn execute_ios(ctx: DevContext) -> Result<()> {
    let platform_name = platform_session_name(PlatformType::Ios);
    let installed_id = match ctx.installed {
        Some(_) => ctx
            .config
            .resolved_package_id_with_suffix("ios", &ctx.resolved_env)?,
        None => String::new(),
    };
    take_over_after_installed_check(
        ctx.installed.is_some(),
        || platform::apple::devicectl::app_installed(&installed_id, ctx.device.as_deref()),
        &installed_id,
        ctx.device.as_deref().unwrap_or("the device"),
        || take_over_target_session(&ctx.project_root, platform_name),
    )?;
    let platform = platform::ios::IosPlatform::new();
    let stop_requested = ctx.stop_requested.clone();
    // The iOS device reaches the host over Wi-Fi, so this bind is open and
    // carries the persistent session token. `[::]` takes IPv4 too: the
    // device's mDNS lookup of this Mac may answer with an IPv6 address.
    let auth_token = persistent_device_token()?;
    let mut server = start_platform_server(&ctx, "[::]", platform_name, Some(auth_token.clone()))?;
    let host_ws_url = lingxia_control_protocol::dev_session::ws_url_with_token(
        &loopback_ws_url(server.port()),
        &auth_token,
    );
    let device_ws_url = lingxia_control_protocol::dev_session::ws_url_with_token(
        &lan_ws_url(server.port())?,
        &auth_token,
    );
    let session = server.session().clone();
    let app_name = ctx
        .config
        .app
        .as_ref()
        .map(|app| app.product_name.clone())
        .unwrap_or_else(|| "this app".to_string());
    let unconnected_hint = ios_unconnected_hint(&app_name);

    let run_result = (|| -> Result<()> {
        let platforms_to_build = vec![PlatformType::Ios];
        prepare_dev_host_assets(
            &ctx,
            &server,
            &platforms_to_build,
            &[],
            Some(&device_ws_url),
        )?;

        if let Some(installed) = ctx.installed {
            let bundle_id = ctx
                .config
                .resolved_package_id_with_suffix("ios", &ctx.resolved_env)?;
            server.expect_installed_host();
            let _session_registration = log_store::register_session(
                &ctx.project_root,
                &session,
                platform_name,
                &host_ws_url,
            )?;
            if installed.launch {
                println!("{}", "Step 1/1: Launching...".bold());
                platform.run(&RunConfig {
                    package_id: bundle_id.clone(),
                    application_id: None,
                    main_activity: None,
                    device_id: ctx.device.clone(),
                    restart: false,
                })?;
            } else {
                print_installed_waiting(&bundle_id, server.port());
            }
            print_mobile_dev_started("iOS", &[("Bundle ID", bundle_id.as_str())]);
            ctx.watch_embedded_lxapps(&mut server);
            wait_for_interrupt_hinting(stop_requested, &server, &unconnected_hint)?;
            return finish_installed(&server);
        }

        // Step 1: Build
        println!("{}", "Step 1/3: Building...".bold());
        let build_config = BuildConfig {
            project_root: ctx.project_root.clone(),
            profile: ctx.build_profile,
            targets: vec![],
            lingxia_config: Some(ctx.config.clone()),
            ipa: false,
            package: false,
            dmg: false,
            android_aab: false,
            macos_arch: None,
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
        let bundle_id = platform.install_app(&install_config)?;

        println!();

        // Step 3: Launch app
        println!("{}", "Step 3/3: Launching...".bold());
        let _session_registration =
            log_store::register_session(&ctx.project_root, &session, platform_name, &host_ws_url)?;

        let run_config = RunConfig {
            package_id: bundle_id.clone(),
            application_id: None,
            main_activity: None,
            device_id: ctx.device.clone(),
            restart: false,
        };
        platform.run(&run_config)?;

        print_mobile_dev_started("iOS", &[("Bundle ID", bundle_id.as_str())]);
        ctx.watch_embedded_lxapps(&mut server);
        wait_for_interrupt_hinting(stop_requested, &server, &unconnected_hint)?;
        Ok(())
    })();

    stop_dev_server(server, run_result)
}
