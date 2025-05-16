use super::bridge::Bridge;
use crate::page::Page;
use rong::{
    Class, JSContext, JSFunc, JSObject, JSResult, JSValue, RongJSError, Source, function::Optional,
    js_class, js_export, js_method,
};
use std::collections::HashMap;

// Page is Send able, but JSFunc is not, we can not let Page hold PageSvc.
#[js_export]
pub(crate) struct PageSvc {
    functions: HashMap<String, JSFunc>,
    this: JSObject,
    pub bridge: Bridge,
}

#[js_class]
impl PageSvc {
    #[js_method(constructor)]
    fn _new() {}

    #[js_method(rename = "_setData")]
    async fn set_data(&self, data: String, callback: Optional<JSFunc>) -> JSResult<()> {
        self.bridge
            .set_data(&data)
            .await
            .map_err(|e| RongJSError::Error(e.to_string()))?;

        // Call the callback if provided
        if let Some(cb) = callback.0 {
            let _ = cb.call::<_, ()>(None, ());
        }

        Ok(())
    }

    #[js_method(gc_mark)]
    pub fn gc_mark_with<F>(&self, mut mark_fn: F)
    where
        F: FnMut(&JSValue),
    {
        for (_, func) in self.functions.iter() {
            mark_fn(func.as_jsvalue());
        }
        mark_fn(self.this.as_jsvalue());
    }
}

impl PageSvc {
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

    pub async fn call(&self, ctx: &JSContext, func_name: &str, args: Option<&str>) -> JSResult<()> {
        if let Some(func) = self.functions.get(func_name) {
            let args = args.and_then(|json| JSObject::from_json_string(ctx, json).ok());
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
        }
        Err(RongJSError::Error(format!("No service: {}", func_name)))
    }

    pub(crate) fn bind(&mut self, page: Page) {
        let func_names: Vec<String> = self.functions.keys().cloned().collect();
        page.register_svc(func_names);
        self.bridge.set_page(page);
    }
}

fn page_func(ctx: JSContext, obj: JSObject) -> JSResult<JSObject> {
    // Get the MiniApp class
    let page_class = Class::get::<PageSvc>(&ctx)?;

    let mut page_svc = PageSvc {
        functions: HashMap::new(),
        this: obj.clone(),
        bridge: Bridge::new(),
    };

    // Extract all functions from the object
    page_svc.assign_funcs(&obj)?;

    // Create a new JSObject using the instance method
    let page = page_class.instance(page_svc);

    // assign this object
    page.borrow_mut::<PageSvc>().unwrap().this = page.clone();

    Ok(page)
}

// Register the global App & getApp function
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    ctx.register_class::<PageSvc>()?;

    let page_func = JSFunc::new(ctx, page_func)?.name("_Page")?;
    ctx.global().set("_Page", page_func)?;

    let page = Source::from_bytes(include_str!("scripts/Page.js"));
    ctx.eval::<()>(page)?;

    Ok(())
}
