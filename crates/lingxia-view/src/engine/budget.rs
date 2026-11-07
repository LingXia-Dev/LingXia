use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::thread_clock::ThreadClock;

/// A hang that burns no CPU (a blocked host call) is caught by wall time at this multiple.
const WALL_MULTIPLE: u32 = 10;

/// A watchdog that wakes this late was not running: the OS froze the process (Harmony does
/// once an app is backgrounded), and the frozen span is no hang.
const FROZEN: Duration = Duration::from_secs(1);

/// One evaluation: its budget is CPU time on the worker; wall time only catches hangs.
#[derive(Clone, Copy)]
struct Evaluating {
    started: Instant,
    budget: Duration,
    clock: Option<(ThreadClock, Duration)>,
}

impl Evaluating {
    /// CPU time used so far, or wall time where the platform has no thread clock.
    fn used(&self) -> (Option<Duration>, Duration) {
        let cpu = self
            .clock
            .and_then(|(clock, start)| Some(clock.elapsed()?.saturating_sub(start)));
        (cpu, self.started.elapsed())
    }

    fn expired(&self) -> bool {
        match self.used() {
            (Some(cpu), wall) => cpu >= self.budget || wall >= self.budget * WALL_MULTIPLE,
            (None, wall) => wall >= self.budget,
        }
    }

    /// Stop charging wall time the process spent frozen, from a wait due at `deadline`.
    fn thaw(&mut self, waited_from: Instant, deadline: Instant, now: Instant) {
        let late = now.saturating_duration_since(deadline);
        if late >= FROZEN && self.started <= waited_from {
            self.started += late;
        }
    }

    /// How long the watchdog may sleep: CPU time never runs ahead of wall time.
    fn remaining(&self) -> Duration {
        match self.used() {
            (Some(cpu), wall) => (self.budget.saturating_sub(cpu))
                .min((self.budget * WALL_MULTIPLE).saturating_sub(wall)),
            (None, wall) => self.budget.saturating_sub(wall),
        }
    }
}

#[derive(Default)]
struct State {
    evaluating: Option<Evaluating>,
    interrupt: Option<rong::InterruptGuard>,
    stopped: bool,
}

/// A sleeping watchdog per preemptible isolate, never a timer or thread per frame.
pub(crate) struct Budget {
    shared: Arc<(Mutex<State>, Condvar)>,
    thread: Option<JoinHandle<()>>,
}

