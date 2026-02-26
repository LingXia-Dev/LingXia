use crate::i18n::{
    err_code_message, js_error_from_business_code_with_detail, js_error_from_lxapp_error,
    js_internal_error,
};
use lxapp::{LxApp, ReleaseType, UpdateManager, register_app_handler};
use rong::{
    Class, HostError, JSContext, JSFunc, JSObject, JSResult, JSValue, js_class, js_export,
    js_method,
};
use std::sync::Arc;

#[derive(Clone, Default)]
struct UpdateManagerState {
    manager: Option<JSObject>,
    on_ready: Option<JSFunc>,
    on_failed: Option<JSFunc>,
    handlers_registered: bool,
}

fn with_update_state(ctx: &JSContext, update: impl FnOnce(&mut UpdateManagerState)) {
    let mut state = ctx
        .get_state::<UpdateManagerState>()
        .cloned()
        .unwrap_or_default();
    update(&mut state);
    ctx.set_state(state);
}

fn manager_from_state(ctx: &JSContext) -> Option<JSObject> {
    ctx.get_state::<UpdateManagerState>()
        .and_then(|state| state.manager.clone())
}

fn callbacks_from_state(ctx: &JSContext) -> (Option<JSFunc>, Option<JSFunc>) {
    ctx.get_state::<UpdateManagerState>()
        .map(|state| (state.on_ready.clone(), state.on_failed.clone()))
        .unwrap_or((None, None))
}

// Register event handlers once per JSContext
fn ensure_update_handlers(ctx: &JSContext) -> JSResult<()> {
    let already_registered = ctx
        .get_state::<UpdateManagerState>()
        .map(|state| state.handlers_registered)
        .unwrap_or(false);

    if already_registered {
        return Ok(());
    }

    let ready_handler = JSFunc::new(ctx, |ctx: JSContext, _payload: JSObject| -> JSResult<()> {
        let (ready_cb, _) = callbacks_from_state(&ctx);
        if let Some(cb) = ready_cb {
            let _ = cb.call::<_, ()>(None, (_payload,));
        }
        Ok(())
    })?;
    register_app_handler(ctx, "UpdateReady", ready_handler)?;

    let failed_handler = JSFunc::new(ctx, |ctx: JSContext, _payload: JSObject| -> JSResult<()> {
        let (_, failed_cb) = callbacks_from_state(&ctx);
        if let Some(cb) = failed_cb {
            let _ = cb.call::<_, ()>(None, (_payload,));
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
        Err(HostError::new(
            rong::error::E_ILLEGAL_CONSTRUCTOR,
            err_code_message(1002),
        )
        .with_data(
            rong::err_data!({ bizCode: (1002), detail: ("UpdateManager cannot be directly constructed") }),
        )
        .into())
    }

    /// Apply update by restarting the app
    #[js_method(rename = "applyUpdate")]
    fn apply_update(&self, ctx: JSContext) -> JSResult<()> {
        let lxapp = LxApp::from_ctx(&ctx)?;
        lxapp.restart().map_err(|e| js_error_from_lxapp_error(&e))
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
            mark_fn(cb.as_js_value());
        }
        if let Some(cb) = &self.on_failed {
            mark_fn(cb.as_js_value());
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
        .map_err(|e| js_internal_error(format!("first-install check failed: {}", e)))?
    {
        return Ok(());
    }

    let pkg = manager
        .check_update(target_appid, release_type, None)
        .await
        .map_err(|e| {
            js_error_from_business_code_with_detail(
                5001,
                format!("failed to query first-install package: {}", e),
            )
        })?
        .ok_or_else(|| {
            js_error_from_business_code_with_detail(
                1003,
                format!("No package available for first install of {}", target_appid),
            )
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
        .map_err(|e| {
            js_error_from_business_code_with_detail(
                5001,
                format!("failed to download first-install package: {}", e),
            )
        })?;

    Ok(())
}
