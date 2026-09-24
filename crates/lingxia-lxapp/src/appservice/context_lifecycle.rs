use rong::{JSContext, JSContextService};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

struct ContextFuture<F> {
    // Field order is intentional: drop the JS future before its context owner.
    future: Pin<Box<F>>,
    _context: JSContext,
}

impl<F: Future> Future for ContextFuture<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.get_mut().future.as_mut().poll(cx)
    }
}

pub(super) fn spawn<F, Fut>(ctx: &JSContext, task: F)
where
    F: FnOnce(JSContext) -> Fut + 'static,
    Fut: Future<Output = ()> + 'static,
{
    let task_context = ctx.clone();
    ctx.spawn_task(ContextFuture {
        future: Box::pin(task(task_context)),
        // Rong JS values do not retain their JSContext wrapper. This field is
        // dropped after the future on completion and cancellation.
        _context: ctx.clone(),
    });
}

pub(super) async fn shutdown(ctx: &JSContext) {
    // Release timer callbacks before draining the context-owned tasks that
    // dispatch them.
    if let Some(timers) = ctx.get_service::<rong_timer::TimerRegistry>() {
        timers.on_shutdown();
    }

    ctx.shutdown_tasks().await;
}

/// Collect what retired Logic contexts left on this worker's engine.
///
/// JavaScriptCore frees a context whose native objects hold JS values only
/// after those objects are finalized, which takes a full collection that
/// also sweeps. The engine schedules that on a run-loop timer, and Logic
/// workers run no run loop, so contexts replaced on one worker (an app
/// reopened for every profile rollback of a test run) pile up, and each new
/// context's collections get slower. Automation builds, which replace
/// contexts at that rate, force the collection. The entry point is not public
/// API: it is looked up at run time, and nothing happens without it.
#[cfg(feature = "automation")]
pub(super) fn collect_retired(runtime: &rong::JSRuntime) {
    #[cfg(any(target_os = "ios", target_os = "macos"))]
    {
        use rong_core::JSContextImpl;
        use std::ffi::c_void;
        use std::sync::OnceLock;

        unsafe extern "C" {
            fn dlsym(handle: *mut c_void, symbol: *const std::ffi::c_char) -> *mut c_void;
        }
        const RTLD_DEFAULT: *mut c_void = -2isize as *mut c_void;
        static COLLECT: OnceLock<usize> = OnceLock::new();
        let address = *COLLECT.get_or_init(|| unsafe {
            dlsym(
                RTLD_DEFAULT,
                c"JSSynchronousGarbageCollectForDebugging".as_ptr(),
            ) as usize
        });
        if address == 0 {
            return;
        }
        // SAFETY: the symbol is JavaScriptCore's `void (JSContextRef)`.
        let collect: unsafe extern "C" fn(*mut c_void) = unsafe { std::mem::transmute(address) };
        // Any live context of the engine will do; the retired ones are gone.
        let scratch = runtime.context();
        unsafe { collect(*scratch.as_ref().as_raw() as *mut c_void) };
    }
    #[cfg(not(any(target_os = "ios", target_os = "macos")))]
    runtime.run_gc();
}
