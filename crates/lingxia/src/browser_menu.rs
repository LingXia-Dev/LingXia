//! The menu of a main-area browser tab, and its move into the side panel.
//!
//! A browser tab is not a graph surface, so its menu is keyed by the browser
//! tab id. It reads like every other main-area row: page actions, "Open in
//! Side Panel", then the close family. Platforms render it with the shared
//! surface menu renderer and run every action over their own tab strip.

use lingxia_shell::{
    BrowserSurfaceMenuAction as Action, SurfaceMenuContext, SurfaceMenuItem as Item,
    SurfaceMenuParts, SurfaceMenuSnapshot,
};
use lxapp::LxAppError;

/// The menu for `tab_id`. `ordered_tab_ids` is the platform's main-area tab
/// order, which the close family follows.
pub fn browser_tab_menu(ordered_tab_ids: &[String], tab_id: &str) -> Option<SurfaceMenuSnapshot> {
    let index = ordered_tab_ids.iter().position(|id| id == tab_id)?;
    let url = lingxia_browser::tabs()
        .into_iter()
        .find(|tab| tab.tab_id == tab_id)
        .and_then(|tab| tab.current_url)
        .unwrap_or_default();
    Some(lingxia_shell::compose_surface_menu_parts(
        SurfaceMenuContext {
            revision: 0,
            surface_id: tab_id.to_string(),
            closable: true,
            renameable: false,
            title_overridden: false,
            has_other_closable: ordered_tab_ids.len() > 1,
            has_closable_before: index > 0,
            has_closable_after: index + 1 < ordered_tab_ids.len(),
        },
        browser_tab_menu_parts(
            is_web_url(&url),
            url_is_pinned(&url),
            lingxia_browser::can_demote_tab_to_standalone(tab_id),
        ),
    ))
}

fn browser_tab_menu_parts(web: bool, pinned: bool, movable: bool) -> SurfaceMenuParts {
    let mut parts = SurfaceMenuParts::default();
    if web {
        let mut page = Vec::with_capacity(2);
        if cfg!(feature = "browser-shell") {
            page.push(Item::browser(if pinned {
                Action::Unpin
            } else {
                Action::Pin
            }));
        }
        page.push(Item::browser(Action::CopyLink));
        parts.content.push(page);
        if movable {
            parts.placement.push(Item::browser(Action::OpenInSide));
        }
    }
    parts
}

/// Hand a main-area tab to the side panel, keeping its WebView, history and
/// state. The platform must already have let go of the tab in its main
/// browser. Returns the side-panel surface id.
pub fn move_browser_tab_to_side_panel(tab_id: &str) -> Result<String, LxAppError> {
    let url = lingxia_browser::tabs()
        .into_iter()
        .find(|tab| tab.tab_id == tab_id)
        .ok_or_else(|| LxAppError::ResourceNotFound(format!("browser tab {tab_id}")))?
        .current_url
        .unwrap_or_default();
    let owner = lingxia_app_context::home_app_id()
        .and_then(lxapp::try_get)
        .ok_or_else(|| LxAppError::ResourceNotFound("shell owner lxapp".to_string()))?;
    lingxia_browser::demote_tab_to_standalone(tab_id, &owner.appid, owner.session_id())?;
    match owner.open_browser_tab_aside(tab_id, &url) {
        Ok(surface) => Ok(surface.id),
        Err(error) => {
            // Back into the main browser rather than leave it nowhere.
            let _ = lingxia_browser::promote_standalone_tab(tab_id);
            Err(error)
        }
    }
}

fn is_web_url(url: &str) -> bool {
    let url = url.trim();
    url.starts_with("http://") || url.starts_with("https://")
}

#[cfg(feature = "browser-shell")]
fn url_is_pinned(url: &str) -> bool {
    is_web_url(url) && lingxia_browser_shell::pinned_bookmark_for_url(url).is_some()
}

#[cfg(not(feature = "browser-shell"))]
fn url_is_pinned(_url: &str) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use lingxia_shell::SurfaceMenuAction;

    fn actions(parts: &SurfaceMenuParts) -> Vec<SurfaceMenuAction> {
        parts
            .content
            .iter()
            .flatten()
            .chain(&parts.placement)
            .map(|item| item.action.clone())
            .collect()
    }

    #[test]
    fn web_tab_offers_copy_and_move_and_an_internal_page_offers_neither() {
        let web = actions(&browser_tab_menu_parts(true, false, true));
        assert!(web.contains(&SurfaceMenuAction::Browser {
            action: Action::CopyLink
        }));
        assert_eq!(
            web.last(),
            Some(&SurfaceMenuAction::Browser {
                action: Action::OpenInSide
            })
        );

        let callback = actions(&browser_tab_menu_parts(true, false, false));
        assert!(!callback.contains(&SurfaceMenuAction::Browser {
            action: Action::OpenInSide
        }));

        assert!(actions(&browser_tab_menu_parts(false, false, true)).is_empty());
    }
}
