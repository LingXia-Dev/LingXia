use crate::error::PlatformError;
use crate::traits::pull_to_refresh::PullToRefresh;

use super::Platform;

impl PullToRefresh for Platform {
    fn start_pull_down_refresh(&self, _app_id: &str, _path: &str) -> Result<(), PlatformError> {
        super::not_supported("start_pull_down_refresh")
    }

    fn stop_pull_down_refresh(&self, _app_id: &str, _path: &str) -> Result<(), PlatformError> {
        super::not_supported("stop_pull_down_refresh")
    }
}
