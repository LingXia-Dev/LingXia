//! The View worker: one thread owning one [`ViewApp`] isolate, shared by the
//! pages of one app session.
use crate::ViewApp;
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::channel;
use std::sync::{Arc, Condvar, LazyLock, Mutex, Weak};
use std::time::{Duration, Instant};

type Job = Box<dyn FnOnce(&mut ViewApp) + Send>;

/// Contexts per isolate; matches the page stack bound.
const PAGES: usize = 16;

/// How long a navigation keeps hidden pages' work back when the incoming page never presents.
pub(crate) const NAVIGATION_HOLD: Duration = Duration::from_millis(500);

#[derive(Default)]
struct Queue {
    jobs: VecDeque<Job>,
    /// Served only when no shown page has work queued: hidden pages catching up.
    background: VecDeque<Job>,
    /// Background work waits until then, so it cannot start ahead of an incoming page's first frame.
    held_until: Option<Instant>,
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
        let held = self.held_until.filter(|until| now < *until && !self.closed);
        if held.is_none() {
            self.held_until = None;
            if let Some(job) = self.background.pop_front() {
                return Next::Run(job);
            }
        }
        // The queue closes when the last owner drops; queued work still runs first.
        if self.closed {
            return Next::Stop;
        }
        Next::Wait(
            held.filter(|_| !self.background.is_empty())
                .map(|until| until - now),
        )
    }
}

pub struct ViewWorker {
    queue: Arc<(Mutex<Queue>, Condvar)>,
    /// The app session key a shared worker serves.
    key: Option<String>,
}

impl Drop for ViewWorker {
    fn drop(&mut self) {
        let (queue, ready) = &*self.queue;
        queue.lock().unwrap_or_else(|p| p.into_inner()).closed = true;
        ready.notify_all();
    }
}

static SHARED: LazyLock<Mutex<HashMap<String, Weak<ViewWorker>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

impl ViewWorker {
    pub fn spawn(name: &str) -> Result<Arc<Self>, String> {
        Self::start(name, None)
    }

    fn start(name: &str, key: Option<String>) -> Result<Arc<Self>, String> {
        let queue: Arc<(Mutex<Queue>, Condvar)> = Default::default();
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
                        None => return,
                    }
                }
            })
            .map_err(|error| format!("UI_WORKER: {error}"))?;
        started
            .recv()
            .map_err(|_| "UI_WORKER: worker exited during startup".to_string())??;
        Ok(Arc::new(Self { queue, key }))
    }

    pub(crate) fn key(&self) -> Option<&str> {
        self.key.as_deref()
    }

    /// The worker for `key`, started on first use and stopped after its last owner drops.
    pub fn shared(key: &str) -> Result<Arc<Self>, String> {
        let mut shared = SHARED.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(worker) = shared.get(key).and_then(Weak::upgrade) {
            return Ok(worker);
        }
        shared.retain(|_, worker| worker.strong_count() > 0);
        let worker = Self::start("lingxia-gpu-view", Some(key.into()))?;
        shared.insert(key.into(), Arc::downgrade(&worker));
        Ok(worker)
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
}
