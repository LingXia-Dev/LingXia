use rong::{JSContext, JSObject, JSResult};

/// `parent[name]`, created on first use, so every module that registers into
/// a namespace shares one object whatever order they initialise in.
pub(crate) fn child(ctx: &JSContext, parent: &JSObject, name: &str) -> JSResult<JSObject> {
    match parent.get::<_, JSObject>(name) {
        Ok(namespace) => Ok(namespace),
        Err(_) => {
            let namespace = JSObject::new(ctx);
            parent.set(name, namespace.clone())?;
            Ok(namespace)
        }
    }
}

pub(crate) fn network(ctx: &JSContext) -> JSResult<JSObject> {
    child(ctx, &ctx.global().get::<_, JSObject>("lx")?, "network")
}

pub(crate) fn wifi(ctx: &JSContext) -> JSResult<JSObject> {
    child(ctx, &network(ctx)?, "wifi")
}

pub(crate) fn device(ctx: &JSContext) -> JSResult<JSObject> {
    child(ctx, &ctx.global().get::<_, JSObject>("lx")?, "device")
}

pub(crate) fn orientation(ctx: &JSContext) -> JSResult<JSObject> {
    child(ctx, &device(ctx)?, "orientation")
}

pub(crate) fn app(ctx: &JSContext) -> JSResult<JSObject> {
    child(ctx, &ctx.global().get::<_, JSObject>("lx")?, "app")
}
