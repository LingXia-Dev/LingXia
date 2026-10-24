use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[derive(Default)]
struct State {
    deadline: Option<Instant>,
    interrupt: Option<rong::InterruptGuard>,
    stopped: bool,
}

/// A sleeping watchdog per QuickJS isolate, never a timer or thread per frame.
pub(crate) struct Budget {
    shared: Arc<(Mutex<State>, Condvar)>,
    thread: Option<JoinHandle<()>>,
}

impl Budget {
    pub(crate) fn new(runtime: &rong::JSRuntime) -> Arc<Self> {
        let shared = Arc::new((Mutex::new(State::default()), Condvar::new()));
        // Apple preemption uses private SPI; ArkJS cannot preempt a running loop.
        let enabled = cfg!(any(
            target_os = "android",
            target_os = "windows",
            all(target_os = "linux", not(target_env = "ohos"))
        ));
        let thread = enabled.then(|| {
            let shared = shared.clone();
            let handle = runtime.interrupt_handle();
            std::thread::Builder::new()
                .name("ui-js-budget".into())
                .spawn(move || {
                    let (lock, wake) = &*shared;
                    let mut state = lock.lock().unwrap();
                    loop {
                        if state.stopped {
                            break;
                        }
                        match state.deadline {
                            Some(deadline) if state.interrupt.is_none() => {
                                if let Some(remaining) =
                                    deadline.checked_duration_since(Instant::now())
                                {
                                    state = wake.wait_timeout(state, remaining).unwrap().0;
                                } else {
                                    // Arm and disarm share this lock, so an expired frame cannot interrupt its successor.
                                    state.interrupt = Some(handle.interrupt_scoped());
                                }
                            }
                            _ => state = wake.wait(state).unwrap(),
                        }
                    }
                })
                .expect("create View JS watchdog")
        });
        Arc::new(Self { shared, thread })
    }

    pub(crate) fn run<T>(
        &self,
        duration: Duration,
        work: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        if self.thread.is_none() {
            return work();
        }
        let deadline = Instant::now() + duration;
        {
            let mut state = self.shared.0.lock().unwrap();
            assert!(state.deadline.is_none(), "View evaluations must be serial");
            state.deadline = Some(deadline);
            self.shared.1.notify_one();
        }
        let guard = Evaluation { budget: self };
        let result = work();
        let expired = Instant::now() >= deadline;
        drop(guard);
        if expired {
            Err(
                "UI_TIMEOUT: View JavaScript exceeded its execution budget; discard this page"
                    .into(),
            )
        } else {
            result
        }
    }
}

struct Evaluation<'a> {
    budget: &'a Budget,
}
impl Drop for Evaluation<'_> {
    fn drop(&mut self) {
        let mut state = self.budget.shared.0.lock().unwrap();
        state.deadline = None;
        state.interrupt = None;
        self.budget.shared.1.notify_one();
    }
}
impl Drop for Budget {
    fn drop(&mut self) {
        {
            let mut state = self.shared.0.lock().unwrap();
            state.stopped = true;
            self.shared.1.notify_one();
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
