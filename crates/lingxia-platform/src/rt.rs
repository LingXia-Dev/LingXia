use crate::error::PlatformError;
use rong_rt::RongExecutor;
#[cfg(any(target_os = "ios", target_os = "macos", target_env = "ohos"))]
use std::future::Future;
#[cfg(any(
    target_os = "ios",
    target_os = "macos",
    target_os = "windows",
    target_env = "ohos"
))]
use tokio::task::JoinHandle;

#[cfg(any(target_os = "ios", target_os = "macos", target_env = "ohos"))]
pub(crate) fn spawn<F>(future: F) -> Option<JoinHandle<F::Output>>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    Some(RongExecutor::global().spawn(future))
}

#[cfg(any(target_env = "ohos", target_os = "macos", target_os = "windows"))]
pub(crate) fn spawn_blocking<F, R>(f: F) -> Option<JoinHandle<R>>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    Some(RongExecutor::global().spawn_blocking(f))
}

pub(crate) async fn blocking<F, R>(f: F) -> R
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    RongExecutor::global()
        .spawn_blocking(f)
        .await
        .expect("blocking task panicked")
}

/// Create a oneshot callback, execute an init closure with the callback_id,
/// then await the native callback result.
///
/// Converts `CallbackResult` into `Result<String, PlatformError>`:
/// - `Success(json)` -> `Ok(json)`
/// - `Error(code)` -> `Err(PlatformError::BusinessError(code))`
/// - Receiver dropped -> `Err(PlatformError::CallbackDropped)`
///
/// If the init closure fails, the callback is cleaned up automatically.
pub(crate) async fn native_call<F>(init: F) -> Result<String, PlatformError>
where
    F: FnOnce(u64) -> Result<(), PlatformError>,
{
    let (callback_id, receiver) = lingxia_messaging::get_callback();
    if let Err(e) = init(callback_id) {
        lingxia_messaging::remove_callback(callback_id);
        return Err(e);
    }
    match receiver.await {
        Ok(lingxia_messaging::CallbackResult::Success(data)) => Ok(data),
        Ok(lingxia_messaging::CallbackResult::Error(code)) => {
            Err(PlatformError::BusinessError(code))
        }
        Err(_) => Err(PlatformError::CallbackDropped),
    }
}
