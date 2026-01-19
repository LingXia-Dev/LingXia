use crate::error::PlatformError;
use crate::traits::pull_to_refresh::PullToRefresh;

use super::Platform;

#[cfg(target_os = "ios")]
use super::ffi::{start_pull_down_refresh, stop_pull_down_refresh};

#[cfg(target_os = "ios")]
impl PullToRefresh for Platform {
    fn start_pull_down_refresh(&self, app_id: &str, path: &str) -> Result<(), PlatformError> {
        let success = start_pull_down_refresh(app_id, path);
        if success {
            Ok(())
        } else {
            Err(PlatformError::Platform(
                "Failed to start pull down refresh".to_string(),
            ))
        }
    }

    fn stop_pull_down_refresh(&self, app_id: &str, path: &str) -> Result<(), PlatformError> {
        let success = stop_pull_down_refresh(app_id, path);
        if success {
            Ok(())
        } else {
            Err(PlatformError::Platform(
                "Failed to stop pull down refresh".to_string(),
            ))
        }
    }
}

#[cfg(not(target_os = "ios"))]
impl PullToRefresh for Platform {
    fn start_pull_down_refresh(&self, _app_id: &str, _path: &str) -> Result<(), PlatformError> {
        Err(PlatformError::Platform(
            "Pull-to-refresh not supported on this platform".to_string(),
        ))
    }

    fn stop_pull_down_refresh(&self, _app_id: &str, _path: &str) -> Result<(), PlatformError> {
        Err(PlatformError::Platform(
            "Pull-to-refresh not supported on this platform".to_string(),
        ))
    }
}
