use crate::i18n::{err_code_message, js_error_from_lxapp_error};
use lxapp::{Channel, LxApp, register_app_handler, try_get, warn};
use rong::{
    Class, HostError, JSContext, JSContextService, JSFunc, JSObject, JSResult, JSValue, js_class,
    js_method,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

/// Identifies one `onUpdateReady`/`onUpdateFailed` registration so an
/// unsubscribe handle clears only its own listener.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct UpdateSlotToken(u64);

struct UpdateListener {
    token: UpdateSlotToken,
    callback: JSFunc,
}

#[derive(Default)]
struct UpdateManagerState {
    manager: Option<JSObject>,
    lxappid: Option<String>,
    on_ready: Vec<UpdateListener>,
    on_failed: Vec<UpdateListener>,
    next_token: u64,
    pending_ready: Option<JSObject>,
    pending_failed: Option<JSObject>,
    handlers_registered: bool,
}

#[derive(Default)]
struct UpdateManagerRegistry {
    /// Shared so an unsubscribe handle can reach the slot without carrying a
    /// `JSContext` clone. Cloning a context bumps its lifecycle-owner count, and
    /// the handle lives *inside* that context — the pair would keep each other
    /// alive and the context would never shut down.
    state: Rc<RefCell<UpdateManagerState>>,
}

impl JSContextService for UpdateManagerRegistry {}

fn update_registry(ctx: &JSContext) -> &UpdateManagerRegistry {
    if ctx.get_service::<UpdateManagerRegistry>().is_none() {
        ctx.set_service(UpdateManagerRegistry::default());
    }
    ctx.get_service::<UpdateManagerRegistry>()
        .expect("update manager registry was inserted above")
}

/// Hands out the next slot token. Zero stays reserved for "no subscriber", so
/// a cleared slot never matches a live handle.
fn claim_slot_token(ctx: &JSContext) -> UpdateSlotToken {
    let mut token = UpdateSlotToken::default();
    with_update_state(ctx, |state| {
        state.next_token += 1;
        token = UpdateSlotToken(state.next_token);
    });
    token
}

fn with_update_state(ctx: &JSContext, update: impl FnOnce(&mut UpdateManagerState)) {
    let registry = update_registry(ctx);
    let mut state = registry.state.borrow_mut();
    update(&mut state);
}

fn read_update_state<R>(ctx: &JSContext, read: impl FnOnce(&UpdateManagerState) -> R) -> R {
    let registry = update_registry(ctx);
    let state = registry.state.borrow();
    read(&state)
}

fn callbacks_from_state(ctx: &JSContext) -> (Vec<JSFunc>, Vec<JSFunc>) {
    read_update_state(ctx, |state| {
        (
            state
                .on_ready
                .iter()
                .map(|listener| listener.callback.clone())
                .collect(),
            state
                .on_failed
                .iter()
                .map(|listener| listener.callback.clone())
                .collect(),
        )
    })
}

fn pending_ready(ctx: &JSContext) -> Option<JSObject> {
    read_update_state(ctx, |state| state.pending_ready.clone())
}

fn pending_failed(ctx: &JSContext) -> Option<JSObject> {
    read_update_state(ctx, |state| state.pending_failed.clone())
}

fn deliver_update_listeners(listeners: &[JSFunc], payload: &JSObject) {
    for callback in listeners {
        if callback.call::<_, ()>(None, (payload.clone(),)).is_err() {
            warn!("Update callback invocation failed; keeping the pending event");
        }
    }
}

// Register event handlers once per JSContext
fn ensure_update_handlers(ctx: &JSContext) -> JSResult<()> {
    let already_registered = read_update_state(ctx, |state| state.handlers_registered);

    if already_registered {
        return Ok(());
    }

    let ready_handler = JSFunc::new(ctx, |ctx: JSContext, _payload: JSObject| -> JSResult<()> {
        with_update_state(&ctx, |state| state.pending_ready = Some(_payload.clone()));
        let (ready_cb, _) = callbacks_from_state(&ctx);
        deliver_update_listeners(&ready_cb, &_payload);
        Ok(())
    })?;
    register_app_handler(ctx, "UpdateReady", ready_handler)?;

    let failed_handler = JSFunc::new(ctx, |ctx: JSContext, _payload: JSObject| -> JSResult<()> {
        with_update_state(&ctx, |state| state.pending_failed = Some(_payload.clone()));
        let (_, failed_cb) = callbacks_from_state(&ctx);
        deliver_update_listeners(&failed_cb, &_payload);
        Ok(())
    })?;
    register_app_handler(ctx, "UpdateFailed", failed_handler)?;

    with_update_state(ctx, |state| state.handlers_registered = true);

    Ok(())
}

/// Callback-based update manager for this lxapp's bundle.
#[js_class(clone)]
pub(crate) struct JSUpdateManager {
    appid: String,
    on_ready: Vec<(UpdateSlotToken, JSFunc)>,
    on_failed: Vec<(UpdateSlotToken, JSFunc)>,
}

impl JSUpdateManager {
    pub fn new(appid: String) -> Self {
        Self {
            appid,
            on_ready: Vec::new(),
            on_failed: Vec::new(),
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

    /// Subscribes to a ready update and returns the unsubscribe fn.
    #[js_method(rename = "onUpdateReady")]
    fn on_update_ready(&mut self, ctx: JSContext, cb: JSFunc) -> JSResult<JSFunc> {
        let token = claim_slot_token(&ctx);
        self.on_ready.push((token, cb.clone()));
        with_update_state(&ctx, |state| {
            state.on_ready.push(UpdateListener {
                token,
                callback: cb.clone(),
            });
        });
        if let Some(payload) = pending_ready(&ctx)
            && cb.call::<_, ()>(None, (payload,)).is_err()
        {
            warn!("Flushing pending UpdateReady failed; keeping event pending");
        }
        let slot = update_registry(&ctx).state.clone();
        JSFunc::new(&ctx, move || {
            let mut state = slot.borrow_mut();
            state.on_ready.retain(|listener| listener.token != token);
        })
    }

    /// Subscribes to a failed update and returns the unsubscribe fn.
    #[js_method(rename = "onUpdateFailed")]
    fn on_update_failed(&mut self, ctx: JSContext, cb: JSFunc) -> JSResult<JSFunc> {
        let token = claim_slot_token(&ctx);
        self.on_failed.push((token, cb.clone()));
        with_update_state(&ctx, |state| {
            state.on_failed.push(UpdateListener {
                token,
                callback: cb.clone(),
            });
        });
        if let Some(payload) = pending_failed(&ctx)
            && cb.call::<_, ()>(None, (payload,)).is_err()
        {
            warn!("Flushing pending UpdateFailed failed; keeping event pending");
        }
        let slot = update_registry(&ctx).state.clone();
        JSFunc::new(&ctx, move || {
            let mut state = slot.borrow_mut();
            state.on_failed.retain(|listener| listener.token != token);
        })
    }

    #[js_method(gc_mark)]
    fn gc_mark(&self, mut mark_fn: impl FnMut(&JSValue)) {
        for (_, cb) in &self.on_ready {
            mark_fn(cb.as_js_value());
        }
        for (_, cb) in &self.on_failed {
            mark_fn(cb.as_js_value());
        }
    }
}

// Register Update-related JS bindings
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    ctx.register_class::<JSUpdateManager>()?;
    update_registry(ctx);
    // Register host event handlers early so UpdateReady/UpdateFailed are not lost
    // before lx.getUpdateManager() is called by app logic.
    ensure_update_handlers(ctx)?;

    register_update_api(ctx)
}

/// Return the callback-based update manager for this lxapp's bundle. This is
/// available to every lxapp and is distinct from the Control-app-only
/// `lx.host.checkUpdate()`, which updates the native host app.
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
        // Drop callbacks/pending payload from any previous app binding. The
        // tokens go with them, so a handle from the old binding cannot match
        // and clear a subscriber the new one installs.
        state.on_ready.clear();
        state.on_failed.clear();
        state.pending_ready = None;
        state.pending_failed = None;
    });
    Ok(instance)
}

rong::js_api! {
    fn register_update_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        fn getUpdateManager(ts_return = "UpdateManager") = get_update_manager;
    }
}

/// Ensure the target app is installed at least once (first-launch preparation).
pub async fn ensure_first_install(
    current_lxapp: &Arc<LxApp>,
    target_appid: &str,
    release_type: Channel,
) -> JSResult<()> {
    lxapp::ensure_first_install(current_lxapp, target_appid, release_type)
        .await
        .map_err(|error| js_error_from_lxapp_error(&error))
}

#[cfg(test)]
mod tests {
    use super::{read_update_state, with_update_state};
    use rong::{JSEngine, RongJS};

    #[test]
    fn update_state_does_not_cross_contexts() {
        let runtime = RongJS::runtime();
        let first = runtime.context();
        with_update_state(&first, |state| state.handlers_registered = true);
        assert!(read_update_state(&first, |state| state.handlers_registered));
        drop(first);

        let second = runtime.context();
        assert!(!read_update_state(&second, |state| state.handlers_registered));
    }
}
