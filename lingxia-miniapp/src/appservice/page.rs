use rong::{
    Class, JSContext, JSFunc, JSObject, JSResult, JSValue, Source, function::Optional, js_class,
    js_export, js_method,
};
use std::collections::HashMap;

#[js_export]
pub(crate) struct PageSvc {
    functions: HashMap<String, JSFunc>,
    this: JSObject,
}

#[js_class]
impl PageSvc {
    #[js_method(constructor)]
    fn _new() {}

    #[js_method(rename = "_setData")]
    pub fn set_data(&self, data: String, callback: Optional<JSFunc>) -> JSResult<()> {
        println!("setData JSON: {}", data);

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

    pub(crate) fn call(&self, ctx: &JSContext, func_name: &str, args: Option<String>) {
        if let Some(func) = self.functions.get(func_name) {
            let args = args.and_then(|json| JSObject::from_json_string(ctx, &json).ok());
            let _ = match args {
                Some(obj) => func.call::<_, u32>(Some(self.this.clone()), (obj,)),
                None => func.call::<_, u32>(Some(self.this.clone()), ()),
            };
        }
    }
}

fn page_func(ctx: JSContext, obj: JSObject) -> JSResult<JSObject> {
    // Get the MiniApp class
    let page_class = Class::get::<PageSvc>(&ctx)?;

    let mut page_svc = PageSvc {
        functions: HashMap::new(),
        this: obj.clone(),
    };

    // Extract all functions from the object
    page_svc.assign_funcs(&obj)?;

    // Create a new JSObject using the instance method
    let page = page_class.instance(page_svc);

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
