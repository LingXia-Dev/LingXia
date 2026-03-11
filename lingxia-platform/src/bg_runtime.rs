use std::future::Future;
use std::sync::OnceLock;
use tokio::runtime::Handle;
use tokio::task::JoinHandle;

static BG_HANDLE: OnceLock<Handle> = OnceLock::new();

/// Called by the upper layer (lxapp init) to inject the shared tokio runtime handle.
pub(crate) fn set_handle(handle: Handle) {
    let _ = BG_HANDLE.set(handle);
}

/// Spawn an async task on the shared background runtime.
///
/// If the shared runtime is not initialized yet, this falls back to a detached thread
/// with a lightweight current-thread tokio runtime so work is not silently dropped.
pub(crate) fn spawn<F>(future: F) -> Option<JoinHandle<F::Output>>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    if let Some(handle) = BG_HANDLE.get() {
        return Some(handle.spawn(future));
    }

    log::warn!("[platform.bg] no runtime handle; using fallback thread");
    let thread_name = "lingxia-bg-fallback-async".to_string();
    let spawn_result = std::thread::Builder::new()
        .name(thread_name)
        .spawn(move || match tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
        {
            Ok(runtime) => {
                runtime.block_on(future);
            }
            Err(error) => {
                log::error!("[platform.bg] fallback async runtime create failed: {}", error);
            }
        });
    if let Err(error) = spawn_result {
        log::error!("[platform.bg] fallback async thread spawn failed: {}", error);
    }
    None
}

/// Spawn a blocking task on the shared background runtime.
///
/// If the shared runtime is not initialized yet, this falls back to a detached thread
/// so work is not silently dropped.
pub(crate) fn spawn_blocking<F, R>(f: F) -> Option<JoinHandle<R>>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    if let Some(handle) = BG_HANDLE.get() {
        return Some(handle.spawn_blocking(f));
    }

    log::warn!("[platform.bg] no runtime handle; using fallback thread");
    let thread_name = "lingxia-bg-fallback-blocking".to_string();
    let spawn_result = std::thread::Builder::new()
        .name(thread_name)
        .spawn(move || {
            let _ = f();
        });
    if let Err(error) = spawn_result {
        log::error!("[platform.bg] fallback blocking thread spawn failed: {}", error);
    }
    None
}
