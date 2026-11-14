//! Live GPU surfaces for hidden pages.
//!
//! A hidden GPU page that keeps its surface returns to the screen from its last
//! presented frame: no re-attach, no View rebuild. Each surface costs its
//! swapchain, about 32 MB for a 1080×2504 phone triple-buffered. The shown
//! app's tab pages all keep theirs (a tab bar is cycled, which defeats any
//! recency cap smaller than it); other hidden pages on the stack keep the most
//! recently shown few. A page off the stack keeps none, and the page just under
//! the top always keeps its own: a back gesture reveals it. Memory pressure
//! releases every other one. Platforms only execute.

use super::runtime_registry::get_lxapps_manager;
use super::{HOST_SURFACE_OWNER_APP_ID, LxAppSessionStatus};
use crate::page::PageView;
use crate::warn;
use std::time::Instant;

pub(crate) const RETAINED_SURFACES: usize = 3;

#[derive(Debug, Clone)]
pub(crate) struct SurfaceSnapshot {
    pub instance_id: String,
    /// `None` for a page never shown, which has no surface to keep.
    pub last_shown: Option<Instant>,
    pub visible: bool,
    /// A tab page of the shown app.
    pub tab: bool,
    /// On its app's page stack; a page that left it is not coming back as itself.
    pub on_stack: bool,
    /// Just under the top of the shown app's stack: a back gesture shows it.
    pub revealing: bool,
}

/// For each hidden page that was shown, whether it keeps its surface. `limit`
/// bounds stacked pages outside the shown app's tab bar; 0 (memory pressure)
/// releases all but the revealing page.
pub(crate) fn decide(pages: &[SurfaceSnapshot], limit: usize) -> Vec<(String, bool)> {
    let shown = |page: &&SurfaceSnapshot| !page.visible && page.last_shown.is_some();
    let mut decided: Vec<(String, bool)> = pages
        .iter()
        .filter(shown)
        .filter(|page| page.revealing)
        .map(|page| (page.instance_id.clone(), true))
        .collect();
    decided.extend(
        pages
            .iter()
            .filter(shown)
            .filter(|page| !page.revealing && page.tab)
            .map(|page| (page.instance_id.clone(), limit > 0)),
    );
    decided.extend(
        pages
            .iter()
            .filter(shown)
            .filter(|page| !page.revealing && !page.tab && !page.on_stack)
            .map(|page| (page.instance_id.clone(), false)),
    );
    let mut hidden: Vec<(&SurfaceSnapshot, Instant)> = pages
        .iter()
        .filter(|page| !page.visible && !page.tab && page.on_stack && !page.revealing)
        .filter_map(|page| Some((page, page.last_shown?)))
        .collect();
    hidden.sort_by_key(|page| std::cmp::Reverse(page.1));
    decided.extend(
        hidden
            .into_iter()
            .enumerate()
            .map(|(rank, (page, _))| (page.instance_id.clone(), rank < limit)),
    );
    decided
}

/// Re-decide after a page became visible or active.
pub fn enforce_surface_retention() {
    enforce_with_limit(RETAINED_SURFACES);
}

/// Memory pressure: every hidden page gives up its surface.
pub fn release_retained_surfaces() {
    enforce_with_limit(0);
}

