//! Async task helpers backed by LingXia's global `RongExecutor`.

use rong_rt::RongExecutor;

/// Task handle returned by LingXia task helpers.
pub type JoinHandle<T> = tokio::task::JoinHandle<T>;

type EarlyTask = std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'static>>;

/// Work handed to [`crate::spawn`] before the runtime is initialized.
/// `Some` while deferring; `None` once the executor takes work directly.
static EARLY: std::sync::Mutex<Option<Vec<EarlyTask>>> = std::sync::Mutex::new(Some(Vec::new()));

/// [`crate::spawn`]'s engine: queue while the runtime is not up yet — the
/// executor's lazy global must not be configured by a stray early spawn —
/// and hand off directly once it is.
pub(crate) fn spawn_or_defer<F>(future: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let Ok(mut early) = EARLY.lock() else {
        return;
    };
    match &mut *early {
        Some(queue) => queue.push(Box::pin(future)),
        None => drop(spawn(future)),
    }
}

/// Flips [`crate::spawn`] to direct mode and starts everything queued before
/// initialization. Called by bootstrap once the executor is installed.
pub(crate) fn release_deferred() {
    let taken = match EARLY.lock() {
        Ok(mut early) => early.take(),
        Err(_) => return,
    };
    for task in taken.into_iter().flatten() {
        drop(spawn(task));
    }
}

/// Spawns an async task onto LingXia's global executor.
pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    RongExecutor::global().spawn(future)
}

/// Runs blocking work on LingXia's blocking executor pool and waits for the result.
pub async fn spawn_blocking<F, R>(f: F) -> crate::Result<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    RongExecutor::global()
        .spawn_blocking(f)
        .await
        .map_err(Into::into)
}

/// Spawns blocking work and returns its join handle without awaiting it.
pub fn spawn_blocking_handle<F, R>(f: F) -> JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    RongExecutor::global().spawn_blocking(f)
}
