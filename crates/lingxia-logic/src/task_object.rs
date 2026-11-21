//! A task is its result plus an async iterable of progress whose return value is that result,
//! so `return yield* task` hands a page stream both. The platform iterators keep their own
//! event shapes; each task maps them to `TransferProgress` here.

use std::cell::Cell;
use std::rc::Rc;

use rong::{JSContext, JSFunc, JSObject, JSResult, JSSymbol, JSValue, Promise};

/// How far a task has got: `fraction` only when `total` is known.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TransferProgress {
    pub loaded: f64,
    pub total: Option<f64>,
}

impl TransferProgress {
    pub(crate) fn bytes(loaded: u64, total: Option<u64>) -> Self {
        Self {
            loaded: loaded as f64,
            total: total.filter(|total| *total > 0).map(|total| total as f64),
        }
    }

    fn to_js(self, ctx: &JSContext) -> JSResult<JSObject> {
        let object = JSObject::new(ctx);
        object.set("loaded", self.loaded)?;
        if let Some(total) = self.total {
            object.set("total", total)?;
            object.set("fraction", (self.loaded / total).clamp(0.0, 1.0))?;
        }
        Ok(object)
    }
}

/// Maps one platform event to progress; `None` skips it (state changes, terminal events).
pub(crate) type ProgressMap = Rc<dyn Fn(&JSObject) -> JSResult<Option<TransferProgress>>>;

/// Reads the byte counters of a `{ kind, <loaded>, totalBytes }` platform event.
pub(crate) fn byte_events(loaded: &'static str) -> ProgressMap {
    Rc::new(move |event| {
        let kind = event.get::<_, String>("kind").unwrap_or_default();
        if kind != "progress" && kind != "completed" {
            return Ok(None);
        }
        let Some(done) = event.get::<_, Option<f64>>(loaded).ok().flatten() else {
            return Ok(None);
        };
        let total = event.get::<_, Option<f64>>("totalBytes").ok().flatten();
        Ok(Some(TransferProgress::bytes(
            done as u64,
            total.map(|total| total as u64),
        )))
    })
}

pub(crate) fn create_task(
    ctx: &JSContext,
    iterator: JSObject,
    result: Promise,
    controls: &[&str],
    map: ProgressMap,
) -> JSResult<JSObject> {
    let task = JSObject::new(ctx);
    task.set("result", result.clone())?;
    for name in controls {
        task.set(*name, iterator.get::<_, JSFunc>(*name)?)?;
        iterator.delete(*name)?;
    }
    let cancel = controls
        .contains(&"cancel")
        .then(|| task.get::<_, JSFunc>("cancel"))
        .transpose()?;
    let symbol = ctx
        .global()
        .get::<_, JSObject>("Symbol")?
        .get::<_, JSSymbol>("asyncIterator")?;
    let claimed = Rc::new(Cell::new(false));
    task.set(
        symbol,
        JSFunc::new(ctx, move |ctx: JSContext| -> JSResult<JSObject> {
            if claimed.replace(true) {
                return Err(rong::HostError::new(
                    rong::error::E_INVALID_STATE,
                    "A task has one progress consumer; create a single observer and share its state",
                )
                .into());
            }
            progress_iterator(&ctx, &iterator, &result, cancel.clone(), map.clone())
        })?,
    )?;
    Ok(task)
}

fn step(
    ctx: &JSContext,
    done: bool,
    value: impl FnOnce(&JSObject) -> JSResult<()>,
) -> JSResult<JSObject> {
    let step = JSObject::new(ctx);
    step.set("done", done)?;
    value(&step)?;
    Ok(step)
}

