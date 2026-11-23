//! The one STA thread that owns every WebView2 controller and every WebView
//! host window.
//!
//! Host layout code calls synchronously into controllers. With a thread per
//! controller those calls formed wait cycles: two UI threads each blocked on a
//! reply only the other could produce, because a waiting thread does not run
//! its own queued commands. On one thread a call either runs in place or is
//! queued behind work already on the stack, so no UI thread waits on another.

use super::*;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, HWND_MESSAGE, KillTimer,
    MSG_WAIT_FOR_MULTIPLE_OBJECTS_EX_FLAGS, MsgWaitForMultipleObjectsEx, PM_REMOVE, PeekMessageW,
    PostMessageW, QS_ALLINPUT, RegisterClassW, SetTimer, TranslateMessage, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_QUIT, WM_TIMER, WNDCLASSW,
};

const WM_LINGXIA_RUN: u32 = WM_APP + 0x155;
const DELAYED_TIMER_ID: usize = 1;
const PUMP_SLICE: Duration = Duration::from_millis(10);

type Task = Box<dyn FnOnce() + Send>;

struct UiThread {
    thread_id: u32,
    dispatcher: isize,
}

static UI_THREAD: OnceLock<UiThread> = OnceLock::new();
static TASKS: Mutex<VecDeque<Task>> = Mutex::new(VecDeque::new());
static WAKE_PENDING: AtomicBool = AtomicBool::new(false);

thread_local! {
    static DELAYED: RefCell<Vec<(Instant, Box<dyn FnOnce()>)>> = const { RefCell::new(Vec::new()) };
}

fn ui_thread() -> &'static UiThread {
    UI_THREAD.get_or_init(spawn_ui_thread)
}

pub(crate) fn is_ui_thread() -> bool {
    UI_THREAD
        .get()
        .is_some_and(|ui| ui.thread_id == unsafe { Threading::GetCurrentThreadId() })
}

