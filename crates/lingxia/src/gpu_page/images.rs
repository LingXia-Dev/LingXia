//! Image loads a native shell requested; dropping a request's future cancels its transport.
use super::{Token, page_of};
use std::{
    collections::HashMap,
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicI64, Ordering},
    },
};
use tokio::sync::oneshot;

type Cancellation = (oneshot::Sender<()>, Option<oneshot::Receiver<()>>);

static REQUESTS: LazyLock<Mutex<HashMap<i64, Cancellation>>> = LazyLock::new(Default::default);
static NEXT: AtomicI64 = AtomicI64::new(1);

fn requests() -> std::sync::MutexGuard<'static, HashMap<i64, Cancellation>> {
    REQUESTS.lock().unwrap_or_else(|p| p.into_inner())
}

/// Reserve a request id before the platform hands it to its loader.
pub(crate) fn begin_image() -> i64 {
    let request = NEXT.fetch_add(1, Ordering::Relaxed);
    let (cancel, cancelled) = oneshot::channel();
    requests().insert(request, (cancel, Some(cancelled)));
    request
}

pub(crate) fn cancel_image(request: i64) {
    if let Some((cancel, _)) = requests().remove(&request) {
        let _ = cancel.send(());
    }
}

/// Load `source` for the page `token` belongs to on the runtime executor; `deliver`
/// runs once with the bytes, or `None` on failure. A cancelled request never delivers.
pub(crate) fn request_image(
    token: Token,
    request: i64,
    source: String,
    deliver: impl FnOnce(Option<Vec<u8>>) + Send + 'static,
) {
    let cancelled = requests()
        .get_mut(&request)
        .and_then(|(_, cancelled)| cancelled.take());
    let (Some(cancelled), Some(page)) = (cancelled, page_of(token)) else {
        requests().remove(&request);
        deliver(None);
        return;
    };
    std::mem::drop(rong_rt::RongExecutor::global().spawn(async move {
        let bytes = tokio::select! {
            biased;
            _ = cancelled => return,
            result = page.load_gpu_image_bytes(&source) => result,
        };
        requests().remove(&request);
        deliver(
            bytes
                .map_err(|error| log::warn!("GPU image resource failed: {error}"))
                .ok(),
        );
    }));
}
