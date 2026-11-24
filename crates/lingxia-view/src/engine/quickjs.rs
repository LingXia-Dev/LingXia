//! QuickJS adapter: native ESM evaluation, bounded microtask draining and preemption.
use super::{Engine, Feature};

pub(crate) struct QuickJs;

impl Engine for QuickJs {
    const FEATURES: &'static [Feature] = &[
        Feature::CandidateTransactions,
        Feature::BoundedJobs,
        Feature::ViewChunks,
    ];
    const PREEMPTIBLE: bool = true;

    fn evaluate_module(
        context: &super::JSContext,
        source: &str,
        chunks: Option<super::chunks::ChunkLoader>,
        expected_dev_build_id: Option<String>,
    ) -> Result<super::bytecode::Key, String> {
        evaluate(context, "page.js", source, chunks, expected_dev_build_id)
    }
    fn evaluate_auxiliary_module(
        context: &super::JSContext,
        specifier: &str,
        source: &str,
    ) -> Result<super::bytecode::Key, String> {
        evaluate(context, specifier, source, None, None)
    }
    fn drain_jobs(context: &super::JSContext) -> Result<(), String> {
        drain_jobs(context)
    }
    fn discard_jobs(context: &super::JSContext) {
        discard_jobs(context)
    }
}

fn drain_jobs(context: &super::JSContext) -> Result<(), String> {
    use rong_core::engine::JSContextImpl;
    use rong_quickjs_sys as qjs;
    let started = std::time::Instant::now();
    unsafe {
        let runtime = qjs::JS_GetRuntime(*context.as_ref().as_raw());
        while qjs::JS_IsJobPending(runtime) {
            if started.elapsed() >= crate::session::eval_budget() {
                return Err("UI_TIMEOUT: microtask budget exceeded".into());
            }
            let mut owner = std::ptr::null_mut();
            if qjs::JS_ExecutePendingJob(runtime, &mut owner) < 0 {
                let mut aborted = false;
                if !owner.is_null() {
                    let error = qjs::JS_GetException(owner);
                    aborted = qjs::JS_IsUncatchableError(error);
                    qjs::JS_FreeValue(owner, error);
                }
                if aborted {
                    return Err(interrupted());
                }
                return Err("UI_ACTION: microtask execution failed".into());
            }
        }
    }
    Ok(())
}

