//! Top-level LxApp runtime operations (open/close/query/page-instance wrappers).

use super::*;

pub fn ensure_lxapp(appid: &str, release_type: ReleaseType) -> Result<Arc<LxApp>, LxAppError> {
    let manager = super::runtime_registry::get_lxapps_manager()
        .ok_or_else(|| LxAppError::Runtime("LxApps manager not initialized".to_string()))?;
    manager.ensure_lxapp(appid.to_string(), release_type)
}

/// Native-host bootstrap for a bundled control surface. Payload app ids never
/// select this class; the host calls it only after resolving its own resource.
#[doc(hidden)]
pub fn ensure_control_lxapp(
    appid: &str,
    release_type: ReleaseType,
) -> Result<Arc<LxApp>, LxAppError> {
    let manager = super::runtime_registry::get_lxapps_manager()
        .ok_or_else(|| LxAppError::Runtime("LxApps manager not initialized".to_string()))?;
    manager.ensure_lxapp_for_native_control(appid.to_string(), release_type)
}

pub fn ensure_builtin_lxapp(appid: &str) -> Result<Arc<LxApp>, LxAppError> {
    let manager = super::runtime_registry::get_lxapps_manager()
        .ok_or_else(|| LxAppError::Runtime("LxApps manager not initialized".to_string()))?;
    manager.ensure_builtin_lxapp(appid)
}

/// Ensure the SDK's content-less desktop surface owner exists. It provides a
/// runtime/session identity for the shared surface graph and managed providers;
/// it is not a product lxapp and never opens a page or Logic worker.
pub fn ensure_host_surface_owner() -> Result<Arc<LxApp>, LxAppError> {
    register_synthetic_lxapp(HOST_SURFACE_OWNER_APP_ID);
    ensure_builtin_lxapp(HOST_SURFACE_OWNER_APP_ID)
}

pub fn open_lxapp(appid: &str, options: LxAppStartupOptions) -> Result<Arc<LxApp>, LxAppError> {
    let manager = super::runtime_registry::get_lxapps_manager()
        .ok_or_else(|| LxAppError::Runtime("LxApps manager not initialized".to_string()))?;

    let app = manager.ensure_lxapp(appid.to_string(), options.release_type)?;
    app.open(options)?;
    Ok(app)
}

/// Bootstrap-TCB entry for reopening the configured control app at a page.
///
/// The app id must be the native-sealed home app id. A missing or stale
/// StandardApp instance is replaced with a ControlApp before navigation.
#[doc(hidden)]
pub fn open_control_lxapp_page(
    appid: &str,
    options: LxAppStartupOptions,
) -> Result<Arc<LxApp>, LxAppError> {
    let expected = lingxia_app_context::home_app_id().ok_or_else(|| {
        LxAppError::Runtime("control app identity is not initialized".to_string())
    })?;
    if appid != expected {
        return Err(LxAppError::InvalidParameter(format!(
            "control app identity mismatch: expected {expected}, got {appid}"
        )));
    }
    let app = ensure_control_lxapp(appid, options.release_type)?;
    if !app.is_control_app() {
        return Err(LxAppError::Runtime(format!(
            "current app session is not ControlApp: {appid}"
        )));
    }
    app.open(options)?;
    let current = super::runtime_registry::try_get(appid).ok_or_else(|| {
        LxAppError::ResourceNotFound(format!("current control app session not found: {appid}"))
    })?;
    if !current.is_control_app() {
        return Err(LxAppError::Runtime(format!(
            "current app session is not ControlApp: {appid}"
        )));
    }
    Ok(current)
}

pub fn list_lxapps() -> Vec<LxAppRuntimeInfo> {
    let Some(manager) = super::runtime_registry::get_lxapps_manager() else {
        return Vec::new();
    };
    let mut apps: Vec<LxAppRuntimeInfo> = manager
        .lxapps
        .iter()
        .filter(|entry| entry.key().as_str() != HOST_SURFACE_OWNER_APP_ID)
        .map(|entry| entry.value().runtime_info())
        .collect();
    apps.sort_by(|a, b| a.appid.cmp(&b.appid));
    apps
}

