use lxapp::{LxApp, ReleaseType, UpdateManager, emit_app_event, register_app_handler, warn};
use rong::{
    Class, JSContext, JSFunc, JSObject, JSResult, JSValue, RongJSError, js_class, js_export,
    js_method, service_executor,
};
use std::sync::Arc;

#[derive(Clone, Default)]
struct UpdateManagerState {
    manager: Option<JSObject>,
    on_ready: Option<JSFunc>,
    on_failed: Option<JSFunc>,
    handlers_registered: bool,
}

impl rong::JSContextService for UpdateManagerState {}

fn with_update_state(ctx: &JSContext, update: impl FnOnce(&mut UpdateManagerState)) {
    let mut state = ctx
        .get_service::<UpdateManagerState>()
        .cloned()
        .unwrap_or_default();
    update(&mut state);
    ctx.set_service::<UpdateManagerState>(state);
}

fn manager_from_state(ctx: &JSContext) -> Option<JSObject> {
    ctx.get_service::<UpdateManagerState>()
        .and_then(|state| state.manager.clone())
}

fn callbacks_from_state(ctx: &JSContext) -> (Option<JSFunc>, Option<JSFunc>) {
    ctx.get_service::<UpdateManagerState>()
        .map(|state| (state.on_ready.clone(), state.on_failed.clone()))
        .unwrap_or((None, None))
}

// Register event handlers once per JSContext
fn ensure_update_handlers(ctx: &JSContext) -> JSResult<()> {
    let already_registered = ctx
        .get_service::<UpdateManagerState>()
        .map(|state| state.handlers_registered)
        .unwrap_or(false);

    if already_registered {
        return Ok(());
    }

    let ready_handler = JSFunc::new(ctx, |ctx: JSContext, _payload: JSObject| -> JSResult<()> {
        let (ready_cb, _) = callbacks_from_state(&ctx);
        if let Some(cb) = ready_cb {
            let _ = cb.call::<_, ()>(None, ());
        }
        Ok(())
    })?;
    register_app_handler(ctx, "UpdateReady", ready_handler)?;

    let failed_handler = JSFunc::new(ctx, |ctx: JSContext, _payload: JSObject| -> JSResult<()> {
        let (_, failed_cb) = callbacks_from_state(&ctx);
        if let Some(cb) = failed_cb {
            let _ = cb.call::<_, ()>(None, ());
        }
        Ok(())
    })?;
    register_app_handler(ctx, "UpdateFailed", failed_handler)?;

    with_update_state(ctx, |state| state.handlers_registered = true);

    Ok(())
}

/// JS Update Manager - simply restarts app to apply downloaded updates
#[js_export]
pub(crate) struct JSUpdateManager {
    on_ready: Option<JSFunc>,
    on_failed: Option<JSFunc>,
}

impl JSUpdateManager {
    pub fn new() -> Self {
        Self {
            on_ready: None,
            on_failed: None,
        }
    }
}

#[js_class]
impl JSUpdateManager {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(RongJSError::TypeError(
            "UpdateManager cannot be directly constructed".to_string(),
        ))
    }

    /// Apply update by restarting the app
    #[js_method(rename = "applyUpdate")]
    fn apply_update(&self, ctx: JSContext) -> JSResult<()> {
        let lxapp = LxApp::from_ctx(&ctx)?;
        let _ = lxapp.restart();
        Ok(())
    }

    #[js_method(rename = "onUpdateReady")]
    fn on_update_ready(&mut self, ctx: JSContext, cb: JSFunc) -> JSResult<()> {
        self.on_ready = Some(cb.clone());
        with_update_state(&ctx, |state| state.on_ready = Some(cb));
        Ok(())
    }

    #[js_method(rename = "onUpdateFailed")]
    fn on_update_failed(&mut self, ctx: JSContext, cb: JSFunc) -> JSResult<()> {
        self.on_failed = Some(cb.clone());
        with_update_state(&ctx, |state| state.on_failed = Some(cb));
        Ok(())
    }

    #[js_method(gc_mark)]
    fn gc_mark(&self, mut mark_fn: impl FnMut(&JSValue)) {
        if let Some(cb) = &self.on_ready {
            mark_fn(cb.as_jsvalue());
        }
        if let Some(cb) = &self.on_failed {
            mark_fn(cb.as_jsvalue());
        }
    }
}

// Register Update-related JS bindings
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    ctx.register_class::<JSUpdateManager>()?;

    // lx.getUpdateManager() -> returns singleton instance
    fn get_update_manager(ctx: JSContext) -> JSResult<JSObject> {
        ensure_update_handlers(&ctx)?;

        if let Some(existing) = manager_from_state(&ctx) {
            return Ok(existing);
        }

        let class = Class::get::<JSUpdateManager>(&ctx)?;
        let instance = class.instance(JSUpdateManager::new());
        with_update_state(&ctx, |state| state.manager = Some(instance.clone()));
        Ok(instance)
    }

    let get_update_manager = JSFunc::new(ctx, get_update_manager)?.name("getUpdateManager")?;
    lxapp::lx::register_js_api(ctx, "getUpdateManager", get_update_manager)?;
    Ok(())
}

/// Ensure the target app is installed at least once (first-launch preparation).
pub async fn ensure_first_install(
    current_lxapp: &Arc<LxApp>,
    target_appid: &str,
    release_type: ReleaseType,
) -> JSResult<()> {
    let manager = UpdateManager::new(current_lxapp.clone());

    if manager
        .is_installed(target_appid, release_type)
        .map_err(|e| RongJSError::Error(e.to_string()))?
    {
        return Ok(());
    }

    let check = manager
        .check_update(target_appid, release_type, None)
        .await
        .map_err(|e| RongJSError::Error(e.to_string()))?;

    let pkg = check.package.ok_or_else(|| {
        RongJSError::Error(format!(
            "No package available for first install of {}",
            target_appid
        ))
    })?;

    manager
        .download_archive_with_checksum(
            target_appid,
            release_type,
            &pkg.url,
            &pkg.checksum_sha256,
            &pkg.version,
        )
        .await
        .map_err(|e| RongJSError::Error(e.to_string()))?;

    Ok(())
}

/// Spawn a background task to check cloud updates for the given app and pre-download newer packages.
pub fn spawn_background_update_check(target_appid: String, release_type: ReleaseType) {
    let _ = service_executor::spawn_async(async move {
        let Some(lxapp) = lxapp::try_get(&target_appid) else {
            warn!(
                "LxApp '{}' not found for background update check",
                target_appid
            );
            return;
        };
        let manager = UpdateManager::new(lxapp.clone());

        let current_version = lxapp.current_version();
        match manager
            .check_update(&target_appid, release_type, Some(current_version.as_str()))
            .await
        {
            Ok(check) => {
                if let Some(pkg) = check.package {
                    if !manager.should_update(&pkg.version) {
                        return;
                    }

                    let already_downloaded_same = matches!(
                        manager.has_downloaded_update(&target_appid, release_type),
                        Ok(Some(info)) if info.version == pkg.version
                    );

                    if already_downloaded_same {
                        return;
                    }

                    let download_res = manager
                        .download_archive_with_checksum(
                            &target_appid,
                            release_type,
                            &pkg.url,
                            &pkg.checksum_sha256,
                            &pkg.version,
                        )
                        .await;

                    if download_res.is_ok() {
                        let _ = emit_app_event(&target_appid, "UpdateReady", None);
                    } else {
                        let _ = emit_app_event(&target_appid, "UpdateFailed", None);
                    }
                }
            }
            Err(_) => {
                // Ignore check errors in background
            }
        }
    });
}
