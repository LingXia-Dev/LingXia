//! The View worker: one thread owning one [`ViewApp`] isolate, shared by the
//! pages of one app session.
use crate::ViewApp;
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::channel;
use std::sync::{Arc, Condvar, LazyLock, Mutex, Weak};

type Job = Box<dyn FnOnce(&mut ViewApp) + Send>;

/// Contexts per isolate; matches the page stack bound.
const PAGES: usize = 16;

#[derive(Default)]
struct Queue {
    jobs: VecDeque<Job>,
    /// Served only when no shown page has work queued: hidden pages catching up.
    background: VecDeque<Job>,
    closed: bool,
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
        // The app session is over; its theme pack goes with it.
        if let Some(key) = &self.key {
            lingxia_theme::set_app_pack(key, None);
        }
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
                            if let Some(job) = queue
                                .jobs
                                .pop_front()
                                .or_else(|| queue.background.pop_front())
                            {
                                break Some(job);
                            }
                            // The queue closes when the last owner drops; queued work still runs first.
                            if queue.closed {
                                break None;
                            }
                            queue = wake.wait(queue).unwrap_or_else(|p| p.into_inner());
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
