use rong::{
    Class, JSContext, JSFunc, JSObject, JSResult, JSValue, RongJSError, Source, js_class,
    js_export, js_method,
};
use std::collections::HashMap;

#[js_export]
pub(crate) struct MiniAppSvc {
    functions: HashMap<String, JSFunc>,
    this: JSObject,
}

#[js_class]
impl MiniAppSvc {
    #[js_method(constructor)]
    fn _new(ctx: JSContext, obj: JSObject) -> JSResult<JSObject> {
        // Get the MiniApp class
        let miniapp_class = Class::get::<MiniAppSvc>(&ctx)?;

        // Create a new MiniApp instance
        let mut app_svc = MiniAppSvc {
            functions: HashMap::new(),
            this: obj.clone(),
        };

        // Extract all functions from the object
        app_svc.assign_funcs(&obj)?;

        // Create a new JSObject using the instance method
        let app = miniapp_class.instance(app_svc);
        ctx.global().set("_MINIAPP_OBJ_", app.clone())?;

        Ok(app)
    }

    #[js_method(gc_mark)]
    fn gc_mark_with<F>(&self, mut mark_fn: F)
    where
        F: FnMut(&JSValue),
    {
        for (_, func) in self.functions.iter() {
            mark_fn(func.as_jsvalue());
        }
        mark_fn(self.this.as_jsvalue());
    }
}

impl MiniAppSvc {
    fn assign_funcs(&mut self, obj: &JSObject) -> JSResult<()> {
        for key_value in obj.keys()? {
            // obj.keys() returns JSValue, not String
            if let Ok(key_string) = key_value.try_into::<String>() {
                if let Ok(func) = obj.get::<_, JSFunc>(key_string.as_str()) {
                    self.functions.insert(key_string, func);
                }
            }
        }
        Ok(())
    }

    pub async fn call(
        &self,
        ctx: &JSContext,
        func_name: &str,
        args: Option<String>,
    ) -> JSResult<()> {
        if let Some(func) = self.functions.get(func_name) {
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
        Err(RongJSError::Error(format!("No service: {}", func_name)))
    }
}

// Register the global App & getApp function
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    ctx.register_class::<MiniAppSvc>()?;

    let app_js = Source::from_bytes(include_str!("scripts/App.js"));
    ctx.eval::<()>(app_js)?;

    Ok(())
}
