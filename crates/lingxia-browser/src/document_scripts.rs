//! Browser-owned scripts injected into every successfully loaded tab document
//! (e.g. the context menu). Registration is native-only and sealed at startup.

use lxapp::LxAppError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

static SCRIPTS: Mutex<Vec<Arc<str>>> = Mutex::new(Vec::new());
static SEALED: AtomicBool = AtomicBool::new(false);

pub(crate) fn register(js: impl Into<String>) -> Result<(), LxAppError> {
    if SEALED.load(Ordering::Acquire) {
        return Err(LxAppError::UnsupportedOperation(
            "browser document scripts are sealed".to_string(),
        ));
    }
    SCRIPTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(Arc::from(js.into()));
    Ok(())
}

pub(crate) fn seal() {
    SEALED.store(true, Ordering::Release);
}

/// The registered scripts, in registration order.
pub(crate) fn snapshot() -> Vec<Arc<str>> {
    SCRIPTS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}
