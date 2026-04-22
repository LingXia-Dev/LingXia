use rong::RongExecutor;

pub use tokio;
pub use tokio::task::JoinHandle;

pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    RongExecutor::global().spawn(future)
}

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

pub fn spawn_blocking_handle<F, R>(f: F) -> JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    RongExecutor::global().spawn_blocking(f)
}
