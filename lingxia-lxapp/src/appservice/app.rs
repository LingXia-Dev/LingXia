use crate::appservice::set_app_svc_for_ctx;
use crate::event::AppServiceEvent;
use rong::{
    JSContext, JSFunc, JSObject, JSResult, JSValue, RongJSError, Source, js_class, js_export,
    js_method,
};
use std::collections::HashMap;

#[js_export]
pub(crate) struct LxAppSvc {
    event_handlers: HashMap<AppServiceEvent, JSFunc>,
    this: JSObject,
}

#[js_class]
impl LxAppSvc {
    #[js_method(constructor)]
    fn _new(ctx: JSContext, obj: JSObject) -> JSResult<Self> {
        let mut app_svc = LxAppSvc {
            event_handlers: HashMap::new(),
            this: obj.clone(),
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
                        | AppServiceEvent::OnHide
                        | AppServiceEvent::OnUserCaptureScreen => {
                            self.event_handlers.insert(evt, func);
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
}

// Register the global App & getApp function
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    ctx.register_class::<LxAppSvc>()?;

    let app_js = Source::from_bytes(include_str!("scripts/App.js"));
    ctx.eval::<()>(app_js)?;

    Ok(())
}
