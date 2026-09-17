//! Live-WebView budget for hidden-main lxapp tab pages.
//!
//! Browser tabs already discard under a memory cap. Tab pages are path-pinned
//! singletons that `switchTab` only hides, so a 3×10 workspace would otherwise
//! keep every visited View resident. Discard is not close: Logic, `data`, the
//! switcher row, and the instance stay; only the native WebView goes away.

use super::host_class::{HostClass, host_class};
use super::runtime_registry::{get_lxapps_manager, get_platform};
use super::{HOST_SURFACE_OWNER_APP_ID, LxAppSessionStatus};
use crate::{LxApp, debug, warn};
use lingxia_platform::traits::device::DeviceHardware;
use std::collections::HashSet;
use std::time::{Duration, Instant};

const ESTIMATED_PAGE_WEBVIEWS_SHARE: u64 = 4;
const ESTIMATED_PAGE_WEBVIEW_BYTES: u64 = 256 * 1024 * 1024;
const MIN_LIVE_PAGE_WEBVIEWS: usize = 4;
const MAX_LIVE_PAGE_WEBVIEWS: usize = 16;
const DEFAULT_LIVE_PAGE_WEBVIEWS: usize = 8;
/// A tab this long untouched in a hidden main is two picks away; its reload
/// costs less than keeping the View resident.
const IDLE_DISCARD_AFTER: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Clone)]
pub(crate) struct LivePageSnapshot {
    pub instance_id: String,
    pub last_active: Instant,
    pub shown_app: bool,
    pub on_stack: bool,
    pub is_tab_page: bool,
    pub isolated: bool,
    pub departing: bool,
    pub live: bool,
}

/// Hidden-main tab pages to reclaim, oldest first: everything idle past
/// `IDLE_DISCARD_AFTER`, then as many more as the live count exceeds `limit`.
///
/// Eligible: an off-stack tab page of a hidden lxapp. Protected: every page of
/// a shown lxapp, every page still on a hidden main's stack (resume page and
/// anything under it), and non-tab instances. Isolated / departing pages are
/// never candidates.
pub(crate) fn discard_candidates(
    pages: &[LivePageSnapshot],
    limit: usize,
    now: Instant,
) -> Vec<String> {
    let live_count = pages.iter().filter(|page| page.live).count();
    let excess = live_count.saturating_sub(limit);
    let mut eligible: Vec<&LivePageSnapshot> = pages
        .iter()
        .filter(|page| {
            page.live
                && page.is_tab_page
                && !page.isolated
                && !page.departing
                && !page.shown_app
                && !page.on_stack
        })
        .collect();
    eligible.sort_by_key(|page| page.last_active);
    let idle = eligible
        .iter()
        .filter(|page| now.saturating_duration_since(page.last_active) >= IDLE_DISCARD_AFTER)
        .count();
    eligible
        .into_iter()
        .take(excess.max(idle))
        .map(|page| page.instance_id.clone())
        .collect()
}

pub(crate) fn live_page_webview_limit_for_memory(total_physical_bytes: u64) -> usize {
    ((total_physical_bytes / ESTIMATED_PAGE_WEBVIEWS_SHARE) / ESTIMATED_PAGE_WEBVIEW_BYTES)
        .try_into()
        .unwrap_or(usize::MAX)
        .clamp(MIN_LIVE_PAGE_WEBVIEWS, MAX_LIVE_PAGE_WEBVIEWS)
}

fn live_page_webview_limit() -> usize {
    get_platform()
        .and_then(|platform| platform.get_memory_info().ok())
        .map(live_page_webview_limit_for_memory)
        .unwrap_or(DEFAULT_LIVE_PAGE_WEBVIEWS)
}

/// Drop hidden-main tab WebViews until the live count fits the memory budget.
pub fn enforce_page_webview_budget() {
    enforce_page_webview_budget_with_limit(live_page_webview_limit());
}

