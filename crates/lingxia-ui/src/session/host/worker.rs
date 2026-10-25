//! The View worker: one thread owning one [`ViewApp`] isolate, shared by the
//! pages of one app session.
use crate::ViewApp;
use std::collections::HashMap;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, LazyLock, Mutex, Weak};

type Job = Box<dyn FnOnce(&mut ViewApp) + Send>;

/// Contexts per isolate; matches the page stack bound.
const PAGES: usize = 16;

pub struct ViewWorker {
    jobs: Mutex<Sender<Job>>,
}

static SHARED: LazyLock<Mutex<HashMap<String, Weak<ViewWorker>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

impl ViewWorker {
    pub fn spawn(name: &str) -> Result<Arc<Self>, String> {
        let (jobs, queue) = channel::<Job>();
        let (ready, started) = channel();
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
                // The queue closes when the last owner drops; queued work still runs first.
                while let Ok(job) = queue.recv() {
                    job(&mut app);
                }
            })
            .map_err(|error| format!("UI_WORKER: {error}"))?;
        started
            .recv()
            .map_err(|_| "UI_WORKER: worker exited during startup".to_string())??;
        Ok(Arc::new(Self {
            jobs: Mutex::new(jobs),
        }))
    }

    /// The worker for `key`, started on first use and stopped after its last owner drops.
    pub fn shared(key: &str) -> Result<Arc<Self>, String> {
        let mut shared = SHARED.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(worker) = shared.get(key).and_then(Weak::upgrade) {
            return Ok(worker);
        }
        shared.retain(|_, worker| worker.strong_count() > 0);
        let worker = Self::spawn("lingxia-gpu-view")?;
        shared.insert(key.into(), Arc::downgrade(&worker));
        Ok(worker)
    }

    pub fn run(&self, job: impl FnOnce(&mut ViewApp) + Send + 'static) {
        let _ = self
            .jobs
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .send(Box::new(job));
    }
}
