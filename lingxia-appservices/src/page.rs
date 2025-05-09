use rong::{
    Class, JSContext, JSFunc, JSObject, JSResult, JSValue, function::Optional, js_class, js_export,
    js_method,
};
use std::collections::HashMap;

#[js_export]
pub struct PageSvc {
    functions: HashMap<String, JSFunc>,
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
            mark_fn(func.as_jsvalue())
        }
    }
}

fn extract_functions(obj: &JSObject, page: &mut PageSvc) -> JSResult<()> {
    for key_value in obj.keys()? {
        // obj.keys() returns JSValue, not String
        if let Ok(key_string) = key_value.try_into::<String>() {
            if let Ok(func) = obj.get::<_, JSFunc>(key_string.as_str()) {
                page.functions.insert(key_string, func);
            }
        }
    }
    Ok(())
}

pub(crate) fn page_func(ctx: JSContext, obj: JSObject) -> JSResult<JSObject> {
    // Get the MiniApp class
    let mini_app_class = Class::get::<PageSvc>(&ctx)?;

    let mut mini_app = PageSvc {
        functions: HashMap::new(),
    };

    // Extract all functions from the object
    extract_functions(&obj, &mut mini_app)?;

    // Create a new JSObject using the instance method
    let page = mini_app_class.instance(mini_app);

    Ok(page)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init;
    use rong_test::*;

    // Helper function to trigger a function by name (moved from MiniApp)
    fn trigger(mini_app: &PageSvc, this: JSObject, name: &str) -> JSResult<()> {
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
                let mini_app = page_obj.borrow::<PageSvc>()?;
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
