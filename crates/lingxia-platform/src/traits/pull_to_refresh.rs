use crate::error::PlatformError;

/// Programmatic control of a page's pull-to-refresh indicator.
///
/// `webtag` is the page instance's full webview tag: two instances of one
/// route each have their own indicator.
pub trait PullToRefresh: Send + Sync {
    /// Show the refresh indicator and run the page's `onPullDownRefresh`.
    fn start_pull_down_refresh(&self, app_id: &str, webtag: &str) -> Result<(), PlatformError>;

    /// Hide the refresh indicator once the refresh is done.
    fn stop_pull_down_refresh(&self, app_id: &str, webtag: &str) -> Result<(), PlatformError>;
}
