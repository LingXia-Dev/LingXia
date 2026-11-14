//! The View worker: one thread owning one [`ViewApp`] isolate, shared by the
//! pages of one app session.
use crate::ViewApp;
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::channel;
use std::sync::{Arc, Condvar, LazyLock, Mutex, Weak};
use std::time::{Duration, Instant};

type Job = Box<dyn FnOnce(&mut ViewApp) + Send>;
type Shared = Arc<(Mutex<Queue>, Condvar)>;

/// Contexts per isolate; matches the page stack bound.
const PAGES: usize = 16;

/// How long a navigation keeps hidden pages' work back when the incoming page never presents.
pub(crate) const NAVIGATION_HOLD: Duration = Duration::from_millis(500);

/// How long an app session's worker outlives its last page: a reLaunch disposes every page
/// before creating the next, and a fresh isolate costs that page its first frame.
const KEEP_ALIVE: Duration = Duration::from_secs(2);

#[derive(Default)]
struct Queue {
    jobs: VecDeque<Job>,
    /// Served only when no shown page has work queued: hidden pages catching up.
    background: VecDeque<Job>,
    /// Background work waits until then, so it cannot start ahead of an incoming page's first frame.
    held_until: Option<Instant>,
    /// Set while no page owns a shared worker; the worker stops then unless a page takes it back.
    expires: Option<Instant>,
    closed: bool,
}

enum Next {
    Run(Job),
    Wait(Option<Duration>),
    Stop,
}

impl Queue {
    fn next(&mut self, now: Instant) -> Next {
        if let Some(job) = self.jobs.pop_front() {
            return Next::Run(job);
        }
        let ending = self.closed || self.expires.is_some_and(|at| now >= at);
        let held = self.held_until.filter(|until| now < *until && !ending);
        if held.is_none() {
            self.held_until = None;
            if let Some(job) = self.background.pop_front() {
                return Next::Run(job);
            }
        }
        // Queued work still runs before the worker stops.
        if ending {
            self.closed = true;
            return Next::Stop;
        }
        let held = held
            .filter(|_| !self.background.is_empty())
            .map(|until| until - now);
        let expires = self.expires.map(|at| at - now);
        Next::Wait(match (held, expires) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        })
    }
}

pub struct ViewWorker {
    queue: Shared,
    /// The app session key a shared worker serves.
    key: Option<String>,
    keep_alive: Duration,
}

impl Drop for ViewWorker {
    fn drop(&mut self) {
        let (queue, ready) = &*self.queue;
        if let Some(key) = &self.key {
            let shared = SHARED.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(slot) = shared
                .get(key)
                .filter(|s| Arc::ptr_eq(&s.queue, &self.queue))
            {
                // A page took the worker back between the last release and this drop.
                if slot.worker.strong_count() == 0 {
                    queue.lock().unwrap_or_else(|p| p.into_inner()).expires =
                        Some(Instant::now() + self.keep_alive);
                    ready.notify_all();
                }
                return;
            }
        }
        queue.lock().unwrap_or_else(|p| p.into_inner()).closed = true;
        ready.notify_all();
    }
}

/// A shared worker's queue outlives its owners for [`KEEP_ALIVE`], so the next page reuses it.
struct Slot {
    worker: Weak<ViewWorker>,
    queue: Shared,
}

static SHARED: LazyLock<Mutex<HashMap<String, Slot>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

impl ViewWorker {
    pub fn spawn(name: &str) -> Result<Arc<Self>, String> {
        Ok(Arc::new(Self {
            queue: Self::start(name, None)?,
            key: None,
            keep_alive: Duration::ZERO,
        }))
    }

