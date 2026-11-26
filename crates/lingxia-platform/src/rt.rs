// Each per-OS backend uses a different subset of these helpers; compiling them
// everywhere avoids hand-maintained cfg lists that drift.
#![allow(dead_code)]

use lingxia_error::Error;
use rong_rt::RongExecutor;
use std::future::Future;
use tokio::task::JoinHandle;

pub(crate) fn spawn<F>(future: F) -> Option<JoinHandle<F::Output>>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    Some(RongExecutor::global().spawn(future))
}

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
/// `Success(json)` is `Ok(json)`, `Error(code)` the code's error, and a dropped receiver an
/// internal error. If the init closure fails, the callback is cleaned up automatically.
pub(crate) async fn native_call<F>(init: F) -> Result<String, Error>
where
    F: FnOnce(u64) -> Result<(), Error>,
{
    native_call_with(init, crate::error::callback_error).await
}

/// [`native_call`] with the error each numeric code becomes.
pub(crate) async fn native_call_with<F>(init: F, on_code: fn(u32) -> Error) -> Result<String, Error>
where
    F: FnOnce(u64) -> Result<(), Error>,
{
    let (callback_id, receiver) = lingxia_messaging::get_callback();
    if let Err(e) = init(callback_id) {
        lingxia_messaging::remove_callback(callback_id);
        return Err(e);
    }
    match receiver.await {
        Ok(lingxia_messaging::CallbackResult::Success(data)) => Ok(data),
        Ok(lingxia_messaging::CallbackResult::Error(code)) => Err(on_code(code)),
        Err(_) => Err(crate::error::callback_dropped()),
    }
}

/// Completion budget for awaited UI updates (see `native_call_ui`).
const UI_CALLBACK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// `native_call` for UI-apply confirmations (tabbar show/hide and friends).
///
/// The completion signal rides the platform's render loop, which can pause
/// outright — vsync stops when the app backgrounds (Harmony), the main queue
/// defers when suspended (Apple), a posted runnable can die with its activity
/// (Android) — and the registry keeps the callback sender alive, so a missed
/// completion would hang the JS promise forever. The state change itself was
/// already committed before dispatch, so on deadline the callback is
/// deregistered and the call resolves Ok: bounded resolution beats a hung
/// `await` over a purely visual confirmation.
pub(crate) async fn native_call_ui<F>(init: F) -> Result<(), Error>
where
    F: FnOnce(u64) -> Result<(), Error>,
{
    native_call_ui_with(init, crate::error::callback_error).await
}

/// [`native_call_ui`] with the error each numeric code becomes.
pub(crate) async fn native_call_ui_with<F>(init: F, on_code: fn(u32) -> Error) -> Result<(), Error>
where
    F: FnOnce(u64) -> Result<(), Error>,
{
    let (callback_id, receiver) = lingxia_messaging::get_callback();
    if let Err(e) = init(callback_id) {
        lingxia_messaging::remove_callback(callback_id);
        return Err(e);
    }
    match tokio::time::timeout(UI_CALLBACK_TIMEOUT, receiver).await {
        Ok(Ok(lingxia_messaging::CallbackResult::Success(_))) => Ok(()),
        Ok(Ok(lingxia_messaging::CallbackResult::Error(code))) => Err(on_code(code)),
        Ok(Err(_)) => Err(crate::error::callback_dropped()),
        Err(_) => {
            lingxia_messaging::remove_callback(callback_id);
            log::warn!("UI update confirmation timed out after {UI_CALLBACK_TIMEOUT:?}; resolving");
            Ok(())
        }
    }
}
