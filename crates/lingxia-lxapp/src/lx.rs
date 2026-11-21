use rong::{JSContext, JSFunc, JSObject, JSResult};

#[cfg(feature = "js-appservice")]
pub mod extension;
#[cfg(not(feature = "js-appservice"))]
pub(crate) mod extension;

#[cfg(feature = "js-appservice")]
pub use extension::{LxLogicExtension, register_logic_extension};
// Automation plumbing for the test runtime, never an app API.
#[cfg(all(feature = "js-appservice", feature = "automation"))]
#[doc(hidden)]
pub use extension::{RongMemberWrapper, register_rong_member_wrapper};

/// Register the global `lx` object in the JavaScript context.
/// This function must be called before using any other `lx` APIs.
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let lx = JSObject::new(ctx);
    ctx.global().set("lx", lx)?;
    Ok(())
}

/// Register a JS function to the lx object.
/// `lx::init` must be called before this function.
pub fn register_js_api(ctx: &JSContext, name: &str, func: JSFunc) -> JSResult<()> {
    let lx = ctx.global().get::<_, JSObject>("lx")?;
    lx.set(name, func)?;
    Ok(())
}

/// Runs a Logic subscription with its `{ signal }` options: aborting the
/// signal unsubscribes, and an already-aborted signal subscribes nothing.
/// `subscribe` returns the unsubscribe function.
pub fn subscribe(
    ctx: &JSContext,
    options: Option<rong::JSValue>,
    subscribe: impl FnOnce() -> JSResult<JSFunc>,
) -> JSResult<JSFunc> {
    let invalid = || {
        rong::RongJSError::from(rong::HostError::new(
            rong::error::E_INVALID_ARG,
            "a subscription takes { signal } as its options",
        ))
    };
    let signal = match options.filter(|value| !value.is_undefined()) {
        None => None,
        Some(options) => {
            let options = options.to_rust::<JSObject>().map_err(|_| invalid())?;
            let signal = options.get::<_, rong::JSValue>("signal")?;
            if signal.is_undefined() {
                None
            } else {
                Some(signal.to_rust::<JSObject>().map_err(|_| invalid())?)
            }
        }
    };
    let Some(signal) = signal else {
        return subscribe();
    };
    let add = signal
        .get::<_, JSFunc>("addEventListener")
        .map_err(|_| invalid())?;
    let remove = signal
        .get::<_, JSFunc>("removeEventListener")
        .map_err(|_| invalid())?;
    if signal.get::<_, bool>("aborted").unwrap_or(false) {
        return JSFunc::new(ctx, || {});
    }
    let off = subscribe()?;
    let once = JSObject::new(ctx);
    once.set("once", true)?;
    add.call::<_, ()>(Some(signal.clone()), ("abort", off.clone(), once))?;
    JSFunc::new(ctx, move || -> JSResult<()> {
        remove.call::<_, ()>(Some(signal.clone()), ("abort", off.clone()))?;
        off.call::<_, ()>(None, ())
    })
}
