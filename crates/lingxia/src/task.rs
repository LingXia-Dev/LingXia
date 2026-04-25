//! Async task helpers backed by LingXia's global `RongExecutor`.

use rong_rt::RongExecutor;

/// Re-export of `tokio` for consumers that want to share the same task types.
pub use tokio;
/// Re-exported task handle returned by LingXia task helpers.
pub use tokio::task::JoinHandle;

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
