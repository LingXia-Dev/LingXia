//! Image loads for a native page: lxapp resolves the source under the owning
//! app's authority; dropping the handle drops the load future and its transport.
use super::{Token, page_of};
use tokio::sync::oneshot;

/// An in-flight load; dropping it cancels the load.
pub(crate) struct ImageLoad {
    _cancel: Option<oneshot::Sender<()>>,
}

/// Load `source` for the page `token` belongs to on the runtime executor;
/// `done` runs once, unless the returned handle is dropped first.
pub(crate) fn load_image(
    token: Token,
    source: String,
    done: impl FnOnce(Result<Vec<u8>, String>) + Send + 'static,
) -> ImageLoad {
    let Some(page) = page_of(token) else {
        done(Err("UI_IMAGE: page closed".into()));
        return ImageLoad { _cancel: None };
    };
    let (cancel, cancelled) = oneshot::channel::<()>();
    std::mem::drop(rong_rt::RongExecutor::global().spawn(async move {
        let result = tokio::select! {
            biased;
            // Sent or dropped: either way the owner gave up on this load.
            _ = cancelled => return,
            result = page.load_gpu_image_bytes(&source) => result,
        };
        done(result.map_err(|error| {
            log::warn!("GPU image resource failed: {error}");
            error.to_string()
        }));
    }));
    ImageLoad {
        _cancel: Some(cancel),
    }
}
