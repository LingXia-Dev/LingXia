//! The main queue, through libdispatch: posted tasks need no Swift round trip.
use lingxia_view::platform::Task;
use std::ffi::c_void;
use std::time::Duration;

#[cfg(target_vendor = "apple")]
unsafe extern "C" {
    static _dispatch_main_q: c_void;
    fn dispatch_async_f(
        queue: *const c_void,
        context: *mut c_void,
        work: extern "C" fn(*mut c_void),
    );
    fn dispatch_after_f(
        when: u64,
        queue: *const c_void,
        context: *mut c_void,
        work: extern "C" fn(*mut c_void),
    );
    fn dispatch_time(when: u64, delta: i64) -> u64;
}

extern "C" fn run(context: *mut c_void) {
    // SAFETY: boxed by `post_after` and handed to libdispatch exactly once.
    let task = unsafe { Box::from_raw(context.cast::<Task>()) };
    task();
}

/// Runs `task` on the main thread after `delay`, exactly once.
pub(crate) fn post_after(delay: Duration, task: Task) {
    let context = Box::into_raw(Box::new(task)).cast::<c_void>();
    #[cfg(target_vendor = "apple")]
    unsafe {
        let queue = std::ptr::addr_of!(_dispatch_main_q);
        if delay.is_zero() {
            dispatch_async_f(queue, context, run);
        } else {
            let nanos = i64::try_from(delay.as_nanos()).unwrap_or(i64::MAX);
            dispatch_after_f(dispatch_time(0, nanos), queue, context, run);
        }
    }
    #[cfg(not(target_vendor = "apple"))]
    {
        let _ = delay;
        run(context);
    }
}
