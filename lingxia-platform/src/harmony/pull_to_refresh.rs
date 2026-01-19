use crate::error::PlatformError;
use crate::traits::pull_to_refresh::PullToRefresh;
use lingxia_webview::tsfn;

use super::Platform;

impl PullToRefresh for Platform {
    fn start_pull_down_refresh(&self, app_id: &str, path: &str) -> Result<(), PlatformError> {
        tsfn::call_arkts("startPullDownRefresh", &[app_id, path])
            .map_err(|e| PlatformError::Platform(e.to_string()))
    }

    fn stop_pull_down_refresh(&self, app_id: &str, path: &str) -> Result<(), PlatformError> {
        tsfn::call_arkts("stopPullDownRefresh", &[app_id, path])
            .map_err(|e| PlatformError::Platform(e.to_string()))
    }
}
