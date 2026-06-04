use crate::i18n::{
    err_code_message, js_error_from_business_code_with_detail, js_error_from_lxapp_error,
    js_internal_error,
};
use lxapp::{
    LxApp, LxAppUpdateQuery, ReleaseType, UpdateManager, register_app_handler, try_get, warn,
};
use rong::{
    Class, HostError, JSContext, JSFunc, JSObject, JSResult, JSRuntimeService, JSValue, js_class,
    js_export, js_method,
};
use std::cell::RefCell;
use std::sync::Arc;

#[derive(Default)]
struct UpdateManagerState {
    manager: Option<JSObject>,
    lxappid: Option<String>,
    on_ready: Option<JSFunc>,
    on_failed: Option<JSFunc>,
    pending_ready: Option<JSObject>,
    pending_failed: Option<JSObject>,
    handlers_registered: bool,
}

#[derive(Default)]
struct UpdateManagerRegistry {
    state: RefCell<UpdateManagerState>,
}

impl JSRuntimeService for UpdateManagerRegistry {}

fn with_update_state(ctx: &JSContext, update: impl FnOnce(&mut UpdateManagerState)) {
    let registry = ctx.runtime().get_or_init_service::<UpdateManagerRegistry>();
    let mut state = registry.state.borrow_mut();
    update(&mut state);
}

fn read_update_state<R>(ctx: &JSContext, read: impl FnOnce(&UpdateManagerState) -> R) -> R {
    let registry = ctx.runtime().get_or_init_service::<UpdateManagerRegistry>();
    let state = registry.state.borrow();
    read(&state)
}

fn callbacks_from_state(ctx: &JSContext) -> (Option<JSFunc>, Option<JSFunc>) {
    read_update_state(ctx, |state| {
        (state.on_ready.clone(), state.on_failed.clone())
    })
}

fn take_pending_ready(ctx: &JSContext) -> Option<JSObject> {
    let mut pending = None;
    with_update_state(ctx, |state| {
        pending = state.pending_ready.take();
    });
    pending
}

fn take_pending_failed(ctx: &JSContext) -> Option<JSObject> {
    let mut pending = None;
    with_update_state(ctx, |state| {
        pending = state.pending_failed.take();
    });
    pending
}

// Register event handlers once per JSContext
fn ensure_update_handlers(ctx: &JSContext) -> JSResult<()> {
    let already_registered = read_update_state(ctx, |state| state.handlers_registered);

    if already_registered {
        return Ok(());
    }

    let ready_handler = JSFunc::new(ctx, |ctx: JSContext, _payload: JSObject| -> JSResult<()> {
        let (ready_cb, _) = callbacks_from_state(&ctx);
        if let Some(cb) = ready_cb {
            if cb.call::<_, ()>(None, (_payload.clone(),)).is_err() {
                warn!("UpdateReady callback invocation failed; preserving as pending event");
                with_update_state(&ctx, |state| state.pending_ready = Some(_payload));
            }
        } else {
            with_update_state(&ctx, |state| state.pending_ready = Some(_payload));
        }
        Ok(())
    })?;
    register_app_handler(ctx, "UpdateReady", ready_handler)?;

    let failed_handler = JSFunc::new(ctx, |ctx: JSContext, _payload: JSObject| -> JSResult<()> {
        let (_, failed_cb) = callbacks_from_state(&ctx);
        if let Some(cb) = failed_cb {
            if cb.call::<_, ()>(None, (_payload.clone(),)).is_err() {
                warn!("UpdateFailed callback invocation failed; preserving as pending event");
                with_update_state(&ctx, |state| state.pending_failed = Some(_payload));
            }
        } else {
            with_update_state(&ctx, |state| state.pending_failed = Some(_payload));
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
    appid: String,
    on_ready: Option<JSFunc>,
    on_failed: Option<JSFunc>,
}

impl JSUpdateManager {
    pub fn new(appid: String) -> Self {
        Self {
            appid,
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
        let target_appid = if !self.appid.is_empty() {
            self.appid.clone()
        } else {
            LxApp::from_ctx(&ctx)?.appid.clone()
        };
        if target_appid.is_empty() {
            return Err(HostError::new(
                rong::error::E_INTERNAL,
                "UpdateManager has no bound appid for applyUpdate",
            )
            .into());
        }

        let lxapp = match try_get(&target_appid) {
            Some(lxapp) => lxapp,
            None => {
                return Err(HostError::new(
                    rong::error::E_INTERNAL,
                    format!("LxApp '{}' not found for applyUpdate", target_appid),
                )
                .into());
            }
        };
        lxapp.restart().map_err(|e| js_error_from_lxapp_error(&e))
    }

    #[js_method(rename = "onUpdateReady")]
    fn on_update_ready(&mut self, ctx: JSContext, cb: JSFunc) -> JSResult<()> {
        self.on_ready = Some(cb.clone());
        with_update_state(&ctx, |state| state.on_ready = Some(cb));
        if let Some(payload) = take_pending_ready(&ctx)
            && let Some(ready_cb) = self.on_ready.as_ref()
            && ready_cb.call::<_, ()>(None, (payload.clone(),)).is_err()
        {
            warn!("Flushing pending UpdateReady failed; keeping event pending");
            with_update_state(&ctx, |state| state.pending_ready = Some(payload));
        }
        Ok(())
    }

    #[js_method(rename = "onUpdateFailed")]
    fn on_update_failed(&mut self, ctx: JSContext, cb: JSFunc) -> JSResult<()> {
        self.on_failed = Some(cb.clone());
        with_update_state(&ctx, |state| state.on_failed = Some(cb));
        if let Some(payload) = take_pending_failed(&ctx)
            && let Some(failed_cb) = self.on_failed.as_ref()
            && failed_cb.call::<_, ()>(None, (payload.clone(),)).is_err()
        {
            warn!("Flushing pending UpdateFailed failed; keeping event pending");
            with_update_state(&ctx, |state| state.pending_failed = Some(payload));
        }
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
    ctx.runtime().get_or_init_service::<UpdateManagerRegistry>();
    // Register host event handlers early so UpdateReady/UpdateFailed are not lost
    // before lx.getUpdateManager() is called by app logic.
    ensure_update_handlers(ctx)?;

    // lx.getUpdateManager() -> returns singleton instance
    fn get_update_manager(ctx: JSContext) -> JSResult<JSObject> {
        ensure_update_handlers(&ctx)?;

        let current_appid = LxApp::from_ctx(&ctx)?.appid.clone();

        let existing = read_update_state(&ctx, |state| {
            if state.lxappid.as_deref() == Some(current_appid.as_str()) {
                state.manager.clone()
            } else {
                None
            }
        });
        if let Some(manager) = existing {
            return Ok(manager);
        }

        let class = Class::lookup::<JSUpdateManager>(&ctx)?;
        let instance = class.instance(JSUpdateManager::new(current_appid.clone()));
        with_update_state(&ctx, |state| {
            state.lxappid = Some(current_appid);
            state.manager = Some(instance.clone());
            // Drop callbacks/pending payload from any previous app binding.
            state.on_ready = None;
            state.on_failed = None;
            state.pending_ready = None;
            state.pending_failed = None;
        });
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
        .check_update(
            target_appid,
            release_type,
            LxAppUpdateQuery::Latest {
                current_version: None,
            },
        )
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
