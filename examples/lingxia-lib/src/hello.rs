//! Hello Extension - demonstrates native Rust extension API.

use lingxia::LxLogicExtension;
use rong::function::Optional;
use rong::{JSContext, JSFunc, JSObject, JSResult};

pub struct HelloExtension;

impl LxLogicExtension for HelloExtension {
    fn init(&self, ctx: &JSContext) -> JSResult<()> {
        let lx = ctx.global().get::<_, JSObject>("lx")?;
        let hello = JSObject::new(ctx);

        hello.set("sayHello", JSFunc::new(ctx, say_hello)?)?;

        lx.set("hello", hello)?;
        log::info!("[HelloExtension] ✅ lx.hello.* registered");
        Ok(())
    }
}

fn say_hello(_: JSContext, name: Optional<String>) -> JSResult<String> {
    let name = name.0.clone().unwrap_or_else(|| "World".into());
    log::info!("[HelloExtension] sayHello() called with name: {}", name);
    Ok(format!("Hello, {}!", name))
}