/// Drain under an interrupt before dropping a failed context or running a sibling page.
fn discard_jobs(context: &super::JSContext) {
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

fn evaluate(
    context: &super::JSContext,
    filename: &str,
    source: &str,
    chunks: Option<super::chunks::ChunkLoader>,
    expected_dev_build_id: Option<String>,
) -> Result<super::bytecode::Key, String> {
    use super::bytecode::{self, Outcome};
    use rong_core::engine::JSContextImpl;
    use rong_quickjs_sys as qjs;

    let ctx = *context.as_ref().as_raw();
    let runtime = unsafe { qjs::JS_GetRuntime(ctx) };
    // The caller confines this context to its View worker and scopes the interrupt budget.
    // This isolate is renderer-owned; cleanup restores this same stack limit.
    unsafe {
        qjs::JS_SetMaxStackSize(runtime, qjs::JS_DEFAULT_STACK_SIZE as usize);
        qjs::JS_SetModuleLoaderFunc(
            runtime,
            Some(normalize_module),
            Some(load_module),
            std::ptr::null_mut(),
        );
    };
    let (result, chunk_error) = super::chunks::with_loader(chunks, expected_dev_build_id, || {
        let started = std::time::Instant::now();
        let key = bytecode::key(source);
        let cached = bytecode::load(&key).and_then(|(bytes, outcome)| {
            let module = unsafe { read(ctx, &bytes) };
            if module.is_none() {
                bytecode::forget(&key);
            }
            Some((module?, outcome, bytes.len()))
        });
        let (module, outcome, size) = match cached {
            Some(cached) => cached,
            None => {
                let module = unsafe { compile(ctx, filename, source)? };
                let bytes = unsafe { write(ctx, module) };
                let size = bytes.as_ref().map_or(0, Vec::len);
                if let Some(bytes) = bytes {
                    bytecode::store(key, bytes);
                }
                (module, Outcome::Miss, size)
            }
        };
        bytecode::report(outcome, size, started.elapsed());
        unsafe { run(ctx, module) }.map(|()| key)
    });
    match (result, chunk_error) {
        (Ok(key), _) => Ok(key),
        (Err(_), Some(err)) => Err(err),
        (Err(err), None) => Err(err),
    }
}

unsafe extern "C" {
    // Frees what `JS_WriteObject` allocated; the generated bindings leave it out.
    fn js_free(ctx: *mut rong_quickjs_sys::JSContext, ptr: *mut std::ffi::c_void);
    fn js_strdup(
        ctx: *mut rong_quickjs_sys::JSContext,
        str_: *const std::os::raw::c_char,
    ) -> *mut std::os::raw::c_char;
}

/// Preserves the raw specifier so `load_module` enforces the exact `lx:view/*` allowlist
/// without QuickJS rewriting relative paths against `page.js`.
unsafe extern "C" fn normalize_module(
    ctx: *mut rong_quickjs_sys::JSContext,
    _base_name: *const std::os::raw::c_char,
    module_name: *const std::os::raw::c_char,
    _opaque: *mut std::ffi::c_void,
) -> *mut std::os::raw::c_char {
    unsafe { js_strdup(ctx, module_name) }
}

/// Resolves `lx:view/*` imports through the active chunk loader and per-module bytecode cache.
unsafe extern "C" fn load_module(
    ctx: *mut rong_quickjs_sys::JSContext,
    module_name: *const std::os::raw::c_char,
    _opaque: *mut std::ffi::c_void,
) -> *mut rong_quickjs_sys::JSModuleDef {
    use super::bytecode::{self, Outcome};
    use rong_quickjs_sys as qjs;

    let specifier = unsafe { std::ffi::CStr::from_ptr(module_name) }
        .to_string_lossy()
        .into_owned();
    let source = match super::chunks::load_active(&specifier) {
        Ok(source) => source,
        Err(err) => {
            super::chunks::record_error(err);
            return std::ptr::null_mut();
        }
    };
    let started = std::time::Instant::now();
    let key = bytecode::key(&source);
    if let Some((bytes, outcome)) = bytecode::load(&key) {
        let val = unsafe {
            qjs::JS_ReadObject(
                ctx,
                bytes.as_ptr(),
                bytes.len(),
                qjs::JS_READ_OBJ_BYTECODE as i32,
            )
        };
        if !unsafe { qjs::QJS_IsException(ctx, val) } {
            bytecode::report(outcome, bytes.len(), started.elapsed());
            return unsafe { val.u.ptr as *mut qjs::JSModuleDef };
        }
        unsafe { qjs::JS_FreeValue(ctx, qjs::JS_GetException(ctx)) };
        bytecode::forget(&key);
    }
    let val = match unsafe { compile(ctx, &specifier, &source) } {
        Ok(val) => val,
        Err(err) => {
            super::chunks::record_error(err);
            return std::ptr::null_mut();
        }
    };
    let bytes = unsafe { write(ctx, val) };
    let size = bytes.as_ref().map_or(0, Vec::len);
    if let Some(bytes) = bytes {
        bytecode::store(key, bytes);
    }
    bytecode::report(Outcome::Miss, size, started.elapsed());
    unsafe { val.u.ptr as *mut qjs::JSModuleDef }
}

/// Parse and resolve without running; `run` evaluates the returned module.
unsafe fn compile(
    ctx: *mut rong_quickjs_sys::JSContext,
    filename: &str,
    source: &str,
) -> Result<rong_quickjs_sys::JSValue, String> {
    use rong_quickjs_sys as qjs;
    let mut code = source.as_bytes().to_vec();
    // QuickJS requires a trailing NUL even when an explicit source length is supplied.
    code.push(0);
    let fname = std::ffi::CString::new(filename)
        .unwrap_or_else(|_| std::ffi::CString::new("page.js").unwrap());
    unsafe {
        let module = qjs::JS_Eval(
            ctx,
            code.as_ptr().cast(),
            source.len(),
            fname.as_ptr(),
            (qjs::JS_EVAL_TYPE_MODULE | qjs::JS_EVAL_FLAG_COMPILE_ONLY) as i32,
        );
        if qjs::QJS_IsException(ctx, module) {
            return Err(message(ctx, qjs::JS_GetException(ctx)));
        }
        Ok(module)
    }
}

unsafe fn write(
    ctx: *mut rong_quickjs_sys::JSContext,
    module: rong_quickjs_sys::JSValue,
) -> Option<Vec<u8>> {
    use rong_quickjs_sys as qjs;
    unsafe {
        let mut size = 0usize;
        let buffer = qjs::JS_WriteObject(ctx, &mut size, module, qjs::JS_WRITE_OBJ_BYTECODE as i32);
        if buffer.is_null() {
            qjs::JS_FreeValue(ctx, qjs::JS_GetException(ctx));
            return None;
        }
        let bytes = std::slice::from_raw_parts(buffer, size).to_vec();
        js_free(ctx, buffer.cast());
        Some(bytes)
    }
}

/// A module from bytecode this build wrote; `None` when QuickJS refuses it.
unsafe fn read(
    ctx: *mut rong_quickjs_sys::JSContext,
    bytes: &[u8],
) -> Option<rong_quickjs_sys::JSValue> {
    use rong_quickjs_sys as qjs;
    unsafe {
        let module = qjs::JS_ReadObject(
            ctx,
            bytes.as_ptr(),
            bytes.len(),
            qjs::JS_READ_OBJ_BYTECODE as i32,
        );
        if qjs::QJS_IsException(ctx, module) {
            qjs::JS_FreeValue(ctx, qjs::JS_GetException(ctx));
            return None;
        }
        if qjs::JS_ResolveModule(ctx, module) < 0 {
            qjs::JS_FreeValue(ctx, qjs::JS_GetException(ctx));
            qjs::JS_FreeValue(ctx, module);
            return None;
        }
        Some(module)
    }
}

/// Link and evaluate a compiled module; it must settle synchronously.
unsafe fn run(
    ctx: *mut rong_quickjs_sys::JSContext,
    module: rong_quickjs_sys::JSValue,
) -> Result<(), String> {
    use rong_quickjs_sys as qjs;
    unsafe {
        let result = qjs::JS_EvalFunction(ctx, module);
        let error = if qjs::QJS_IsException(ctx, result) {
            Some(qjs::JS_GetException(ctx))
        } else if qjs::JS_PromiseState(ctx, result) == qjs::JSPromiseStateEnum_JS_PROMISE_REJECTED {
            Some(qjs::JS_PromiseResult(ctx, result))
        } else {
            None
        };
        if let Some(error) = error {
            qjs::JS_FreeValue(ctx, result);
            return Err(message(ctx, error));
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

/// QuickJS aborts on an interrupt with an uncatchable error; report it as rong does.
fn interrupted() -> String {
    format!(
        "{}: JavaScript execution was interrupted",
        rong::error::E_INTERRUPTED
    )
}

/// Consumes `error`.
unsafe fn message(
    ctx: *mut rong_quickjs_sys::JSContext,
    error: rong_quickjs_sys::JSValue,
) -> String {
    use rong_quickjs_sys as qjs;
    unsafe {
        if qjs::JS_IsUncatchableError(error) {
            qjs::JS_FreeValue(ctx, error);
            return interrupted();
        }
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
        format!("UI_MODULE: {message}")
    }
}

#[cfg(test)]
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
    fn unreadable_bytecode_falls_back_to_the_source() {
        use crate::engine::bytecode;
        let module =
            source("function render(){return h('text',{},'from source')}") + "\nexport { render };";
        let key = bytecode::key(&module);
        bytecode::remember(key, vec![1, 2, 3].into());
        let mut app = ViewApp::new(1).unwrap();
        let page = app.create(&module, false).unwrap();
        assert_eq!(
            app.page(page).unwrap().layout(320, 640, 1.0).unwrap().nodes[0].text,
            "from source"
        );
        let (cached, _) = bytecode::load(&key).unwrap();
        assert_ne!(
            &cached[..],
            &[1, 2, 3][..],
            "the compiled module replaced it"
        );
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
