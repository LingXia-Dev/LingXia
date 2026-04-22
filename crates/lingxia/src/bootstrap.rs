use rong::{InstallGlobalExecutorError, RongExecutor};

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
    use std::time::Duration;

    let _ = crate::lxapp_dev::install_lxapp_dev_config_from_env();
    crate::host_addon::run_before_init();

    let runtime = std::sync::Arc::new(platform.clone());
    let app_config = crate::lxapp_dev::load_host_app_config(&runtime, load_bundled_app_config)?;
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
    #[cfg(feature = "js-lxapp")]
    lingxia_logic::register_logic_runtime();
    crate::lxapp_dev::register_bundle_source_override();
    let home_app_id = lxapp::init(
        platform,
        lxapp::LxAppRuntimeConfig {
            home_appid: app_config.home_app_id,
            home_app_version: app_config.home_app_version,
            temp_max_size_bytes: lingxia_app_context::temp_max_size_bytes(),
            cache_max_age: Duration::from_secs(
                lingxia_app_context::cache_max_age_days().saturating_mul(86400),
            ),
            cache_max_size_bytes: lingxia_app_context::cache_max_size_bytes(),
        },
    );
    crate::browser::register_builtin_assets();
    crate::host_addon::run_after_init();
    crate::browser::warmup();
    crate::host_addon::run_start_services();
    home_app_id
}