    fn start(name: &str, key: Option<String>) -> Result<Shared, String> {
        let queue: Shared = Default::default();
        let (ready, started) = channel();
        let shared = queue.clone();
        std::thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                let mut app = match ViewApp::new(PAGES) {
                    Ok(app) => {
                        let _ = ready.send(Ok(()));
                        app
                    }
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        return;
                    }
                };
                let (queue, wake) = &*shared;
                loop {
                    let job = {
                        let mut queue = queue.lock().unwrap_or_else(|p| p.into_inner());
                        loop {
                            match queue.next(Instant::now()) {
                                Next::Run(job) => break Some(job),
                                Next::Stop => break None,
                                Next::Wait(None) => {
                                    queue = wake.wait(queue).unwrap_or_else(|p| p.into_inner())
                                }
                                Next::Wait(Some(left)) => {
                                    queue = wake
                                        .wait_timeout(queue, left)
                                        .unwrap_or_else(|p| p.into_inner())
                                        .0
                                }
                            }
                        }
                    };
                    match job {
                        Some(job) => job(&mut app),
                        None => break,
                    }
                }
                drop(app);
                if let Some(key) = key {
                    let mut workers = SHARED.lock().unwrap_or_else(|p| p.into_inner());
                    if workers
                        .get(&key)
                        .is_some_and(|slot| Arc::ptr_eq(&slot.queue, &shared))
                    {
                        workers.remove(&key);
                    }
                }
            })
            .map_err(|error| format!("UI_WORKER: {error}"))?;
        started
            .recv()
            .map_err(|_| "UI_WORKER: worker exited during startup".to_string())??;
        Ok(queue)
    }

    pub(crate) fn key(&self) -> Option<&str> {
        self.key.as_deref()
    }

    /// The worker for `key`, started on first use and stopped [`KEEP_ALIVE`] after its last
    /// owner drops unless another page takes it first.
    pub fn shared(key: &str) -> Result<Arc<Self>, String> {
        Self::shared_for(key, KEEP_ALIVE)
    }

    fn shared_for(key: &str, keep_alive: Duration) -> Result<Arc<Self>, String> {
        let mut shared = SHARED.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(slot) = shared.get(key) {
            if let Some(worker) = slot.worker.upgrade() {
                return Ok(worker);
            }
            let mut queue = slot.queue.0.lock().unwrap_or_else(|p| p.into_inner());
            if !queue.closed {
                queue.expires = None;
                drop(queue);
                let queue = slot.queue.clone();
                return Ok(Self::own(&mut shared, key, queue, keep_alive));
            }
        }
        let queue = Self::start("lingxia-gpu-view", Some(key.into()))?;
        Ok(Self::own(&mut shared, key, queue, keep_alive))
    }

    fn own(
        shared: &mut HashMap<String, Slot>,
        key: &str,
        queue: Shared,
        keep_alive: Duration,
    ) -> Arc<Self> {
        let worker = Arc::new(Self {
            queue: queue.clone(),
            key: Some(key.into()),
            keep_alive,
        });
        shared.insert(
            key.into(),
            Slot {
                worker: Arc::downgrade(&worker),
                queue,
            },
        );
        worker
    }

    /// The app session behind `key` closed: its worker stops once no page holds it, without
    /// waiting out [`KEEP_ALIVE`].
    pub fn close(key: &str) {
        let Some(slot) = SHARED.lock().unwrap_or_else(|p| p.into_inner()).remove(key) else {
            return;
        };
        if slot.worker.strong_count() == 0 {
            let (queue, ready) = &*slot.queue;
            queue.lock().unwrap_or_else(|p| p.into_inner()).closed = true;
            ready.notify_all();
        }
    }

    pub fn run(&self, job: impl FnOnce(&mut ViewApp) + Send + 'static) {
        self.push(Box::new(job), false);
    }

    /// Runs after every job a shown page has queued.
    pub fn run_background(&self, job: impl FnOnce(&mut ViewApp) + Send + 'static) {
        self.push(Box::new(job), true);
    }

    /// A shown page went away: hidden pages' work waits for the next shown frame, at most
    /// [`NAVIGATION_HOLD`].
    pub(crate) fn hold(&self) {
        let (queue, _) = &*self.queue;
        queue.lock().unwrap_or_else(|p| p.into_inner()).held_until =
            Some(Instant::now() + NAVIGATION_HOLD);
    }

    /// A shown page presented; held background work may run.
    pub(crate) fn release(&self) {
        let (queue, ready) = &*self.queue;
        let mut queue = queue.lock().unwrap_or_else(|p| p.into_inner());
        if queue.held_until.take().is_some() {
            ready.notify_one();
        }
    }

    fn push(&self, job: Job, background: bool) {
        let (queue, ready) = &*self.queue;
        let mut queue = queue.lock().unwrap_or_else(|p| p.into_inner());
        if background {
            queue.background.push_back(job);
        } else {
            queue.jobs.push_back(job);
        }
        ready.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::ThreadId;

    const DEADLINE: Duration = Duration::from_secs(5);
    /// Longer than any wait here, so only a close can stop the worker in time.
    const LONG: Duration = Duration::from_secs(60);

    fn job() -> Job {
        Box::new(|_| {})
    }

    #[test]
    fn a_held_queue_runs_shown_work_and_keeps_background_work_until_the_cap() {
        let start = Instant::now();
        let mut queue = Queue {
            held_until: Some(start + NAVIGATION_HOLD),
            ..Queue::default()
        };
        queue.background.push_back(job());
        queue.jobs.push_back(job());
        assert!(matches!(queue.next(start), Next::Run(_)));
        let Next::Wait(Some(left)) = queue.next(start + Duration::from_millis(100)) else {
            panic!("background work ran while held");
        };
        assert_eq!(left, Duration::from_millis(400));
        // A navigation whose incoming page never presents frees the hidden pages at the cap.
        assert!(matches!(queue.next(start + NAVIGATION_HOLD), Next::Run(_)));
        assert!(queue.held_until.is_none());
        assert!(matches!(
            queue.next(start + NAVIGATION_HOLD),
            Next::Wait(None)
        ));
    }

    #[test]
    fn a_release_or_a_close_frees_held_background_work() {
        let start = Instant::now();
        let mut queue = Queue {
            held_until: Some(start + NAVIGATION_HOLD),
            ..Queue::default()
        };
        assert!(matches!(queue.next(start), Next::Wait(None)));
        queue.background.push_back(job());
        queue.held_until = None;
        assert!(matches!(queue.next(start), Next::Run(_)));

        queue.held_until = Some(start + NAVIGATION_HOLD);
        queue.background.push_back(job());
        queue.closed = true;
        assert!(matches!(queue.next(start), Next::Run(_)));
        assert!(matches!(queue.next(start), Next::Stop));
    }

    #[test]
    fn an_unowned_queue_waits_out_its_keep_alive_then_stops_after_its_work() {
        let start = Instant::now();
        let mut queue = Queue {
            expires: Some(start + KEEP_ALIVE),
            held_until: Some(start + NAVIGATION_HOLD),
            ..Queue::default()
        };
        let Next::Wait(Some(left)) = queue.next(start) else {
            panic!("an unowned worker waits without its deadline");
        };
        assert_eq!(left, KEEP_ALIVE);
        queue.background.push_back(job());
        let Next::Wait(Some(left)) = queue.next(start) else {
            panic!("held work ran early");
        };
        assert_eq!(left, NAVIGATION_HOLD);
        queue.held_until = None;
        queue.background.clear();
        let Next::Wait(Some(left)) = queue.next(start + Duration::from_millis(500)) else {
            panic!("an unowned worker stopped early");
        };
        assert_eq!(left, KEEP_ALIVE - Duration::from_millis(500));

        // Work queued before the deadline still runs, held or not.
        queue.held_until = Some(start + KEEP_ALIVE * 2);
        queue.background.push_back(job());
        queue.jobs.push_back(job());
        assert!(matches!(queue.next(start + KEEP_ALIVE), Next::Run(_)));
        assert!(matches!(queue.next(start + KEEP_ALIVE), Next::Run(_)));
        assert!(matches!(queue.next(start + KEEP_ALIVE), Next::Stop));
        assert!(queue.closed);

        // A page taking the worker back clears the deadline.
        let mut queue = Queue {
            expires: Some(start + KEEP_ALIVE),
            ..Queue::default()
        };
        queue.expires = None;
        assert!(matches!(queue.next(start + KEEP_ALIVE), Next::Wait(None)));
    }

    #[test]
    fn a_hold_never_released_expires_on_a_live_worker() {
        let worker = ViewWorker::spawn("hold-test").unwrap();
        worker.hold();
        let (sent, ran) = channel();
        let start = Instant::now();
        worker.run_background(move |_| sent.send(Instant::now()).unwrap());
        let at = ran.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(at - start >= NAVIGATION_HOLD - Duration::from_millis(5));

        worker.hold();
        let (sent, ran) = channel();
        worker.run_background(move |_| sent.send(()).unwrap());
        worker.release();
        ran.recv_timeout(Duration::from_millis(250)).unwrap();
    }

    /// The isolate's thread: the same thread means the same isolate, never restarted.
    fn isolate(worker: &ViewWorker) -> ThreadId {
        let (sent, ran) = channel();
        worker.run(move |_| sent.send(std::thread::current().id()).unwrap());
        ran.recv_timeout(DEADLINE).unwrap()
    }

    fn stopped(queue: &Shared) -> bool {
        let until = Instant::now() + DEADLINE;
        while Arc::strong_count(queue) > 1 {
            if Instant::now() >= until {
                return false;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        true
    }

    fn slot(key: &str) -> bool {
        SHARED.lock().unwrap().contains_key(key)
    }

    #[test]
    fn a_page_created_within_the_keep_alive_reuses_the_isolate() {
        let key = "keep-alive:reuse";
        let first = ViewWorker::shared_for(key, LONG).unwrap();
        let thread = isolate(&first);
        let queue = first.queue.clone();
        drop(first);
        let mut next = ViewWorker::shared_for(key, LONG).unwrap();
        assert!(Arc::ptr_eq(&next.queue, &queue));
        assert_eq!(isolate(&next), thread);
        // Released and taken back many times: still the first isolate.
        for _ in 0..20 {
            drop(next);
            next = ViewWorker::shared_for(key, LONG).unwrap();
        }
        assert_eq!(isolate(&next), thread);
        ViewWorker::close(key);
        drop(next);
        assert!(stopped(&queue), "a closed app's worker kept running");
    }

    #[test]
    fn an_unowned_worker_stops_at_its_deadline() {
        let key = "keep-alive:deadline";
        let worker = ViewWorker::shared_for(key, Duration::from_millis(50)).unwrap();
        let thread = isolate(&worker);
        let queue = worker.queue.clone();
        let released = Instant::now();
        drop(worker);
        assert!(
            Arc::strong_count(&queue) > 2,
            "stopped without a keep-alive"
        );
        assert!(stopped(&queue), "worker outlived its deadline");
        assert!(released.elapsed() >= Duration::from_millis(45));
        assert!(!slot(key));
        let next = ViewWorker::shared_for(key, Duration::from_millis(50)).unwrap();
        assert_ne!(isolate(&next), thread);
    }

    #[test]
    fn closing_the_app_stops_its_worker_at_once_and_spares_other_apps() {
        let (closed, kept) = ("keep-alive:closed", "keep-alive:kept");
        let first = ViewWorker::shared_for(closed, LONG).unwrap();
        let other = ViewWorker::shared_for(kept, LONG).unwrap();
        let other_thread = isolate(&other);
        let (first_queue, other_queue) = (first.queue.clone(), other.queue.clone());
        drop(first);
        drop(other);
        ViewWorker::close(closed);
        assert!(stopped(&first_queue), "a closed app's worker kept running");
        assert!(!slot(closed));

        // The other app's grace is its own.
        assert!(Arc::strong_count(&other_queue) > 1);
        let other = ViewWorker::shared_for(kept, LONG).unwrap();
        assert_eq!(isolate(&other), other_thread);

        // An app closed while a page still holds its worker stops when that page goes.
        ViewWorker::close(kept);
        assert!(Arc::strong_count(&other_queue) > 1);
        drop(other);
        assert!(
            stopped(&other_queue),
            "a closed app's last page left its worker running"
        );
    }

    /// Shared workers started and released many times run what was queued, stop their thread and
    /// leave no entry behind.
    #[test]
    fn shared_workers_drain_and_stop_after_their_last_owner() {
        const CYCLES: usize = 100;
        for cycle in 0..CYCLES {
            let key = format!("leak-worker:{cycle}");
            let worker = ViewWorker::shared_for(&key, Duration::ZERO).unwrap();
            let again = ViewWorker::shared_for(&key, Duration::ZERO).unwrap();
            assert!(Arc::ptr_eq(&worker, &again));
            let (done, ran) = channel();
            let shown = done.clone();
            worker.run(move |_| {
                let _ = shown.send("shown");
            });
            worker.hold();
            worker.run_background(move |_| {
                let _ = done.send("background");
            });
            let queue = worker.queue.clone();
            drop(worker);
            drop(again);
            // Stopping still runs what was queued, held work included, then the thread lets go.
            assert_eq!(ran.recv_timeout(DEADLINE).unwrap(), "shown");
            assert_eq!(ran.recv_timeout(DEADLINE).unwrap(), "background");
            assert!(stopped(&queue), "worker {cycle} kept running");
            let queue = queue.0.lock().unwrap();
            assert!(queue.closed && queue.jobs.is_empty() && queue.background.is_empty());
        }
        let shared = SHARED.lock().unwrap();
        assert!(!shared.keys().any(|key| key.starts_with("leak-worker:")));
    }
}
