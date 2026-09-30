use std::sync::{Arc, Mutex};

use crate::error::PlatformError;
use crate::traits::pull_to_refresh::PullToRefresh;

use super::Platform;

/// `(page webtag, refreshing)`; returns whether the indicator took the state.
type WindowsPullToRefreshHandler = Arc<dyn Fn(&str, bool) -> bool + Send + Sync>;
static WINDOWS_PULL_TO_REFRESH_HANDLER: Mutex<Option<WindowsPullToRefreshHandler>> =
    Mutex::new(None);

pub fn set_windows_pull_to_refresh_handler(handler: WindowsPullToRefreshHandler) {
    if let Ok(mut slot) = WINDOWS_PULL_TO_REFRESH_HANDLER.lock() {
        *slot = Some(handler);
    }
}

fn invoke_windows_pull_to_refresh_handler(webtag: &str, refreshing: bool) -> bool {
    let handler = WINDOWS_PULL_TO_REFRESH_HANDLER
        .lock()
        .ok()
        .and_then(|slot| slot.clone());
    handler.is_some_and(|handler| handler(webtag, refreshing))
}

impl PullToRefresh for Platform {
    fn start_pull_down_refresh(&self, _app_id: &str, webtag: &str) -> Result<(), PlatformError> {
        if invoke_windows_pull_to_refresh_handler(webtag, true) {
            Ok(())
        } else {
            Err(PlatformError::Platform(
                "Failed to start pull down refresh".to_string(),
            ))
        }
    }

    fn stop_pull_down_refresh(&self, _app_id: &str, webtag: &str) -> Result<(), PlatformError> {
        if invoke_windows_pull_to_refresh_handler(webtag, false) {
            Ok(())
        } else {
            Err(PlatformError::Platform(
                "Failed to stop pull down refresh".to_string(),
            ))
        }
    }
}
