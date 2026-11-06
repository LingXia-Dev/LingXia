//! The one thread that does GPU work for every page: attach, draw and present. The View
//! worker hands it display lists and goes on to the next frame; vsync asks it to present.
use lingxia_view::DisplayList;
use lingxia_view::renderer::surfaces;
use std::collections::VecDeque;
use std::sync::{Arc, Condvar, LazyLock, Mutex, MutexGuard};
use std::time::Instant;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|p| p.into_inner())
}

/// A frame's draw: `None` until it ran, then what `surfaces::prepare` returned.
#[derive(Default)]
pub struct Drawn(Mutex<Option<Result<Option<serde_json::Value>, String>>>);

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

#[derive(Default)]
struct Queue {
    jobs: VecDeque<Job>,
}

static QUEUE: LazyLock<Arc<(Mutex<Queue>, Condvar)>> = LazyLock::new(|| {
    let queue: Arc<(Mutex<Queue>, Condvar)> = Default::default();
    let shared = queue.clone();
    let spawned = std::thread::Builder::new()
        .name("lingxia-gpu-render".into())
        .spawn(move || {
            let (queue, wake) = &*shared;
            loop {
                let job = {
                    let mut queue = lock(queue);
                    loop {
                        if let Some(job) = queue.jobs.pop_front() {
                            break job;
                        }
                        queue = wake.wait(queue).unwrap_or_else(|p| p.into_inner());
                    }
                };
                match job {
                    Job::Draw {
                        surface,
                        revision,
                        display,
                        drawn,
                    } => {
                        let result = surfaces::prepare(surface, &display, revision);
                        *lock(&drawn.0) = Some(result);
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
    let mut queue = lock(queue);
    if front {
        queue.jobs.push_front(job);
    } else {
        queue.jobs.push_back(job);
    }
    wake.notify_one();
}

/// Draws `display` as `revision` of `surface`, behind the frames already queued.
pub(crate) fn draw(surface: u64, revision: u64, display: DisplayList) -> Arc<Drawn> {
    let drawn = Arc::new(Drawn::default());
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
/// and the draw's metrics.
pub(crate) fn present(
    surface: u64,
    revision: u64,
    drawn: Arc<Drawn>,
    report: impl FnOnce(Result<bool, String>, Option<serde_json::Value>) + Send + 'static,
) {
    let target = drawn.clone();
    let task: Task = Box::new(move || {
        let started = Instant::now();
        let outcome = match lock(&target.0).take() {
            Some(Ok(Some(metrics))) => surfaces::present(surface, revision).map(|timings| {
                if timings.is_none() {
                    log::debug!(target: "lingxia_view", "gpu present refused surface={surface} revision={revision}");
                }
                (timings.is_some(), Some(metrics))
            }),
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
            Ok((shown, mut metrics)) => {
                if let Some(metrics) = metrics.as_mut().and_then(|m| m.as_object_mut()) {
                    metrics.insert(
                        "presentUs".into(),
                        (started.elapsed().as_micros() as u64).into(),
                    );
                }
                report(Ok(shown), metrics)
            }
            Err(error) => report(Err(error), None),
        }
    });
    // The draw runs before the present: queued behind it when still waiting, else first.
    let (queue, wake) = &**QUEUE;
    let mut queue = lock(queue);
    let at = queue.jobs.iter().position(
        |job| matches!(job, Job::Draw { drawn: queued, .. } if Arc::ptr_eq(queued, &drawn)),
    );
    match at {
        Some(at) => queue.jobs.insert(at + 1, Job::Run(task)),
        None => queue.jobs.push_front(Job::Run(task)),
    }
    wake.notify_one();
}

/// Runs `task` on the render thread after the work already queued.
pub(crate) fn run(task: impl FnOnce() + Send + 'static) {
    push(Job::Run(Box::new(task)), false);
}

/// Runs `task` on the render thread ahead of queued work and waits for it.
pub(crate) fn run_now<T: Send + 'static>(task: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (sender, receiver) = std::sync::mpsc::channel();
    push(
        Job::Run(Box::new(move || {
            let _ = sender.send(task());
        })),
        true,
    );
    receiver.recv().ok()
}
