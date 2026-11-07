//! The one thread that does GPU work for every page: attach, draw and present. The View
//! worker hands it display lists and goes on to the next frame; vsync asks it to present.
//! Reconfiguring a surface waits for the device to idle, so it must not race a submit.
use crate::DisplayList;
use std::collections::VecDeque;
use std::sync::{Arc, Condvar, LazyLock, Mutex, MutexGuard};
use std::time::Instant;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|p| p.into_inner())
}

/// Encodes a frame for a presenter id: its metrics, `None` when the target declined it.
pub type PrepareFn = fn(u64, &DisplayList, u64) -> Result<Option<serde_json::Value>, String>;
/// Presents a drawn revision: the presenter's timings, `None` when it did not reach the screen.
pub type PresentFn = fn(u64, u64) -> Result<Option<[u64; 4]>, String>;

/// The presenter registry a page draws through.
#[derive(Clone, Copy)]
pub struct Backend {
    pub prepare: PrepareFn,
    pub present: PresentFn,
}

/// The registry shared by adapters that bind windows through [`super::surfaces`].
pub const SURFACES: Backend = Backend {
    prepare: super::surfaces::prepare,
    present: super::surfaces::present,
};

/// A frame's draw: `None` until it ran, then what the backend's prepare returned.
pub struct Drawn {
    backend: Backend,
    result: Mutex<Option<Result<Option<serde_json::Value>, String>>>,
}

type Task = Box<dyn FnOnce() + Send>;

enum Job {
    Draw {
        surface: u64,
        revision: u64,
        display: Box<DisplayList>,
        drawn: Arc<Drawn>,
    },
    Run(Task),
}

static QUEUE: LazyLock<Arc<(Mutex<VecDeque<Job>>, Condvar)>> = LazyLock::new(|| {
    let queue: Arc<(Mutex<VecDeque<Job>>, Condvar)> = Default::default();
    let shared = queue.clone();
    let spawned = std::thread::Builder::new()
        .name("lingxia-gpu-render".into())
        .spawn(move || {
            let (queue, wake) = &*shared;
            loop {
                let job = {
                    let mut jobs = lock(queue);
                    loop {
                        if let Some(job) = jobs.pop_front() {
                            break job;
                        }
                        jobs = wake.wait(jobs).unwrap_or_else(|p| p.into_inner());
                    }
                };
                match job {
                    Job::Draw {
                        surface,
                        revision,
                        display,
                        drawn,
                    } => {
                        let result = (drawn.backend.prepare)(surface, &display, revision);
                        *lock(&drawn.result) = Some(result);
                    }
                    Job::Run(task) => task(),
                }
            }
        });
    if let Err(error) = spawned {
        log::error!(target: "lingxia_view", "GPU render thread did not start: {error}");
    }
    queue
});

fn push(job: Job, front: bool) {
    let (queue, wake) = &**QUEUE;
    let mut jobs = lock(queue);
    if front {
        jobs.push_front(job);
    } else {
        jobs.push_back(job);
    }
    wake.notify_one();
}

/// Draws `display` as `revision` of `surface`, behind the frames already queued.
pub fn draw(backend: Backend, surface: u64, revision: u64, display: DisplayList) -> Arc<Drawn> {
    let drawn = Arc::new(Drawn {
        backend,
        result: Mutex::new(None),
    });
    push(
        Job::Draw {
            surface,
            revision,
            display: Box::new(display),
            drawn: drawn.clone(),
        },
        false,
    );
    drawn
}

/// Presents `revision` once it is drawn: before any frame queued after its draw, so a frame
/// built ahead never delays the one already due. `report` gets whether it reached the screen
/// and the draw's scene, encode, submit and present µs.
pub fn present(
    surface: u64,
    revision: u64,
    drawn: Arc<Drawn>,
    report: impl FnOnce(Result<bool, String>, [u64; 4]) + Send + 'static,
) {
    let target = drawn.clone();
    let task: Task = Box::new(move || {
        let started = Instant::now();
        let outcome = match lock(&target.result).take() {
            Some(Ok(Some(metrics))) => {
                (target.backend.present)(surface, revision).map(|timings| {
                    if timings.is_none() {
                        log::debug!(target: "lingxia_view", "gpu present refused surface={surface} revision={revision}");
                    }
                    (timings.is_some(), Some(metrics))
                })
            }
            Some(Ok(None)) => {
                log::debug!(target: "lingxia_view", "gpu draw declined surface={surface} revision={revision}");
                Ok((false, None))
            }
            None => {
                log::debug!(target: "lingxia_view", "gpu present before draw surface={surface} revision={revision}");
                Ok((false, None))
            }
            Some(Err(error)) => Err(error),
        };
        match outcome {
            Ok((shown, metrics)) => {
                let us = |key: &str| {
                    metrics
                        .as_ref()
                        .and_then(|m| m.get("prepare")?.get(key)?.as_u64())
                        .unwrap_or(0)
                };
                let present = if metrics.is_some() {
                    started.elapsed().as_micros() as u64
                } else {
                    0
                };
                report(
                    Ok(shown),
                    [us("sceneUs"), us("encodeUs"), us("submitUs"), present],
                )
            }
            Err(error) => report(Err(error), [0; 4]),
        }
    });
    // The draw runs before the present: queued behind it when still waiting, else first.
    let (queue, wake) = &**QUEUE;
    let mut jobs = lock(queue);
    let at = jobs.iter().position(
        |job| matches!(job, Job::Draw { drawn: queued, .. } if Arc::ptr_eq(queued, &drawn)),
    );
    match at {
        Some(at) => jobs.insert(at + 1, Job::Run(task)),
        None => jobs.push_front(Job::Run(task)),
    }
    wake.notify_one();
}

/// Runs `task` on the render thread after the work already queued.
pub fn run(task: impl FnOnce() + Send + 'static) {
    push(Job::Run(Box::new(task)), false);
}

/// Runs `task` on the render thread ahead of queued work and waits for it.
pub fn run_now<T: Send + 'static>(task: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (sender, receiver) = std::sync::mpsc::channel();
    push(
        Job::Run(Box::new(move || {
            let _ = sender.send(task());
        })),
        true,
    );
    receiver.recv().ok()
}
