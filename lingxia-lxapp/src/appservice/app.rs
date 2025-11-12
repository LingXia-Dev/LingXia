use crate::event::AppServiceEvent;
use rong::{
    Class, JSContext, JSFunc, JSObject, JSResult, JSValue, RongJSError, Source, js_class,
    js_export, js_method,
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

        ctx.set_user_data(app_svc.clone());

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

                if let Ok(func) = obj.get::<_, JSFunc>(key_string.as_str()) {
                    if let Some(evt) = AppServiceEvent::from_name(&key_string) {
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
            let args = args.and_then(|json| JSObject::from_json_string(ctx, json.as_ref()).ok());
            match args {
                Some(obj) => {
                    func.call_async::<_, ()>(Some(self.this.clone()), (obj,))
                        .await?
                }
                None => {
                    func.call_async::<_, ()>(Some(self.this.clone()), ())
                        .await?
                }
            };
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
