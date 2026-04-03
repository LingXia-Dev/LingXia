use crate::error::PlatformError;

/// Pull-to-refresh functionality for WebView pages.
///
/// This trait provides methods to programmatically control the pull-to-refresh
/// state for a specific page (identified by app_id and path).
pub trait PullToRefresh: Send + Sync {
    /// Start pull-to-refresh animation programmatically.
    ///
    /// This will show the refresh indicator and trigger the refresh callback
    /// (which calls the page's onPullDownRefresh lifecycle method).
    ///
    /// # Arguments
    /// * `app_id` - The application ID
    /// * `path` - The page path
    fn start_pull_down_refresh(&self, app_id: &str, path: &str) -> Result<(), PlatformError>;

    /// Stop pull-to-refresh animation.
    ///
    /// This should be called after the refresh operation is complete to hide
    /// the refresh indicator and reset the state.
    ///
    /// # Arguments
    /// * `app_id` - The application ID
    /// * `path` - The page path
    fn stop_pull_down_refresh(&self, app_id: &str, path: &str) -> Result<(), PlatformError>;
}
