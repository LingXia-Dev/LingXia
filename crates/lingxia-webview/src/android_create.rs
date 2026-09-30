//! WebView creations Android has been asked for and not yet answered, keyed
//! by the request id the Java side echoes back.
use crate::WebViewError;
use crate::webview::{EffectiveWebViewCreateOptions, WebViewCreateSender, WebViewCreateStage};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

pub(crate) struct PendingWebViewCreation {
    pub(crate) sender: WebViewCreateSender,
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub(crate) effective_options: EffectiveWebViewCreateOptions,
}

static PENDING: OnceLock<Mutex<HashMap<u64, PendingWebViewCreation>>> = OnceLock::new();
static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

fn pending() -> std::sync::MutexGuard<'static, HashMap<u64, PendingWebViewCreation>> {
    PENDING
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Park `sender` until Java answers; returns the request id to hand to Java.
pub(crate) fn register(
    sender: WebViewCreateSender,
    effective_options: EffectiveWebViewCreateOptions,
) -> u64 {
    let request_id = NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
    pending().insert(
        request_id,
        PendingWebViewCreation {
            sender,
            effective_options,
        },
    );
    request_id
}

pub(crate) fn take(request_id: u64) -> Option<PendingWebViewCreation> {
    pending().remove(&request_id)
}

/// Resolve a parked creation as failed. False when the request was already
/// answered.
pub(crate) fn fail(request_id: u64, message: String) -> bool {
    let Some(creation) = take(request_id) else {
        return false;
    };
    creation.sender.fail(
        WebViewCreateStage::Requested,
        WebViewError::WebView(message),
    );
    true
}
