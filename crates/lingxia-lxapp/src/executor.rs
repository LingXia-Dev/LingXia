use std::future::Future;

use rong::RongExecutor;
use tokio::task::JoinHandle;

pub(crate) fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    RongExecutor::global().spawn(future)
}

pub(crate) fn spawn_blocking<F, T>(func: F) -> JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    RongExecutor::global().spawn_blocking(func)
}