/// `limit == 0` discards every eligible page (critical memory pressure).
pub fn enforce_page_webview_budget_with_limit(limit: usize) {
    // Compact hosts keep native page containers bound to their WebViews.
    if host_class() != HostClass::Desktop {
        return;
    }
    let Some(manager) = get_lxapps_manager() else {
        return;
    };
    let apps: Vec<std::sync::Arc<LxApp>> = manager
        .lxapps
        .iter()
        .filter(|entry| entry.key().as_str() != HOST_SURFACE_OWNER_APP_ID)
        .filter(|entry| {
            matches!(
                entry.value().status(),
                LxAppSessionStatus::Opened | LxAppSessionStatus::Opening
            )
        })
        .map(|entry| entry.value().clone())
        .collect();

    let mut snapshots = Vec::new();
    let mut pages_by_id = Vec::new();
    for app in &apps {
        let shown = app.is_shown();
        let stack_ids: HashSet<String> = app
            .get_page_stack_pages()
            .into_iter()
            .map(|page| page.instance_id_string())
            .collect();
        let hidden_since = app.hidden_since();
        for page in app.live_page_instances() {
            // Idle counts from the app leaving the screen: a tab of the app
            // the user sat in is one pick away however long ago it was shown.
            let last_active = page.get_last_active_time().unwrap_or_else(Instant::now);
            snapshots.push(LivePageSnapshot {
                instance_id: page.instance_id_string(),
                last_active: hidden_since.map_or(last_active, |since| since.max(last_active)),
                shown_app: shown,
                on_stack: stack_ids.contains(&page.instance_id_string()),
                is_tab_page: page.is_tabbar_page(),
                isolated: page.is_isolated(),
                departing: page.document_is_departing(),
                live: page.has_live_webview(),
            });
            pages_by_id.push(page);
        }
    }

    for instance_id in discard_candidates(&snapshots, limit, Instant::now()) {
        let Some(page) = pages_by_id
            .iter()
            .find(|page| page.instance_id_string() == instance_id)
        else {
            continue;
        };
        if let Err(error) = page.discard_webview() {
            warn!("failed to discard hidden-main tab WebView: {error}")
                .with_appid(page.appid())
                .with_path(page.path());
        } else {
            debug!("discarded hidden-main tab WebView")
                .with_appid(page.appid())
                .with_path(page.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(
        id: &str,
        age_secs: u64,
        shown_app: bool,
        on_stack: bool,
        is_tab_page: bool,
        live: bool,
    ) -> LivePageSnapshot {
        LivePageSnapshot {
            instance_id: id.to_string(),
            last_active: Instant::now() - Duration::from_secs(age_secs),
            shown_app,
            on_stack,
            is_tab_page,
            isolated: false,
            departing: false,
            live,
        }
    }

    #[test]
    fn shown_app_pages_are_never_candidates() {
        let pages = [
            page("a1", 90, true, true, true, true),
            page("a2", 80, true, false, true, true),
            page("a3", 70, true, false, true, true),
        ];
        assert!(discard_candidates(&pages, 1, Instant::now()).is_empty());
    }

    #[test]
    fn hidden_off_stack_tabs_are_discarded_oldest_first() {
        let pages = [
            page("home", 10, false, true, true, true),
            page("guest-current", 5, false, true, true, true),
            page("guest-old", 50, false, false, true, true),
            page("guest-older", 80, false, false, true, true),
        ];
        assert_eq!(
            discard_candidates(&pages, 2, Instant::now()),
            vec!["guest-older".to_string(), "guest-old".to_string()]
        );
    }

    #[test]
    fn hidden_stack_under_a_detail_stays_protected() {
        let pages = [
            page("tab-root", 40, false, true, true, true),
            page("detail", 5, false, true, false, true),
            page("other-tab", 90, false, false, true, true),
        ];
        assert_eq!(
            discard_candidates(&pages, 2, Instant::now()),
            vec!["other-tab".to_string()]
        );
    }

    #[test]
    fn isolated_departing_and_non_tab_pages_are_not_candidates() {
        let mut isolated = page("iso", 90, false, false, true, true);
        isolated.isolated = true;
        let mut departing = page("gone", 80, false, false, true, true);
        departing.departing = true;
        let pages = [
            isolated,
            departing,
            page("warm-detail", 70, false, false, false, true),
            page("other-tab", 60, false, false, true, true),
        ];
        assert_eq!(
            discard_candidates(&pages, 1, Instant::now()),
            vec!["other-tab".to_string()]
        );
    }

    #[test]
    fn under_the_cap_nothing_is_discarded() {
        let pages = [
            page("current", 1, false, true, true, true),
            page("other", 20, false, false, true, true),
        ];
        assert!(discard_candidates(&pages, 4, Instant::now()).is_empty());
    }

    #[test]
    fn idle_hidden_tabs_go_even_under_the_cap() {
        let pages = [
            page("current", 1, false, true, true, true),
            page("recent", 60, false, false, true, true),
            page("idle", 11 * 60, false, false, true, true),
            page("idle-shown", 20 * 60, true, false, true, true),
        ];
        assert_eq!(
            discard_candidates(&pages, 16, Instant::now()),
            vec!["idle".to_string()]
        );
    }

    #[test]
    fn memory_limit_clamps_like_the_browser_budget() {
        let four_gib = 4u64 * 1024 * 1024 * 1024;
        assert_eq!(live_page_webview_limit_for_memory(four_gib), 4);
        let eight_gib = 8u64 * 1024 * 1024 * 1024;
        assert_eq!(live_page_webview_limit_for_memory(eight_gib), 8);
        let sixty_four_gib = 64u64 * 1024 * 1024 * 1024;
        assert_eq!(live_page_webview_limit_for_memory(sixty_four_gib), 16);
        assert_eq!(live_page_webview_limit_for_memory(0), 4);
    }
}