impl Budget {
    pub(crate) fn new(runtime: &super::JSRuntime) -> Arc<Self> {
        let shared = Arc::new((Mutex::new(State::default()), Condvar::new()));
        let enabled = <super::Active as super::Engine>::PREEMPTIBLE;
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
                        match state.evaluating {
                            Some(evaluating) if state.interrupt.is_none() => {
                                if evaluating.expired() {
                                    // Arm and disarm share this lock, so an expired frame cannot interrupt its successor.
                                    state.interrupt = Some(handle.interrupt_scoped());
                                } else {
                                    let remaining =
                                        evaluating.remaining().max(Duration::from_millis(1));
                                    let waited_from = Instant::now();
                                    state = wake.wait_timeout(state, remaining).unwrap().0;
                                    if let Some(evaluating) = state.evaluating.as_mut() {
                                        evaluating.thaw(
                                            waited_from,
                                            waited_from + remaining,
                                            Instant::now(),
                                        );
                                    }
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

    /// Run `work`, interrupting it once it passes `limit`; an evaluation that finished past
    /// `budget` but within `limit` is reported as an [`Overrun`] beside its result.
    pub(crate) fn run<T>(
        &self,
        budget: Duration,
        limit: Duration,
        work: impl FnOnce() -> Result<T, String>,
    ) -> (Result<T, String>, Option<Overrun>) {
        if self.thread.is_none() {
            return (work(), None);
        }
        // Read on the worker: the watchdog thread charges this thread's clock.
        let evaluating = Evaluating {
            started: Instant::now(),
            budget: limit,
            clock: ThreadClock::current().and_then(|clock| Some((clock, clock.elapsed()?))),
        };
        {
            let mut state = self.shared.0.lock().unwrap();
            assert!(
                state.evaluating.is_none(),
                "View evaluations must be serial"
            );
            state.evaluating = Some(evaluating);
            self.shared.1.notify_one();
        }
        let guard = Evaluation { budget: self };
        let result = work();
        // Wall time alone fails only through the watchdog, which leaves out a frozen process.
        let (interrupted, evaluating) = {
            let state = self.shared.0.lock().unwrap();
            (
                state.interrupt.is_some(),
                state.evaluating.unwrap_or(evaluating),
            )
        };
        let (cpu, wall) = evaluating.used();
        let expired = interrupted
            || match cpu {
                Some(cpu) => cpu >= limit,
                None => wall >= limit,
            };
        drop(guard);
        let used = cpu.map_or("n/a".to_owned(), |cpu| format!("{}ms", cpu.as_millis()));
        if expired {
            return (
                Err(format!(
                    "UI_TIMEOUT: View JavaScript exceeded its execution limit (cpu {used}, wall {}ms, limit {}ms); discard this page",
                    wall.as_millis(),
                    limit.as_millis()
                )),
                None,
            );
        }
        let over = match cpu {
            Some(cpu) => cpu > budget,
            None => wall > budget,
        };
        let overrun = over.then(|| Overrun {
            detail: format!(
                "cpu {used}, wall {}ms, budget {}ms",
                wall.as_millis(),
                budget.as_millis()
            ),
        });
        (result, overrun)
    }
}

/// An evaluation that finished past its budget but within its limit.
pub(crate) struct Overrun {
    pub detail: String,
}

struct Evaluation<'a> {
    budget: &'a Budget,
}
impl Drop for Evaluation<'_> {
    fn drop(&mut self) {
        let mut state = self.budget.shared.0.lock().unwrap();
        state.evaluating = None;
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

#[cfg(test)]
mod tests {
    use super::*;
    use rong::JSEngine;

    /// Time the worker spends descheduled (host load, other threads) is not charged; time it
    /// computes is, and a hang that computes nothing still ends at ten times the budget.
    #[test]
    fn the_budget_charges_the_workers_cpu_time_not_the_hosts_load() {
        if !<crate::engine::Active as crate::engine::Engine>::PREEMPTIBLE
            || ThreadClock::current().is_none()
        {
            return;
        }
        let runtime = crate::engine::RongJS::runtime();
        let budget = Budget::new(&runtime);
        let limit = Duration::from_millis(40);
        // Load: spinning threads on every core while the worker waits off-CPU for 3× the budget.
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let spinners: Vec<_> = (0..std::thread::available_parallelism().map_or(4, |n| n.get()) * 2)
            .map(|_| {
                let stop = stop.clone();
                std::thread::spawn(move || {
                    while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                        std::hint::spin_loop();
                    }
                })
            })
            .collect();
        let (waited, overrun) = budget.run(limit, limit, || {
            std::thread::sleep(limit * 3);
            Ok(())
        });
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        for spinner in spinners {
            let _ = spinner.join();
        }
        assert!(waited.is_ok() && overrun.is_none(), "{waited:?}");
        // Computing past the limit fails, naming both clocks.
        let (burned, _) = budget.run(limit, limit, || {
            let started = Instant::now();
            let mut sum = 0u64;
            while started.elapsed() < limit * 3 {
                sum = std::hint::black_box(sum.wrapping_add(1));
            }
            Ok(sum)
        });
        let error = burned.unwrap_err();
        assert!(
            error.starts_with("UI_TIMEOUT:") && error.contains("cpu ") && error.contains("wall "),
            "{error}"
        );
        // Past the budget but within the limit, the work finishes and is reported as an overrun.
        let (over, overrun) = budget.run(limit, limit * 4, || {
            let started = Instant::now();
            while started.elapsed() < limit * 2 {
                std::hint::black_box(0u64);
            }
            Ok(())
        });
        assert!(over.is_ok() && overrun.is_some(), "{over:?}");
        // A hang that computes nothing ends at ten times the limit.
        let (hung, _) = budget.run(Duration::from_millis(10), Duration::from_millis(10), || {
            std::thread::sleep(Duration::from_millis(150));
            Ok(())
        });
        assert!(hung.is_err());
    }

    #[test]
    fn a_frozen_process_is_not_charged_as_a_hang() {
        let now = Instant::now();
        let limit = Duration::from_millis(400);
        let mut evaluating = Evaluating {
            started: now,
            budget: limit,
            clock: None,
        };
        // The watchdog slept 4 s due and woke after a 120 s freeze.
        let deadline = now + limit * WALL_MULTIPLE;
        let woke = deadline + Duration::from_secs(120);
        evaluating.thaw(now, deadline, woke);
        assert_eq!(evaluating.started, now + Duration::from_secs(120));
        // A wake within the slack is an ordinary late timer and charges everything.
        let mut ordinary = Evaluating {
            started: now,
            budget: limit,
            clock: None,
        };
        ordinary.thaw(now, deadline, deadline + Duration::from_millis(200));
        assert_eq!(ordinary.started, now);
        // An evaluation that began after the wait started is not credited.
        let mut later = Evaluating {
            started: now + Duration::from_secs(1),
            budget: limit,
            clock: None,
        };
        later.thaw(now, deadline, woke);
        assert_eq!(later.started, now + Duration::from_secs(1));
    }
}
