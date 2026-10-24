//! Engine-specific ESM entry until Rong exposes module evaluation through its portable API.

#[cfg(any(
    target_os = "android",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
pub(crate) fn drain_jobs(context: &rong::JSContext) -> Result<(), String> {
    use rong_core::engine::JSContextImpl;
    use rong_quickjs_sys as qjs;
    let started = std::time::Instant::now();
    unsafe {
        let runtime = qjs::JS_GetRuntime(*context.as_ref().as_raw());
        while qjs::JS_IsJobPending(runtime) {
            if started.elapsed() >= std::time::Duration::from_millis(100) {
                return Err("UI_TIMEOUT: microtask budget exceeded".into());
            }
            let mut owner = std::ptr::null_mut();
            if qjs::JS_ExecutePendingJob(runtime, &mut owner) < 0 {
                if !owner.is_null() {
                    qjs::JS_FreeValue(owner, qjs::JS_GetException(owner));
                }
                return Err("UI_ACTION: microtask execution failed".into());
            }
        }
    }
    Ok(())
}

/// Drain under an interrupt before dropping a failed context or running a sibling page.
#[cfg(any(
    target_os = "android",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
pub(crate) fn discard_jobs(context: &rong::JSContext) {
    use rong_core::engine::JSContextImpl;
    use rong_quickjs_sys as qjs;
    let _interrupt = context.runtime().interrupt_handle().interrupt_scoped();
    unsafe {
        let runtime = qjs::JS_GetRuntime(*context.as_ref().as_raw());
        // Branch-free Promise chains may never poll the interrupt handler. Reject
        // every JS call while draining, so jobs cannot enqueue another callback.
        qjs::JS_UpdateStackTop(runtime);
        qjs::JS_SetMaxStackSize(runtime, 1);
        while qjs::JS_IsJobPending(runtime) {
            let mut owner = std::ptr::null_mut();
            if qjs::JS_ExecutePendingJob(runtime, &mut owner) < 0 && !owner.is_null() {
                qjs::JS_FreeValue(owner, qjs::JS_GetException(owner));
            }
        }
        qjs::JS_SetMaxStackSize(runtime, qjs::JS_DEFAULT_STACK_SIZE as usize);
    }
}

#[cfg(not(any(
    target_os = "android",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
)))]
pub(crate) fn drain_jobs(_: &rong::JSContext) -> Result<(), String> {
    Ok(())
}
#[cfg(not(any(
    target_os = "android",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
)))]
pub(crate) fn discard_jobs(_: &rong::JSContext) {}

#[cfg(any(
    target_os = "android",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
pub(crate) fn evaluate(context: &rong::JSContext, source: &str) -> Result<(), String> {
    use rong_core::engine::JSContextImpl;
    use rong_quickjs_sys as qjs;

    let ctx = *context.as_ref().as_raw();
    let mut code = source.as_bytes().to_vec();
    code.push(0);
    // The caller confines this context to its View worker and scopes the interrupt budget.
    // QuickJS requires a trailing NUL even when an explicit source length is supplied.
    unsafe {
        // This isolate is renderer-owned; cleanup restores this same stack limit.
        qjs::JS_SetMaxStackSize(qjs::JS_GetRuntime(ctx), qjs::JS_DEFAULT_STACK_SIZE as usize);
        let result = qjs::JS_Eval(
            ctx,
            code.as_ptr().cast(),
            source.len(),
            c"page.ui.js".as_ptr(),
            qjs::JS_EVAL_TYPE_MODULE as i32,
        );
        let error = if qjs::QJS_IsException(ctx, result) {
            Some(qjs::JS_GetException(ctx))
        } else if qjs::JS_PromiseState(ctx, result) == qjs::JSPromiseStateEnum_JS_PROMISE_REJECTED {
            Some(qjs::JS_PromiseResult(ctx, result))
        } else {
            None
        };
        if let Some(error) = error {
            let text = qjs::JS_ToCStringLen2(ctx, std::ptr::null_mut(), error, false);
            let message = if text.is_null() {
                let secondary = qjs::JS_GetException(ctx);
                qjs::JS_FreeValue(ctx, secondary);
                "module evaluation failed".into()
            } else {
                let message = std::ffi::CStr::from_ptr(text)
                    .to_string_lossy()
                    .into_owned();
                qjs::JS_FreeCString(ctx, text);
                message
            };
            qjs::JS_FreeValue(ctx, error);
            qjs::JS_FreeValue(ctx, result);
            return Err(format!("UI_MODULE: {message}"));
        }
        let pending =
            qjs::JS_PromiseState(ctx, result) == qjs::JSPromiseStateEnum_JS_PROMISE_PENDING;
        qjs::JS_FreeValue(ctx, result);
        if pending {
            return Err("UI_MODULE: asynchronous module initialization is unsupported".into());
        }
    }
    Ok(())
}

#[cfg(not(any(
    target_os = "android",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
)))]
pub(crate) fn evaluate(context: &rong::JSContext, source: &str) -> Result<(), String> {
    context
        .eval::<()>(rong::Source::from_bytes(source))
        .map_err(|e| e.to_string())
}

#[cfg(all(
    test,
    any(
        target_os = "android",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    )
))]
mod tests {
    use crate::{ViewApp, session::tests::source};

    #[test]
    fn esm_exports_and_module_locals_work_in_isolated_pages() {
        let mut app = ViewApp::new(2).unwrap();
        let module =
            source("function render(){return h('text',{},'ESM hello')}") + "\nexport { render };";
        let first = app.create(&module, false).unwrap();
        let second = app.create(&module, false).unwrap();
        for id in [first, second] {
            assert_eq!(
                app.page(id).unwrap().layout(320, 640, 1.0).unwrap().nodes[0].text,
                "ESM hello"
            );
        }
    }

    #[test]
    fn failed_or_async_modules_do_not_admit_a_page_or_poison_siblings() {
        let mut app = ViewApp::new(1).unwrap();
        for module in [
            "export const broken = ;",
            "throw Error('module rejected'); export {};",
            "await new Promise(()=>{}); export {};",
            "import 'unbundled-package';",
            "export const render = 42;",
        ] {
            let error = app.create(module, false).unwrap_err();
            assert!(error.contains("UI_MODULE"), "{error}");
        }
        let id = app
            .create(
                &(source("function render(){return h('text',{},'healthy')}")
                    + "\nexport {render};"),
                false,
            )
            .unwrap();
        assert_eq!(
            app.page(id).unwrap().layout(320, 640, 1.0).unwrap().nodes[0].text,
            "healthy"
        );
    }
}
