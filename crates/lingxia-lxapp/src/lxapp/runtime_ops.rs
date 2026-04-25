//! Top-level LxApp runtime operations (open/close/query/page-instance wrappers).

use super::*;

pub fn ensure_lxapp(appid: &str, release_type: ReleaseType) -> Result<Arc<LxApp>, LxAppError> {
    let manager = super::runtime_registry::get_lxapps_manager()
        .ok_or_else(|| LxAppError::Runtime("LxApps manager not initialized".to_string()))?;
    manager.ensure_lxapp(appid.to_string(), release_type)
}

pub fn ensure_builtin_lxapp(appid: &str) -> Result<Arc<LxApp>, LxAppError> {
    let manager = super::runtime_registry::get_lxapps_manager()
        .ok_or_else(|| LxAppError::Runtime("LxApps manager not initialized".to_string()))?;
    if let Some(app) = manager.lxapps.get(appid) {
        return Ok(app.clone());
    }
    if !matches!(
        lxapp_bundle_source_for(appid),
        Some(LxAppBundleSource::BuiltinAssets { .. })
    ) {
        return Err(LxAppError::ResourceNotFound(format!(
            "builtin lxapp asset bundle not registered: {appid}"
        )));
    }

    let app = Arc::new(LxApp::new(
        appid.to_string(),
        manager.runtime.clone(),
        manager.executor.clone(),
        ReleaseType::Release,
    )?);
    manager.lxapps.insert(appid.to_string(), app.clone());
    Ok(app)
}

pub fn open_lxapp(appid: &str, options: LxAppStartupOptions) -> Result<Arc<LxApp>, LxAppError> {
    let manager = super::runtime_registry::get_lxapps_manager()
        .ok_or_else(|| LxAppError::Runtime("LxApps manager not initialized".to_string()))?;

    let app = manager.ensure_lxapp(appid.to_string(), options.release_type)?;
    app.open(options)?;
    Ok(app)
}

pub fn list_lxapps() -> Vec<LxAppRuntimeInfo> {
    let Some(manager) = super::runtime_registry::get_lxapps_manager() else {
        return Vec::new();
    };
    let mut apps: Vec<LxAppRuntimeInfo> = manager
        .lxapps
        .iter()
        .map(|entry| entry.value().runtime_info())
        .collect();
    apps.sort_by(|a, b| a.appid.cmp(&b.appid));
    apps
}

pub fn close_lxapp(appid: &str) -> Result<(), LxAppError> {
    let app = super::runtime_registry::try_get(appid)
        .ok_or_else(|| LxAppError::ResourceNotFound(appid.to_string()))?;
    app.shutdown()?;
    if let Some(manager) = super::runtime_registry::get_lxapps_manager() {
        manager.remove_from_stack(appid);
    }
    Ok(())
}

pub fn restart_lxapp(appid: &str) -> Result<(), LxAppError> {
    let app = super::runtime_registry::try_get(appid)
        .ok_or_else(|| LxAppError::ResourceNotFound(appid.to_string()))?;
    app.restart()
}

pub fn uninstall_lxapp(appid: &str) -> Result<(), LxAppError> {
    let manager = super::runtime_registry::get_lxapps_manager()
        .ok_or_else(|| LxAppError::Runtime("LxApps manager not initialized".to_string()))?;
    let app = if let Some(app) = super::runtime_registry::try_get(appid) {
        manager.destroy_lxapp_with_options(appid, true);
        app
    } else {
        manager
            .lxapps
            .iter()
            .next()
            .map(|entry| entry.value().clone())
            .ok_or_else(|| LxAppError::Runtime("No LxApp runtime available".to_string()))?
    };
    let updater = UpdateManager::new(app);
    updater.uninstall_all(appid)
}

pub fn installed_lxapp_path(appid: &str, release_type: ReleaseType) -> Option<String> {
    metadata::get(appid, release_type)
        .ok()
        .flatten()
        .map(|record| record.install_path)
}

