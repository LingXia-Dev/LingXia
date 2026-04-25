use rong_rt::{InstallGlobalExecutorError, RongExecutor};

fn default_runtime_threads() -> usize {
    std::thread::available_parallelism()
        .map(|count| count.get().min(4))
        .unwrap_or(1)
}

fn install_global_executor() {
    let executor = match RongExecutor::builder()
        .threads(default_runtime_threads())
        .thread_name("lingxia")
        .build()
    {
        Ok(executor) => executor,
        Err(err) => {
            log::warn!("Failed to build dedicated RongExecutor: {}", err);
            return;
        }
    };

    match executor.install_global() {
        Ok(()) => {
            log::info!("Installed dedicated RongExecutor for host async work");
        }
        Err(InstallGlobalExecutorError::AlreadyInstalled) => {}
    }
}

fn load_bundled_app_config(
    runtime: &std::sync::Arc<lingxia_platform::Platform>,
) -> Option<lingxia_app_context::AppConfig> {
    use lingxia_platform::traits::app_runtime::AppRuntime;
    use std::io::Read;

    let mut reader = match runtime.read_asset("app.json") {
        Ok(reader) => reader,
        Err(e) => {
            log::error!("Failed to read app.json: {}", e);
            return None;
        }
    };
    let mut content = String::new();
    if let Err(e) = reader.read_to_string(&mut content) {
        log::error!("Failed to read app.json: {}", e);
        return None;
    }
    match lingxia_app_context::AppConfig::parse_and_validate(&content) {
        Ok(config) => Some(config),
        Err(e) => {
            log::error!("Failed to load app configuration: {}", e);
            None
        }
    }
}

/// Common initialization after Platform is created.
/// Registers built-in runtime and initializes the lxapp system.
pub(crate) fn init_with_platform(platform: lingxia_platform::Platform) -> Option<String> {
    use lingxia_platform::traits::app_runtime::AppRuntime;

    #[cfg(feature = "devtool")]
    let _ = crate::devtool::install_lxapp_dev_config_from_env();
    crate::host_addon::run_before_init();

    let runtime = std::sync::Arc::new(platform.clone());
    #[cfg(feature = "devtool")]
    let app_config = crate::devtool::load_host_app_config(&runtime, load_bundled_app_config)?;
    #[cfg(not(feature = "devtool"))]
    let app_config = load_bundled_app_config(&runtime)?;
    crate::app::set_data_dir(runtime.app_data_dir());
    install_global_executor();
    if let Err(err) = lingxia_app_context::set_app_config(app_config.clone()) {
        log::error!("Failed to initialize app configuration: {}", err);
        return None;
    }
    crate::host_addon::run_install_logic_extensions();
    crate::host_addon::run_install_host_apis();
    crate::browser::register_bundled_app();
    crate::browser::register_builtin_runtime();
    crate::applink::install_handler();
    #[cfg(feature = "standard")]
    lingxia_logic::register_logic_runtime();
    #[cfg(feature = "devtool")]
    crate::devtool::register_bundle_source_override();
    let home_app_id = lxapp::init(platform);
    crate::update::spawn_host_app_update_flow(runtime.clone());
    crate::browser::register_builtin_assets();
    crate::host_addon::run_after_init();
    crate::browser::warmup();
    crate::host_addon::run_start_services();
    home_app_id
}
