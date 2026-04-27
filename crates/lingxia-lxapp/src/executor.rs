use std::future::Future;

use rong_rt::RongExecutor;
use tokio::task::JoinHandle;

pub(crate) fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    RongExecutor::global().spawn(future)
}
