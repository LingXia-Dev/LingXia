//! The one thread that does GPU work for every page: attach, draw and present. The View
//! worker hands it display lists and goes on to the next frame; vsync asks it to present.
//! Reconfiguring a surface waits for the device to idle, so it must not race a submit.
use crate::DisplayList;
use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Condvar, LazyLock, Mutex, MutexGuard};
use std::time::Instant;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|p| p.into_inner())
}

/// Encodes a frame for a presenter id: its metrics, `None` when the target declined it.
pub type PrepareFn = fn(u64, &DisplayList, u64) -> Result<Option<serde_json::Value>, String>;
/// Presents a drawn revision: the presenter's timings, `None` when it did not reach the screen.
pub type PresentFn = fn(u64, u64) -> Result<Option<[u64; 4]>, String>;
/// The presenter behind an id, for redrawing a kept video frame.
pub type PresenterFn = fn(u64) -> Result<Option<super::surfaces::Shared>, String>;
/// Moves every bound presenter to a new device after the old one was lost.
pub type ResetFn = fn() -> Result<(), String>;

/// The presenter registry a page draws through.
#[derive(Clone, Copy)]
pub struct Backend {
    pub prepare: PrepareFn,
    pub present: PresentFn,
    pub presenter: PresenterFn,
    pub reset: ResetFn,
}

/// The registry shared by adapters that bind windows through [`super::surfaces`].
pub const SURFACES: Backend = Backend {
    prepare: super::surfaces::prepare,
    present: super::surfaces::present,
    presenter: super::surfaces::presenter,
    reset: super::surfaces::reset,
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
    /// Redraw a surface's kept frame for new video frames; see [`super::surfaces::rebind`].
    Rebind(u64),
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
                pooled(|| match job {
                    Job::Draw {
                        surface,
                        revision,
                        display,
                        drawn,
                    } => {
                        let backend = drawn.backend;
                        let result =
                            catch_unwind(AssertUnwindSafe(|| (backend.prepare)(surface, &display, revision)))
                                .unwrap_or_else(|panic| lost(&backend, surface, panic).map(|()| None));
                        let kept = matches!(result, Ok(Some(_)));
                        *lock(&drawn.result) = Some(result);
                        if kept {
                            super::surfaces::retain(
                                drawn.backend.presenter,
                                surface,
                                revision,
                                display,
                            );
                        }
                    }
                    Job::Rebind(surface) => {
                        let redrawn = catch_unwind(|| super::surfaces::rebind(surface))
                            .unwrap_or_else(|panic| lost(&SURFACES, surface, panic));
                        if let Err(error) = redrawn {
                            log::warn!(target: "lingxia_view", "video redraw: {error}");
                        }
                    }
                    Job::Run(task) => {
                        if let Err(panic) = catch_unwind(AssertUnwindSafe(task)) {
                            log::error!(target: "lingxia_view", "GPU task panicked: {}", message(&*panic));
                        }
                    }
                });
            }
        });
    if let Err(error) = spawned {
        log::error!(target: "lingxia_view", "GPU render thread did not start: {error}");
    }
    queue
});

/// One job's Objective-C temporaries end with it. This thread has no run loop to drain them,
/// and Metal hands some out autoreleased (a layer's delegate is its view): without a pool
/// every page view this thread touched would live, with its drawable, until the process ends.
#[cfg(target_vendor = "apple")]
fn pooled<T>(job: impl FnOnce() -> T) -> T {
    objc2::rc::autoreleasepool(|_| job())
}

#[cfg(not(target_vendor = "apple"))]
fn pooled<T>(job: impl FnOnce() -> T) -> T {
    job()
}

/// Device losses this process has recovered from.
static LOSSES: AtomicU32 = AtomicU32::new(0);

fn message(panic: &(dyn std::any::Any + Send)) -> &str {
    panic
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("non-string panic")
}

