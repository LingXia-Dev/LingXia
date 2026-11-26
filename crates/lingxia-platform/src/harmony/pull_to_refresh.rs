use crate::traits::pull_to_refresh::PullToRefresh;
use lingxia_webview::platform::harmony::tsfn;

use super::Platform;
use lingxia_error::Error;

impl PullToRefresh for Platform {
    fn start_pull_down_refresh(&self, app_id: &str, webtag: &str) -> Result<(), Error> {
        tsfn::call_arkts("startPullDownRefresh", &[app_id, webtag])
            .map_err(|e| Error::internal(e.to_string()))
    }

    fn stop_pull_down_refresh(&self, app_id: &str, webtag: &str) -> Result<(), Error> {
        tsfn::call_arkts("stopPullDownRefresh", &[app_id, webtag])
            .map_err(|e| Error::internal(e.to_string()))
    }
}
