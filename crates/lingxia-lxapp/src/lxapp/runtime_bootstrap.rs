//! Runtime bootstrap flow (`init`) and startup housekeeping.

use super::*;

/// Prepares the base directory structure for lxapps
fn prepare_directory_structure(runtime: Arc<Platform>) -> Result<(), LxAppError> {
    let data_dir = runtime.app_data_dir();
    let cache_dir = runtime.app_cache_dir();

    // Create required directories
    let dirs = [
        data_dir.join(LINGXIA_DIR).join(LXAPPS_DIR),
        data_dir.join(LINGXIA_DIR).join(PLUGINS_DIR),
        data_dir.join(LINGXIA_DIR).join(USER_DATA_DIR),
        data_dir.join(LINGXIA_DIR).join(USER_CACHE_DIR),
        lingxia_transfer::dir(&runtime.app_data_dir()),
        data_dir.join(LINGXIA_DIR).join(STORAGE_DIR),
        cache_dir.join(LINGXIA_DIR).join(LXAPPS_DIR).join(TEMP_DIR),
    ];

    for dir in &dirs {
        fs::create_dir_all(dir)?;
    }

    let metadata_path = data_dir.join(LINGXIA_DIR).join(LXAPPS_DB_FILE);
    metadata::init(metadata_path)
}

fn spawn_cache_cleanup(runtime: Arc<Platform>) {
    let max_bytes = lingxia_app_context::cache_max_size_bytes();
    if max_bytes == 0 {
        info!("Cache cleanup disabled (cacheMaxSizeMB=0)");
        return;
    }

    std::mem::drop(crate::executor::spawn(async move {
        let cache_base_dir = runtime
            .app_data_dir()
            .join(LINGXIA_DIR)
            .join(USER_CACHE_DIR);
        cleanup_cache_base_dir(&cache_base_dir, max_bytes);
    }));
}

fn cleanup_cache_base_dir(cache_base_dir: &Path, max_bytes: u64) {
    if let Ok(entries) = fs::read_dir(cache_base_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() && !file_type.is_symlink() {
                lingxia_service::storage::cleanup_cache_dir(&path, max_bytes);
            }
        }
    }
}

fn installed_home_version(
    appid: &str,
    release_type: ReleaseType,
) -> Result<Option<Version>, LxAppError> {
    let Some(record) = metadata::get(appid, release_type)? else {
        return Ok(None);
    };

    let install_path = Path::new(&record.install_path);
    let manifest_path = install_path.join("lxapp.json");
    if record.install_path.trim().is_empty() || !install_path.is_dir() || !manifest_path.is_file() {
        let _ = metadata::remove(appid, release_type);
        return Ok(None);
    }

    let manifest = fs::read_to_string(&manifest_path).map_err(LxAppError::from)?;
    let manifest_json: serde_json::Value = serde_json::from_str(&manifest)
        .map_err(|e| LxAppError::InvalidJsonFile(format!("{}: {}", manifest_path.display(), e)))?;
    let config = LxAppConfig::from_value(manifest_json)
        .map_err(|e| LxAppError::InvalidJsonFile(format!("{}: {}", manifest_path.display(), e)))?;
    if config.get_initial_route().trim().is_empty() {
        warn!(
            "Installed home lxapp manifest has no pages: {}; reinstalling bundled home app",
            manifest_path.display()
        )
        .with_appid(appid.to_string());
        let _ = metadata::remove(appid, release_type);
        return Ok(None);
    }

    Ok(Some(Version {
        major: record.version.major,
        minor: record.version.minor,
        patch: record.version.patch,
    }))
}