/// Re-resolve every `auto` lxapp after the host's system appearance changes.
pub fn refresh_auto_appearances() {
    let Some(manager) = super::runtime_registry::get_lxapps_manager() else {
        return;
    };
    let apps: Vec<_> = manager
        .lxapps
        .iter()
        .filter_map(|entry| {
            let app = entry.value().clone();
            (app.appearance_state().preference == AppearancePreference::Auto).then_some(app)
        })
        .collect();
    for app in apps {
        std::mem::drop(crate::executor::spawn(async move {
            let _ = app
                .set_appearance_preference(AppearancePreference::Auto)
                .await;
        }));
    }
}

pub fn close_lxapp(appid: &str) -> Result<(), LxAppError> {
    let app = super::runtime_registry::try_get(appid)
        .ok_or_else(|| LxAppError::ResourceNotFound(appid.to_string()))?;
    let session_id = app.session_id();
    if !app.begin_programmatic_close(session_id) {
        return Ok(());
    }
    // Leave the navigation stack before the shutdown hides the webview, so
    // the host's hide path restores the previous lxapp (not the closing one)
    // as the visible content.
    if let Some(manager) = super::runtime_registry::get_lxapps_manager() {
        manager.remove_from_stack(appid);
    }
    app.shutdown()?;
    app.complete_programmatic_close(session_id);
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
    app.refresh_page_instance_dispose_ttl(&id)
}

pub fn create_page_instance(
    req: CreatePageInstanceRequest,
) -> Result<CreatedPageInstance, LxAppError> {
    let app = super::runtime_registry::try_get(&req.appid)
        .ok_or_else(|| LxAppError::ResourceNotFound(req.appid.clone()))?;
    app.create_page_instance(req.owner, req.target, req.query, req.surface, None)
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
        let current_path = lxapp.peek_current_page_path().unwrap_or_default();
        let current_session = lxapp.session_id();
        info!(
            "Peek {}:{} (session={}) from lxapp stack",
            current_appid, current_path, current_session
        );
        return (current_appid, current_path, current_session);
    }
    (String::new(), String::new(), 0)
}

/// Move an opened lxapp to the top of the runtime navigation stack.
pub fn mark_lxapp_active(appid: &str) -> bool {
    let Some(manager) = super::runtime_registry::get_lxapps_manager() else {
        return false;
    };
    if !manager.lxapps.contains_key(appid) {
        return false;
    }
    manager.remove_from_stack(appid);
    manager.push_lxapp_stack(appid.to_string());
    true
}

pub fn notify_lxapp_host_visibility(appid: &str, visible: bool) -> Result<(), LxAppError> {
    let app = super::runtime_registry::try_get(appid)
        .ok_or_else(|| LxAppError::ResourceNotFound(appid.to_string()))?;
    // Destroying a WebView publishes its final host visibility asynchronously.
    // The close-specific onHide is queued before shutdown, so this late event
    // must not target an AppService that is already terminating.
    if matches!(
        app.status(),
        LxAppSessionStatus::Closing | LxAppSessionStatus::Closed
    ) {
        return Ok(());
    }
    let args = crate::lifecycle::AppServiceEventArgs {
        source: crate::lifecycle::AppServiceEventSource::Host,
        reason: if visible {
            crate::lifecycle::AppServiceEventReason::Foreground
        } else {
            crate::lifecycle::AppServiceEventReason::Background
        },
    }
    .to_json_string();
    app.appservice_notify(
        if visible {
            crate::lifecycle::AppServiceEvent::OnShow
        } else {
            crate::lifecycle::AppServiceEvent::OnHide
        },
        Some(args),
    )
}

pub fn notify_page_host_visibility(
    appid: &str,
    path: &str,
    visible: bool,
) -> Result<(), LxAppError> {
    let app = super::runtime_registry::try_get(appid)
        .ok_or_else(|| LxAppError::ResourceNotFound(appid.to_string()))?;
    if matches!(
        app.status(),
        LxAppSessionStatus::Closing | LxAppSessionStatus::Closed
    ) {
        return Ok(());
    }
    let page = app.require_page(path)?;
    page.dispatch_lifecycle_event(if visible {
        crate::lifecycle::PageLifecycleEvent::OnShow
    } else {
        crate::lifecycle::PageLifecycleEvent::OnHide
    });
    if visible {
        page.mark_active();
    }
    Ok(())
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
