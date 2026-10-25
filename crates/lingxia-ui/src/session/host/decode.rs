//! Image decoding off the View worker. A large photo decodes for seconds in
//! debug builds; on the shared worker it would hold up every page's frames.
use crate::scene::image::Bitmap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, LazyLock, Mutex};

type Job = Box<dyn FnOnce() + Send>;

/// At most this many decodes run at once, process-wide.
const THREADS: usize = 2;

static POOL: LazyLock<Mutex<Sender<Job>>> = LazyLock::new(|| {
    let (jobs, queue) = channel::<Job>();
    let queue: Arc<Mutex<Receiver<Job>>> = Arc::new(Mutex::new(queue));
    for index in 0..THREADS {
        let queue = queue.clone();
        let _ = std::thread::Builder::new()
            .name(format!("lingxia-ui-decode-{index}"))
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

static PENDING: AtomicUsize = AtomicUsize::new(0);

/// Decode `bytes` on the pool; `done` runs there with the result.
pub(super) fn decode(bytes: Vec<u8>, done: impl FnOnce(Result<Bitmap, String>) + Send + 'static) {
    PENDING.fetch_add(1, Ordering::AcqRel);
    let job: Job = Box::new(move || {
        done(Bitmap::decode(&bytes));
        PENDING.fetch_sub(1, Ordering::AcqRel);
    });
    let _ = POOL.lock().unwrap_or_else(|p| p.into_inner()).send(job);
}

/// Tests settle only after every queued decode has handed its result on.
#[cfg(test)]
pub(crate) fn wait_idle() {
    while PENDING.load(Ordering::Acquire) != 0 {
        std::thread::yield_now();
    }
}

/// Occupies every decode thread until `release` fires or drops.
#[cfg(test)]
pub(crate) fn occupy(release: std::sync::mpsc::Receiver<()>) {
    let release = Arc::new(Mutex::new(release));
    for _ in 0..THREADS {
        let release = release.clone();
        PENDING.fetch_add(1, Ordering::AcqRel);
        let job: Job = Box::new(move || {
            let _ = release.lock().unwrap_or_else(|p| p.into_inner()).recv();
            PENDING.fetch_sub(1, Ordering::AcqRel);
        });
        let _ = POOL.lock().unwrap_or_else(|p| p.into_inner()).send(job);
    }
}
