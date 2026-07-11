//! Global website-data operations backed by the shared WebView2 profile.

use std::collections::HashSet;

use crate::WebViewError;
use crate::webview::first_browser_webview;

#[derive(Debug, Clone, Copy)]
pub(crate) enum BrowsingDataKind {
    Cache,
    SiteData,
}

fn live_webview() -> Result<std::sync::Arc<crate::WebView>, WebViewError> {
    first_browser_webview().ok_or_else(|| {
        WebViewError::WebView("website data requires a live WebView2 profile".to_string())
    })
}

/// WebView2 does not expose an origin count for its HTTP cache. Clearing is
/// supported, but usage is reported as zero rather than treating the whole
/// privacy surface as unsupported.
pub async fn cache_site_count() -> Result<usize, WebViewError> {
    let _ = live_webview()?;
    Ok(0)
}

pub async fn site_data_usage() -> Result<(usize, usize), WebViewError> {
    let cookies = live_webview()?.list_cookies().await?;
    let sites = cookies
        .iter()
        .map(|cookie| cookie.domain.trim_start_matches('.').to_ascii_lowercase())
        .filter(|domain| !domain.is_empty())
        .collect::<HashSet<_>>()
        .len();
    Ok((sites, cookies.len()))
}

pub async fn clear_cache() -> Result<(), WebViewError> {
    clear_cache_since(None).await
}

pub async fn clear_site_data() -> Result<(), WebViewError> {
    clear_site_data_since(None).await
}

pub async fn clear_cache_since(since_unix_ms: Option<u64>) -> Result<(), WebViewError> {
    live_webview()?
        .inner
        .clear_profile_data(BrowsingDataKind::Cache, since_unix_ms)
}

pub async fn clear_site_data_since(since_unix_ms: Option<u64>) -> Result<(), WebViewError> {
    live_webview()?
        .inner
        .clear_profile_data(BrowsingDataKind::SiteData, since_unix_ms)
}
