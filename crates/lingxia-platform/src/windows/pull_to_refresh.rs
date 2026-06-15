use std::sync::{Arc, Mutex};

use crate::error::PlatformError;
use crate::traits::pull_to_refresh::PullToRefresh;

use super::Platform;

type WindowsPullToRefreshHandler = Arc<dyn Fn(String, String, bool) -> bool + Send + Sync>;
static WINDOWS_PULL_TO_REFRESH_HANDLER: Mutex<Option<WindowsPullToRefreshHandler>> =
    Mutex::new(None);

pub fn set_windows_pull_to_refresh_handler(handler: WindowsPullToRefreshHandler) {
    if let Ok(mut slot) = WINDOWS_PULL_TO_REFRESH_HANDLER.lock() {
        *slot = Some(handler);
    }
}

fn invoke_windows_pull_to_refresh_handler(app_id: &str, path: &str, refreshing: bool) -> bool {
    let handler = WINDOWS_PULL_TO_REFRESH_HANDLER
        .lock()
        .ok()
        .and_then(|slot| slot.clone());
    handler
        .map(|handler| handler(app_id.to_string(), path.to_string(), refreshing))
        .unwrap_or(false)
}

impl PullToRefresh for Platform {
    fn start_pull_down_refresh(&self, app_id: &str, path: &str) -> Result<(), PlatformError> {
        if invoke_windows_pull_to_refresh_handler(app_id, path, true) {
            Ok(())
        } else {
            Err(PlatformError::Platform(
                "Failed to start pull down refresh".to_string(),
            ))
        }
    }

    fn stop_pull_down_refresh(&self, app_id: &str, path: &str) -> Result<(), PlatformError> {
        if invoke_windows_pull_to_refresh_handler(app_id, path, false) {
            Ok(())
        } else {
            Err(PlatformError::Platform(
                "Failed to stop pull down refresh".to_string(),
            ))
        }
    }
}
