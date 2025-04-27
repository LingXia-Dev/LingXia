use rong::{
    Class, JSContext, JSFunc, JSObject, JSResult, JSValue, RongJSError, Source, function::Optional,
    js_class, js_export, js_method,
};
use std::collections::HashMap;

/// Enum to represent the type of mini app component
#[derive(Debug, Clone)]
enum InstanceType {
    App,
    Page,
}

#[js_export]
pub struct AppPage {
    functions: HashMap<String, JSFunc>,
    kind: InstanceType,
}

#[js_class]
impl AppPage {
    #[js_method(constructor)]
    fn _new() {}

    #[js_method(rename = "_setData")]
    pub fn set_data(&self, data: String, callback: Optional<JSFunc>) -> JSResult<()> {
        // Only Page type can use setData
        if let InstanceType::Page = self.kind {
            println!("setData JSON: {}", data);

            // Call the callback if provided
            if let Some(cb) = callback.0 {
                let _ = cb.call::<_, ()>(None, ());
            }

            Ok(())
        } else {
            Err(RongJSError::TypeError(
                "setData can only be called on Page".to_string(),
            ))
        }
    }

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

fn extract_functions(obj: &JSObject, mini_app: &mut AppPage) -> JSResult<()> {
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

fn app(ctx: JSContext, obj: JSObject) -> JSResult<JSObject> {
    // Get the MiniApp class
    let mini_app_class = Class::get::<AppPage>(&ctx)?;

    // Create a new MiniApp instance
    let mut mini_app = AppPage {
        functions: HashMap::new(),
        kind: InstanceType::App,
    };

    // Extract all functions from the object
    extract_functions(&obj, &mut mini_app)?;

    // Create a new JSObject using the instance method
    let app = mini_app_class.instance(mini_app);

    Ok(app)
}

fn page(ctx: JSContext, obj: JSObject) -> JSResult<JSObject> {
    // Get the MiniApp class
    let mini_app_class = Class::get::<AppPage>(&ctx)?;

    let mut mini_app = AppPage {
        functions: HashMap::new(),
        kind: InstanceType::Page,
    };

    // Extract all functions from the object
    extract_functions(&obj, &mut mini_app)?;

    // Create a new JSObject using the instance method
    let page = mini_app_class.instance(mini_app);

    Ok(page)
}

const PAGE_JS: &str = include_str!("../scripts/Page.js");

pub fn init(ctx: &JSContext) -> JSResult<()> {
    // Register the MiniApp class
    ctx.register_class::<AppPage>()?;

    // Register the global App function
    let app_func = JSFunc::new(ctx, app)?.name("App")?;
    ctx.global().set("App", app_func)?;

    // Register the global Page function
    let page_func = JSFunc::new(ctx, page)?.name("_Page")?;
    ctx.global().set("_Page", page_func)?;

    let page = Source::from_bytes(PAGE_JS);
    ctx.eval::<()>(page)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rong_test::*;

    // Helper function to trigger a function by name (moved from MiniApp)
    fn trigger(mini_app: &AppPage, this: JSObject, name: &str) -> JSResult<()> {
        if let Some(func) = mini_app.functions.get(name) {
            let _ = func.call::<_, ()>(Some(this), ());
            Ok(())
        } else {
            Err(RongJSError::Error(format!("Function '{}' not found", name)))
        }
    }

    #[test]
    fn test_page() {
        async_run!(|ctx: JSContext| async move {
            rong_modules::init(&ctx)?;
            init(&ctx)?;

            // Create a Page and get the returned object
            let page_obj = ctx.eval::<JSObject>(Source::from_bytes(
                r#"
                    let triggered = false;
                    let callbackCalled = false;

                    const page = Page({
                        onLoad: function() {
                            triggered = true;
                            console.log("onLoad called");

                            this.setData({count: 1}, function() {
                                callbackCalled = true;
                                console.log("setData callback called");
                            });
                        },
                        data: { "a": 1},
                    });
                    page
                "#,
            ))?;

            // Access the MiniApp instance and trigger the onLoad function using our test helper
            {
                let mini_app = page_obj.borrow::<AppPage>()?;
                trigger(&mini_app, page_obj.clone(), "onLoad")?;
            }

            // Check if the function was triggered
            let triggered = ctx.eval::<bool>(Source::from_bytes("triggered"))?;
            assert!(triggered, "onLoad function should have been triggered");

            // Wait a short time to for callback execution
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;

            // Check if the callback was called
            let callback_called = ctx.eval::<bool>(Source::from_bytes("callbackCalled"))?;
            assert!(callback_called, "setData callback should have been called");

            Ok(())
        });
    }
}
