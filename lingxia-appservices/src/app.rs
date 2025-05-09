use rong::{Class, JSContext, JSFunc, JSObject, JSResult, JSValue, js_class, js_export, js_method};
use std::collections::HashMap;

#[js_export]
pub struct AppSvc {
    functions: HashMap<String, JSFunc>,
}

#[js_class]
impl AppSvc {
    #[js_method(constructor)]
    fn _new() {}

    #[js_method(gc_mark)]
    pub fn gc_mark_with<F>(&self, mut mark_fn: F)
    where
        F: FnMut(&JSValue),
    {
        for (_, func) in self.functions.iter() {
            mark_fn(func.as_jsvalue())
        }
    }
}

fn extract_functions(obj: &JSObject, mini_app: &mut AppSvc) -> JSResult<()> {
    for key_value in obj.keys()? {
        // obj.keys() returns JSValue, not String
        if let Ok(key_string) = key_value.try_into::<String>() {
            if let Ok(func) = obj.get::<_, JSFunc>(key_string.as_str()) {
                mini_app.functions.insert(key_string, func);
            }
        }
    }
    Ok(())
}

pub(crate) fn app_func(ctx: JSContext, obj: JSObject) -> JSResult<JSObject> {
    // Get the MiniApp class
    let mini_app_class = Class::get::<AppSvc>(&ctx)?;

    // Create a new MiniApp instance
    let mut mini_app = AppSvc {
        functions: HashMap::new(),
    };

    // Extract all functions from the object
    extract_functions(&obj, &mut mini_app)?;

    // Create a new JSObject using the instance method
    let app = mini_app_class.instance(mini_app);

    Ok(app)
}
