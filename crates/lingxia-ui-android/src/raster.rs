//! CPU frames rasterize off the View worker: on a device without a GPU target a
//! full-screen debug raster takes seconds and would hold up every page sharing
//! the worker. Each page keeps one pending frame; a newer one replaces it.
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, LazyLock, Mutex};

type Job = Box<dyn FnOnce() + Send>;

/// At most this many pages rasterize at once, process-wide.
const THREADS: usize = 2;

static POOL: LazyLock<Mutex<Sender<Job>>> = LazyLock::new(|| {
    let (jobs, queue) = channel::<Job>();
    let queue: Arc<Mutex<Receiver<Job>>> = Arc::new(Mutex::new(queue));
    for index in 0..THREADS {
        let queue = queue.clone();
        let _ = std::thread::Builder::new()
            .name(format!("lingxia-ui-raster-{index}"))
            .spawn(move || {
                loop {
                    let job = queue.lock().unwrap_or_else(|p| p.into_inner()).recv();
                    match job {
                        Ok(job) => job(),
                        Err(_) => return,
                    }
                }
            });
    }
    Mutex::new(jobs)
});

struct State<T> {
    next: Option<T>,
    running: bool,
}

/// One page's frames, rendered in submission order with superseded ones skipped.
pub(super) struct Mailbox<T> {
    state: Mutex<State<T>>,
    render: Box<dyn Fn(T) + Send + Sync>,
}

impl<T: Send + 'static> Mailbox<T> {
    pub(super) fn new(render: impl Fn(T) + Send + Sync + 'static) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State {
                next: None,
                running: false,
            }),
            render: Box::new(render),
        })
    }

    /// Queue `frame` on the pool, replacing a frame not yet started.
    pub(super) fn submit(self: &Arc<Self>, frame: T) {
        {
            let mut state = self.lock();
            state.next = Some(frame);
            if state.running {
                return;
            }
            state.running = true;
        }
        let mailbox = self.clone();
        let job: Job = Box::new(move || {
            loop {
                let frame = {
                    let mut state = mailbox.lock();
                    match state.next.take() {
                        Some(frame) => frame,
                        None => {
                            state.running = false;
                            return;
                        }
                    }
                };
                (mailbox.render)(frame);
            }
        });
        let _ = POOL.lock().unwrap_or_else(|p| p.into_inner()).send(job);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State<T>> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn a_slow_frame_never_holds_up_the_submitter_and_newer_frames_replace_pending_ones() {
        let (rendered, frames) = channel();
        let (release, gate) = channel::<()>();
        let (started, running) = channel();
        let gate = Mutex::new(gate);
        let rendered = Mutex::new(rendered);
        let mailbox = Mailbox::new(move |frame: u32| {
            if frame == 1 {
                started.send(()).unwrap();
                let _ = gate.lock().unwrap().recv();
            }
            rendered.lock().unwrap().send(frame).unwrap();
        });
        mailbox.submit(1);
        running.recv_timeout(Duration::from_secs(5)).unwrap();
        // Each returns at once although frame 1 is still rendering.
        for frame in 2..=4 {
            mailbox.submit(frame);
        }
        release.send(()).unwrap();
        let order: Vec<u32> = frames.iter().take(2).collect();
        assert_eq!(
            order,
            [1, 4],
            "frames 2 and 3 were superseded before they started"
        );
        assert!(frames.recv_timeout(Duration::from_millis(100)).is_err());
    }
}
