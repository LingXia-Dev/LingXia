//! Shared plumbing for "task" JS objects — long-running operations exposed
//! as PromiseLike + AsyncIterable (host-app update, compressVideo). The
//! object doubles as its own async iterator and forwards
//! then/catch/finally/wait to a final-result Promise.

use rong::function::Optional;
use rong::{JSContext, JSFunc, JSObject, JSResult, JSSymbol, JSValue, Promise};

pub(crate) fn install_async_iterator(ctx: &JSContext, iterator: &JSObject) -> JSResult<()> {
    let symbol = ctx
        .global()
        .get::<_, JSObject>("Symbol")?
        .get::<_, JSSymbol>("asyncIterator")?;
    iterator.set(
        symbol,
        JSFunc::new(ctx, move |this: rong::function::This<JSObject>| {
            (*this).clone()
        })?,
    )?;
    Ok(())
}

pub(crate) fn install_promise_methods(
    ctx: &JSContext,
    iterator: &JSObject,
    promise: Promise,
) -> JSResult<()> {
    let then_promise = promise.clone();
    let then_ctx = ctx.clone();
    iterator.set(
        "then",
        JSFunc::new(
            ctx,
            move |on_fulfilled: Optional<JSValue>,
                  on_rejected: Optional<JSValue>|
                  -> JSResult<JSObject> {
                let then = then_promise.then()?;
                then.call(
                    Some(then_promise.clone().into_object()),
                    (
                        on_fulfilled
                            .0
                            .unwrap_or_else(|| JSValue::undefined(&then_ctx)),
                        on_rejected
                            .0
                            .unwrap_or_else(|| JSValue::undefined(&then_ctx)),
                    ),
                )
            },
        )?,
    )?;

    let catch_promise = promise.clone();
    let catch_ctx = ctx.clone();
    iterator.set(
        "catch",
        JSFunc::new(
            ctx,
            move |on_rejected: Optional<JSValue>| -> JSResult<JSObject> {
                let catch_fn = catch_promise.catch()?;
                catch_fn.call(
                    Some(catch_promise.clone().into_object()),
                    (on_rejected
                        .0
                        .unwrap_or_else(|| JSValue::undefined(&catch_ctx)),),
                )
            },
        )?,
    )?;

    let finally_promise = promise.clone();
    let finally_ctx = ctx.clone();
    iterator.set(
        "finally",
        JSFunc::new(
            ctx,
            move |on_finally: Optional<JSValue>| -> JSResult<JSObject> {
                let finally_fn = finally_promise.get::<_, JSFunc>("finally")?;
                finally_fn.call(
                    Some(finally_promise.clone().into_object()),
                    (on_finally
                        .0
                        .unwrap_or_else(|| JSValue::undefined(&finally_ctx)),),
                )
            },
        )?,
    )?;

    let wait_promise = promise;
    iterator.set("wait", JSFunc::new(ctx, move || wait_promise.clone())?)?;
    Ok(())
}