/// Queues `task` behind everything already posted to the UI thread.
pub(crate) fn post(task: Task) {
    TASKS
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .push_back(task);
    if !WAKE_PENDING.swap(true, Ordering::AcqRel) {
        let ui = ui_thread();
        unsafe {
            let _ = PostMessageW(
                Some(hwnd_from_handle(ui.dispatcher)),
                WM_LINGXIA_RUN,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }
}

/// Runs `task` on the UI thread after `delay`. Must be called on the UI thread.
pub(crate) fn post_after(delay: Duration, task: impl FnOnce() + 'static) {
    debug_assert!(is_ui_thread());
    DELAYED.with(|delayed| {
        delayed
            .borrow_mut()
            .push((Instant::now() + delay, Box::new(task)))
    });
    arm_delayed_timer();
}

/// Runs `f` on the UI thread and returns its result; `None` if the UI thread
/// dropped the task.
pub fn run_on_webview_ui_thread<T: Send + 'static>(
    f: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    if is_ui_thread() {
        return Some(f());
    }
    let (tx, rx) = mpsc::channel();
    post(Box::new(move || {
        let _ = tx.send(f());
    }));
    super::controller::recv_reply_pumping(&rx, None).ok()
}

/// Whether the caller is the thread that owns WebView controllers and host
/// windows.
pub fn is_webview_ui_thread() -> bool {
    is_ui_thread()
}

/// Waits on the UI thread for `rx`, running every message meanwhile: the
/// reply usually arrives through a WebView2 callback on this same thread.
pub(crate) fn pump_until<T>(
    rx: &Receiver<T>,
    timeout: Option<Duration>,
) -> std::result::Result<T, mpsc::RecvTimeoutError> {
    let deadline = timeout.map(|timeout| Instant::now() + timeout);
    loop {
        match rx.try_recv() {
            Ok(reply) => return Ok(reply),
            Err(mpsc::TryRecvError::Disconnected) => {
                return Err(mpsc::RecvTimeoutError::Disconnected);
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
        let slice = match deadline {
            Some(deadline) => {
                let Some(left) = deadline.checked_duration_since(Instant::now()) else {
                    return Err(mpsc::RecvTimeoutError::Timeout);
                };
                PUMP_SLICE.min(left)
            }
            None => PUMP_SLICE,
        };
        let mut msg = MSG::default();
        unsafe {
            if PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message != WM_QUIT {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            } else {
                MsgWaitForMultipleObjectsEx(
                    None,
                    slice.as_millis() as u32,
                    QS_ALLINPUT,
                    MSG_WAIT_FOR_MULTIPLE_OBJECTS_EX_FLAGS(0x0004), // MWMO_INPUTAVAILABLE
                );
            }
        }
    }
}

fn spawn_ui_thread() -> UiThread {
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name("lingxia-webview-ui".to_string())
        .spawn(move || {
            unsafe {
                // OleInitialize = CoInitializeEx(STA) + OLE (clipboard/drag-drop);
                // composition hosting registers surface windows as drop targets.
                if let Err(err) = windows::Win32::System::Ole::OleInitialize(None) {
                    log::error!("WebView UI thread OleInitialize failed: {err}");
                }
            }
            let dispatcher = create_dispatcher_window();
            let _ = tx.send((unsafe { Threading::GetCurrentThreadId() }, dispatcher));
            run_loop();
        })
        .expect("spawn WebView UI thread");
    let (thread_id, dispatcher) = rx.recv().expect("WebView UI thread failed to start");
    if dispatcher == 0 {
        log::error!("WebView UI dispatcher window could not be created");
    }
    UiThread {
        thread_id,
        dispatcher,
    }
}

fn run_loop() -> ! {
    let mut msg = MSG::default();
    loop {
        // WM_QUIT is ignored: closing the last host window posts it on this
        // thread, but the app exits through the host's own message loop.
        if unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 > 0 {
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}

fn create_dispatcher_window() -> isize {
    let class_name = windows::core::w!("LingXiaWebViewUiDispatcher");
    let class = WNDCLASSW {
        lpfnWndProc: Some(dispatcher_proc),
        lpszClassName: class_name,
        ..Default::default()
    };
    unsafe {
        RegisterClassW(&class);
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            PCWSTR::null(),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            None,
            None,
        )
        .map(|hwnd| hwnd.0 as isize)
        .unwrap_or(0)
    }
}

// A message-only window rather than thread messages: modal loops (menus,
// window sizing, WebView2's own creation pump) dispatch window messages but
// drop thread messages.
unsafe extern "system" fn dispatcher_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    match msg {
        WM_LINGXIA_RUN => {
            run_posted_tasks();
            windows::Win32::Foundation::LRESULT(0)
        }
        WM_TIMER if wparam.0 == DELAYED_TIMER_ID => {
            run_due_delayed_tasks();
            windows::Win32::Foundation::LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn run_posted_tasks() {
    WAKE_PENDING.store(false, Ordering::Release);
    loop {
        let task = TASKS
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .pop_front();
        let Some(task) = task else {
            return;
        };
        run_guarded(task);
    }
}

fn run_due_delayed_tasks() {
    let now = Instant::now();
    let due = DELAYED.with(|delayed| {
        let mut delayed = delayed.borrow_mut();
        let (due, pending): (Vec<_>, Vec<_>) = delayed.drain(..).partition(|(at, _)| *at <= now);
        *delayed = pending;
        due
    });
    arm_delayed_timer();
    for (_, task) in due {
        run_guarded(task);
    }
}

fn arm_delayed_timer() {
    let dispatcher = hwnd_from_handle(ui_thread().dispatcher);
    let next = DELAYED.with(|delayed| delayed.borrow().iter().map(|(at, _)| *at).min());
    unsafe {
        match next {
            Some(at) => {
                let wait = at.saturating_duration_since(Instant::now()).as_millis();
                SetTimer(
                    Some(dispatcher),
                    DELAYED_TIMER_ID,
                    (wait as u32).max(1),
                    None,
                );
            }
            None => {
                let _ = KillTimer(Some(dispatcher), DELAYED_TIMER_ID);
            }
        }
    }
}

// A panic must not unwind out of the window procedure, and one failing task
// must not take every WebView down with the thread.
fn run_guarded(task: impl FnOnce()) {
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(task)).is_err() {
        log::error!("WebView UI task panicked");
    }
}
