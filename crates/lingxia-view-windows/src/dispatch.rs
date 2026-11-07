//! A page thread's task queue: a message-only window runs each posted task once, on the
//! thread that made it. Thread messages would be dropped by any modal loop on that thread.
use lingxia_view::platform::Task;
use std::cell::Cell;
use std::collections::BinaryHeap;
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, HWND_MESSAGE, PostMessageW, RegisterClassW, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_APP, WNDCLASSW,
};
use windows::core::w;

const WM_TASK: u32 = WM_APP + 0x31;

/// Posts tasks to the thread that created it. Copy and send freely.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Dispatcher(isize);

thread_local! {
    static CURRENT: Cell<Option<Dispatcher>> = const { Cell::new(None) };
}

impl Dispatcher {
    /// The calling thread's dispatcher, made on first use. The thread must pump messages.
    pub fn current() -> Result<Self, String> {
        if let Some(dispatcher) = CURRENT.get() {
            return Ok(dispatcher);
        }
        let class = w!("LingXiaViewDispatch");
        // SAFETY: plain Win32 calls with static strings; a repeat registration fails harmlessly.
        let hwnd = unsafe {
            let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
            RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(proc),
                hInstance: instance.into(),
                lpszClassName: class,
                ..Default::default()
            });
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class,
                None,
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(instance.into()),
                None,
            )
            .map_err(|e| format!("dispatch window: {e}"))?
        };
        let dispatcher = Self(hwnd.0 as isize);
        CURRENT.set(Some(dispatcher));
        Ok(dispatcher)
    }

    /// Runs `task` on the dispatcher's thread, exactly once; dropped if that thread is gone.
    pub fn post(self, task: Task) {
        let raw = Box::into_raw(Box::new(task));
        // SAFETY: on success the window procedure takes the box back exactly once.
        let posted = unsafe {
            PostMessageW(
                Some(HWND(self.0 as *mut _)),
                WM_TASK,
                WPARAM(0),
                LPARAM(raw as isize),
            )
        };
        if posted.is_err() {
            // SAFETY: never delivered, so still ours.
            drop(unsafe { Box::from_raw(raw) });
        }
    }

    pub fn post_after(self, delay: Duration, task: Task) {
        if delay.is_zero() {
            return self.post(task);
        }
        timers().schedule(Instant::now() + delay, self, task);
    }
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_TASK && lparam.0 != 0 {
        // SAFETY: boxed by `post`, delivered once.
        let task = unsafe { Box::from_raw(lparam.0 as *mut Task) };
        task();
        return LRESULT(0);
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

struct Due {
    at: Instant,
    seq: u64,
    target: Dispatcher,
    task: Task,
}

impl PartialEq for Due {
    fn eq(&self, other: &Self) -> bool {
        (self.at, self.seq) == (other.at, other.seq)
    }
}
impl Eq for Due {}
impl PartialOrd for Due {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Due {
    // Earliest first out of the max-heap.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (other.at, other.seq).cmp(&(self.at, self.seq))
    }
}

struct Timers {
    queue: Mutex<(BinaryHeap<Due>, u64)>,
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
            .spawn(move || timers.run());
        timers
    })
}

impl Timers {
    fn schedule(&self, at: Instant, target: Dispatcher, task: Task) {
        let mut queue = self.queue.lock().unwrap_or_else(|p| p.into_inner());
        queue.1 += 1;
        let seq = queue.1;
        queue.0.push(Due {
            at,
            seq,
            target,
            task,
        });
        self.wake.notify_one();
    }

    fn run(&self) {
        let mut queue = self.queue.lock().unwrap_or_else(|p| p.into_inner());
        loop {
            let now = Instant::now();
            match queue.0.peek().map(|due| due.at) {
                Some(at) if at <= now => {
                    let due = queue.0.pop().expect("peeked");
                    drop(queue);
                    due.target.post(due.task);
                    queue = self.queue.lock().unwrap_or_else(|p| p.into_inner());
                }
                Some(at) => {
                    queue = self
                        .wake
                        .wait_timeout(queue, at - now)
                        .unwrap_or_else(|p| p.into_inner())
                        .0;
                }
                None => queue = self.wake.wait(queue).unwrap_or_else(|p| p.into_inner()),
            }
        }
    }
}
