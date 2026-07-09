use super::forward::DevPortForward;
use super::*;

pub(super) fn execute_harmony(ctx: DevContext) -> Result<()> {
    let platform_name = platform_session_name(PlatformType::Harmony);
    precheck_platform_session(&ctx.project_root, platform_name)?;
    let harmony_platform = platform::harmony::HarmonyPlatform::new();
    let stop_requested = Arc::new(AtomicBool::new(false));
    let server = server::start_server_fixed_with_stop(
        &ctx.project_root,
        "127.0.0.1",
        platform_name,
        stop_requested.clone(),
    )?;
    let host_ws_url = server.ws_url();
    let device_ws_url = loopback_ws_url(server.port());
    let session = server.session().clone();

    let run_result = (|| -> Result<()> {
        let platforms_to_build = vec![PlatformType::Harmony];
        prepare_dev_host_assets(&ctx, &platforms_to_build, &[], Some(&device_ws_url))?;

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
            android_aab: false,
            macos_arch: None,
            framework: ctx.framework,
            native_features: dev_native_features(
                &ctx.config,
                "harmony",
                &ctx.extra_native_features,
            ),
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

        println!();

        // Step 3: Port reverse
        println!("{}", "Step 3/4: Preparing dev connection...".bold());
        let _forward = DevPortForward::harmony(ctx.device.as_deref(), server.port())?;

        println!();

        // Step 4: Launch app
        println!("{}", "Step 4/4: Launching app...".bold());
        install_ctrlc_handler(stop_requested.clone())?;
        let _session_registration =
            log_store::register_session(&ctx.project_root, &session, platform_name, &host_ws_url);

        // Read bundleName from app.json5 (authoritative source).
        let run_config = RunConfig {
            package_id: bundle_name.clone(),
            main_activity: None, // defaults to "EntryAbility" in harmony platform
            device_id: ctx.device.clone(),
            restart: false,
        };

        harmony_platform.run(&run_config)?;

        print_mobile_dev_started("HarmonyOS", &[("Bundle", bundle_name.as_str())]);
        wait_for_interrupt(stop_requested)?;
        Ok(())
    })();

    stop_dev_server(server, run_result)
}
