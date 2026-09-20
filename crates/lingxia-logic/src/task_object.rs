//! A task owns its result and controls; progress is a separate, single-consumer stream.

use std::cell::Cell;
use std::rc::Rc;

use rong::{JSContext, JSFunc, JSObject, JSResult, JSSymbol, Promise};

pub(crate) fn create_task(
    ctx: &JSContext,
    iterator: JSObject,
    result: Promise,
    controls: &[&str],
) -> JSResult<JSObject> {
    let task = JSObject::new(ctx);
    task.set("result", result)?;
    for name in controls {
        task.set(*name, iterator.get::<_, JSFunc>(*name)?)?;
        iterator.delete(*name)?;
    }
    let progress = JSObject::new(ctx);
    let symbol = ctx
        .global()
        .get::<_, JSObject>("Symbol")?
        .get::<_, JSSymbol>("asyncIterator")?;
    let claimed = Rc::new(Cell::new(false));
    progress.set(
        symbol,
        JSFunc::new(ctx, move || -> JSResult<JSObject> {
            if claimed.replace(true) {
                return Err(rong::HostError::new(
                    rong::error::E_INVALID_STATE,
                    "Task progress has one consumer; create a single observer and share its state",
                )
                .into());
            }
            Ok(iterator.clone())
        })?,
    )?;
    task.set("progress", progress)?;
    Ok(task)
}

pub(crate) fn validate_abort_signal(signal: Option<&JSObject>) -> JSResult<()> {
    if let Some(signal) = signal {
        let valid = signal.get::<_, bool>("aborted").is_ok()
            && signal.get::<_, JSFunc>("addEventListener").is_ok()
            && signal.get::<_, JSFunc>("removeEventListener").is_ok();
        if !valid {
            return Err(rong::HostError::new(
                rong::error::E_INVALID_ARG,
                "signal must be an AbortSignal",
            )
            .into());
        }
    }
    Ok(())
}

/// Attach external cancellation and release the listener on either terminal outcome.
pub(crate) fn bind_abort_signal(
    ctx: &JSContext,
    signal: Option<JSObject>,
    task: &JSObject,
) -> JSResult<()> {
    let Some(signal) = signal else {
        return Ok(());
    };
    let cancel: JSFunc = task.get("cancel")?;
    let cancel = JSFunc::new(ctx, move || -> JSResult<()> {
        let pending: Promise = cancel.call(None, ())?;
        // Abort events cannot await controls; callers observe task.result.
        let ignore = JSFunc::new(&pending.context(), || {})?;
        pending
            .catch()?
            .call::<_, JSObject>(Some(pending.into_object()), (ignore,))?;
        Ok(())
    })?;
    if signal.get::<_, bool>("aborted")? {
        cancel.call::<_, ()>(None, ())?;
        return Ok(());
    }
    let add: JSFunc = signal.get("addEventListener")?;
    let options = JSObject::new(ctx);
    options.set("once", true)?;
    add.call::<_, ()>(Some(signal.clone()), ("abort", cancel.clone(), options))?;
    let cleanup = JSFunc::new(ctx, move || -> JSResult<()> {
        let remove: JSFunc = signal.get("removeEventListener")?;
        remove.call(Some(signal.clone()), ("abort", cancel.clone()))
    })?;
    let result: Promise = task.get("result")?;
    result
        .then()?
        .call::<_, JSObject>(Some(result.into_object()), (cleanup.clone(), cleanup))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rong::{JSEngine, Source};

    #[test]
    fn separates_result_progress_and_controls_and_rejects_second_observer() {
        let runtime = <rong::RongJS as JSEngine>::runtime();
        let ctx = runtime.context();
        let iterator: JSObject = ctx.eval(Source::from_bytes(
            "({ next() { return Promise.resolve({done:true}); }, return() { return Promise.resolve({done:true}); }, cancel() { return Promise.resolve(); } })",
        )).unwrap();
        let result: Promise = ctx.eval(Source::from_bytes("Promise.resolve(42)")).unwrap();
        let task = create_task(&ctx, iterator, result, &["cancel"]).unwrap();
        ctx.global().set("task", task).unwrap();
        let valid: bool = ctx.eval(Source::from_bytes(
            "task.result instanceof Promise && !('then' in task) && !('next' in task) && typeof task.cancel === 'function' && !(Symbol.asyncIterator in task)",
        )).unwrap();
        assert!(valid);
        let valid: bool = ctx.eval(Source::from_bytes(
            "(() => { const iterator = task.progress[Symbol.asyncIterator](); if ('cancel' in iterator || typeof iterator.next !== 'function') return false; try { task.progress[Symbol.asyncIterator](); return false; } catch (e) { return e.code === 'E_INVALID_STATE'; } })()",
        )).unwrap();
        assert!(valid);
    }

    #[test]
    fn already_aborted_signal_cancels_without_retaining_listener() {
        let runtime = <rong::RongJS as JSEngine>::runtime();
        let ctx = runtime.context();
        let task: JSObject = ctx.eval(Source::from_bytes(
            "({ calls: 0, result: Promise.resolve(), cancel() { globalThis.cancelCalls = (globalThis.cancelCalls || 0) + 1; return Promise.resolve(); } })",
        )).unwrap();
        let signal: JSObject = ctx
            .eval(Source::from_bytes(
                "({ aborted: true, addEventListener() { throw new Error('must not attach'); } })",
            ))
            .unwrap();
        bind_abort_signal(&ctx, Some(signal), &task).unwrap();
        assert_eq!(ctx.global().get::<_, i32>("cancelCalls").unwrap(), 1);
    }
}
