use super::set_app_svc_for_ctx;
use crate::lifecycle::AppServiceEvent;
use rong::{
    JSContext, JSFunc, JSObject, JSResult, JSValue, Source, error::HostError, function::Optional,
    js_class, js_method,
};
use std::collections::HashMap;

#[js_class(clone)]
pub(crate) struct LxAppSvc {
    event_handlers: HashMap<AppServiceEvent, JSFunc>,
    this: JSObject,
}

#[js_class]
impl LxAppSvc {
    #[js_method(constructor)]
    fn _new(ctx: JSContext, obj: JSObject, handler_names_json: Optional<String>) -> JSResult<Self> {
        let mut app_svc = LxAppSvc {
            event_handlers: HashMap::new(),
            this: obj.clone(),
        };

        app_svc.assign_funcs(&obj, handler_names_json.0.as_deref())?;

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
            mark_fn(func.as_js_value());
        }
        mark_fn(self.this.as_js_value());
    }
}

impl LxAppSvc {
    fn assign_funcs(&mut self, obj: &JSObject, handler_names_json: Option<&str>) -> JSResult<()> {
        let handler_names: Vec<String> = handler_names_json
            .map(serde_json::from_str)
            .transpose()
            .map_err(|e| HostError::new(rong::error::E_INTERNAL, e.to_string()))?
            .unwrap_or_default();

        for key_string in handler_names {
            if let Some(evt) = AppServiceEvent::from_name(&key_string)
                && let Ok(func) = obj.get::<_, JSFunc>(key_string.as_str())
            {
                self.event_handlers.insert(evt, func);
            }
        }

        // Metadata is a fast path, but still reflect over the realized object so
        // spread-derived and aliased lifecycle handlers remain compatible.
        for key_value in obj.keys()? {
            let Ok(key_string) = key_value.to_rust::<String>() else {
                continue;
            };
            if key_string.starts_with('_') {
                continue;
            }
            if let Some(evt) = AppServiceEvent::from_name(&key_string)
                && let Ok(func) = obj.get::<_, JSFunc>(key_string.as_str())
            {
                self.event_handlers.insert(evt, func);
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
            // The caller runs this outside the worker pump. Await the handler so
            // its task retains the JSContext through any returned Promise. Call
            // directly because Rong's runtime-wide invoke queue can outlive this
            // app context when an LxApp is restarted.
            let args_obj = args
                .as_ref()
                .and_then(|json| JSObject::from_json_string(ctx, json).ok());
            return match args_obj {
                Some(obj) => {
                    func.call_async::<_, ()>(Some(self.this.clone()), (obj,))
                        .await
                }
                None => func.call_async::<_, ()>(Some(self.this.clone()), ()).await,
            };
        }
        // App lifecycle handlers are optional, matching PageInstance handlers.
        Ok(())
    }
}

// Register the global App & getApp function
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    ctx.register_class::<LxAppSvc>()?;

    let app_js = Source::from_bytes(include_str!("scripts/App.js"));
    ctx.eval::<()>(app_js)?;

    Ok(())
}
