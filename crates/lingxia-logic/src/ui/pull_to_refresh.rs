use crate::i18n::{js_error_from_business_code_with_detail, js_error_from_platform_error};
use lingxia_platform::traits::pull_to_refresh::PullToRefresh;
use lxapp::{LxApp, PageInstance};
use rong::JSResult;

/// Show `page`'s refresh indicator and run its `onRefresh`, as a
/// pull would. Throws `E_INVALID_STATE` (`data.bizCode === 4004`) unless the
/// page config sets `enablePullDownRefresh: true`.
pub(super) fn start(app: &LxApp, page: &PageInstance) -> JSResult<()> {
    if !page.is_pull_down_refresh_enabled() {
        return Err(js_error_from_business_code_with_detail(
            4004,
            "page.refresh requires enablePullDownRefresh: true in the page config",
        ));
    }
    app.runtime
        .start_pull_down_refresh(&app.appid, page.webtag().key())
        .map_err(|e| js_error_from_platform_error(&e))
}

/// Put `page`'s refresh indicator away.
pub(super) fn stop(app: &LxApp, page: &PageInstance) -> JSResult<()> {
    app.runtime
        .stop_pull_down_refresh(&app.appid, page.webtag().key())
        .map_err(|e| js_error_from_platform_error(&e))
}
