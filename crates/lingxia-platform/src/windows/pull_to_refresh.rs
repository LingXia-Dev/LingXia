use std::sync::{Arc, Mutex};

use crate::traits::gesture::PullRefreshIndicator;
use crate::traits::pull_to_refresh::PullToRefresh;

use super::Platform;
use lingxia_error::Error;

/// `(page webtag, refreshing)`; returns whether the indicator took the state.
type WindowsPullToRefreshHandler = Arc<dyn Fn(&str, bool) -> bool + Send + Sync>;
static WINDOWS_PULL_TO_REFRESH_HANDLER: Mutex<Option<WindowsPullToRefreshHandler>> =
    Mutex::new(None);

pub fn set_windows_pull_to_refresh_handler(handler: WindowsPullToRefreshHandler) {
    if let Ok(mut slot) = WINDOWS_PULL_TO_REFRESH_HANDLER.lock() {
        *slot = Some(handler);
    }
}

/// A step of a pull on a page that measures its own (a GPU page), for the host to draw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PagePull {
    /// Travel in page points after the rubber band, and whether it is past the trigger.
    Moved { travel: f64, armed: bool },
    /// Dropped short of the trigger, or taken away.
    Cancelled,
    /// Let go past the trigger: the page refreshes.
    Released,
}

type PagePullHandler = Arc<dyn Fn(&str, PagePull) + Send + Sync>;
static PAGE_PULL_HANDLER: Mutex<Option<PagePullHandler>> = Mutex::new(None);

type IndicatorReader = Arc<dyn Fn(&str) -> Option<PullRefreshIndicator> + Send + Sync>;
static INDICATOR_READER: Mutex<Option<IndicatorReader>> = Mutex::new(None);

pub fn set_windows_page_pull_handler(handler: PagePullHandler) {
    *PAGE_PULL_HANDLER.lock().unwrap_or_else(|p| p.into_inner()) = Some(handler);
}

/// How the host reads a page's indicator; `None` from it when the page has none.
pub fn set_windows_pull_refresh_indicator_reader(reader: IndicatorReader) {
    *INDICATOR_READER.lock().unwrap_or_else(|p| p.into_inner()) = Some(reader);
}

/// Hands a step of `webtag`'s pull to the host.
pub fn report_windows_page_pull(webtag: &str, pull: PagePull) {
    let handler = PAGE_PULL_HANDLER
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    if let Some(handler) = handler {
        handler(webtag, pull);
    }
}

/// Unsupported when no host reads indicators.
pub fn windows_pull_refresh_indicator(webtag: &str) -> Result<Option<PullRefreshIndicator>, Error> {
    let reader = INDICATOR_READER
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
        .ok_or_else(|| Error::not_supported("this host has no pull-to-refresh indicator"))?;
    Ok(reader(webtag))
}

pub(crate) fn has_indicator_reader() -> bool {
    INDICATOR_READER
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .is_some()
}

fn invoke_windows_pull_to_refresh_handler(webtag: &str, refreshing: bool) -> bool {
    let handler = WINDOWS_PULL_TO_REFRESH_HANDLER
        .lock()
        .ok()
        .and_then(|slot| slot.clone());
    handler.is_some_and(|handler| handler(webtag, refreshing))
}

impl PullToRefresh for Platform {
    fn start_pull_down_refresh(&self, _app_id: &str, webtag: &str) -> Result<(), Error> {
        if invoke_windows_pull_to_refresh_handler(webtag, true) {
            Ok(())
        } else {
            Err(Error::internal(
                "Failed to start pull down refresh".to_string(),
            ))
        }
    }

    fn stop_pull_down_refresh(&self, _app_id: &str, webtag: &str) -> Result<(), Error> {
        if invoke_windows_pull_to_refresh_handler(webtag, false) {
            Ok(())
        } else {
            Err(Error::internal(
                "Failed to stop pull down refresh".to_string(),
            ))
        }
    }
}
