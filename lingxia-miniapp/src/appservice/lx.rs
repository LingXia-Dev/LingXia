use std::collections::HashMap;
use std::rc::Rc;

use rong::{JSContext, JSFunc, JSObject, JSResult, JSValue};

mod device;
mod navigator;

/// Rust implemented JS function and it return result directly and quickly
pub(crate) struct FastJSApi {
    functions: HashMap<String, JSFunc>,
}

impl FastJSApi {
    fn register_fast_api(&mut self, name: String, api: JSFunc) {
        self.functions.insert(name, api);
    }

    fn new() -> Self {
        Self {
            functions: HashMap::new(),
        }
    }

    pub fn has_fast_api(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }

    pub async fn call_fast_api(
        &self,
        ctx: &JSContext,
        func_name: &str,
        args: Option<&str>,
        this: JSObject,
    ) -> JSResult<JSValue> {
        let func = self.functions.get(func_name).unwrap();
        let args_obj = args.and_then(|json| JSObject::from_json_string(ctx, json).ok());
        match args_obj {
            Some(obj) => {
                func.call_async::<_, JSValue>(Some(this.clone()), (obj,))
                    .await
            }
            None => func.call_async::<_, JSValue>(Some(this), ()).await,
        }
    }
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let lx = JSObject::new(ctx);
    ctx.global().set("lx", lx.clone())?;

    let navigator_miniapp = JSFunc::new(ctx, navigator::navigator_to_miniapp)?;
    lx.set("navigateToMiniProgram", navigator_miniapp)?;

    let device_info = JSFunc::new(ctx, device::derive_info)?;
    lx.set("getDeviceInfo", device_info.clone())?;

    let mut api = FastJSApi::new();
    api.register_fast_api("lx.getDeviceInfo".to_string(), device_info);
    let api_rc = Rc::new(api);

    ctx.set_user_data(api_rc);

    Ok(())
}