/// Iterates mapped progress and returns the task's result; leaving early cancels the task.
fn progress_iterator(
    ctx: &JSContext,
    source: &JSObject,
    result: &Promise,
    cancel: Option<JSFunc>,
    map: ProgressMap,
) -> JSResult<JSObject> {
    let iterator = JSObject::new(ctx);
    let next: JSFunc = source.get("next")?;
    let (next_source, next_result) = (source.clone(), result.clone());
    iterator.set(
        "next",
        JSFunc::new(ctx, move |ctx: JSContext| {
            let (source, next, result, map) = (
                next_source.clone(),
                next.clone(),
                next_result.clone(),
                map.clone(),
            );
            async move {
                loop {
                    let raw = match next
                        .call_async::<_, JSObject>(Some(source.clone()), ())
                        .await
                    {
                        Ok(raw) => raw,
                        // The result carries the task's own error; the iterator's is a symptom.
                        Err(error) => {
                            result.clone().into_future::<JSValue>().await?;
                            return Err(error);
                        }
                    };
                    if raw.get::<_, bool>("done").unwrap_or(false) {
                        let value = result.clone().into_future::<JSValue>().await?;
                        return step(&ctx, true, |step| step.set("value", value).map(|_| ()));
                    }
                    let Some(event) = raw.get::<_, Option<JSObject>>("value").ok().flatten() else {
                        continue;
                    };
                    if let Some(progress) = map(&event)? {
                        let progress = progress.to_js(&ctx)?;
                        return step(&ctx, false, |step| step.set("value", progress).map(|_| ()));
                    }
                }
            }
        })?,
    )?;
    let close: Option<JSFunc> = source.get("return").ok();
    let return_source = source.clone();
    iterator.set(
        "return",
        JSFunc::new(ctx, move |ctx: JSContext| {
            let (source, close, cancel) = (return_source.clone(), close.clone(), cancel.clone());
            async move {
                if let Some(cancel) = cancel {
                    let pending: Promise = cancel.call(None, ())?;
                    pending.into_future::<JSValue>().await?;
                }
                if let Some(close) = close {
                    close.call_async::<_, JSValue>(Some(source), ()).await?;
                }
                step(&ctx, true, |_| Ok(()))
            }
        })?,
    )?;
    Ok(iterator)
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

    fn task_from(ctx: &JSContext, source: &str, result: &str, controls: &[&str]) -> JSObject {
        let iterator: JSObject = ctx.eval(Source::from_bytes(source)).unwrap();
        let result: Promise = ctx.eval(Source::from_bytes(result)).unwrap();
        let task = create_task(
            ctx,
            iterator,
            result,
            controls,
            byte_events("downloadedBytes"),
        )
        .unwrap();
        ctx.global().set("task", task.clone()).unwrap();
        task
    }

    #[test]
    fn the_task_is_its_result_controls_and_one_progress_iterable() {
        let runtime = <rong::RongJS as JSEngine>::runtime();
        let ctx = runtime.context();
        task_from(
            &ctx,
            "({ next() { return Promise.resolve({done:true}); }, cancel() { return Promise.resolve(); } })",
            "Promise.resolve(42)",
            &["cancel"],
        );
        let valid: bool = ctx.eval(Source::from_bytes(
            "task.result instanceof Promise && !('then' in task) && !('progress' in task) && typeof task.cancel === 'function' && typeof task[Symbol.asyncIterator] === 'function'",
        )).unwrap();
        assert!(valid);
        let valid: bool = ctx.eval(Source::from_bytes(
            "(() => { const iterator = task[Symbol.asyncIterator](); if ('cancel' in iterator || typeof iterator.next !== 'function') return false; try { task[Symbol.asyncIterator](); return false; } catch (e) { return e.code === 'E_INVALID_STATE'; } })()",
        )).unwrap();
        assert!(valid);
    }

    #[test]
    fn progress_maps_platform_events_and_the_iteration_returns_the_result() {
        let runtime = <rong::RongJS as JSEngine>::runtime();
        let ctx = runtime.context();
        task_from(
            &ctx,
            "(() => { const events = [{ kind: 'progress', downloadedBytes: 5 }, { kind: 'paused' }, { kind: 'progress', downloadedBytes: 5, totalBytes: 10 }, { kind: 'completed', downloadedBytes: 10, totalBytes: 10 }]; return { next() { const value = events.shift(); return Promise.resolve(value ? { done: false, value } : { done: true }); } }; })()",
            "Promise.resolve({ path: 'lx://temp/a' })",
            &[],
        );
        let script = "(async () => { const seen = []; const iterator = task[Symbol.asyncIterator](); for (;;) { const step = await iterator.next(); if (step.done) return seen.join('|') + '=>' + step.value.path; seen.push(JSON.stringify(step.value)); } })()";
        let local = tokio::task::LocalSet::new();
        let answer = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(local.run_until(ctx.eval_async::<String>(Source::from_bytes(script))))
            .unwrap();
        assert_eq!(
            answer,
            r#"{"loaded":5}|{"loaded":5,"total":10,"fraction":0.5}|{"loaded":10,"total":10,"fraction":1}=>lx://temp/a"#
        );
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
