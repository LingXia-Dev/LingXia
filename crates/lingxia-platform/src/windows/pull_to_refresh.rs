//! Programmatic pull-to-refresh for the Windows desktop shell.
//!
//! `lx.startPullDownRefresh()` / `lx.stopPullDownRefresh()` reach the platform
//! through the [`PullToRefresh`] trait. The platform layer cannot depend on
//! lingxia-logic/lxapp, so the actual work — firing the page's
//! `onPullDownRefresh` lifecycle (via `on_lxapp_event`) and driving the refresh
//! indicator — is delivered through a callback the `lingxia` facade registers,
//! the same inversion the surface and app-exit seams use.

use std::sync::{Arc, Mutex};

use crate::error::PlatformError;
use crate::traits::pull_to_refresh::PullToRefresh;

use super::Platform;

/// `(app_id, path, start)` — `start = true` begins a refresh, `false` ends it.
type PullToRefreshHandler = Arc<dyn Fn(&str, &str, bool) + Send + Sync>;
static PULL_TO_REFRESH_HANDLER: Mutex<Option<PullToRefreshHandler>> = Mutex::new(None);

/// Registers the callback the `lingxia` facade wires to dispatch
/// `onPullDownRefresh` and show/hide the refresh indicator.
pub fn set_windows_pull_to_refresh_handler(handler: PullToRefreshHandler) {
    if let Ok(mut slot) = PULL_TO_REFRESH_HANDLER.lock() {
        *slot = Some(handler);
    }
}

fn notify(app_id: &str, path: &str, start: bool) -> Result<(), PlatformError> {
    // Clone the handler out before invoking it so the lock is released first —
    // the start handler dispatches onPullDownRefresh, which may re-enter with a
    // stop when the page has pull-down disabled.
    let handler = PULL_TO_REFRESH_HANDLER
        .lock()
        .ok()
        .and_then(|slot| slot.clone());
    match handler {
        Some(handler) => {
            handler(app_id, path, start);
            Ok(())
        }
        None => super::not_supported("pull_to_refresh"),
    }
}

impl PullToRefresh for Platform {
    fn start_pull_down_refresh(&self, app_id: &str, path: &str) -> Result<(), PlatformError> {
        notify(app_id, path, true)
    }

    fn stop_pull_down_refresh(&self, app_id: &str, path: &str) -> Result<(), PlatformError> {
        notify(app_id, path, false)
    }
}
