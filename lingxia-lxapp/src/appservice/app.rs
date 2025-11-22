use crate::appservice::{set_app_svc_for_ctx, update::JSUpdateManager};
use crate::event::AppServiceEvent;
use rong::{
    Class, JSContext, JSFunc, JSObject, JSResult, JSValue, RongJSError, Source, js_class,
    js_export, js_method,
};
use std::cell::RefCell;
use std::collections::HashMap;

#[js_export]
pub(crate) struct LxAppSvc {
    event_handlers: HashMap<AppServiceEvent, JSFunc>,
    this: JSObject,
    update_manager: RefCell<Option<JSObject>>,
}

#[js_class]
impl LxAppSvc {
    #[js_method(constructor)]
    fn _new(ctx: JSContext, obj: JSObject) -> JSResult<Self> {
        let mut app_svc = LxAppSvc {
            event_handlers: HashMap::new(),
            this: obj.clone(),
            update_manager: RefCell::new(None),
        };

        // Extract all functions from the object
        app_svc.assign_funcs(&obj)?;

        // Bind this AppSvc instance to the LxApp runtime context associated with ctx.
        set_app_svc_for_ctx(&ctx, app_svc.clone())?;

        Ok(app_svc)
    }

    #[js_method(gc_mark)]
    fn gc_mark_with<F>(&self, mut mark_fn: F)
    where
        F: FnMut(&JSValue),
    {
        for (_, func) in self.event_handlers.iter() {
            mark_fn(func.as_jsvalue());
        }
        mark_fn(self.this.as_jsvalue());
        if let Some(obj) = self.update_manager.borrow().as_ref() {
            mark_fn(obj.as_jsvalue());
        }
    }
}

impl LxAppSvc {
    fn assign_funcs(&mut self, obj: &JSObject) -> JSResult<()> {
        for key_value in obj.keys()? {
            // obj.keys() returns JSValue, not String
            if let Ok(key_string) = key_value.try_into::<String>() {
                if key_string.starts_with('_') {
                    continue;
                }

                if let Ok(func) = obj.get::<_, JSFunc>(key_string.as_str())
                    && let Some(evt) = AppServiceEvent::from_name(&key_string)
                {
                    // Only lifecycle events should be stored here; update events are internal
                    match evt {
                        AppServiceEvent::OnLaunch
                        | AppServiceEvent::OnShow
                        | AppServiceEvent::OnHide => {
                            self.event_handlers.insert(evt, func);
                        }
                        _ => {
                            // Ignore update events as JSUpdateManager handles them via callbacks
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn call_event(
        &self,
        ctx: &JSContext,
        event: AppServiceEvent,
        args: Option<String>,
    ) -> JSResult<()> {
        if let Some(func) = self.event_handlers.get(&event) {
            // Lifecycle events: schedule with High priority and wait for completion
            let args_obj = args
                .as_ref()
                .and_then(|json| JSObject::from_json_string(ctx, json).ok());
            rong::enqueue_js_invoke(
                ctx,
                func.clone(),
                Some(self.this.clone()),
                args_obj,
                rong::JsInvokePriority::High,
                None,
                true,
            )
            .await?;
            return Ok(());
        }
        Err(RongJSError::Error(format!(
            "No event handler for {}",
            event.as_str()
        )))
    }

    // create or return cached UpdateManager object
    pub(crate) fn get_or_create_update_manager(&self, ctx: &JSContext) -> JSResult<JSObject> {
        if let Some(obj) = self.update_manager.borrow().as_ref() {
            return Ok(obj.clone());
        }

        let class = Class::get::<JSUpdateManager>(ctx)?;
        let instance = class.instance(JSUpdateManager::new());
        // Cache instance so native update events can notify it later
        self.update_manager.borrow_mut().replace(instance.clone());
        Ok(instance)
    }

    // Notify JS update callbacks if a JSUpdateManager is registered; otherwise ignore
    pub(crate) async fn notify_update_ready(&self) {
        let ready_callback = {
            let update_manager = self.update_manager.borrow();
            update_manager.as_ref().and_then(|obj| {
                obj.borrow::<JSUpdateManager>()
                    .ok()
                    .and_then(|mgr| mgr.ready_callback())
            })
        };

        if let Some(cb) = ready_callback {
            let _ = cb.call_async::<_, ()>(None, ()).await;
        } else {
            // Not created yet (lx.getUpdateManager() hasn't been called)
            crate::error!("UpdateManager not initialized");
        }
    }

    pub(crate) async fn notify_update_failed(&self) {
        let failed_callback = {
            let update_manager = self.update_manager.borrow();
            update_manager.as_ref().and_then(|obj| {
                obj.borrow::<JSUpdateManager>()
                    .ok()
                    .and_then(|mgr| mgr.failed_callback())
            })
        };

        if let Some(cb) = failed_callback {
            let _ = cb.call_async::<_, ()>(None, ()).await;
        }
    }
}

// Register the global App & getApp function
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    ctx.register_class::<LxAppSvc>()?;
    ctx.register_class::<JSUpdateManager>()?;

    let app_js = Source::from_bytes(include_str!("scripts/App.js"));
    ctx.eval::<()>(app_js)?;

    Ok(())
}