/// A panic inside wgpu or the renderer leaves the device unusable. The first moves every
/// presenter to a new device and drops the frame, which the page then draws again; after a
/// second the page gets the error and shows its recovery screen instead of staying blank.
fn lost(
    backend: &Backend,
    surface: u64,
    panic: Box<dyn std::any::Any + Send>,
) -> Result<(), String> {
    let adapter = (backend.presenter)(surface)
        .ok()
        .flatten()
        .map(|presenter| {
            presenter
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .adapter()
                .to_owned()
        })
        .unwrap_or_default();
    let error = format!("UI_GPU_LOST: {} ({adapter})", message(&*panic));
    log::error!(target: "lingxia_view", "{error}");
    if LOSSES.fetch_add(1, Ordering::AcqRel) > 0 {
        return Err(error);
    }
    match catch_unwind(backend.reset) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(reset)) => Err(format!("{error}; rebuilding failed: {reset}")),
        Err(panic) => Err(format!(
            "{error}; rebuilding panicked: {}",
            message(&*panic)
        )),
    }
}

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
/// and what the draw took. With `present` false it only waits for the draw: true means the
/// frame is ready for the caller to present itself ([`super::surfaces::present_in_transaction`]).
pub fn present(
    surface: u64,
    revision: u64,
    drawn: Arc<Drawn>,
    present: bool,
    report: impl FnOnce(Result<bool, String>, crate::session::host::DrawStats) + Send + 'static,
) {
    let target = drawn.clone();
    let task: Task = Box::new(move || {
        let started = Instant::now();
        let outcome = match lock(&target.result).take() {
            Some(Ok(Some(metrics))) if !present => Ok((Some([0; 4]), Some(metrics))),
            Some(Ok(Some(metrics))) => {
                let backend = target.backend;
                catch_unwind(|| (backend.present)(surface, revision))
                    .unwrap_or_else(|panic| lost(&backend, surface, panic).map(|()| None))
                    .map(|timings| {
                    if timings.is_none() {
                        log::debug!(target: "lingxia_view", "gpu present refused surface={surface} revision={revision}");
                    }
                    (timings, Some(metrics))
                })
            }
            Some(Ok(None)) => {
                log::debug!(target: "lingxia_view", "gpu draw declined surface={surface} revision={revision}");
                Ok((None, None))
            }
            None => {
                log::debug!(target: "lingxia_view", "gpu present before draw surface={surface} revision={revision}");
                Ok((None, None))
            }
            Some(Err(error)) => Err(error),
        };
        match outcome {
            Ok((timings, metrics)) => {
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
                let presenter = metrics.as_ref().map(|m| {
                    serde_json::json!({
                        "composite": m.get("composite"),
                        "backdropCache": m.get("backdropCache"),
                        "passes": m.get("passes"),
                        "acquireEncodeSubmitPresentUs": timings,
                    })
                });
                report(
                    Ok(timings.is_some()),
                    crate::session::host::DrawStats {
                        gpu_us: [us("sceneUs"), us("encodeUs"), us("submitUs"), present],
                        presenter,
                    },
                )
            }
            Err(error) => report(Err(error), Default::default()),
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

/// Queues a redraw of `surface`'s kept frame, unless one is queued already.
pub fn rebind(surface: u64) {
    let (queue, wake) = &**QUEUE;
    let mut jobs = lock(queue);
    if !jobs
        .iter()
        .any(|job| matches!(job, Job::Rebind(queued) if *queued == surface))
    {
        jobs.push_back(Job::Rebind(surface));
        wake.notify_one();
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc::channel;
    use std::time::Duration;

    /// Surfaces holding a swapchain, and whether the platform has destroyed their window.
    static SWAPCHAINS: Mutex<Option<HashSet<u64>>> = Mutex::new(None);
    static WINDOW_GONE: AtomicBool = AtomicBool::new(false);

    fn prepare(_: u64, _: &DisplayList, _: u64) -> Result<Option<serde_json::Value>, String> {
        Ok(Some(serde_json::json!({})))
    }

    fn present(surface: u64, _: u64) -> Result<Option<[u64; 4]>, String> {
        if !lock(&SWAPCHAINS)
            .as_ref()
            .is_some_and(|s| s.contains(&surface))
        {
            return Ok(None);
        }
        if WINDOW_GONE.load(Ordering::SeqCst) {
            return Err("queued a buffer to a window that is gone".into());
        }
        Ok(Some([0; 4]))
    }

    fn presenter(_: u64) -> Result<Option<super::super::surfaces::Shared>, String> {
        Ok(None)
    }

    fn reset() -> Result<(), String> {
        Ok(())
    }

    #[test]
    fn a_surface_released_while_a_frame_waits_takes_its_swapchain_down_first() {
        let surface = 9_000_001;
        *lock(&SWAPCHAINS) = Some(HashSet::from([surface]));
        let display = crate::session::tests::source("function render(){return h('text',{},'A')}");
        let display = crate::ViewSession::new(&display)
            .unwrap()
            .layout(32, 32, 1.0)
            .unwrap()
            .retained
            .compose(&Default::default())
            .unwrap();
        // The render thread is busy while the frame is queued and the page goes away.
        let (open, gate) = channel::<()>();
        run(move || {
            let _ = gate.recv();
        });
        let backend = Backend {
            prepare,
            present,
            presenter,
            reset,
        };
        let drawn = draw(backend, surface, 1, display);
        let (sent, reported) = channel();
        super::present(surface, 1, drawn, true, move |shown, _| {
            let _ = sent.send(shown);
        });
        let release = std::thread::spawn(move || {
            run_now(move || {
                lock(&SWAPCHAINS).as_mut().unwrap().remove(&surface);
            })
        });
        std::thread::sleep(Duration::from_millis(100));
        open.send(()).unwrap();
        release.join().unwrap();
        // The platform destroys the window once the release returns.
        WINDOW_GONE.store(true, Ordering::SeqCst);
        let shown = reported.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(
            shown,
            Ok(false),
            "the waiting frame never reaches the dead window"
        );
    }

    static RESETS: AtomicU32 = AtomicU32::new(0);

    /// An object a job autoreleases is gone when the job is: the render thread drains per job.
    #[cfg(target_vendor = "apple")]
    #[test]
    fn a_jobs_autoreleased_objects_end_with_the_job() {
        use objc2::rc::{Retained, Weak};
        use objc2::runtime::NSObject;
        // Loaded only on the render thread, where the object lived.
        struct Held(Weak<NSObject>);
        unsafe impl Send for Held {}
        let held = run_now(|| {
            let object = NSObject::new();
            let held = Held(Weak::from_retained(&object));
            let _ = Retained::autorelease_ptr(object);
            held
        })
        .unwrap();
        assert!(
            run_now(move || {
                let held = held;
                held.0.load().is_none()
            })
            .unwrap()
        );
    }

    fn panicking_prepare(
        _: u64,
        _: &DisplayList,
        _: u64,
    ) -> Result<Option<serde_json::Value>, String> {
        panic!("Capacity handled in creation")
    }

    fn counted_reset() -> Result<(), String> {
        RESETS.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn panicking_frame(surface: u64) -> Result<bool, String> {
        let backend = Backend {
            prepare: panicking_prepare,
            present,
            presenter,
            reset: counted_reset,
        };
        let display = crate::session::tests::source("function render(){return h('text',{},'A')}");
        let display = crate::ViewSession::new(&display)
            .unwrap()
            .layout(32, 32, 1.0)
            .unwrap()
            .retained
            .compose(&Default::default())
            .unwrap();
        let drawn = draw(backend, surface, 1, display);
        let (sent, reported) = channel();
        super::present(surface, 1, drawn, true, move |shown, _| {
            let _ = sent.send(shown);
        });
        reported.recv_timeout(Duration::from_secs(5)).unwrap()
    }

    #[test]
    fn a_device_lost_mid_frame_is_rebuilt_once_then_reported_and_the_thread_lives_on() {
        assert_eq!(
            panicking_frame(9_000_002),
            Ok(false),
            "the first loss rebuilds the presenters and drops the frame for a redraw"
        );
        assert_eq!(RESETS.load(Ordering::SeqCst), 1);
        let second = panicking_frame(9_000_003);
        assert!(
            second.as_ref().is_err_and(|e| e.starts_with("UI_GPU_LOST")),
            "a second loss reaches the page as an error: {second:?}"
        );
        assert_eq!(RESETS.load(Ordering::SeqCst), 1);
        assert_eq!(run_now(|| 7), Some(7), "the render thread still runs work");
    }
}