pub fn touch_page_instance_by_id(id: &str) -> Result<(), LxAppError> {
    let id = PageInstanceId::parse(id.to_string()).ok_or_else(|| {
        LxAppError::InvalidParameter("page instance id must not be empty".to_string())
    })?;
    let page = super::runtime_registry::find_page_by_instance_id(id.as_str())
        .ok_or_else(|| LxAppError::ResourceNotFound(format!("page instance id: {}", id)))?;
    let app = super::runtime_registry::try_get(&page.appid())
        .ok_or_else(|| LxAppError::ResourceNotFound(page.appid()))?;
    app.refresh_page_instance_warm_ttl(&id)
}

pub fn create_page_instance(
    req: CreatePageInstanceRequest,
) -> Result<CreatedPageInstance, LxAppError> {
    let app = super::runtime_registry::try_get(&req.appid)
        .ok_or_else(|| LxAppError::ResourceNotFound(req.appid.clone()))?;
    app.create_page_instance(
        req.owner,
        req.target,
        req.query,
        req.presentation,
        req.warm_dispose_policy,
    )
}

pub fn notify_page_instance(
    id: &PageInstanceId,
    event: PageInstanceEvent,
) -> Result<(), LxAppError> {
    let page = super::runtime_registry::find_page_by_instance_id(id.as_str())
        .ok_or_else(|| LxAppError::ResourceNotFound(format!("page instance id: {}", id)))?;
    let app = super::runtime_registry::try_get(&page.appid())
        .ok_or_else(|| LxAppError::ResourceNotFound(page.appid()))?;
    app.notify_page_instance(id, event)
}

pub fn notify_page_instance_by_id(id: &str, event: PageInstanceEvent) -> Result<(), LxAppError> {
    let id = PageInstanceId::parse(id.to_string()).ok_or_else(|| {
        LxAppError::InvalidParameter("page instance id must not be empty".to_string())
    })?;
    notify_page_instance(&id, event)
}

pub fn dispose_page_instance(id: &PageInstanceId, reason: CloseReason) -> Result<(), LxAppError> {
    let page = super::runtime_registry::find_page_by_instance_id(id.as_str())
        .ok_or_else(|| LxAppError::ResourceNotFound(format!("page instance id: {}", id)))?;
    let app = super::runtime_registry::try_get(&page.appid())
        .ok_or_else(|| LxAppError::ResourceNotFound(page.appid()))?;
    app.dispose_page_instance(id, reason)
}

pub fn dispose_page_instance_by_id(id: &str, reason: CloseReason) -> Result<(), LxAppError> {
    let id = PageInstanceId::parse(id.to_string()).ok_or_else(|| {
        LxAppError::InvalidParameter("page instance id must not be empty".to_string())
    })?;
    dispose_page_instance(&id, reason)
}

/// Triggers memory cleanup for LxApps.
/// This function should be called by the platform when the system is under memory pressure.
pub fn on_low_memory() {
    if let Some(manager) = super::runtime_registry::get_lxapps_manager() {
        info!("on_low_memory triggered, evicting least recently used app.");
        manager.evict_lru_lxapp();
    }
}

/// Get the current lxapp from the navigation stack and its current page path/session.
/// Returns (appid, current_page_path, session_id) or empty/0 if not found.
pub fn get_current_lxapp() -> (String, String, u64) {
    if let Some(manager) = super::runtime_registry::get_lxapps_manager()
        && let Some(current_appid) = manager.peek_lxapp_stack()
        && let Some(lxapp) = manager.lxapps.get(&current_appid)
    {
        let current_path = lxapp.peek_current_page().unwrap_or_default();
        let current_session = lxapp.session_id();
        info!(
            "Peek {}:{} (session={}) from lxapp stack",
            current_appid, current_path, current_session
        );
        return (current_appid, current_path, current_session);
    }
    (String::new(), String::new(), 0)
}

/// Check if pull-to-refresh is enabled for a specific page
/// Returns false if the app or page is not found
pub fn is_pull_down_refresh_enabled(appid: &str, path: &str) -> bool {
    super::runtime_registry::try_get(appid)
        .map(|lxapp| lxapp.is_pull_down_refresh_enabled(path))
        .unwrap_or(false)
}

/// Check whether a given appid is currently opened (in-memory and marked opened).
pub fn is_lxapp_open(lxappid: &str) -> bool {
    if let Some(manager) = super::runtime_registry::get_lxapps_manager()
        && let Some(app) = manager.lxapps.get(lxappid)
    {
        return app.is_opened();
    }
    false
}