/// Initialize the LxApps singleton using the host app configuration from app-context.
pub fn init(runtime: Platform) -> Option<String> {
    // Set up panic hook to capture panic information
    std::panic::set_hook(Box::new(|panic_info| {
        let location = panic_info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".to_string());
        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic message".to_string()
        };

        error!("RUST PANIC: {} at {}", message, location);
    }));

    // Register built-in Host API set. This ensures view->host calls work regardless of
    // which logic extensions are loaded.
    crate::host::register_all();

    let runtime_arc = Arc::new(runtime.clone());
    super::runtime_registry::set_runtime(runtime_arc.clone());

    // Prepare directory structure
    if let Err(e) = prepare_directory_structure(runtime_arc.clone()) {
        error!("Failed to prepare directory structure: {}", e);
        return None;
    }

    let home_app_id = match lingxia_app_context::home_app_id() {
        Some(appid) => appid.to_string(),
        None => {
            error!("Host app configuration is not initialized");
            return None;
        }
    };
    let home_app_version = match lingxia_app_context::home_app_version() {
        Some(version) => version,
        None => {
            error!("Host app configuration is not initialized");
            return None;
        }
    };

    let bundled_home_version = match Version::parse(home_app_version) {
        Ok(version) => version,
        Err(e) => {
            error!(
                "Invalid bundled home lxapp version '{}': {}",
                home_app_version, e
            )
            .with_appid(home_app_id.clone());
            return None;
        }
    };
    let installed_home_version = match installed_home_version(&home_app_id, ReleaseType::Release) {
        Ok(version) => version,
        Err(e) => {
            warn!("Failed to inspect installed home lxapp version: {}", e)
                .with_appid(home_app_id.clone());
            None
        }
    };

    let should_reinstall_home = installed_home_version
        .as_ref()
        .map(|installed| installed < &bundled_home_version)
        .unwrap_or(true);

    if should_reinstall_home {
        let reason = match installed_home_version {
            None => "home lxapp is not installed or install is invalid".to_string(),
            Some(installed) => format!(
                "bundled version {} is newer than installed {}",
                bundled_home_version, installed
            ),
        };
        info!("Installing home lxapp from bundled assets: {}", reason)
            .with_appid(home_app_id.clone());
        if let Err(e) = crate::update::UpdateManager::install_from_assets(
            runtime_arc.clone(),
            &home_app_id,
            home_app_version,
        ) {
            error!("Failed to install home LxApp: {}", e);
            return None;
        }
    } else {
        let has_pending_home_update = metadata::downloaded_get(&home_app_id, ReleaseType::Release)
            .map(|record| record.is_some())
            .unwrap_or(false);
        if has_pending_home_update {
            match crate::update::UpdateManager::apply_downloaded_update(
                runtime_arc.clone(),
                &home_app_id,
                ReleaseType::Release,
            ) {
                Ok(()) => {
                    info!("Applied pending home lxapp update before startup")
                        .with_appid(home_app_id.clone());
                }
                Err(e) => {
                    warn!("Failed to apply pending home lxapp update: {}", e)
                        .with_appid(home_app_id.clone());
                }
            }
        }
    }
    let num_workers = get_num_workers();
    let executor = LxAppWorkers::init(num_workers);

    // Create LxApps manager BEFORE creating home_lxapp
    // This makes get_platform() available as early as possible
    let lxapps_manager = Arc::new(LxApps::new(runtime, executor.clone(), num_workers));

    // Set global instance early so get_platform() works
    if let Err(e) = super::runtime_registry::set_lxapps_manager(lxapps_manager.clone()) {
        error!("{}", e);
        return None;
    }

    // Create the home LxApp instance (loads lxapp.json once)
    let home_lxapp =
        match LxApp::new_as_home(home_app_id.clone(), runtime_arc.clone(), executor.clone()) {
            Ok(app) => app,
            Err(e) => {
                error!("Failed to setup home LxApp: {}", e).with_appid(home_app_id.clone());
                return None;
            }
        };

    let initial_route = home_lxapp.config.get_initial_route();
    home_lxapp.state.lock().unwrap().startup_options.path = initial_route;

    // Add home lxapp to the manager
    let home_app = Arc::new(home_lxapp);
    lxapps_manager
        .lxapps
        .insert(home_app_id.clone(), home_app.clone());

    // Pre-create JS worker for home lxapp when enabled. Native-only hosts skip this path.
    if let Err(e) = home_app.executor.create_app_svc(home_app.clone()) {
        error!("Failed to trigger home app service: {}", e).with_appid(home_app_id.clone());
    }

    info!("LxApps initialized successfully");

    spawn_cache_cleanup(runtime_arc.clone());
    Some(home_app_id)
}
