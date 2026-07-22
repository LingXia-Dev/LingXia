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

use std::sync::atomic::{AtomicBool, Ordering};

/// Set by dev/test hosts (the Runner) to grant `lx.automation()` without an
/// lxapp declaring the `automation`/`host` privilege. Off for product hosts,
/// where the manifest gates as usual. See `set_automation_auto_grant`.
static AUTOMATION_AUTO_GRANT: AtomicBool = AtomicBool::new(false);

/// Grant automation privileges to every lxapp in this process, bypassing the
/// manifest privilege check. Call once at host startup — the Runner does this
/// so lxapps launched for testing need not declare `automation`/`host`.
pub fn set_automation_auto_grant(enabled: bool) {
    AUTOMATION_AUTO_GRANT.store(enabled, Ordering::Relaxed);
}

/// Whether this host auto-grants automation (Runner/dev harness). A dev session
/// also implies auto-grant, so callers usually check both.
pub fn automation_auto_grant() -> bool {
    AUTOMATION_AUTO_GRANT.load(Ordering::Relaxed)
}

/// Whether this process is an active `lingxia dev` session: a dev websocket is
/// configured either via the `LINGXIA_DEV_WS_URL` env var or `app.json`'s
/// `dev_ws_url` (written by `lingxia dev`). Drives dev-only behaviour such as
/// forcing a bundled-asset refresh and enabling WebView debugging.
pub fn dev_session_active() -> bool {
    let env_active = std::env::var("LINGXIA_DEV_WS_URL")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    if env_active {
        return true;
    }
    lingxia_app_context::app_config()
        .and_then(|config| config.dev_ws_url.as_deref())
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

/// Whether this process is the LingXia Runner (the `lingxia dev` device
/// simulator), which sets `LINGXIA_RUNNER` on its child process. Unlike a real
/// host app in dev mode, the Runner lacks host-declared surfaces such as the
/// terminal; the bridge exposes this so apps can hide those affordances.
pub fn runner_active() -> bool {
    std::env::var("LINGXIA_RUNNER")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
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

    let num_workers = get_num_workers();
    let executor = LxAppWorkers::init(num_workers);

    // Hosts may launch native or web content without a home lxapp. Initialize
    // the manager first so lxapps opened later still have the complete runtime.
    let lxapps_manager = Arc::new(LxApps::new(runtime, executor.clone(), num_workers));
    if let Err(e) = super::runtime_registry::set_lxapps_manager(lxapps_manager.clone()) {
        error!("{}", e);
        return None;
    }

    let (Some(home_app_id), Some(home_app_version)) = (
        lingxia_app_context::home_app_id(),
        lingxia_app_context::home_app_version(),
    ) else {
        info!("LxApps initialized without a home lxapp");
        spawn_cache_cleanup(runtime_arc);
        return None;
    };
    let home_app_id = home_app_id.to_string();

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

    let dev_session_active = dev_session_active();
    let should_reinstall_home = dev_session_active
        || installed_home_version
            .as_ref()
            .map(|installed| installed < &bundled_home_version)
            .unwrap_or(true);

    // In dev mode the home app is served directly from its `DevPath` root (the
    // freshly-built `dist`), not from bundled assets — so it must not be
    // installed/reinstalled from the runner's assets (which don't contain the
    // dev lxapp). `LxApp::new_as_home` loads it straight from the dev root.
    let home_is_dev_sourced = matches!(
        super::lxapp_bundle_source_for(&home_app_id),
        Some(super::LxAppBundleSource::DevPath { .. })
    );

    if home_is_dev_sourced {
        info!("Home lxapp is dev-sourced; serving from dev root, skipping bundled-asset install")
            .with_appid(home_app_id.clone());
    } else if should_reinstall_home {
        let reason = if dev_session_active {
            "dev session active; refreshing from bundled assets".to_string()
        } else {
            match installed_home_version {
                None => "home lxapp is not installed or install is invalid".to_string(),
                Some(installed) => format!(
                    "bundled version {} is newer than installed {}",
                    bundled_home_version, installed
                ),
            }
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
