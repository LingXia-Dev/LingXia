//! The ArkUI main thread, reached through one thread-safe function with no
//! JavaScript callee: its native callback runs each posted task, so posting a
//! task costs no ArkTS frame. Delays wait on one timer thread, then post.
use lingxia_view::platform::Task;
use napi_sys_ohos as sys;
use std::collections::BinaryHeap;
use std::ffi::c_void;
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

struct Function(sys::napi_threadsafe_function);
// SAFETY: a thread-safe function is callable from any thread by contract.
unsafe impl Send for Function {}
unsafe impl Sync for Function {}

static MAIN: OnceLock<Function> = OnceLock::new();
static MAIN_THREAD: OnceLock<std::thread::ThreadId> = OnceLock::new();

unsafe extern "C" fn run(
    _env: sys::napi_env,
    _callee: sys::napi_value,
    _context: *mut c_void,
    data: *mut c_void,
) {
    if data.is_null() {
        return;
    }
    // SAFETY: boxed by `post` and handed to the function exactly once.
    let task = unsafe { Box::from_raw(data.cast::<Task>()) };
    task();
}

/// Creates the main-thread function; call once, on the main thread, before any page exists.
///
/// # Safety
/// `env` must be the main thread's live environment.
pub unsafe fn install(env: sys::napi_env) -> Result<(), String> {
    if MAIN.get().is_some() {
        return Ok(());
    }
    let mut name = std::ptr::null_mut();
    let label = c"lingxia-view";
    let mut function = std::ptr::null_mut();
    // SAFETY: `env` is live on this thread; a null callee is allowed with a native callback.
    let status = unsafe {
        sys::napi_create_string_utf8(env, label.as_ptr(), label.count_bytes() as _, &mut name);
        sys::napi_create_threadsafe_function(
            env,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            name,
            0,
            1,
            std::ptr::null_mut(),
            None,
            std::ptr::null_mut(),
            Some(run),
            &mut function,
        )
    };
    if status != sys::Status::napi_ok {
        return Err(format!("UI_MAIN: thread-safe function failed ({status})"));
    }
    // The app's lifetime keeps the loop alive; the function must not.
    // SAFETY: created above on this thread.
    unsafe { sys::napi_unref_threadsafe_function(env, function) };
    let _ = MAIN.set(Function(function));
    let _ = MAIN_THREAD.set(std::thread::current().id());
    Ok(())
}

fn post(task: Task) {
    let data = Box::into_raw(Box::new(task)).cast::<c_void>();
    let queued = MAIN.get().is_some_and(|main| {
        // SAFETY: the function lives for the process; `data` is reclaimed by `run`.
        let status = unsafe {
            sys::napi_call_threadsafe_function_with_priority(
                main.0,
                data,
                sys::napi_task_priority::napi_priority_immediate,
                true,
            )
        };
        status == sys::Status::napi_ok
    });
    if !queued {
        // SAFETY: the function never took the pointer.
        drop(unsafe { Box::from_raw(data.cast::<Task>()) });
        log::warn!(target: "lingxia_view", "UI_MAIN: main thread unavailable; task dropped");
    }
}

struct Timed(Instant, u64, Task);
impl PartialEq for Timed {
    fn eq(&self, other: &Self) -> bool {
        (self.0, self.1) == (other.0, other.1)
    }
}
impl Eq for Timed {}
impl PartialOrd for Timed {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Timed {
    // Earliest first in the max-heap; equal deadlines keep posting order.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (other.0, other.1).cmp(&(self.0, self.1))
    }
}

struct Timers {
    queue: Mutex<(BinaryHeap<Timed>, u64)>,
    wake: Condvar,
}

fn timers() -> &'static Timers {
    static TIMERS: OnceLock<&'static Timers> = OnceLock::new();
    TIMERS.get_or_init(|| {
        let timers: &'static Timers = Box::leak(Box::new(Timers {
            queue: Mutex::new((BinaryHeap::new(), 0)),
            wake: Condvar::new(),
        }));
        let _ = std::thread::Builder::new()
            .name("lingxia-view-timers".into())
            .spawn(move || {
                let mut queue = timers.queue.lock().unwrap_or_else(|p| p.into_inner());
                loop {
                    let now = Instant::now();
                    match queue.0.peek().map(|t| t.0) {
                        Some(due) if due <= now => {
                            let Timed(_, _, task) = queue.0.pop().expect("peeked");
                            post(task);
                        }
                        Some(due) => {
                            queue = timers
                                .wake
                                .wait_timeout(queue, due - now)
                                .unwrap_or_else(|p| p.into_inner())
                                .0;
                        }
                        None => {
                            queue = timers.wake.wait(queue).unwrap_or_else(|p| p.into_inner());
                        }
                    }
                }
            });
        timers
    })
}

/// Runs `task` on the main thread after `delay`, exactly once while the app lives.
pub fn post_after(delay: Duration, task: Task) {
    if delay.is_zero() {
        return post(task);
    }
    let timers = timers();
    let mut queue = timers.queue.lock().unwrap_or_else(|p| p.into_inner());
    queue.1 += 1;
    let order = queue.1;
    queue.0.push(Timed(Instant::now() + delay, order, task));
    timers.wake.notify_one();
}

/// Runs `task` on the main thread, exactly once.
pub fn post_main(task: Task) {
    post(task);
}

/// Runs `work` on the main thread and waits for it: for system callbacks that arrive on their
/// own threads but must answer synchronously from main-thread state. `None` on timeout.
pub(crate) fn run_on_main<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    if MAIN_THREAD.get() == Some(&std::thread::current().id()) {
        return Some(work());
    }
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    post(Box::new(move || {
        let _ = send.send(work());
    }));
    receive.recv_timeout(Duration::from_secs(2)).ok()
}