fn enforce_with_limit(limit: usize) {
    let Some(manager) = get_lxapps_manager() else {
        return;
    };
    let mut pages = Vec::new();
    for entry in manager.lxapps.iter() {
        if entry.key().as_str() == HOST_SURFACE_OWNER_APP_ID
            || !matches!(
                entry.value().status(),
                LxAppSessionStatus::Opened | LxAppSessionStatus::Opening
            )
        {
            continue;
        }
        let shown = entry.value().is_shown();
        let stack = entry.value().get_page_stack();
        let revealing = stack
            .len()
            .checked_sub(2)
            .and_then(|under| stack.get(under))
            .filter(|_| shown)
            .cloned();
        for page in entry.value().live_page_instances() {
            if let Some(PageView::Gpu(view)) = page.page_view() {
                let snapshot = SurfaceSnapshot {
                    instance_id: page.instance_id_string(),
                    last_shown: page.last_shown(),
                    visible: shown && page.is_visible(),
                    tab: shown && page.is_tabbar_page(),
                    on_stack: stack.contains(&page.instance_id_string()),
                    revealing: revealing.as_deref() == Some(page.instance_id_string().as_str()),
                };
                pages.push((snapshot, view, page));
            }
        }
    }
    let snapshots: Vec<SurfaceSnapshot> = pages.iter().map(|(s, ..)| s.clone()).collect();
    for (id, retained) in decide(&snapshots, limit) {
        let Some((snapshot, view, page)) = pages.iter().find(|(s, ..)| s.instance_id == id) else {
            continue;
        };
        let mut result = view.retain_surface(retained);
        // A hidden tab keeps its last frame for an instant switch, not the targets to draw a new one.
        if retained && snapshot.tab && result.is_ok() {
            result = view.trim_surface();
        }
        if let Err(error) = result {
            warn!("GPU surface retention failed: {error}")
                .with_appid(page.appid())
                .with_path(page.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn page(id: &str, age: u64, visible: bool) -> SurfaceSnapshot {
        SurfaceSnapshot {
            instance_id: id.into(),
            last_shown: Some(Instant::now() - Duration::from_secs(age)),
            visible,
            tab: false,
            on_stack: true,
            revealing: false,
        }
    }

    #[test]
    fn the_shown_apps_tab_pages_all_keep_their_surfaces() {
        let tab = |id, age| SurfaceSnapshot {
            tab: true,
            ..page(id, age, false)
        };
        let pages = [
            page("current", 0, true),
            tab("t1", 5),
            tab("t2", 10),
            tab("t3", 20),
            tab("t4", 30),
            page("detail", 40, false),
        ];
        let decided = decide(&pages, 3);
        assert!(
            decided
                .iter()
                .filter(|(id, _)| id.starts_with('t'))
                .all(|(_, kept)| *kept)
        );
        assert!(decided.contains(&("detail".to_string(), true)));
        assert!(
            decide(&pages, 0).iter().all(|(_, kept)| !kept),
            "memory pressure releases tabs too"
        );
    }

    #[test]
    fn the_most_recent_hidden_pages_keep_their_surfaces() {
        let pages = [
            page("current", 0, true),
            page("a", 5, false),
            page("b", 10, false),
            page("c", 20, false),
            page("d", 30, false),
        ];
        assert_eq!(
            decide(&pages, 3),
            [
                ("a".to_string(), true),
                ("b".to_string(), true),
                ("c".to_string(), true),
                ("d".to_string(), false),
            ]
        );
    }

    #[test]
    fn pages_never_shown_take_no_slot() {
        let mut preloaded = page("preloaded", 0, false);
        preloaded.last_shown = None;
        let pages = [page("current", 0, true), preloaded, page("a", 5, false)];
        assert_eq!(decide(&pages, 1), [("a".to_string(), true)]);
    }

    #[test]
    fn memory_pressure_releases_every_hidden_surface() {
        let pages = [page("current", 0, true), page("a", 5, false)];
        assert_eq!(decide(&pages, 0), [("a".to_string(), false)]);
    }

    #[test]
    fn a_page_off_the_stack_takes_no_slot_and_keeps_no_surface() {
        let popped = SurfaceSnapshot {
            on_stack: false,
            ..page("popped", 1, false)
        };
        let pages = [
            page("current", 0, true),
            popped,
            page("a", 5, false),
            page("b", 10, false),
        ];
        let decided = decide(&pages, 2);
        assert!(decided.contains(&("popped".to_string(), false)));
        assert!(decided.contains(&("a".to_string(), true)));
        assert!(
            decided.contains(&("b".to_string(), true)),
            "a popped page took a stacked page's slot"
        );
    }

    #[test]
    fn the_page_a_back_gesture_reveals_keeps_its_surface_even_under_pressure() {
        let under = SurfaceSnapshot {
            revealing: true,
            ..page("under", 50, false)
        };
        let pages = [
            page("current", 0, true),
            page("a", 5, false),
            page("b", 10, false),
            under,
        ];
        assert!(decide(&pages, 2).contains(&("under".to_string(), true)));
        let pressed = decide(&pages, 0);
        assert!(pressed.contains(&("under".to_string(), true)));
        assert!(pressed.contains(&("a".to_string(), false)));
    }
}
