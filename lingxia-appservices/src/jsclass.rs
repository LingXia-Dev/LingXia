use rong::{
    Class, JSContext, JSFunc, JSObject, JSResult, RongJSError, function::Optional, js_class,
    js_export, js_method,
};
use std::collections::HashMap;

/// Enum to represent the type of mini app component
#[derive(Debug, Clone)]
enum InstanceType {
    App,
    Page,
}

#[js_export]
pub struct MiniApp {
    functions: HashMap<String, JSFunc>,
    kind: InstanceType,
}

#[js_class]
impl MiniApp {
    #[js_method(constructor)]
    fn _new() {}

    #[js_method(rename = "setData")]
    pub fn set_data(&self, data: JSObject, callback: Optional<JSFunc>) -> JSResult<()> {
        // Only Page type can use setData
        if let InstanceType::Page = self.kind {
            let json_string = data.json_stringify()?;

            println!("setData JSON: {}", json_string);

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
}

fn extract_functions(obj: &JSObject, mini_app: &mut MiniApp) -> JSResult<()> {
    // Use entries_as to directly get typed key-value pairs
    if let Ok(entries) = obj.entries_as::<String, JSFunc>() {
        for (key, func) in entries {
            mini_app.functions.insert(key, func);
        }
    }
    Ok(())
}

fn app(ctx: JSContext, obj: JSObject) -> JSResult<JSObject> {
    // Get the MiniApp class
    let mini_app_class = Class::get::<MiniApp>(&ctx)?;

    // Create a new MiniApp instance
    let mut mini_app = MiniApp {
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
    let mini_app_class = Class::get::<MiniApp>(&ctx)?;

    let mut mini_app = MiniApp {
        functions: HashMap::new(),
        kind: InstanceType::Page,
    };

    // Extract all functions from the object
    extract_functions(&obj, &mut mini_app)?;

    // Create a new JSObject using the instance method
    let page = mini_app_class.instance(mini_app);

    Ok(page)
}

pub fn init(ctx: &JSContext) -> JSResult<()> {
    // Register the MiniApp class
    ctx.register_class::<MiniApp>()?;

    // Register the global App function
    let app_func = JSFunc::new(ctx, app)?.name("App")?;
    ctx.global().set("App", app_func)?;

    // Register the global Page function
    let page_func = JSFunc::new(ctx, page)?.name("Page")?;
    ctx.global().set("Page", page_func)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rong_test::*;

    // Helper function to trigger a function by name (moved from MiniApp)
    fn trigger(mini_app: &MiniApp, this: JSObject, name: &str) -> JSResult<()> {
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
                    });
                    page
                "#,
            ))?;

            // Access the MiniApp instance and trigger the onLoad function using our test helper
            let mini_app = page_obj.borrow::<MiniApp>()?;
            trigger(&mini_app, page_obj.clone(), "onLoad")?;

            // Check if the function was triggered
            let triggered = ctx.eval::<bool>(Source::from_bytes("triggered"))?;
            assert!(triggered, "onLoad function should have been triggered");

            // Check if the callback was called
            let callback_called = ctx.eval::<bool>(Source::from_bytes("callbackCalled"))?;
            assert!(callback_called, "setData callback should have been called");

            Ok(())
        });
    }
}
