/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! LingXia patch: top-level navigation outcomes that Servo's embedder API
//! does not expose — the main-frame load failure and the download response.

use std::sync::{Arc, OnceLock, RwLock};

use http::HeaderMap;
use net_traits::NetworkError;
use net_traits::request::{Destination, Request, RequestMode};
use net_traits::response::Response;
use servo_base::id::WebViewId;
use servo_url::ServoUrl;

/// Callbacks run on Servo networking threads and must not block.
pub trait NavigationObserver: Send + Sync + 'static {
    /// A top-level navigation fetch ended in a network error. Servo still
    /// renders its own error document for it.
    fn navigation_failed(&self, webview_id: WebViewId, url: &ServoUrl, error: &NetworkError);

    /// Return true to take a top-level navigation response as a download.
    /// The fetch is then cancelled, which leaves the current document shown.
    fn claim_download(
        &self,
        webview_id: WebViewId,
        url: &ServoUrl,
        status: u16,
        headers: &HeaderMap,
    ) -> bool;
}

static NAVIGATION_OBSERVER: OnceLock<RwLock<Option<Arc<dyn NavigationObserver>>>> =
    OnceLock::new();

fn navigation_observer() -> Option<Arc<dyn NavigationObserver>> {
    NAVIGATION_OBSERVER
        .get_or_init(|| RwLock::new(None))
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
}

/// Install or remove the process-wide navigation observer.
pub fn set_navigation_observer(observer: Option<Arc<dyn NavigationObserver>>) {
    *NAVIGATION_OBSERVER
        .get_or_init(|| RwLock::new(None))
        .write()
        .unwrap_or_else(|error| error.into_inner()) = observer;
}

/// Returns the response to continue with, and whether a download claimed it.
pub(crate) fn observe_navigation_response(
    request: &Request,
    response: Response,
) -> (Response, bool) {
    if !matches!(request.destination, Destination::Document)
        || !matches!(request.mode, RequestMode::Navigate)
    {
        return (response, false);
    }
    let (Some(webview_id), Some(observer)) = (request.target_webview_id, navigation_observer())
    else {
        return (response, false);
    };
    let url = request.current_url();
    if let Some(error) = response.get_network_error() {
        if !matches!(error, NetworkError::LoadCancelled) {
            observer.navigation_failed(webview_id, &url, error);
        }
        return (response, false);
    }
    if observer.claim_download(webview_id, &url, response.status.raw_code(), &response.headers) {
        return (Response::network_error(NetworkError::LoadCancelled), true);
    }
    (response, false)
}
